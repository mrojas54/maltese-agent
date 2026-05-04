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
