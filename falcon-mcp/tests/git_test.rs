//! Integration tests for the git tools, exercised through the real MCP stdio
//! transport against a spawned falcon-mcp child process. Each test stands up
//! a fresh temp-dir repo via [`init_git`] so worktree state never leaks
//! between tests.

use rmcp::{
    ServiceExt,
    model::CallToolRequestParams,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use serde_json::json;
use tempfile::TempDir;
use tokio::process::Command;

/// Spawn a falcon-mcp child rooted at `dir`.
async fn spawn_at(dir: &std::path::Path) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let cmd = Command::new(env!("CARGO_BIN_EXE_falcon-mcp")).configure(|c| {
        c.arg("--stdio").arg("--root").arg(dir).kill_on_drop(true);
    });
    ()
        .serve(TokioChildProcess::new(cmd).unwrap())
        .await
        .expect("connect to falcon-mcp")
}

/// Initialize a fresh git repo at `path` with one committed file. Uses inline
/// `-c user.email/user.name` so the test doesn't depend on whatever git
/// identity is configured on the host (CI has none by default).
fn init_git(path: &std::path::Path) {
    let run = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(path)
            .output()
            .unwrap_or_else(|e| panic!("git {args:?} failed: {e}"))
    };
    run(&["init", "-b", "main"]);
    std::fs::write(path.join("a.txt"), "hi").unwrap();
    run(&[
        "-c",
        "user.email=t@t",
        "-c",
        "user.name=t",
        "add",
        ".",
    ]);
    run(&[
        "-c",
        "user.email=t@t",
        "-c",
        "user.name=t",
        "commit",
        "-m",
        "initial",
    ]);
}

/// `git_worktree_add` should create a worktree under `.runs/<name>` and
/// return its absolute path. Asserting the path actually exists on disk
/// pins both the call shape and the side effect.
#[tokio::test]
async fn worktree_add_creates_path() {
    let dir = TempDir::new().unwrap();
    init_git(dir.path());
    let client = spawn_at(dir.path()).await;

    let r = client
        .call_tool(
            CallToolRequestParams::new("git_worktree_add").with_arguments(
                json!({"name": "demo"}).as_object().unwrap().clone(),
            ),
        )
        .await
        .expect("call git_worktree_add");

    assert!(
        !r.is_error.unwrap_or(false),
        "git_worktree_add returned tool-level error: {r:?}"
    );
    let out = r.structured_content.expect("structured result");
    let p = std::path::Path::new(out["path"].as_str().expect("path string"));
    assert!(p.exists(), "expected worktree path to exist on disk: {p:?}");
    client.cancel().await.unwrap();
}

/// Round-trip: stage a modified file with `git_add`, commit it with
/// `git_commit`, then verify the returned SHA actually exists. Pins the
/// 40-char SHA shape and the staged-index → commit handoff. Also exercises
/// `git_diff` against the prior revision so the diff text contains the
/// modification we made.
#[tokio::test]
async fn add_commit_diff_round_trip() {
    let dir = TempDir::new().unwrap();
    init_git(dir.path());
    let client = spawn_at(dir.path()).await;

    // Modify the seed file so there's something to stage.
    std::fs::write(dir.path().join("a.txt"), "hi\nthere\n").unwrap();

    let r_add = client
        .call_tool(
            CallToolRequestParams::new("git_add").with_arguments(
                json!({"paths": ["a.txt"]}).as_object().unwrap().clone(),
            ),
        )
        .await
        .expect("call git_add");
    assert!(
        !r_add.is_error.unwrap_or(false),
        "git_add returned tool-level error: {r_add:?}"
    );

    let r_commit = client
        .call_tool(
            CallToolRequestParams::new("git_commit").with_arguments(
                json!({"message": "test commit"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .expect("call git_commit");
    assert!(
        !r_commit.is_error.unwrap_or(false),
        "git_commit returned tool-level error: {r_commit:?}"
    );
    let sha = r_commit.structured_content.unwrap()["sha"]
        .as_str()
        .expect("sha string")
        .to_string();
    assert_eq!(sha.len(), 40, "expected 40-char SHA, got: {sha:?}");

    // The new commit should resolve via `git cat-file` against the host repo.
    let cat = std::process::Command::new("git")
        .args(["cat-file", "-t", &sha])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&cat.stdout).trim(),
        "commit",
        "expected sha to be a commit object, got: {cat:?}"
    );

    // git_diff against HEAD~1 should mention the file we touched.
    let r_diff = client
        .call_tool(
            CallToolRequestParams::new("git_diff").with_arguments(
                json!({"rev": "HEAD~1"}).as_object().unwrap().clone(),
            ),
        )
        .await
        .expect("call git_diff");
    assert!(
        !r_diff.is_error.unwrap_or(false),
        "git_diff returned tool-level error: {r_diff:?}"
    );
    let diff = r_diff.structured_content.unwrap()["diff"]
        .as_str()
        .expect("diff string")
        .to_string();
    assert!(
        diff.contains("a.txt"),
        "expected diff to mention a.txt, got: {diff:?}"
    );
    client.cancel().await.unwrap();
}

/// Round-trip: add a worktree, then remove it. `git_worktree_remove` returns
/// `{ok: true}` and the directory should be gone afterwards.
#[tokio::test]
async fn worktree_remove_round_trip() {
    let dir = TempDir::new().unwrap();
    init_git(dir.path());
    let client = spawn_at(dir.path()).await;

    let added = client
        .call_tool(
            CallToolRequestParams::new("git_worktree_add").with_arguments(
                json!({"name": "to_remove"}).as_object().unwrap().clone(),
            ),
        )
        .await
        .expect("call git_worktree_add");
    let path = added.structured_content.unwrap()["path"]
        .as_str()
        .unwrap()
        .to_string();

    let r = client
        .call_tool(
            CallToolRequestParams::new("git_worktree_remove").with_arguments(
                json!({"path": ".runs/to_remove"}).as_object().unwrap().clone(),
            ),
        )
        .await
        .expect("call git_worktree_remove");

    assert!(
        !r.is_error.unwrap_or(false),
        "git_worktree_remove returned tool-level error: {r:?}"
    );
    let out = r.structured_content.expect("structured result");
    assert_eq!(out["ok"].as_bool(), Some(true));
    assert!(
        !std::path::Path::new(&path).exists(),
        "expected worktree path to be removed: {path}"
    );
    client.cancel().await.unwrap();
}

/// `git_log` against a fresh repo with one commit must return that commit
/// in the structured `commits` array, with a 40-char SHA and a non-empty
/// subject. Tab-separated parsing means a malformed format string would
/// produce empty fields, so we assert non-empty as a regression guard.
#[tokio::test]
async fn log_returns_commits() {
    let dir = TempDir::new().unwrap();
    init_git(dir.path());
    let client = spawn_at(dir.path()).await;

    let r = client
        .call_tool(
            CallToolRequestParams::new("git_log").with_arguments(
                json!({"n": 5}).as_object().unwrap().clone(),
            ),
        )
        .await
        .expect("call git_log");
    assert!(
        !r.is_error.unwrap_or(false),
        "git_log returned tool-level error: {r:?}"
    );

    let commits = r.structured_content.unwrap()["commits"]
        .as_array()
        .expect("commits array")
        .clone();
    assert!(!commits.is_empty(), "expected at least one commit");
    let head = &commits[0];
    assert_eq!(
        head["sha"].as_str().map(|s| s.len()),
        Some(40),
        "expected 40-char SHA in head commit, got: {head:?}"
    );
    assert!(
        !head["subject"].as_str().unwrap_or("").is_empty(),
        "expected non-empty subject, got: {head:?}"
    );
    client.cancel().await.unwrap();
}

/// `git_blame` on the seed file's first line must return the initial commit's
/// SHA and the original line text. Pins the porcelain parser's three load-
/// bearing fields (sha, author, line_text). The init_git fixture's identity
/// is `t <t@t>` (porcelain "author t").
#[tokio::test]
async fn blame_returns_line_metadata() {
    let dir = TempDir::new().unwrap();
    init_git(dir.path());
    let client = spawn_at(dir.path()).await;

    let r = client
        .call_tool(
            CallToolRequestParams::new("git_blame").with_arguments(
                json!({"path": "a.txt", "line": 1})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .expect("call git_blame");
    assert!(
        !r.is_error.unwrap_or(false),
        "git_blame returned tool-level error: {r:?}"
    );

    let out = r.structured_content.expect("structured result");
    assert_eq!(
        out["sha"].as_str().map(|s| s.len()),
        Some(40),
        "expected 40-char SHA, got: {out:?}"
    );
    assert_eq!(
        out["author"].as_str(),
        Some("t"),
        "expected author 't', got: {out:?}"
    );
    assert_eq!(
        out["line_text"].as_str(),
        Some("hi"),
        "expected line_text 'hi', got: {out:?}"
    );
    client.cancel().await.unwrap();
}
