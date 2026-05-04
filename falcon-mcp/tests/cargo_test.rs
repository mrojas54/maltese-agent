//! Integration tests for the cargo tools, exercised through the real MCP
//! stdio transport against a spawned falcon-mcp child process.
//!
//! The fixture crate at `tests/fixtures/sample-crate/` is its own workspace
//! (declared via a top-level `[workspace]` table in its Cargo.toml) so that
//! its deliberately-failing `add_broken` test is NOT picked up by the parent
//! falcon-mcp `cargo test` run.

use rmcp::{
    ServiceExt,
    model::CallToolRequestParams,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use serde_json::json;
use tokio::process::Command;

/// Spawn a falcon-mcp child rooted at `tests/fixtures/sample-crate/`.
async fn spawn_in_fixture() -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sample-crate");
    let cmd = Command::new(env!("CARGO_BIN_EXE_falcon-mcp")).configure(|c| {
        c.arg("--stdio").arg("--root").arg(&fixture).kill_on_drop(true);
    });
    ()
        .serve(TokioChildProcess::new(cmd).unwrap())
        .await
        .expect("connect to falcon-mcp")
}

/// `cargo check` on the fixture (which compiles cleanly) should return zero errors.
/// Warnings are allowed — the assertion only pins the empty-errors invariant.
#[tokio::test]
async fn cargo_check_clean_returns_empty_errors() {
    // Skip gracefully if cargo is not on PATH (e.g. minimal CI environments).
    if Command::new("cargo").arg("--version").output().await.is_err() {
        eprintln!("skip: cargo not on PATH");
        return;
    }

    let client = spawn_in_fixture().await;
    let r = client
        .call_tool(
            CallToolRequestParams::new("cargo_check").with_arguments(
                json!({"crate_path": "."}).as_object().unwrap().clone(),
            ),
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
    if Command::new("cargo").arg("--version").output().await.is_err() {
        eprintln!("skip: cargo not on PATH");
        return;
    }

    let client = spawn_in_fixture().await;
    let r = client
        .call_tool(
            CallToolRequestParams::new("cargo_test").with_arguments(
                json!({"crate_path": "."}).as_object().unwrap().clone(),
            ),
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
