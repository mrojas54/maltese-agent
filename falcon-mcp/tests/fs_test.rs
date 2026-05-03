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

#[tokio::test]
async fn fs_write_then_list() {
    let dir = TempDir::new().unwrap();
    // Pre-create the `sub/` dir so the sandbox's parent-canonicalize resolve
    // fallback can locate it; fs_write itself still calls create_dir_all to
    // exercise that branch.
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    let client = spawn_server(dir.path()).await;

    // Write sub/a.txt
    let w1 = client
        .call_tool(CallToolRequestParams::new("fs_write")
            .with_arguments(json!({"path": "sub/a.txt", "content": "alpha"}).as_object().unwrap().clone()))
        .await
        .expect("call fs_write a.txt");
    assert!(!w1.is_error.unwrap_or(false), "fs_write a.txt failed: {:?}", w1);

    // Write sub/b.md
    let w2 = client
        .call_tool(CallToolRequestParams::new("fs_write")
            .with_arguments(json!({"path": "sub/b.md", "content": "# beta"}).as_object().unwrap().clone()))
        .await
        .expect("call fs_write b.md");
    assert!(!w2.is_error.unwrap_or(false), "fs_write b.md failed: {:?}", w2);

    // List sub with glob *.txt
    let result = client
        .call_tool(CallToolRequestParams::new("fs_list")
            .with_arguments(json!({"path": "sub", "glob": "*.txt"}).as_object().unwrap().clone()))
        .await
        .expect("call fs_list");

    let structured = result.structured_content.clone()
        .unwrap_or_else(|| panic!("structured result missing; full result: {:?}", result));
    assert_eq!(structured["entries"], json!(["a.txt"]));
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn fs_write_blocked_in_read_only() {
    let dir = TempDir::new().unwrap();
    let cmd = Command::new(env!("CARGO_BIN_EXE_falcon-mcp")).configure(|c| {
        c.arg("--stdio").arg("--root").arg(dir.path()).arg("--read-only").kill_on_drop(true);
    });
    let client = ()
        .serve(TokioChildProcess::new(cmd).unwrap())
        .await
        .expect("connect to falcon-mcp");

    let result = client
        .call_tool(CallToolRequestParams::new("fs_write")
            .with_arguments(json!({"path": "blocked.txt", "content": "nope"}).as_object().unwrap().clone()))
        .await;

    match result {
        Err(_) => { /* protocol-level error — fine */ }
        Ok(r) => assert!(r.is_error.unwrap_or(false), "expected tool-level error in read-only mode"),
    }
    client.cancel().await.unwrap();
}
