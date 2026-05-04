//! Integration tests for the `exec_run` tool. Each test spins up a real
//! falcon-mcp child process over stdio so the `--enable-exec` CLI gate is
//! exercised end-to-end, not just at the library level.

use rmcp::{
    ServiceExt,
    model::CallToolRequestParams,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use serde_json::json;
use tempfile::TempDir;
use tokio::process::Command;

/// Spawn a falcon-mcp child rooted at `dir`. Pass `with_exec=true` to add
/// `--enable-exec`, which is what flips the gate from rejecting every call
/// to actually running the allowlisted binary.
async fn spawn_at(
    dir: &std::path::Path,
    with_exec: bool,
) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let cmd = Command::new(env!("CARGO_BIN_EXE_falcon-mcp")).configure(|c| {
        c.arg("--stdio").arg("--root").arg(dir).kill_on_drop(true);
        if with_exec {
            c.arg("--enable-exec");
        }
    });
    ()
        .serve(TokioChildProcess::new(cmd).unwrap())
        .await
        .expect("connect to falcon-mcp")
}

/// Without `--enable-exec`, even an allowlisted command must be rejected.
/// This pins the safety default: the tool is registered but refuses to run.
#[tokio::test]
async fn exec_disabled_by_default() {
    let dir = TempDir::new().unwrap();
    let client = spawn_at(dir.path(), false).await;

    let r = client
        .call_tool(
            CallToolRequestParams::new("exec_run").with_arguments(
                json!({"cmd": "git", "args": ["--version"]})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await;

    // The gate surfaces as a tool-level error string. `call_tool` may either
    // return Err (transport/protocol-level error) or Ok with is_error=true
    // (tool-handler-level error). Either form is acceptable here as long as
    // the call did NOT succeed.
    let denied = match r {
        Err(_) => true,
        Ok(resp) => resp.is_error.unwrap_or(false),
    };
    assert!(denied, "exec_run should be denied without --enable-exec");

    client.cancel().await.unwrap();
}

/// With `--enable-exec`, an allowlisted command (`git --version`) actually
/// runs and we get its stdout back through structured_content.
#[tokio::test]
async fn exec_runs_allowlisted_command() {
    let dir = TempDir::new().unwrap();
    let client = spawn_at(dir.path(), true).await;

    let r = client
        .call_tool(
            CallToolRequestParams::new("exec_run").with_arguments(
                json!({"cmd": "git", "args": ["--version"]})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .expect("call exec_run");

    assert!(
        !r.is_error.unwrap_or(false),
        "exec_run returned tool-level error: {r:?}"
    );
    let out = r.structured_content.expect("structured result");
    let stdout = out["stdout"].as_str().expect("stdout string");
    assert!(
        stdout.starts_with("git version"),
        "expected stdout to start with 'git version', got: {stdout:?}"
    );
    assert_eq!(out["exit"].as_i64().expect("exit code"), 0);

    client.cancel().await.unwrap();
}

/// Even with `--enable-exec`, a binary that isn't on the sandbox allowlist
/// (`curl`) must be rejected by `Sandbox::check_bin` before we ever spawn it.
/// This is the second line of defense after the CLI gate.
#[tokio::test]
async fn exec_rejects_non_allowlisted_binary() {
    let dir = TempDir::new().unwrap();
    let client = spawn_at(dir.path(), true).await;

    let r = client
        .call_tool(
            CallToolRequestParams::new("exec_run").with_arguments(
                json!({"cmd": "curl", "args": ["--version"]})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await;

    let denied = match r {
        Err(_) => true,
        Ok(resp) => resp.is_error.unwrap_or(false),
    };
    assert!(denied, "exec_run should reject non-allowlisted binary 'curl'");

    client.cancel().await.unwrap();
}

/// `cwd` is sandbox-relative and must be honoured: running `git rev-parse
/// --show-toplevel` inside an init'd subdir should resolve to that subdir's
/// canonical path (which lives under the sandbox root).
#[tokio::test]
async fn exec_honours_cwd_inside_sandbox() {
    let dir = TempDir::new().unwrap();
    let sub = dir.path().join("sub");
    std::fs::create_dir(&sub).unwrap();

    // Initialise a tiny git repo in sub/ so `git rev-parse --show-toplevel`
    // produces a deterministic answer rooted there.
    let run_git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(&sub)
            .output()
            .unwrap_or_else(|e| panic!("git {args:?} failed: {e}"));
    };
    run_git(&["init", "-b", "main"]);

    let client = spawn_at(dir.path(), true).await;

    let r = client
        .call_tool(
            CallToolRequestParams::new("exec_run").with_arguments(
                json!({
                    "cmd": "git",
                    "args": ["rev-parse", "--show-toplevel"],
                    "cwd": "sub",
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .expect("call exec_run");

    assert!(
        !r.is_error.unwrap_or(false),
        "exec_run returned tool-level error: {r:?}"
    );
    let out = r.structured_content.expect("structured result");
    let stdout = out["stdout"].as_str().expect("stdout string").trim();
    let expected = sub.canonicalize().unwrap();
    assert_eq!(
        std::path::Path::new(stdout).canonicalize().unwrap(),
        expected,
        "git rev-parse --show-toplevel did not match the sub/ cwd"
    );

    client.cancel().await.unwrap();
}
