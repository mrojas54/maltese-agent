//! Integration tests for the cargo tools, exercised through the real MCP
//! stdio transport against a spawned falcon-mcp child process.
//!
//! The fixture crate at `tests/fixtures/sample-crate/` is its own workspace
//! (declared via a top-level `[workspace]` table in its Cargo.toml) so that
//! its deliberately-failing `add_broken` test is NOT picked up by the parent
//! falcon-mcp `cargo test` run.

use rmcp::{
    model::CallToolRequestParams,
    transport::{ConfigureCommandExt, TokioChildProcess},
    ServiceExt,
};
use serde_json::json;
use tempfile::TempDir;
use tokio::process::Command;

/// Spawn a falcon-mcp child rooted at `tests/fixtures/sample-crate/`.
async fn spawn_in_fixture() -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-crate");
    spawn_at(&fixture).await
}

/// Spawn a falcon-mcp child rooted at the given path.
async fn spawn_at(root: &std::path::Path) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let cmd = Command::new(env!("CARGO_BIN_EXE_falcon-mcp")).configure(|c| {
        c.arg("--stdio").arg("--root").arg(root).kill_on_drop(true);
    });
    ().serve(TokioChildProcess::new(cmd).unwrap())
        .await
        .expect("connect to falcon-mcp")
}

/// Spawn a falcon-mcp child rooted at the given path with `--read-only` set.
async fn spawn_read_only_at(
    root: &std::path::Path,
) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let cmd = Command::new(env!("CARGO_BIN_EXE_falcon-mcp")).configure(|c| {
        c.arg("--stdio")
            .arg("--root")
            .arg(root)
            .arg("--read-only")
            .kill_on_drop(true);
    });
    ().serve(TokioChildProcess::new(cmd).unwrap())
        .await
        .expect("connect to falcon-mcp")
}

/// `cargo check` on the fixture (which compiles cleanly) should return zero errors.
/// Warnings are allowed — the assertion only pins the empty-errors invariant.
#[tokio::test]
async fn cargo_check_clean_returns_empty_errors() {
    let client = spawn_in_fixture().await;
    let r = client
        .call_tool(
            CallToolRequestParams::new("cargo_check")
                .with_arguments(json!({"crate_path": "."}).as_object().unwrap().clone()),
        )
        .await
        .expect("call cargo_check");

    assert!(
        !r.is_error.unwrap_or(false),
        "cargo_check returned tool-level error: {:?}",
        r
    );
    let out = r.structured_content.expect("structured result");
    let errors = out["errors"].as_array().expect("errors array");
    assert!(
        errors.is_empty(),
        "expected no compile errors for clean fixture, got: {out:?}"
    );
    client.cancel().await.unwrap();
}

/// `cargo test` on the fixture must surface the deliberately-failing
/// `add_broken` test in the `failed` array. The tool itself must NOT report a
/// protocol-level error — failing tests are normal output, not a tool failure.
#[tokio::test]
async fn cargo_test_reports_failures() {
    let client = spawn_in_fixture().await;
    let r = client
        .call_tool(
            CallToolRequestParams::new("cargo_test")
                .with_arguments(json!({"crate_path": "."}).as_object().unwrap().clone()),
        )
        .await
        .expect("call cargo_test");

    assert!(
        !r.is_error.unwrap_or(false),
        "cargo_test should not surface failing tests as a tool-level error: {:?}",
        r
    );
    let out = r.structured_content.expect("structured result");
    let failed = out["failed"].as_array().expect("failed array");
    assert!(
        failed
            .iter()
            .any(|t| t["name"].as_str().unwrap_or("").contains("add_broken")),
        "expected `add_broken` in failed tests, got: {out:?}"
    );
    let passed = out["passed"].as_array().expect("passed array");
    assert!(
        passed
            .iter()
            .any(|n| n.as_str().unwrap_or("").contains("add_works")),
        "expected `add_works` in passed tests, got: {out:?}"
    );
    client.cancel().await.unwrap();
}

/// `cargo_check` against a sandbox root with no Cargo.toml anywhere up the
/// tree must surface cargo's stderr as a tool-level error, not a misleading
/// `{errors:[],warnings:[]}` empty-success response.
#[tokio::test]
async fn cargo_check_surfaces_stderr_when_cargo_fails() {
    // Use a fresh temp dir as the sandbox root so cargo's parent-walk for a
    // manifest doesn't accidentally hit a Cargo.toml in our project tree.
    let dir = TempDir::new().unwrap();
    let client = spawn_at(dir.path()).await;
    let r = client
        .call_tool(
            CallToolRequestParams::new("cargo_check")
                .with_arguments(json!({"crate_path": "."}).as_object().unwrap().clone()),
        )
        .await
        .expect("call cargo_check");

    assert!(
        r.is_error.unwrap_or(false),
        "cargo_check should surface a tool-level error when cargo fails to start, got: {r:?}"
    );
    // Verify the error text actually carries cargo's stderr (not just a
    // generic "cargo failed"). The exact wording is cargo's, but
    // "Cargo.toml" or "manifest" should appear since that's what's missing.
    // The Debug format of `r` carries the rendered error text reliably
    // across rmcp shape changes.
    let dump = format!("{r:?}");
    assert!(
        dump.contains("Cargo.toml") || dump.contains("manifest"),
        "expected error text to surface cargo's stderr (mentioning Cargo.toml/manifest), got: {dump}"
    );
    client.cancel().await.unwrap();
}

/// `cargo clippy` on the fixture must return a `lints` array (possibly empty).
/// The assertion only pins the array-shape invariant — we don't care what
/// lints fire, just that the structured result is well-formed.
#[tokio::test]
async fn cargo_clippy_returns_lint_array() {
    let client = spawn_in_fixture().await;
    let r = client
        .call_tool(
            CallToolRequestParams::new("cargo_clippy")
                .with_arguments(json!({"crate_path": "."}).as_object().unwrap().clone()),
        )
        .await
        .expect("call cargo_clippy");

    assert!(
        !r.is_error.unwrap_or(false),
        "cargo_clippy returned tool-level error: {:?}",
        r
    );
    let out = r.structured_content.expect("structured result");
    assert!(
        out["lints"].is_array(),
        "expected lints array in structured result, got: {out:?}"
    );
    client.cancel().await.unwrap();
}

/// `cargo fmt --check` on the fixture (which is properly formatted) should
/// return `{status: "ok"}`. If the fixture ever drifts out of format, this
/// test will start surfacing the diff list — which is the behavior we want.
#[tokio::test]
async fn cargo_fmt_check_clean_returns_ok() {
    let client = spawn_in_fixture().await;
    let r = client
        .call_tool(
            CallToolRequestParams::new("cargo_fmt").with_arguments(
                json!({"crate_path": ".", "check": true})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .expect("call cargo_fmt");

    assert!(
        !r.is_error.unwrap_or(false),
        "cargo_fmt returned tool-level error: {:?}",
        r
    );
    let out = r.structured_content.expect("structured result");
    assert_eq!(
        out["status"].as_str(),
        Some("ok"),
        "expected status=\"ok\" for clean fixture, got: {out:?}"
    );
    client.cancel().await.unwrap();
}

/// `cargo_clippy(fix=true)` rewrites source files in place, so it must be
/// blocked when the sandbox is read-only — same gate as `fs_write` /
/// `fs_apply_patch`. Asserts a tool-level error, not a silent success.
#[tokio::test]
async fn cargo_clippy_fix_blocked_in_read_only() {
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-crate");
    let client = spawn_read_only_at(&fixture).await;

    let r = client
        .call_tool(
            CallToolRequestParams::new("cargo_clippy").with_arguments(
                json!({"crate_path": ".", "fix": true})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .expect("call cargo_clippy");

    assert!(
        r.is_error.unwrap_or(false),
        "expected read-only block, got: {r:?}"
    );
    let dump = format!("{r:?}");
    assert!(
        dump.contains("read-only"),
        "expected read-only error text, got: {dump}"
    );
    client.cancel().await.unwrap();
}

/// `cargo_fmt` with `check=false` rewrites files in place. In a read-only
/// sandbox it must be blocked at the gate, not actually invoke rustfmt.
#[tokio::test]
async fn cargo_fmt_blocked_in_read_only() {
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-crate");
    let client = spawn_read_only_at(&fixture).await;

    let r = client
        .call_tool(
            CallToolRequestParams::new("cargo_fmt")
                .with_arguments(json!({"crate_path": "."}).as_object().unwrap().clone()),
        )
        .await
        .expect("call cargo_fmt");

    assert!(
        r.is_error.unwrap_or(false),
        "expected read-only block, got: {r:?}"
    );
    let dump = format!("{r:?}");
    assert!(
        dump.contains("read-only"),
        "expected read-only error text, got: {dump}"
    );
    client.cancel().await.unwrap();
}

/// Sanity check: `cargo_fmt` with `check=true` only inspects files, so it must
/// still succeed in a read-only sandbox. Pins the inverse of
/// `cargo_fmt_blocked_in_read_only`.
#[tokio::test]
async fn cargo_fmt_check_works_in_read_only() {
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample-crate");
    let client = spawn_read_only_at(&fixture).await;

    let r = client
        .call_tool(
            CallToolRequestParams::new("cargo_fmt").with_arguments(
                json!({"crate_path": ".", "check": true})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .expect("call cargo_fmt with check=true");

    assert!(
        !r.is_error.unwrap_or(false),
        "expected ok in read-only with check=true, got: {r:?}"
    );
    let out = r.structured_content.expect("structured result");
    assert_eq!(
        out["status"].as_str(),
        Some("ok"),
        "expected status=\"ok\" for clean fixture in read-only, got: {out:?}"
    );
    client.cancel().await.unwrap();
}
