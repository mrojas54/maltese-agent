//! Filesystem tools (basic): read, write, list, search, apply_patch.
//! Task 3 implements only `fs_read`; the rest land in Tasks 4-6.

use crate::sandbox::Sandbox;
use anyhow::Context;
use patch::Line;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
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
    Ok(FsReadResult { bytes: len, content })
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
        tokio::fs::create_dir_all(parent).await.context("creating parent dirs")?;
    }
    let bytes = args.content.as_bytes();
    tokio::fs::write(&path, bytes).await.context("writing file")?;
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

    let mut rd = tokio::fs::read_dir(&path).await.context("reading directory")?;
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
    /// `"ok"` if the patch was applied, `"conflict"` if it was rejected.
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
        Self { result: "ok".into(), lines_changed: Some(lines_changed), reason: None }
    }

    fn conflict(reason: impl Into<String>) -> Self {
        Self { result: "conflict".into(), lines_changed: None, reason: Some(reason.into()) }
    }
}

/// Apply a unified diff to a file inside the sandbox.
///
/// The algorithm:
///  1. Parse the diff with the `patch` crate.
///  2. Walk hunks in order, verifying that context lines match the file.
///  3. Reconstruct the output by interleaving unchanged lines with hunk output.
///  4. Write the result back to disk.
///
/// Returns `Conflict` for: malformed diff, context-line mismatch, or hunk
/// targeting a line range outside the file.  Never panics on user-controlled
/// input.
pub async fn fs_apply_patch(
    sandbox: Arc<Sandbox>,
    args: FsApplyPatchArgs,
) -> anyhow::Result<FsApplyPatchResult> {
    sandbox.check_writable()?;
    let path = sandbox.resolve(&args.path).context("resolving path")?;

    let original = tokio::fs::read_to_string(&path)
        .await
        .context("reading file")?;

    // Parse the unified diff; borrow args.unified_diff for the lifetime of the parse.
    let parsed = match patch::Patch::from_single(&args.unified_diff) {
        std::result::Result::Ok(p) => p,
        Err(e) => {
            return Ok(FsApplyPatchResult::conflict(format!("malformed diff: {e}")));
        }
    };

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
        let hunk_start = hunk.old_range.start as usize; // 1-indexed
        let hunk_old_count = hunk.old_range.count as usize;

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
        if target + hunk_old_count > orig_lines.len() {
            return Ok(FsApplyPatchResult::conflict(format!(
                "hunk range {}-{} extends beyond file length {}",
                hunk_start,
                hunk_start + hunk_old_count - 1,
                orig_lines.len()
            )));
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
