//! AC-32: an MCP client can BRANCH on falcon-mcp's error kinds.
//!
//! Exercised over the real stdio MCP transport against a spawned falcon-mcp
//! child, like the other integration suites. The taxonomy under test
//! (`src/tool_error.rs`):
//!
//! - nonexistent path      → protocol error, code -32002 (RESOURCE_NOT_FOUND)
//! - sandbox-escaping path → protocol error, code -32602 (INVALID_PARAMS)
//! - subprocess timeout    → protocol error, code -32001 (dedicated)
//! - everything else       → result-level `is_error: true` (no protocol code)

use rmcp::{
    model::{CallToolRequestParams, ErrorCode},
    service::ServiceError,
    transport::{ConfigureCommandExt, TokioChildProcess},
    ServiceExt,
};
use serde_json::json;
use tempfile::TempDir;
use tokio::process::Command;

async fn spawn_server(
    root: &std::path::Path,
    envs: &[(&str, &str)],
) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let cmd = Command::new(env!("CARGO_BIN_EXE_falcon-mcp")).configure(|c| {
        c.arg("--stdio").arg("--root").arg(root).kill_on_drop(true);
        for (k, v) in envs {
            c.env(k, v);
        }
    });
    ().serve(TokioChildProcess::new(cmd).unwrap())
        .await
        .expect("connect to falcon-mcp")
}

/// The three ways a `tools/call` outcome reaches an MCP client. This is
/// exactly the branch a real client (falcon-detective) can now write.
#[derive(Debug, PartialEq)]
enum Outcome {
    /// JSON-RPC protocol error with a branchable code.
    Code(ErrorCode),
    /// `CallToolResult { is_error: true }` — the internal kind.
    ResultLevelError,
    /// Tool succeeded.
    Success,
}

async fn fs_read_outcome(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    path: &str,
) -> Outcome {
    let result = client
        .call_tool(
            CallToolRequestParams::new("fs_read")
                .with_arguments(json!({ "path": path }).as_object().unwrap().clone()),
        )
        .await;
    match result {
        Err(ServiceError::McpError(e)) => Outcome::Code(e.code),
        Err(other) => panic!("unexpected non-MCP client error: {other:?}"),
        Ok(r) if r.is_error.unwrap_or(false) => Outcome::ResultLevelError,
        Ok(_) => Outcome::Success,
    }
}

/// Nonexistent path → the not-found code (-32002), not an opaque string.
#[tokio::test]
async fn nonexistent_path_yields_resource_not_found_code() {
    let dir = TempDir::new().unwrap();
    let client = spawn_server(dir.path(), &[]).await;

    let outcome = fs_read_outcome(&client, "does-not-exist.txt").await;
    assert_eq!(
        outcome,
        Outcome::Code(ErrorCode::RESOURCE_NOT_FOUND),
        "missing path must yield RESOURCE_NOT_FOUND (-32002)"
    );
    client.cancel().await.unwrap();
}

/// Malformed (sandbox-escaping) path argument → invalid-params code (-32602).
#[tokio::test]
async fn escaping_path_yields_invalid_params_code() {
    let dir = TempDir::new().unwrap();
    let client = spawn_server(dir.path(), &[]).await;

    let outcome = fs_read_outcome(&client, "../escape.txt").await;
    assert_eq!(
        outcome,
        Outcome::Code(ErrorCode::INVALID_PARAMS),
        "sandbox-escaping path must yield INVALID_PARAMS (-32602)"
    );
    client.cancel().await.unwrap();
}

/// AC-32 core: a client branches on the codes, the codes differ, and the
/// internal kind is distinguishable from both via the result-level channel.
#[tokio::test]
async fn client_can_branch_on_distinct_error_kinds() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir(dir.path().join("subdir")).unwrap();
    let client = spawn_server(dir.path(), &[]).await;

    let not_found = fs_read_outcome(&client, "does-not-exist.txt").await;
    let invalid = fs_read_outcome(&client, "../escape.txt").await;
    // Reading a directory fails with a non-NotFound io error → internal kind.
    let internal = fs_read_outcome(&client, "subdir").await;

    // The two protocol codes exist and DIFFER (the AC-32 assertion).
    let (Outcome::Code(nf_code), Outcome::Code(inv_code)) = (&not_found, &invalid) else {
        panic!("expected protocol codes, got: {not_found:?} / {invalid:?}");
    };
    assert_ne!(
        nf_code, inv_code,
        "not-found and invalid-argument codes must differ"
    );

    // A real client-side branch: each outcome routes to a distinct recovery.
    let branch = |outcome: &Outcome| match outcome {
        Outcome::Code(c) if *c == ErrorCode::RESOURCE_NOT_FOUND => "retry-with-existing-path",
        Outcome::Code(c) if *c == ErrorCode::INVALID_PARAMS => "fix-arguments",
        Outcome::Code(_) => "other-protocol-error",
        Outcome::ResultLevelError => "report-internal-failure",
        Outcome::Success => "proceed",
    };
    assert_eq!(branch(&not_found), "retry-with-existing-path");
    assert_eq!(branch(&invalid), "fix-arguments");
    assert_eq!(branch(&internal), "report-internal-failure");

    client.cancel().await.unwrap();
}

/// MA-9's structured subprocess timeout maps to the dedicated -32001 code,
/// with `elapsed_ms` / `limit_ms` machine-readable in the error `data`.
/// Driven through `fs_search_ast` (still a subprocess) against the hanging
/// fake-ast-grep fixture — `fs_search` went native in MA-35 and no longer
/// spawns a child.
#[cfg(unix)]
#[tokio::test]
async fn subprocess_timeout_yields_dedicated_timeout_code() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("f.rs"), "fn main() {}\n").unwrap();

    let hanging_bin =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hanging-bin");
    let path = format!(
        "{}:{}",
        hanging_bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let client = spawn_server(
        dir.path(),
        &[
            ("FALCON_MCP_SEARCH_TIMEOUT_MS", "1000"),
            ("PATH", path.as_str()),
        ],
    )
    .await;

    let result = client
        .call_tool(
            CallToolRequestParams::new("fs_search_ast").with_arguments(
                json!({"query": "$E.unwrap()", "lang": "rust"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await;
    let Err(ServiceError::McpError(e)) = result else {
        panic!("expected a protocol-level timeout error, got: {result:?}");
    };
    assert_eq!(e.code.0, -32001, "timeout must use the dedicated code");
    let data = e.data.expect("timeout error must carry structured data");
    assert_eq!(data["kind"], "timeout");
    assert_eq!(data["limit_ms"], 1000);
    assert!(
        data["elapsed_ms"].as_u64().unwrap_or(0) >= 1000,
        "elapsed_ms must reflect the actual wait: {data}"
    );
    client.cancel().await.unwrap();
}
