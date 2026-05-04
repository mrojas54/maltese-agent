//! Git tools: `git_worktree_add` / `git_worktree_remove` (Task 10),
//! `git_add` / `git_commit` / `git_diff` (Task 11), and `git_log` / `git_blame`
//! (Task 12). All shell out to `git` inside the sandbox; writable-mutating
//! operations are gated through [`Sandbox::check_writable`].

use crate::sandbox::Sandbox;
use crate::tools::util::{OkResult, format_stderr_for_bail};
use anyhow::Context;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Spawn a `git` subcommand with the same stdio hygiene the cargo tools use:
/// stdin detached so a child can't try to interact, stdout/stderr piped so we
/// can capture them without inheriting the MCP server's JSON-RPC channel.
fn git_command() -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new("git");
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd
}

// ---- git_worktree_add --------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WorktreeAddArgs {
    /// Worktree name. Created under `.runs/<name>` inside the sandbox root.
    pub name: String,
    /// Revision to base the new worktree on. Defaults to `HEAD`.
    #[serde(default = "default_base")]
    pub base: String,
}

fn default_base() -> String {
    "HEAD".into()
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct WorktreeAddResult {
    /// Absolute path to the new worktree on disk.
    pub path: String,
}

/// Run `git worktree add <root>/.runs/<name> <base>`. The sandbox's `resolve`
/// gate runs even though the path doesn't exist yet (it walks up to the first
/// existing ancestor and re-attaches the suffix), which is what blocks names
/// like `../escape` from creating worktrees outside the jail.
pub async fn worktree_add(
    sandbox: Arc<Sandbox>,
    args: WorktreeAddArgs,
) -> anyhow::Result<WorktreeAddResult> {
    sandbox.check_writable()?;
    sandbox.check_bin("git")?;
    let path = sandbox
        .resolve(format!(".runs/{}", args.name))
        .context("resolving worktree path")?;

    let out = git_command()
        .arg("worktree")
        .arg("add")
        .arg(&path)
        .arg(&args.base)
        .current_dir(sandbox.root())
        .output()
        .await
        .context("running git worktree add")?;
    if !out.status.success() {
        let stderr = format_stderr_for_bail(&out.stderr);
        anyhow::bail!("git worktree add failed: {stderr}");
    }
    Ok(WorktreeAddResult {
        path: path.to_string_lossy().into_owned(),
    })
}

// ---- git_worktree_remove -----------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WorktreeRemoveArgs {
    /// Path of the worktree to remove. Must resolve inside the sandbox root.
    pub path: String,
}

/// Run `git worktree remove --force <path>` and return [`OkResult`] on
/// success. Force is required so the agent can clean up worktrees with
/// uncommitted changes; the sandbox jail already prevents touching arbitrary
/// paths outside the root.
pub async fn worktree_remove(
    sandbox: Arc<Sandbox>,
    args: WorktreeRemoveArgs,
) -> anyhow::Result<OkResult> {
    sandbox.check_writable()?;
    sandbox.check_bin("git")?;
    let path = sandbox
        .resolve(&args.path)
        .context("resolving worktree path")?;

    let out = git_command()
        .arg("worktree")
        .arg("remove")
        .arg("--force")
        .arg(&path)
        .current_dir(sandbox.root())
        .output()
        .await
        .context("running git worktree remove")?;
    if !out.status.success() {
        let stderr = format_stderr_for_bail(&out.stderr);
        anyhow::bail!("git worktree remove failed: {stderr}");
    }
    Ok(OkResult::ok())
}
