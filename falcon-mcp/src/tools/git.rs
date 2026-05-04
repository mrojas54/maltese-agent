//! Git tools: `git_worktree_add` / `git_worktree_remove` (Task 10),
//! `git_add` / `git_commit` / `git_diff` (Task 11), and `git_log` / `git_blame`
//! (Task 12). All shell out to `git` inside the sandbox; writable-mutating
//! operations are gated through [`Sandbox::check_writable`].

use crate::sandbox::Sandbox;
use crate::tools::util::{format_stderr_for_bail, OkResult};
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

// ---- git_add -----------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GitAddArgs {
    /// Paths to stage, each resolved against the sandbox root.
    pub paths: Vec<String>,
}

/// Stage one or more paths via `git add`. Each path is run through
/// [`Sandbox::resolve`] before being handed to git so traversal-style inputs
/// (`../something`) are rejected at the jail boundary, not inside git.
pub async fn git_add(sandbox: Arc<Sandbox>, args: GitAddArgs) -> anyhow::Result<OkResult> {
    sandbox.check_writable()?;
    sandbox.check_bin("git")?;

    let mut cmd = git_command();
    cmd.arg("add");
    for p in &args.paths {
        let resolved = sandbox.resolve(p).context("resolving add path")?;
        cmd.arg(&resolved);
    }
    cmd.current_dir(sandbox.root());

    let out = cmd.output().await.context("running git add")?;
    if !out.status.success() {
        let stderr = format_stderr_for_bail(&out.stderr);
        anyhow::bail!("git add failed: {stderr}");
    }
    Ok(OkResult::ok())
}

// ---- git_commit --------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GitCommitArgs {
    /// Commit message. Passed verbatim as `-m <message>`.
    pub message: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GitCommitResult {
    /// The full 40-char SHA of the new commit.
    pub sha: String,
}

/// Run `git commit -m <message>` against the sandbox root, then resolve the
/// new HEAD's SHA. Author/committer are pinned to `falcon-detective@local`
/// via inline `-c user.email/user.name` so the agent's commits don't depend
/// on whatever git identity the host happens to have configured.
pub async fn git_commit(
    sandbox: Arc<Sandbox>,
    args: GitCommitArgs,
) -> anyhow::Result<GitCommitResult> {
    sandbox.check_writable()?;
    sandbox.check_bin("git")?;

    let out = git_command()
        .arg("-c")
        .arg("user.email=falcon-detective@local")
        .arg("-c")
        .arg("user.name=falcon-detective")
        .arg("commit")
        .arg("-m")
        .arg(&args.message)
        .current_dir(sandbox.root())
        .output()
        .await
        .context("running git commit")?;
    if !out.status.success() {
        let stderr = format_stderr_for_bail(&out.stderr);
        anyhow::bail!("git commit failed: {stderr}");
    }

    let sha_out = git_command()
        .arg("rev-parse")
        .arg("HEAD")
        .current_dir(sandbox.root())
        .output()
        .await
        .context("resolving HEAD sha")?;
    if !sha_out.status.success() {
        let stderr = format_stderr_for_bail(&sha_out.stderr);
        anyhow::bail!("git rev-parse HEAD failed: {stderr}");
    }
    Ok(GitCommitResult {
        sha: String::from_utf8_lossy(&sha_out.stdout).trim().to_string(),
    })
}

// ---- git_diff ----------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GitDiffArgs {
    /// Optional revision (or revision range) to diff against. When omitted,
    /// returns the unstaged working-tree diff (`git diff` with no args).
    #[serde(default)]
    pub rev: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GitDiffResult {
    /// Unified-diff text exactly as `git diff` produced it.
    pub diff: String,
}

/// Run `git diff [rev]` and return its stdout verbatim. This is a read-only
/// operation, so it works in a read-only sandbox. Non-zero exit from `git
/// diff` is surfaced as a tool-level error (it doesn't differentiate "no
/// changes" — that's a clean exit with empty stdout).
pub async fn git_diff(sandbox: Arc<Sandbox>, args: GitDiffArgs) -> anyhow::Result<GitDiffResult> {
    sandbox.check_bin("git")?;

    let mut cmd = git_command();
    cmd.arg("diff");
    if let Some(r) = &args.rev {
        cmd.arg(r);
    }
    cmd.current_dir(sandbox.root());

    let out = cmd.output().await.context("running git diff")?;
    if !out.status.success() {
        let stderr = format_stderr_for_bail(&out.stderr);
        anyhow::bail!("git diff failed: {stderr}");
    }
    Ok(GitDiffResult {
        diff: String::from_utf8_lossy(&out.stdout).into_owned(),
    })
}

// ---- git_log -----------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GitLogArgs {
    /// Optional path filter forwarded as `-- <path>` to git.
    #[serde(default)]
    pub path: Option<String>,
    /// Max commits to return. Defaults to 20.
    #[serde(default = "default_n")]
    pub n: usize,
}

fn default_n() -> usize {
    20
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GitCommit {
    /// Full 40-char commit SHA.
    pub sha: String,
    /// Author identity in the form `Name <email>`.
    pub author: String,
    /// Author date in ISO-8601 format (`%aI`).
    pub date: String,
    /// First line of the commit message (`%s`).
    pub subject: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GitLogResult {
    pub commits: Vec<GitCommit>,
}

/// Run `git log -n<n> --pretty=format:%H<TAB>%an <%ae><TAB>%aI<TAB>%s` and
/// parse the tab-separated lines. Tab separation is safe here: commit SHAs,
/// ISO dates, and the rendered author identity are all tab-free, and the
/// subject is the last field so any tabs it contains stay attached to it.
pub async fn git_log(sandbox: Arc<Sandbox>, args: GitLogArgs) -> anyhow::Result<GitLogResult> {
    sandbox.check_bin("git")?;

    let fmt = "--pretty=format:%H%x09%an <%ae>%x09%aI%x09%s";
    let mut cmd = git_command();
    cmd.arg("log").arg(format!("-n{}", args.n)).arg(fmt);
    if let Some(p) = &args.path {
        let resolved = sandbox.resolve(p).context("resolving log path")?;
        cmd.arg("--").arg(&resolved);
    }
    cmd.current_dir(sandbox.root());

    let out = cmd.output().await.context("running git log")?;
    if !out.status.success() {
        let stderr = format_stderr_for_bail(&out.stderr);
        anyhow::bail!("git log failed: {stderr}");
    }

    let mut commits = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let parts: Vec<&str> = line.splitn(4, '\t').collect();
        if parts.len() < 4 {
            continue;
        }
        commits.push(GitCommit {
            sha: parts[0].to_string(),
            author: parts[1].to_string(),
            date: parts[2].to_string(),
            subject: parts[3].to_string(),
        });
    }
    Ok(GitLogResult { commits })
}

// ---- git_blame ---------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GitBlameArgs {
    /// File path (relative to sandbox root) to blame.
    pub path: String,
    /// 1-based line number to blame.
    pub line: u32,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GitBlameResult {
    /// SHA of the commit that last touched this line.
    pub sha: String,
    /// Author name as recorded by git (no email, matches `author` porcelain field).
    pub author: String,
    /// Unix epoch seconds of the author timestamp (porcelain `author-time`).
    pub date: String,
    /// The actual source line text (the porcelain line prefixed by a tab).
    pub line_text: String,
}

/// Blame a single line via `git blame --porcelain -L<line>,<line>` and parse
/// the porcelain header for sha/author/author-time plus the tab-prefixed
/// content line. Porcelain output starts with `<sha> <orig> <final> <count>`,
/// followed by header fields, followed by a `\t<source line>` entry.
pub async fn git_blame(
    sandbox: Arc<Sandbox>,
    args: GitBlameArgs,
) -> anyhow::Result<GitBlameResult> {
    sandbox.check_bin("git")?;
    let path = sandbox
        .resolve(&args.path)
        .context("resolving blame path")?;

    let out = git_command()
        .arg("blame")
        .arg("--porcelain")
        .arg("-L")
        .arg(format!("{},{}", args.line, args.line))
        .arg(&path)
        .current_dir(sandbox.root())
        .output()
        .await
        .context("running git blame")?;
    if !out.status.success() {
        let stderr = format_stderr_for_bail(&out.stderr);
        anyhow::bail!("git blame failed: {stderr}");
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let mut sha = String::new();
    let mut author = String::new();
    let mut date = String::new();
    let mut line_text = String::new();
    for line in text.lines() {
        if sha.is_empty() && line.len() >= 40 {
            sha = line[..40].to_string();
        } else if let Some(rest) = line.strip_prefix("author ") {
            author = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("author-time ") {
            date = rest.to_string();
        } else if let Some(rest) = line.strip_prefix('\t') {
            line_text = rest.to_string();
        }
    }
    Ok(GitBlameResult {
        sha,
        author,
        date,
        line_text,
    })
}
