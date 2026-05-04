//! Filesystem tools: read, write, list, apply_patch, search.

use crate::sandbox::Sandbox;
use anyhow::Context;
use patch::Line;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::AsyncBufReadExt as _;

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

/// Apply a unified diff to a file inside the sandbox.
///
/// The algorithm:
///  1. Parse the diff with the `patch` crate.
///  2. Walk hunks in order, verifying that context lines match the file.
///  3. Reconstruct the output by interleaving unchanged lines with hunk output.
///  4. Write the result back to disk.
///
/// Returns `result: "conflict"` for: diff exceeding size limits, malformed diff,
/// context-line mismatch, or hunk targeting a line range outside the file.
/// Never panics on user-controlled input.
pub async fn fs_apply_patch(
    sandbox: Arc<Sandbox>,
    args: FsApplyPatchArgs,
) -> anyhow::Result<FsApplyPatchResult> {
    sandbox.check_writable()?;

    // Cap input size before any parsing to guard against OOM from multi-GB diffs.
    if args.unified_diff.len() > MAX_DIFF_BYTES {
        return Ok(FsApplyPatchResult::conflict(format!(
            "diff exceeds maximum allowed size of {} bytes",
            MAX_DIFF_BYTES
        )));
    }

    let path = sandbox.resolve(&args.path).context("resolving path")?;

    let original = tokio::fs::read_to_string(&path)
        .await
        .context("reading file")?;

    // Parse the unified diff; borrow args.unified_diff for the lifetime of the parse.
    // patch-0.7 panics (rather than returning Err) on inputs with trailing
    // unrecognized content — common with LLM-generated diffs that include
    // stray prose or duplicate hunks. Wrap the parse in catch_unwind so a
    // panic surfaces as a clean conflict response instead of crashing the
    // worker thread.
    let parsed = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        patch::Patch::from_single(&args.unified_diff)
    })) {
        Ok(Ok(p)) => p,
        Ok(Err(e)) => {
            return Ok(FsApplyPatchResult::conflict(format!("malformed diff: {e}")));
        }
        Err(panic) => {
            let msg = panic
                .downcast_ref::<&'static str>()
                .map(|s| (*s).to_string())
                .or_else(|| panic.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "patch parser panicked".to_string());
            return Ok(FsApplyPatchResult::conflict(format!(
                "malformed diff (parser panicked): {msg}"
            )));
        }
    };

    // Cap the number of hunks to guard against memory exhaustion.
    if parsed.hunks.len() > MAX_HUNK_COUNT {
        return Ok(FsApplyPatchResult::conflict(format!(
            "diff contains {} hunks, exceeding maximum of {}",
            parsed.hunks.len(),
            MAX_HUNK_COUNT
        )));
    }

    // Split original into lines WITHOUT their trailing newlines (matching what
    // the patch crate stores in Line::Context/Remove).  We track the final
    // newline separately via `end_newline`.
    let orig_lines: Vec<&str> = original.lines().collect();
    let file_ends_with_newline = original.ends_with('\n');

    let mut out: Vec<&str> = Vec::with_capacity(orig_lines.len());
    // old_line is 0-indexed cursor into orig_lines; hunks use 1-indexed start.
    let mut old_line: usize = 0;

    // Count of `+` lines across all hunks — used as lines_changed metric.
    // Counting Add lines is a stable, easy-to-explain metric for "lines changed".
    let mut add_count: usize = 0;

    for hunk in &parsed.hunks {
        // Convert u64 hunk fields to usize; reject on truncation (32-bit targets).
        let hunk_start = match usize::try_from(hunk.old_range.start) {
            Ok(v) => v,
            Err(_) => {
                return Ok(FsApplyPatchResult::conflict(
                    "hunk old_range.start overflows usize".to_string(),
                ));
            }
        };
        let hunk_old_count = match usize::try_from(hunk.old_range.count) {
            Ok(v) => v,
            Err(_) => {
                return Ok(FsApplyPatchResult::conflict(
                    "hunk old_range.count overflows usize".to_string(),
                ));
            }
        };

        // hunk_start == 0 can happen for empty-file diffs; treat as line 1.
        let target = if hunk_start == 0 { 0 } else { hunk_start - 1 };

        // Validate that the hunk doesn't reference lines beyond EOF.
        if target > orig_lines.len() {
            return Ok(FsApplyPatchResult::conflict(format!(
                "hunk targets line {} but file has only {} lines",
                hunk_start,
                orig_lines.len()
            )));
        }

        // Use checked_add to prevent overflow when hunk_old_count is huge.
        match target.checked_add(hunk_old_count) {
            Some(end) if end <= orig_lines.len() => {}
            _ => {
                let last = hunk_start.saturating_add(hunk_old_count).saturating_sub(1);
                return Ok(FsApplyPatchResult::conflict(format!(
                    "hunk range {}-{} extends beyond file length {}",
                    hunk_start,
                    last,
                    orig_lines.len()
                )));
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
                        return Ok(FsApplyPatchResult::conflict(format!(
                            "context mismatch at line {}: expected {:?}, got {:?}",
                            old_line + 1,
                            ctx,
                            file_line
                        )));
                    }
                    out.push(orig_lines[old_line]);
                    old_line += 1;
                }
                Line::Remove(rem) => {
                    // Remove line must also match.
                    if old_line >= orig_lines.len() || orig_lines[old_line] != *rem {
                        let file_line = orig_lines.get(old_line).copied().unwrap_or("<eof>");
                        return Ok(FsApplyPatchResult::conflict(format!(
                            "remove mismatch at line {}: expected {:?}, got {:?}",
                            old_line + 1,
                            rem,
                            file_line
                        )));
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

    tokio::fs::write(&path, new_content.as_bytes())
        .await
        .context("writing patched file")?;

    Ok(FsApplyPatchResult::ok(add_count))
}

// ---- fs_search ---------------------------------------------------------------

/// Hard ceiling on the number of matches that can be requested via `max`.
/// Callers specifying a larger value are silently clamped to this.
const MAX_SEARCH_RESULTS: usize = 10_000;

/// Truncate rg stderr to this many bytes before embedding it in an error
/// message, to avoid propagating multi-megabyte crash dumps.
const MAX_STDERR_DISPLAY: usize = 1024;

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
/// the sandbox root. Streams stdout line-by-line; stops reading and kills the
/// child once `max` match records have been collected. Returns
/// `{ matches, truncated }`.
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
    cmd.arg("--max-filesize=10M");
    if let Some(ref g) = args.glob {
        cmd.arg("--glob").arg(g);
    }
    // Use -e PATTERN instead of a bare positional arg to prevent argument
    // injection: a pattern starting with `--` would otherwise be parsed by rg
    // as a flag (e.g. `--pre=/bin/sh` invokes an external preprocessor).
    cmd.arg("-e").arg(&args.pattern);
    cmd.current_dir(sandbox.root());

    // Explicitly isolate the child's stdio from the parent's.
    // In stdio-MCP mode the parent's stdin/stdout carry the JSON-RPC channel;
    // if rg inherits them it deadlocks trying to read from the protocol stream.
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    // stderr piped so rg diagnostics can surface in error messages without leaking to the JSON-RPC channel.
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().context("spawning rg")?;

    // Drain stderr concurrently into a capped buffer so it is available for
    // error messages and can never deadlock the stdout reader (a child blocked
    // writing stderr would stall the whole operation if we drained stdout only).
    let stderr_handle =
        crate::tools::util::drain_stderr_capped(child.stderr.take().expect("stderr is piped"));

    let stdout = child.stdout.take().expect("stdout is piped");
    let mut reader = tokio::io::BufReader::new(stdout).lines();

    let mut matches: Vec<FsMatch> = Vec::new();
    let mut truncated = false;

    'outer: while let Some(line) = reader.next_line().await? {
        if line.is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v["type"] != "match" {
            continue;
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
                    truncated = true;
                    break 'outer;
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
    }

    // Kill the child — a no-op if it already exited (e.g. after streaming its
    // full output).  We must not leave it running when we break early.
    let _ = child.kill().await;
    let status = child.wait().await.context("waiting for rg")?;

    // Collect stderr (the drainer task should be nearly done by now).
    let stderr_bytes = stderr_handle.await.unwrap_or_default();

    // exit 0 = matches found; exit 1 = no matches (normal); anything else is an error.
    // A signal kill yields code() == None.  We killed the child ourselves on
    // early exit, so None is expected in the truncated path — treat it as OK.
    if !status.success() && status.code() != Some(1) && !(truncated && status.code().is_none()) {
        let display_len = stderr_bytes.len().min(MAX_STDERR_DISPLAY);
        let stderr_snippet = String::from_utf8_lossy(&stderr_bytes[..display_len]);
        anyhow::bail!("rg failed: {}", stderr_snippet);
    }

    Ok(FsSearchResult { matches, truncated })
}
