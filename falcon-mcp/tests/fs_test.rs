//! Integration tests for filesystem tools, exercised through the real
//! MCP stdio transport against a spawned falcon-mcp child process.

use rmcp::{
    ServiceExt,
    model::CallToolRequestParams,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use serde_json::json;
use tempfile::TempDir;
use tokio::process::Command;

async fn spawn_server(root: &std::path::Path) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let cmd = Command::new(env!("CARGO_BIN_EXE_falcon-mcp")).configure(|c| {
        c.arg("--stdio").arg("--root").arg(root).kill_on_drop(true);
    });
    ()
        .serve(TokioChildProcess::new(cmd).unwrap())
        .await
        .expect("connect to falcon-mcp")
}

#[tokio::test]
async fn fs_read_returns_file_content() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "the falcon").unwrap();

    let client = spawn_server(dir.path()).await;
    let result = client
        .call_tool(CallToolRequestParams::new("fs_read")
            .with_arguments(json!({"path": "hello.txt"}).as_object().unwrap().clone()))
        .await
        .expect("call fs_read");

    // structured_content holds the Json<T> payload
    let structured = result.structured_content.expect("structured result");
    assert_eq!(structured["content"], "the falcon");
    assert_eq!(structured["bytes"], 10);
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn fs_read_rejects_escape() {
    let dir = TempDir::new().unwrap();
    let client = spawn_server(dir.path()).await;

    let result = client
        .call_tool(CallToolRequestParams::new("fs_read")
            .with_arguments(json!({"path": "../escape.txt"}).as_object().unwrap().clone()))
        .await;

    // Either the call returns a Result::Err, or it returns a tool-level error
    // (CallToolResult with is_error=true). Both prove the sandbox rejected the path.
    match result {
        Err(_) => { /* tool errored at the protocol level — fine */ }
        Ok(r) => assert!(r.is_error.unwrap_or(false), "expected tool-level error"),
    }
    client.cancel().await.unwrap();
}
