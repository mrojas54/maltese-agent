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

/// A strictly parsed `@@ -old_start[,old_count] +new_start[,new_count] @@[hint]`
/// hunk header. Omitted counts default to 1, matching both the unified-diff
/// format and patch-0.7's `range` parser.
struct HunkHeader<'a> {
    old_start: u64,
    old_count: u64,
    new_start: u64,
    new_count: u64,
    /// Everything after the closing `@@` (git's section hint), preserved
    /// verbatim on rewrite.
    hint: &'a str,
}

/// Split a line (as produced by `split_inclusive('\n')`) into its content and
/// line terminator (`"\r\n"`, `"\n"`, or `""` for an unterminated final line).
fn split_line_terminator(line: &str) -> (&str, &str) {
    if let Some(content) = line.strip_suffix("\r\n") {
        (content, "\r\n")
    } else if let Some(content) = line.strip_suffix('\n') {
        (content, "\n")
    } else {
        (line, "")
    }
}

/// Consume leading ASCII digits from `s`, returning the parsed value and the
/// rest. `None` when there are no digits or the value overflows `u64`.
fn take_u64(s: &str) -> Option<(u64, &str)> {
    let digits = s.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let value: u64 = s[..digits].parse().ok()?;
    Some((value, &s[digits..]))
}

/// Parse a hunk header line's content (no line terminator). Any deviation from
/// the strict grammar returns `None`, and the line passes through
/// [`normalize_hunk_counts`] untouched.
fn parse_hunk_header(content: &str) -> Option<HunkHeader<'_>> {
    let rest = content.strip_prefix("@@ -")?;
    let (old_start, rest) = take_u64(rest)?;
    let (old_count, rest) = match rest.strip_prefix(',') {
        Some(r) => take_u64(r)?,
        None => (1, rest),
    };
    let rest = rest.strip_prefix(" +")?;
    let (new_start, rest) = take_u64(rest)?;
    let (new_count, rest) = match rest.strip_prefix(',') {
        Some(r) => take_u64(r)?,
        None => (1, rest),
    };
    let hint = rest.strip_prefix(" @@")?;
    Some(HunkHeader {
        old_start,
        old_count,
        new_start,
        new_count,
        hint,
    })
}

/// How a line inside a hunk body counts toward the header's old/new totals.
enum BodyLine {
    /// `' '`-prefixed context — counts toward both sides.
    Context,
    /// `'-'`-prefixed removal — old side only.
    Remove,
    /// `'+'`-prefixed addition — new side only.
    Add,
    /// `\ No newline at end of file` marker — body, counted on neither side.
    Marker,
}

/// Classify a body line's content exactly as patch-0.7's `chunk_line` parser
/// does: `"--- "` / `"+++ "` are file headers (not removals/additions), which
/// keeps concatenated-patch inputs terminating the body here just as they
/// terminate the hunk in the parser.
fn classify_body_line(content: &str) -> Option<BodyLine> {
    if content.starts_with("--- ") || content.starts_with("+++ ") {
        return None;
    }
    match content.as_bytes().first() {
        Some(b' ') => Some(BodyLine::Context),
        Some(b'-') => Some(BodyLine::Remove),
        Some(b'+') => Some(BodyLine::Add),
        Some(b'\\') => Some(BodyLine::Marker),
        _ => None,
    }
}

/// Deterministically repair model-emitted unified diffs before parsing.
///
/// LLMs (observed live with gemini-3.1-pro-preview; cassette
/// `falcon-detective/fixtures/cassettes/896f6e7b4af1a7c7.json`) emit hunk
/// headers that declare full context-window counts while the body truncates
/// trailing context, and drop the final line's newline. patch-0.7's parser is
/// count-blind (`many1(chunk_line)`) but requires every body line to end with
/// a line terminator, so the missing newline panics the parse (recovered by
/// [`parse_hunks`]'s catch_unwind into a conflict), while the inflated counts
/// separately break [`splice_lines`]' bounds checks. Two repairs fix both:
///
/// 1. **Header recount (shrink-only).** For each strictly well-formed `@@`
///    header, the old (context + removals) and new (context + additions)
///    counts are recounted from the actual body lines and the header is
///    rewritten to match — but only when both recounts are less than or equal
///    to the declared counts and at least one is nonzero. Grown bodies are
///    left alone so a crafted huge-count header (see the overflow regression
///    test) still reaches `splice_lines`' checked arithmetic and conflicts,
///    and a body-less header is never rewritten into a zero-count no-op hunk.
///    Correct-count diffs pass through byte-identical (omitted-count style
///    like `@@ -1 +1 @@` included).
/// 2. **Final-newline termination.** If the last body line of a hunk lacks a
///    trailing newline, one is appended. The `\ No newline at end of file`
///    marker is already accepted unterminated by the parser and is not
///    counted toward either side.
///
/// Pure text transform (`&str -> String`), no filesystem access. Anything
/// after the last hunk body line — trailing prose, concatenated patches — is
/// left untouched so the existing panic-recovery rejection paths in
/// [`parse_hunks`] are preserved.
fn normalize_hunk_counts(unified_diff: &str) -> String {
    let mut out = String::with_capacity(unified_diff.len() + 1);
    let mut lines = unified_diff.split_inclusive('\n').peekable();

    while let Some(line) = lines.next() {
        let (content, terminator) = split_line_terminator(line);
        let Some(header) = parse_hunk_header(content) else {
            out.push_str(line);
            continue;
        };

        // Greedily consume the body lines that follow, mirroring the parser.
        let mut old_recount: u64 = 0;
        let mut new_recount: u64 = 0;
        let mut body: Vec<&str> = Vec::new();
        while let Some(&next) = lines.peek() {
            let (next_content, _) = split_line_terminator(next);
            match classify_body_line(next_content) {
                Some(BodyLine::Context) => {
                    old_recount += 1;
                    new_recount += 1;
                }
                Some(BodyLine::Remove) => old_recount += 1,
                Some(BodyLine::Add) => new_recount += 1,
                Some(BodyLine::Marker) => {}
                None => break,
            }
            body.push(next);
            lines.next();
        }

        // Repair 1: shrink-only header recount.
        let counts_differ = (old_recount, new_recount) != (header.old_count, header.new_count);
        let shrink_only = old_recount <= header.old_count && new_recount <= header.new_count;
        let has_body = old_recount > 0 || new_recount > 0;
        if counts_differ && shrink_only && has_body {
            out.push_str(&format!(
                "@@ -{},{} +{},{} @@{}",
                header.old_start, old_recount, header.new_start, new_recount, header.hint
            ));
            out.push_str(terminator);
        } else {
            out.push_str(line);
        }

        for body_line in &body {
            out.push_str(body_line);
        }

        // Repair 2: terminate an unterminated final body line (the marker is
        // already tolerated unterminated by the parser; leave it alone).
        if let Some(last) = body.last() {
            let (last_content, last_terminator) = split_line_terminator(last);
            if last_terminator.is_empty() && !last_content.starts_with('\\') {
                out.push('\n');
            }
        }
    }

    out
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
/// Composes four separately tested units:
///  1. [`validate_limits`] — pre-parse input caps.
///  2. [`normalize_hunk_counts`] — deterministic repair of model-emitted
///     hunk headers (pure text transform).
///  3. [`parse_hunks`] — unified diff → structured hunks (panic-safe).
///  4. [`splice_lines`] — hunks applied to the file content.
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

    // Repair model-emitted hunk headers (wrong counts, missing final newline)
    // before parsing. The catch_unwind backstop in parse_hunks stays in place
    // as defense in depth.
    let normalized = normalize_hunk_counts(&args.unified_diff);

    let path = sandbox.resolve(&args.path).context("resolving path")?;

    let original = tokio::fs::read_to_string(&path)
        .await
        .context("reading file")?;

    // parse_hunks borrows `normalized` for the lifetime of the parse.
    let parsed = match parse_hunks(&normalized) {
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

    // ---- normalize_hunk_counts ----------------------------------------------

    /// The exact failing diff recorded from gemini-3.1-pro-preview, preserved
    /// VERBATIM from falcon-detective/fixtures/cassettes/896f6e7b4af1a7c7.json:
    /// the header declares 6 old / 5 new lines, the body carries 4 old / 3 new,
    /// and the final line has no trailing newline. The planted-lint marker text
    /// below is diff DATA quoted from the cassette, not source (G-1:
    /// falcon-agent itself is untouched).
    const GEMINI_TRUNCATED_DIFF: &str = concat!(
        "--- falcon-agent/src/interrogate.rs\n",
        "+++ falcon-agent/src/interrogate.rs\n",
        "@@ -1,6 +1,5 @@\n",
        " use axum::{extract::State, Json};\n",
        " use serde::{Deserialize, Serialize};\n",
        "-use std::io::Write; // BUG (intentional): planted lint #1 — unused import\n",
        " use std::sync::Arc;",
    );

    #[test]
    fn normalize_rewrites_recorded_gemini_truncated_hunk() {
        let normalized = normalize_hunk_counts(GEMINI_TRUNCATED_DIFF);
        let expected = concat!(
            "--- falcon-agent/src/interrogate.rs\n",
            "+++ falcon-agent/src/interrogate.rs\n",
            "@@ -1,4 +1,3 @@\n",
            " use axum::{extract::State, Json};\n",
            " use serde::{Deserialize, Serialize};\n",
            "-use std::io::Write; // BUG (intentional): planted lint #1 — unused import\n",
            " use std::sync::Arc;\n",
        );
        assert_eq!(normalized, expected);
    }

    #[test]
    fn normalize_then_parse_then_splice_applies_recorded_diff() {
        // Original shaped like the head of the real falcon-agent/src/interrogate.rs.
        let original = concat!(
            "use axum::{extract::State, Json};\n",
            "use serde::{Deserialize, Serialize};\n",
            "use std::io::Write; // BUG (intentional): planted lint #1 — unused import\n",
            "use std::sync::Arc;\n",
            "\n",
            "#[derive(Debug, Deserialize)]\n",
            "pub struct InterrogateRequest {\n",
        );

        // The un-normalized diff is the live failure: patch-0.7 panics on the
        // unterminated final line, recovered by the catch_unwind backstop.
        let reason = parse_hunks(GEMINI_TRUNCATED_DIFF).unwrap_err();
        assert!(
            reason.contains("failed to parse entire input"),
            "unexpected reason: {reason}"
        );

        // Full seam: validate → normalize → parse → splice.
        assert!(validate_limits(GEMINI_TRUNCATED_DIFF).is_ok());
        let normalized = normalize_hunk_counts(GEMINI_TRUNCATED_DIFF);
        let parsed = parse_hunks(&normalized).expect("normalized diff parses");
        let (content, added) = splice_lines(original, &parsed).expect("patch applies");
        assert_eq!(
            content,
            concat!(
                "use axum::{extract::State, Json};\n",
                "use serde::{Deserialize, Serialize};\n",
                "use std::sync::Arc;\n",
                "\n",
                "#[derive(Debug, Deserialize)]\n",
                "pub struct InterrogateRequest {\n",
            )
        );
        assert_eq!(added, 0);
    }

    #[test]
    fn normalize_passes_through_correct_counts_byte_identical() {
        let single = "--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n hello\n-world\n+falcon\n";
        assert_eq!(normalize_hunk_counts(single), single);

        let multi = "--- a/f.txt\n+++ b/f.txt\n\
                     @@ -1,2 +1,2 @@\n one\n-two\n+TWO\n\
                     @@ -7,2 +7,3 @@\n seven\n-eight\n+EIGHT\n+nine\n";
        assert_eq!(normalize_hunk_counts(multi), multi);

        let marker =
            "--- a/f.txt\n+++ b/f.txt\n@@ -1,1 +1,1 @@\n-world\n+falcon\n\\ No newline at end of file\n";
        assert_eq!(normalize_hunk_counts(marker), marker);
    }

    #[test]
    fn normalize_recounts_truncated_hunk_adjacent_to_correct_hunk() {
        // Hunk 1 declares 3/3 but carries 2/2 and is immediately followed by
        // hunk 2, whose counts are correct: only hunk 1's header is rewritten.
        let diff = "--- a/f.txt\n+++ b/f.txt\n\
                    @@ -1,3 +1,3 @@\n one\n-two\n+TWO\n\
                    @@ -7,2 +7,2 @@\n seven\n-eight\n+EIGHT\n";
        let expected = "--- a/f.txt\n+++ b/f.txt\n\
                        @@ -1,2 +1,2 @@\n one\n-two\n+TWO\n\
                        @@ -7,2 +7,2 @@\n seven\n-eight\n+EIGHT\n";
        let normalized = normalize_hunk_counts(diff);
        assert_eq!(normalized, expected);

        let parsed = parse_hunks(&normalized).expect("normalized diff parses");
        let original = "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\n";
        let (content, added) = splice_lines(original, &parsed).expect("patch applies");
        assert_eq!(content, "one\nTWO\nthree\nfour\nfive\nsix\nseven\nEIGHT\n");
        assert_eq!(added, 2);
    }

    #[test]
    fn normalize_does_not_count_no_newline_marker() {
        // The header over-declares 3/3; the actual body is 1/1 plus the
        // marker, which counts toward neither side and is preserved.
        let diff =
            "--- a/f.txt\n+++ b/f.txt\n@@ -1,3 +1,3 @@\n-world\n+falcon\n\\ No newline at end of file\n";
        let expected =
            "--- a/f.txt\n+++ b/f.txt\n@@ -1,1 +1,1 @@\n-world\n+falcon\n\\ No newline at end of file\n";
        let normalized = normalize_hunk_counts(diff);
        assert_eq!(normalized, expected);

        let parsed = parse_hunks(&normalized).expect("normalized diff parses");
        assert!(!parsed.end_newline);
        let (content, added) = splice_lines("world\n", &parsed).expect("patch applies");
        assert_eq!(content, "falcon");
        assert_eq!(added, 1);
    }

    #[test]
    fn normalize_interprets_omitted_counts_as_one() {
        // Counts omitted on both sides and the body matches: byte-identical,
        // preserving the omitted-count header style.
        let identity = "--- a/f.txt\n+++ b/f.txt\n@@ -1 +1 @@\n-a\n+b\n";
        assert_eq!(normalize_hunk_counts(identity), identity);

        // Old count omitted (= 1) and correct; new count over-declared: the
        // mismatch triggers a rewrite, which emits explicit counts.
        let truncated = "--- a/f.txt\n+++ b/f.txt\n@@ -1 +1,2 @@\n-a\n+b\n";
        let expected = "--- a/f.txt\n+++ b/f.txt\n@@ -1,1 +1,1 @@\n-a\n+b\n";
        assert_eq!(normalize_hunk_counts(truncated), expected);
    }

    #[test]
    fn normalize_appends_missing_final_newline() {
        // Correct counts, but the final body line is unterminated — the exact
        // patch-0.7 panic trigger. Normalization repairs the terminator so the
        // diff parses and applies.
        let diff = "--- a/f.txt\n+++ b/f.txt\n@@ -1,1 +1,1 @@\n-world\n+falcon";
        let normalized = normalize_hunk_counts(diff);
        assert_eq!(
            normalized,
            "--- a/f.txt\n+++ b/f.txt\n@@ -1,1 +1,1 @@\n-world\n+falcon\n"
        );

        assert!(
            parse_hunks(diff).is_err(),
            "unterminated diff must fail to parse without normalization"
        );
        let parsed = parse_hunks(&normalized).expect("normalized diff parses");
        let (content, added) = splice_lines("world\n", &parsed).expect("patch applies");
        assert_eq!(content, "falcon\n");
        assert_eq!(added, 1);
    }

    #[test]
    fn normalize_leaves_grown_body_counts_untouched() {
        // Unit mirror of the huge-count integration regression, now routed
        // through normalize: the body carries MORE new lines (2) than the
        // header declares (1), so the shrink-only rule refuses to rewrite and
        // the crafted huge old-count still hits splice_lines' bounds check.
        let diff = "--- a/tiny.txt\n+++ b/tiny.txt\n@@ -1,4294967295 +1,1 @@\n one\n+replaced\n";
        assert_eq!(normalize_hunk_counts(diff), diff);

        let parsed = parse_hunks(diff).expect("count-blind parser accepts the diff");
        let reason = splice_lines("one\ntwo\n", &parsed).unwrap_err();
        assert!(
            reason.starts_with("hunk range "),
            "unexpected reason: {reason}"
        );
    }

    #[test]
    fn normalize_leaves_trailing_content_untouched() {
        // Trailing prose after a correct-count hunk: byte-identical, and the
        // parser-panic recovery path still reports the conflict.
        let prose =
            "--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n hello\n-world\n+falcon\nstray prose\n";
        assert_eq!(normalize_hunk_counts(prose), prose);
        let reason = parse_hunks(prose).unwrap_err();
        assert!(
            reason.starts_with("malformed diff (parser panicked):"),
            "unexpected reason: {reason}"
        );

        // Concatenated second patch: `--- `/`+++ ` are file headers, not body
        // lines, so the first hunk's counts stay put and rejection is intact.
        let concatenated = "--- a/f.txt\n+++ b/f.txt\n@@ -1,1 +1,1 @@\n-a\n+b\n\
                            --- a/g.txt\n+++ b/g.txt\n@@ -1,1 +1,1 @@\n-c\n+d\n";
        assert_eq!(normalize_hunk_counts(concatenated), concatenated);
        let reason = parse_hunks(concatenated).unwrap_err();
        assert!(
            reason.starts_with("malformed diff (parser panicked):"),
            "unexpected reason: {reason}"
        );
    }

    #[test]
    fn normalize_leaves_bodyless_header_untouched() {
        // A header with no body lines after it must not be rewritten into a
        // zero-count no-op hunk.
        let followed_by_prose = "--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\nstray prose\n";
        assert_eq!(normalize_hunk_counts(followed_by_prose), followed_by_prose);

        let at_eof = "--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n";
        assert_eq!(normalize_hunk_counts(at_eof), at_eof);
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
