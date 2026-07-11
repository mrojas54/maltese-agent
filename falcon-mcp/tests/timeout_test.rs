//! Integration tests for wall-clock subprocess timeouts (AC-10, AC-11),
//! exercised through the real MCP stdio transport against a spawned
//! falcon-mcp child process — one test per child-spawning tool family
//! (cargo, git, search, exec), each against a deliberately hanging fixture
//! with a low timeout configured via the `FALCON_MCP_<NAME>_MS` env override
//! on the spawned server (which is itself the AC-11 override surface).
//!
//! Fixtures:
//! - `tests/fixtures/hanging-crate/` — crate whose build.rs sleeps, so
//!   `cargo check` (real cargo) can only end via the timeout kill.
//! - a hanging `pre-commit` hook written into a TempDir repo, so
//!   `git commit` (real git) blocks until killed.
//! - `tests/fixtures/hanging-bin/` — fake `rg`/`ast-grep` shell scripts that
//!   sleep; prepended to the server's PATH. `fs_search_ast` still spawns
//!   `ast-grep`, so `Sandbox::check_bin` validates the NAME only and
//!   resolution happens at spawn via the child's PATH; for exec_run the
//!   allowlist resolves to absolute paths at server startup (AC-21) — the
//!   fixture dir is on the spawned server's startup PATH, so it pins the fake
//!   either way. (`fs_search` is native since MA-35 and spawns nothing.)
//!
//! Every test doubles as a kill/reap check: the fixtures sleep 120s, so the
//! test finishing in a couple of seconds proves the child was killed rather
//! than awaited to completion.

use rmcp::{
    model::CallToolRequestParams,
    transport::{ConfigureCommandExt, TokioChildProcess},
    ServiceExt,
};
use serde_json::json;
use tempfile::TempDir;
use tokio::process::Command;

/// Directory containing the hanging fake `rg` / `ast-grep` executables.
fn hanging_bin_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hanging-bin")
}

/// PATH value with the hanging-bin fixture dir prepended.
fn hanging_path() -> String {
    format!(
        "{}:{}",
        hanging_bin_dir().display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

/// Spawn a falcon-mcp child rooted at `root` with extra env vars applied.
async fn spawn_server(
    root: &std::path::Path,
    envs: &[(&str, &str)],
    enable_exec: bool,
) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let cmd = Command::new(env!("CARGO_BIN_EXE_falcon-mcp")).configure(|c| {
        c.arg("--stdio").arg("--root").arg(root).kill_on_drop(true);
        if enable_exec {
            c.arg("--enable-exec");
        }
        for (k, v) in envs {
            c.env(k, v);
        }
    });
    ().serve(TokioChildProcess::new(cmd).unwrap())
        .await
        .expect("connect to falcon-mcp")
}

/// Call `tool` with `args` and return the tool-level error text; panics if
/// the call succeeded. Timeout errors surface as tool-level errors (the
/// handler maps the anyhow chain via `format!("{e:#}")`), so the structured
/// `elapsed_ms=` / `limit_ms=` fields must appear in this text.
async fn expect_tool_error(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    tool: &str,
    args: serde_json::Value,
) -> String {
    let r = client
        .call_tool(
            CallToolRequestParams::new(tool.to_string())
                .with_arguments(args.as_object().unwrap().clone()),
        )
        .await;
    match r {
        Err(e) => format!("{e:?}"),
        Ok(resp) => {
            assert!(
                resp.is_error.unwrap_or(false),
                "expected {tool} to fail with a timeout error, got success: {resp:?}"
            );
            format!("{:?}", resp.content)
        }
    }
}

fn assert_structured_timeout(text: &str, limit_ms: u64) {
    assert!(
        text.contains("elapsed_ms="),
        "timeout error must carry elapsed_ms, got: {text}"
    );
    assert!(
        text.contains(&format!("limit_ms={limit_ms}")),
        "timeout error must carry limit_ms={limit_ms}, got: {text}"
    );
}

/// cargo family (AC-10): `cargo_check` against the hanging-build-script
/// fixture, with the CARGO_TIMEOUT default overridden down to 2s via env
/// (AC-11 env-overridability). Real cargo gets killed mid-hang.
#[tokio::test]
async fn cargo_check_times_out_on_hanging_build_script() {
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hanging-crate");
    let client = spawn_server(&fixture, &[("FALCON_MCP_CARGO_TIMEOUT_MS", "2000")], false).await;

    let start = std::time::Instant::now();
    let text = expect_tool_error(&client, "cargo_check", json!({"crate_path": "."})).await;
    assert_structured_timeout(&text, 2000);
    assert!(
        start.elapsed() < std::time::Duration::from_secs(60),
        "cargo_check was not killed promptly ({}s)",
        start.elapsed().as_secs()
    );
    client.cancel().await.unwrap();
}

/// git family (AC-10): `git_commit` blocked by a hanging pre-commit hook,
/// with GIT_TIMEOUT overridden down to 1s. Real git gets killed mid-hook.
#[cfg(unix)]
#[tokio::test]
async fn git_commit_times_out_on_hanging_pre_commit_hook() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = TempDir::new().unwrap();
    let run_git = |args: &[&str]| {
        let out = std::process::Command::new("git")
            .args(args)
            .current_dir(dir.path())
            .output()
            .unwrap_or_else(|e| panic!("git {args:?} failed to spawn: {e}"));
        assert!(out.status.success(), "git {args:?} failed: {out:?}");
    };
    run_git(&["init", "-b", "main"]);
    std::fs::write(dir.path().join("a.txt"), "content\n").unwrap();

    let hook = dir.path().join(".git/hooks/pre-commit");
    std::fs::write(
        &hook,
        "#!/bin/sh\n# hanging hook (timeout fixture)\nsleep 120\n",
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();

    let client = spawn_server(dir.path(), &[("FALCON_MCP_GIT_TIMEOUT_MS", "1000")], false).await;

    // Staging is fast and must still succeed under the low git timeout.
    let add = client
        .call_tool(
            CallToolRequestParams::new("git_add")
                .with_arguments(json!({"paths": ["a.txt"]}).as_object().unwrap().clone()),
        )
        .await
        .expect("call git_add");
    assert!(!add.is_error.unwrap_or(false), "git_add failed: {add:?}");

    let text = expect_tool_error(&client, "git_commit", json!({"message": "hang"})).await;
    assert_structured_timeout(&text, 1000);
    client.cancel().await.unwrap();
}

// NOTE: fs_search no longer spawns a subprocess (MA-35 made it native), so
// there is no `rg` child to hang — its wall-clock bound is now a per-file
// deadline check, not a subprocess kill. The search *family*'s AC-10
// subprocess-timeout coverage is carried by the ast-grep half below, which
// is still a real child process.

/// search family, ast-grep half (AC-10): fs_search_ast against a hanging fake
/// ast-grep resolved via PATH prepend, SEARCH_TIMEOUT overridden down to 1s.
/// Works even on hosts without a real ast-grep — the fake IS the binary.
#[cfg(unix)]
#[tokio::test]
async fn fs_search_ast_times_out_on_hanging_ast_grep() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("f.rs"), "fn main() {}\n").unwrap();

    let path = hanging_path();
    let client = spawn_server(
        dir.path(),
        &[
            ("FALCON_MCP_SEARCH_TIMEOUT_MS", "1000"),
            ("PATH", path.as_str()),
        ],
        false,
    )
    .await;

    let text = expect_tool_error(
        &client,
        "fs_search_ast",
        json!({"query": "$E.unwrap()", "lang": "rust"}),
    )
    .await;
    assert_structured_timeout(&text, 1000);
    client.cancel().await.unwrap();
}

/// exec family (AC-11): the per-call `timeout_ms` override in exec_run's
/// input schema beats the module default (30s) — the hanging fake rg is
/// killed after the caller's 500ms, not the default, which is proven both by
/// `limit_ms=500` in the error and by the test finishing in well under 30s.
#[cfg(unix)]
#[tokio::test]
async fn exec_run_honors_per_call_timeout_ms_override() {
    let dir = TempDir::new().unwrap();

    let path = hanging_path();
    let client = spawn_server(dir.path(), &[("PATH", path.as_str())], true).await;

    let start = std::time::Instant::now();
    let text = expect_tool_error(
        &client,
        "exec_run",
        json!({"cmd": "rg", "args": [], "timeout_ms": 500}),
    )
    .await;
    assert_structured_timeout(&text, 500);
    assert!(
        start.elapsed() < std::time::Duration::from_secs(20),
        "per-call timeout_ms=500 was not honored ({}ms elapsed)",
        start.elapsed().as_millis()
    );
    client.cancel().await.unwrap();
}
