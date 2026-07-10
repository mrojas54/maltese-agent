//! Filesystem tools: read, write, list, apply_patch, search.

use crate::limits;
use crate::sandbox::Sandbox;
use crate::tools::subprocess;
use anyhow::Context;
use patch::Line;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::ControlFlow;
use std::sync::Arc;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FsReadArgs {
    /// Path relative to the sandbox root.
    pub path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FsReadResult {
    pub content: String,
    pub bytes: usize,
}

pub async fn fs_read(sandbox: Arc<Sandbox>, args: FsReadArgs) -> anyhow::Result<FsReadResult> {
    let path = sandbox.resolve(&args.path).context("resolving path")?;
    let bytes = tokio::fs::read(&path).await.context("reading file")?;
    let len = bytes.len();
    let content = String::from_utf8(bytes).context("file is not valid UTF-8")?;
    Ok(FsReadResult {
        bytes: len,
        content,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FsWriteArgs {
    /// Path relative to the sandbox root.
    pub path: String,
    /// UTF-8 content to write.
    pub content: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FsWriteResult {
    pub bytes: usize,
}

pub async fn fs_write(sandbox: Arc<Sandbox>, args: FsWriteArgs) -> anyhow::Result<FsWriteResult> {
    sandbox.check_writable()?;
    let path = sandbox.resolve(&args.path).context("resolving path")?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("creating parent dirs")?;
    }
    let bytes = args.content.as_bytes();
    tokio::fs::write(&path, bytes)
        .await
        .context("writing file")?;
    Ok(FsWriteResult { bytes: bytes.len() })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FsListArgs {
    /// Path relative to the sandbox root.
    pub path: String,
    /// Optional glob filter applied to entry names (not full paths).
    #[serde(default)]
    pub glob: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FsListResult {
    pub entries: Vec<String>,
}

pub async fn fs_list(sandbox: Arc<Sandbox>, args: FsListArgs) -> anyhow::Result<FsListResult> {
    let path = sandbox.resolve(&args.path).context("resolving path")?;
    let pattern = args
        .glob
        .as_deref()
        .map(glob::Pattern::new)
        .transpose()
        .context("invalid glob pattern")?;

    let mut rd = tokio::fs::read_dir(&path)
        .await
        .context("reading directory")?;
    let mut entries = Vec::new();
    while let Some(entry) = rd.next_entry().await.context("iterating dir entries")? {
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(ref p) = pattern {
            if !p.matches(&name) {
                continue;
            }
        }
        entries.push(name);
    }
    entries.sort();
    Ok(FsListResult { entries })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FsApplyPatchArgs {
    /// Path to the file to patch, relative to the sandbox root.
    pub path: String,
    /// Unified diff (output of `diff -u`) to apply to the file.
    pub unified_diff: String,
}

/// Result of applying a unified diff.
///
/// `result` is always `"ok"` or `"conflict"`.  On `"ok"`, `lines_changed` is
/// populated.  On `"conflict"`, `reason` describes why the diff was rejected.
#[derive(Debug, Serialize, JsonSchema)]
pub struct FsApplyPatchResult {
    /// `"ok"` if the patch was applied, `result: "conflict"` if it was rejected.
    pub result: String,
    /// Number of `+` lines in the diff (lines added in the new file). Present when `result == "ok"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines_changed: Option<usize>,
    /// Why the patch was rejected. Present when `result == "conflict"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl FsApplyPatchResult {
    fn ok(lines_changed: usize) -> Self {
        Self {
            result: "ok".into(),
            lines_changed: Some(lines_changed),
            reason: None,
        }
    }

    fn conflict(reason: impl Into<String>) -> Self {
        Self {
            result: "conflict".into(),
            lines_changed: None,
            reason: Some(reason.into()),
        }
    }
}

/// Maximum byte length of an accepted unified diff.
const MAX_DIFF_BYTES: usize = 8 * 1024 * 1024; // 8 MiB

/// Maximum number of hunks in an accepted unified diff.
const MAX_HUNK_COUNT: usize = 10_000;

/// Validate pre-parse input limits for `fs_apply_patch`.
///
/// Caps the diff's byte length *before any parsing* to guard against OOM from
/// multi-GB diffs. On rejection, returns the conflict reason.
fn validate_limits(unified_diff: &str) -> Result<(), String> {
    if unified_diff.len() > MAX_DIFF_BYTES {
        return Err(format!(
            "diff exceeds maximum allowed size of {} bytes",
            MAX_DIFF_BYTES
        ));
    }
    Ok(())
}

/// Parse a unified diff into structured hunks.
///
/// patch-0.7 panics (rather than returning Err) on inputs with trailing
/// unrecognized content — common with LLM-generated diffs that include
/// stray prose or duplicate hunks. The parse is wrapped in catch_unwind so a
/// panic surfaces as a clean conflict reason instead of crashing the
/// worker thread.
///
/// Also enforces the hunk-count cap ([`MAX_HUNK_COUNT`]) to guard against
/// memory exhaustion. The cap lives here rather than in [`validate_limits`]
/// because it can only be checked after parsing, whereas the byte cap must
/// run before parsing.
///
/// On rejection, returns the conflict reason.
fn parse_hunks(unified_diff: &str) -> Result<patch::Patch<'_>, String> {
    let parsed = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        patch::Patch::from_single(unified_diff)
    })) {
        Ok(Ok(p)) => p,
        Ok(Err(e)) => {
            return Err(format!("malformed diff: {e}"));
        }
        Err(panic) => {
            let msg = panic
                .downcast_ref::<&'static str>()
                .map(|s| (*s).to_string())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "patch parser panicked".to_string());
            return Err(format!("malformed diff (parser panicked): {msg}"));
        }
    };

    // Cap the number of hunks to guard against memory exhaustion.
    if parsed.hunks.len() > MAX_HUNK_COUNT {
        return Err(format!(
            "diff contains {} hunks, exceeding maximum of {}",
            parsed.hunks.len(),
            MAX_HUNK_COUNT
        ));
    }

    Ok(parsed)
}

/// Apply parsed hunks to file content.
///
/// Walks hunks in order, verifying that context and remove lines match the
/// original file, and reconstructs the output by interleaving unchanged lines
/// with hunk output. Returns `(new_content, add_count)` where `add_count` is
/// the number of `+` lines across all hunks — a stable, easy-to-explain
/// metric for "lines changed".
///
/// On rejection (context-line mismatch, or a hunk targeting a line range
/// outside the file), returns the conflict reason.
fn splice_lines(original: &str, parsed: &patch::Patch<'_>) -> Result<(String, usize), String> {
    // Split original into lines WITHOUT their trailing newlines (matching what
    // the patch crate stores in Line::Context/Remove).  We track the final
    // newline separately via `end_newline`.
    let orig_lines: Vec<&str> = original.lines().collect();
    let file_ends_with_newline = original.ends_with('\n');

    let mut out: Vec<&str> = Vec::with_capacity(orig_lines.len());
    // old_line is 0-indexed cursor into orig_lines; hunks use 1-indexed start.
    let mut old_line: usize = 0;

    // Count of `+` lines across all hunks — used as lines_changed metric.
    let mut add_count: usize = 0;

    for hunk in &parsed.hunks {
        // Convert u64 hunk fields to usize; reject on truncation (32-bit targets).
        let hunk_start = match usize::try_from(hunk.old_range.start) {
            Ok(v) => v,
            Err(_) => {
                return Err("hunk old_range.start overflows usize".to_string());
            }
        };
        let hunk_old_count = match usize::try_from(hunk.old_range.count) {
            Ok(v) => v,
            Err(_) => {
                return Err("hunk old_range.count overflows usize".to_string());
            }
        };

        // hunk_start == 0 can happen for empty-file diffs; treat as line 1.
        let target = if hunk_start == 0 { 0 } else { hunk_start - 1 };

        // Validate that the hunk doesn't reference lines beyond EOF.
        if target > orig_lines.len() {
            return Err(format!(
                "hunk targets line {} but file has only {} lines",
                hunk_start,
                orig_lines.len()
            ));
        }

        // Use checked_add to prevent overflow when hunk_old_count is huge.
        match target.checked_add(hunk_old_count) {
            Some(end) if end <= orig_lines.len() => {}
            _ => {
                let last = hunk_start.saturating_add(hunk_old_count).saturating_sub(1);
                return Err(format!(
                    "hunk range {}-{} extends beyond file length {}",
                    hunk_start,
                    last,
                    orig_lines.len()
                ));
            }
        }

        // Emit all lines before this hunk unchanged.
        while old_line < target {
            out.push(orig_lines[old_line]);
            old_line += 1;
        }

        // Walk the hunk lines, verifying context matches the original file.
        for line in &hunk.lines {
            match line {
                Line::Context(ctx) => {
                    // Context line must match the file; if not, the diff is stale.
                    if old_line >= orig_lines.len() || orig_lines[old_line] != *ctx {
                        let file_line = orig_lines.get(old_line).copied().unwrap_or("<eof>");
                        return Err(format!(
                            "context mismatch at line {}: expected {:?}, got {:?}",
                            old_line + 1,
                            ctx,
                            file_line
                        ));
                    }
                    out.push(orig_lines[old_line]);
                    old_line += 1;
                }
                Line::Remove(rem) => {
                    // Remove line must also match.
                    if old_line >= orig_lines.len() || orig_lines[old_line] != *rem {
                        let file_line = orig_lines.get(old_line).copied().unwrap_or("<eof>");
                        return Err(format!(
                            "remove mismatch at line {}: expected {:?}, got {:?}",
                            old_line + 1,
                            rem,
                            file_line
                        ));
                    }
                    // Consume from original but do not push to output.
                    old_line += 1;
                }
                Line::Add(add) => {
                    out.push(add);
                    add_count += 1;
                }
            }
        }
    }

    // Emit remaining lines after the last hunk.
    while old_line < orig_lines.len() {
        out.push(orig_lines[old_line]);
        old_line += 1;
    }

    // Re-join with newlines.  Honour the patch's `end_newline` flag: if the
    // patch says no trailing newline, omit it; otherwise add one (or preserve
    // the original file's convention when the patch flag is true).
    let mut new_content = out.join("\n");
    let want_trailing_newline = parsed.end_newline && (file_ends_with_newline || !out.is_empty());
    if want_trailing_newline {
        new_content.push('\n');
    }

    Ok((new_content, add_count))
}

/// Apply a unified diff to a file inside the sandbox.
///
/// Composes three separately tested units:
///  1. [`validate_limits`] — pre-parse input caps.
///  2. [`parse_hunks`] — unified diff → structured hunks (panic-safe).
///  3. [`splice_lines`] — hunks applied to the file content.
///
/// Returns `result: "conflict"` for: diff exceeding size limits, malformed diff,
/// context-line mismatch, or hunk targeting a line range outside the file.
/// Never panics on user-controlled input.
pub async fn fs_apply_patch(
    sandbox: Arc<Sandbox>,
    args: FsApplyPatchArgs,
) -> anyhow::Result<FsApplyPatchResult> {
    sandbox.check_writable()?;

    if let Err(reason) = validate_limits(&args.unified_diff) {
        return Ok(FsApplyPatchResult::conflict(reason));
    }

    let path = sandbox.resolve(&args.path).context("resolving path")?;

    let original = tokio::fs::read_to_string(&path)
        .await
        .context("reading file")?;

    // parse_hunks borrows args.unified_diff for the lifetime of the parse.
    let parsed = match parse_hunks(&args.unified_diff) {
        Ok(p) => p,
        Err(reason) => return Ok(FsApplyPatchResult::conflict(reason)),
    };

    let (new_content, add_count) = match splice_lines(&original, &parsed) {
        Ok(v) => v,
        Err(reason) => return Ok(FsApplyPatchResult::conflict(reason)),
    };

    tokio::fs::write(&path, new_content.as_bytes())
        .await
        .context("writing patched file")?;

    Ok(FsApplyPatchResult::ok(add_count))
}

// ---- fs_search ---------------------------------------------------------------

/// Hard ceiling on the number of matches that can be requested via `max`.
/// Callers specifying a larger value are silently clamped to this.
const MAX_SEARCH_RESULTS: usize = 10_000;

fn default_max() -> usize {
    200
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FsSearchArgs {
    /// Regex pattern to search for (passed directly to `rg`).
    pub pattern: String,
    /// Optional glob to restrict which files are searched (e.g. `"*.rs"`).
    #[serde(default)]
    pub glob: Option<String>,
    /// Maximum number of match records to return (default 200, hard cap 10 000).
    #[serde(default = "default_max")]
    pub max: usize,
}

/// A single match location returned by `fs_search`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct FsMatch {
    /// Relative path of the file containing the match.
    pub file: String,
    /// 1-based line number.
    pub line: u64,
    /// 1-based byte-column of the match start within the line.
    pub column: u64,
    /// The matching line's text (trailing whitespace stripped).
    pub text: String,
}

/// Result returned by `fs_search`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct FsSearchResult {
    /// Collected match records, up to `max`.
    pub matches: Vec<FsMatch>,
    /// `true` if the result was capped before ripgrep finished all output.
    pub truncated: bool,
}

/// Search files inside the sandbox using ripgrep.
///
/// Shells out to `rg --json -e PATTERN [--glob G]` with `current_dir` set to
/// the sandbox root, via the shared streaming-subprocess helper
/// ([`subprocess::run_streaming`]): stdout is streamed line-by-line, the
/// child is killed once `max` match records have been collected, and the
/// whole run is bounded by the search wall-clock timeout
/// ([`limits::SEARCH_TIMEOUT`], env-overridable via
/// `FALCON_MCP_SEARCH_TIMEOUT_MS`). Returns `{ matches, truncated }`.
///
/// `-e PATTERN` is used instead of a bare positional argument to prevent
/// argument injection: a pattern like `--pre=/bin/sh` would otherwise be
/// interpreted by rg as a preprocessor flag (an RCE vector).
///
/// `column` values are 1-based; ripgrep's submatch `start` field is a 0-based
/// byte offset.
///
/// rg exit codes: 0 = found matches, 1 = no matches (not an error), 2+ = rg error.
pub async fn fs_search(
    sandbox: Arc<Sandbox>,
    args: FsSearchArgs,
) -> anyhow::Result<FsSearchResult> {
    sandbox.check_bin("rg")?;

    // Clamp caller-supplied max to the hard ceiling.
    let effective_max = args.max.min(MAX_SEARCH_RESULTS);

    let mut cmd = tokio::process::Command::new("rg");
    cmd.arg("--json").arg("--no-heading");
    // Defense-in-depth: cap per-file matches and file size so that a single
    // large file cannot generate unbounded output even before the streaming
    // early-exit kicks in.
    cmd.arg(format!("--max-count={}", effective_max.saturating_add(1)));
    cmd.arg(format!("--max-filesize={}", limits::SEARCH_MAX_FILESIZE));
    if let Some(ref g) = args.glob {
        cmd.arg("--glob").arg(g);
    }
    // Use -e PATTERN instead of a bare positional arg to prevent argument
    // injection: a pattern starting with `--` would otherwise be parsed by rg
    // as a flag (e.g. `--pre=/bin/sh` invokes an external preprocessor).
    cmd.arg("-e").arg(&args.pattern);
    cmd.current_dir(sandbox.root());

    let mut matches: Vec<FsMatch> = Vec::new();

    let outcome = subprocess::run_streaming(cmd, "rg", limits::search_timeout(), |line| {
        if line.is_empty() {
            return ControlFlow::Continue(());
        }
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => return ControlFlow::Continue(()),
        };
        if v["type"] != "match" {
            return ControlFlow::Continue(());
        }
        let m = &v["data"];
        let file = m["path"]["text"].as_str().unwrap_or("").to_string();
        let line_number = m["line_number"].as_u64().unwrap_or(0);
        let text = m["lines"]["text"]
            .as_str()
            .unwrap_or("")
            .trim_end()
            .to_string();

        if let Some(submatches) = m["submatches"].as_array() {
            for sub in submatches {
                if matches.len() >= effective_max {
                    return ControlFlow::Break(());
                }
                // ripgrep start is 0-based byte offset; add 1 for 1-based column.
                let column = sub["start"].as_u64().unwrap_or(0).saturating_add(1);
                matches.push(FsMatch {
                    file: file.clone(),
                    line: line_number,
                    column,
                    text: text.clone(),
                });
            }
        }
        ControlFlow::Continue(())
    })
    .await?;

    // exit 0 = matches found; exit 1 = no matches (normal); a self-inflicted
    // kill after truncation is also OK — shared policy in check_status.
    outcome.check_status("rg", 1)?;

    Ok(FsSearchResult {
        matches,
        truncated: outcome.truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- validate_limits ---------------------------------------------------

    #[test]
    fn validate_limits_accepts_small_diff() {
        assert!(validate_limits("--- a/f\n+++ b/f\n@@ -1,1 +1,1 @@\n-a\n+b\n").is_ok());
    }

    #[test]
    fn validate_limits_accepts_exactly_max_bytes() {
        let diff = "x".repeat(MAX_DIFF_BYTES);
        assert!(validate_limits(&diff).is_ok());
    }

    #[test]
    fn validate_limits_rejects_oversized_diff() {
        let diff = "x".repeat(MAX_DIFF_BYTES + 1);
        let reason = validate_limits(&diff).unwrap_err();
        assert_eq!(
            reason,
            format!(
                "diff exceeds maximum allowed size of {} bytes",
                MAX_DIFF_BYTES
            )
        );
    }

    // ---- parse_hunks -------------------------------------------------------

    #[test]
    fn parse_hunks_parses_valid_single_hunk() {
        let diff = "--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n hello\n-world\n+falcon\n";
        let parsed = parse_hunks(diff).expect("valid diff parses");
        assert_eq!(parsed.hunks.len(), 1);
        assert_eq!(parsed.hunks[0].old_range.start, 1);
        assert_eq!(parsed.hunks[0].old_range.count, 2);
        assert_eq!(parsed.hunks[0].new_range.start, 1);
        assert_eq!(parsed.hunks[0].new_range.count, 2);
        assert!(parsed.end_newline);
    }

    #[test]
    fn parse_hunks_preserves_no_newline_flag() {
        let diff =
            "--- a/f.txt\n+++ b/f.txt\n@@ -1,1 +1,1 @@\n-world\n+falcon\n\\ No newline at end of file\n";
        let parsed = parse_hunks(diff).expect("valid diff parses");
        assert!(!parsed.end_newline);
    }

    #[test]
    fn parse_hunks_reports_malformed_diff() {
        let reason = parse_hunks("this is not a diff at all").unwrap_err();
        assert!(
            reason.starts_with("malformed diff:"),
            "unexpected reason: {reason}"
        );
    }

    #[test]
    fn parse_hunks_recovers_parser_panic_on_trailing_prose() {
        // patch-0.7 panics (not Err) on trailing unparsed content; parse_hunks
        // must convert the panic into a conflict reason.
        let diff =
            "--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n hello\n-world\n+falcon\nstray prose\n";
        let reason = parse_hunks(diff).unwrap_err();
        assert!(
            reason.starts_with("malformed diff (parser panicked):"),
            "unexpected reason: {reason}"
        );
    }

    #[test]
    fn parse_hunks_recovers_parser_panic_on_concatenated_patches() {
        let diff = "--- a/f.txt\n+++ b/f.txt\n@@ -1,1 +1,1 @@\n-a\n+b\n\
                    --- a/g.txt\n+++ b/g.txt\n@@ -1,1 +1,1 @@\n-c\n+d\n";
        let reason = parse_hunks(diff).unwrap_err();
        assert!(
            reason.starts_with("malformed diff (parser panicked):"),
            "unexpected reason: {reason}"
        );
    }

    #[test]
    fn parse_hunks_rejects_hunk_count_over_cap() {
        let mut diff = String::from("--- a/f.txt\n+++ b/f.txt\n");
        for i in 0..(MAX_HUNK_COUNT + 1) {
            let line = i * 2 + 1;
            diff.push_str(&format!("@@ -{line},1 +{line},1 @@\n-a\n+b\n"));
        }
        let reason = parse_hunks(&diff).unwrap_err();
        assert_eq!(
            reason,
            format!(
                "diff contains {} hunks, exceeding maximum of {}",
                MAX_HUNK_COUNT + 1,
                MAX_HUNK_COUNT
            )
        );
    }

    // ---- splice_lines ------------------------------------------------------

    /// Parse `diff` (must be a valid unified diff) and splice it onto `original`.
    fn splice(original: &str, diff: &str) -> Result<(String, usize), String> {
        let parsed = parse_hunks(diff).expect("test diff must parse");
        splice_lines(original, &parsed)
    }

    #[test]
    fn splice_replaces_line_happy_path() {
        let diff = "--- a/greet.txt\n+++ b/greet.txt\n@@ -1,2 +1,2 @@\n hello\n-world\n+falcon\n";
        let (content, added) = splice("hello\nworld\n", diff).expect("patch applies");
        assert_eq!(content, "hello\nfalcon\n");
        assert_eq!(added, 1);
    }

    #[test]
    fn splice_applies_multiple_hunks() {
        let original = "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\n";
        let diff = "--- a/f.txt\n+++ b/f.txt\n\
                    @@ -1,2 +1,2 @@\n one\n-two\n+TWO\n\
                    @@ -7,2 +7,3 @@\n seven\n-eight\n+EIGHT\n+nine\n";
        let (content, added) = splice(original, diff).expect("patch applies");
        assert_eq!(
            content,
            "one\nTWO\nthree\nfour\nfive\nsix\nseven\nEIGHT\nnine\n"
        );
        assert_eq!(added, 3);
    }

    #[test]
    fn splice_counts_only_added_lines() {
        let original = "keep\ndrop\n";
        let diff = "--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,3 @@\n keep\n-drop\n+new one\n+new two\n";
        let (content, added) = splice(original, diff).expect("patch applies");
        assert_eq!(content, "keep\nnew one\nnew two\n");
        assert_eq!(added, 2);
    }

    #[test]
    fn splice_context_mismatch_conflicts() {
        let diff = "--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n hello\n-world\n+falcon\n";
        let reason = splice("goodbye\nworld\n", diff).unwrap_err();
        assert!(
            reason.starts_with("context mismatch at line 1:"),
            "unexpected reason: {reason}"
        );
    }

    #[test]
    fn splice_remove_mismatch_conflicts() {
        let diff = "--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n hello\n-world\n+falcon\n";
        let reason = splice("hello\nplanet\n", diff).unwrap_err();
        assert!(
            reason.starts_with("remove mismatch at line 2:"),
            "unexpected reason: {reason}"
        );
    }

    #[test]
    fn splice_hunk_offset_drift_conflicts() {
        // The hunk's lines exist in the file, but the header start has drifted
        // by one line; exact-offset semantics reject this (no fuzzy matching).
        let diff = "--- a/f.txt\n+++ b/f.txt\n@@ -2,2 +2,2 @@\n hello\n-world\n+falcon\n";
        let reason = splice("hello\nworld\nend\n", diff).unwrap_err();
        assert!(
            reason.starts_with("context mismatch at line 2:"),
            "unexpected reason: {reason}"
        );
    }

    #[test]
    fn splice_hunk_beyond_eof_conflicts() {
        let diff = "--- a/f.txt\n+++ b/f.txt\n@@ -10,1 +10,1 @@\n-x\n+y\n";
        let reason = splice("one\ntwo\n", diff).unwrap_err();
        assert_eq!(reason, "hunk targets line 10 but file has only 2 lines");
    }

    #[test]
    fn splice_hunk_range_extends_beyond_file_conflicts() {
        let diff = "--- a/f.txt\n+++ b/f.txt\n@@ -2,4 +2,4 @@\n two\n-x\n-y\n-z\n+a\n+b\n+c\n";
        let reason = splice("one\ntwo\nthree\n", diff).unwrap_err();
        assert_eq!(reason, "hunk range 2-5 extends beyond file length 3");
    }

    #[test]
    fn splice_huge_count_overflow_conflicts() {
        // Unit-level mirror of the integration regression test: a crafted
        // header with a huge count must conflict, not overflow or panic.
        let diff = "--- a/f.txt\n+++ b/f.txt\n@@ -1,4294967295 +1,1 @@\n one\n+replaced\n";
        let reason = splice("one\ntwo\n", diff).unwrap_err();
        assert!(
            reason.starts_with("hunk range "),
            "unexpected reason: {reason}"
        );
    }

    #[test]
    fn splice_no_trailing_newline_patch() {
        let diff =
            "--- a/f.txt\n+++ b/f.txt\n@@ -1,1 +1,1 @@\n-world\n+falcon\n\\ No newline at end of file\n";
        let (content, added) = splice("world\n", diff).expect("patch applies");
        assert_eq!(content, "falcon");
        assert_eq!(added, 1);
    }

    #[test]
    fn splice_empty_file_insert() {
        let diff = "--- a/f.txt\n+++ b/f.txt\n@@ -0,0 +1,1 @@\n+first\n";
        let (content, added) = splice("", diff).expect("patch applies");
        assert_eq!(content, "first\n");
        assert_eq!(added, 1);
    }
}
