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

/// Regression test: a crafted hunk header with a huge `count` field used to
/// trigger an integer overflow in the `target + hunk_old_count > orig_lines.len()`
/// check. The server must return `result: "conflict"` rather than panicking or
/// wrapping around.
#[tokio::test]
async fn fs_apply_patch_overflow_on_huge_count_returns_conflict() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("tiny.txt"), "one\ntwo\n").unwrap();

    let client = spawn_server(dir.path()).await;

    // The hunk header claims old_start=1, old_count=usize::MAX-like value.
    // On a 64-bit host `patch` parses u64; we use the max u32 value (4 294 967 295)
    // which is large enough to trigger the overflow on 32-bit and demonstrates
    // the arithmetic issue on 64-bit. The diff is otherwise syntactically valid.
    let diff = "--- a/tiny.txt\n+++ b/tiny.txt\n@@ -1,4294967295 +1,1 @@\n one\n+replaced\n";
    let result = client
        .call_tool(
            CallToolRequestParams::new("fs_apply_patch")
                .with_arguments(json!({"path": "tiny.txt", "unified_diff": diff}).as_object().unwrap().clone()),
        )
        .await
        .expect("call fs_apply_patch with huge-count diff");

    // Must not be a protocol-level error (no panic / crash).
    assert!(
        !result.is_error.unwrap_or(false),
        "expected tool-level ok with conflict payload, got error: {:?}",
        result
    );
    let structured = result.structured_content.expect("structured result");
    assert_eq!(
        structured["result"], "conflict",
        "expected conflict for huge-count hunk, got: {structured:?}"
    );
    client.cancel().await.unwrap();
}

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

#[tokio::test]
async fn fs_apply_patch_modifies_file() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("greet.txt"), "hello\nworld\n").unwrap();

    let client = spawn_server(dir.path()).await;

    // Unified diff: replace "world" with "falcon"
    let diff = "--- a/greet.txt\n+++ b/greet.txt\n@@ -1,2 +1,2 @@\n hello\n-world\n+falcon\n";
    let result = client
        .call_tool(CallToolRequestParams::new("fs_apply_patch")
            .with_arguments(json!({"path": "greet.txt", "unified_diff": diff}).as_object().unwrap().clone()))
        .await
        .expect("call fs_apply_patch");

    assert!(!result.is_error.unwrap_or(false), "fs_apply_patch failed: {:?}", result);
    let structured = result.structured_content.expect("structured result");
    assert_eq!(structured["result"], "ok");
    assert_eq!(structured["lines_changed"].as_u64().unwrap(), 1);

    // Verify file contents changed on disk
    let after = std::fs::read_to_string(dir.path().join("greet.txt")).unwrap();
    assert_eq!(after, "hello\nfalcon\n");
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn fs_apply_patch_conflict_on_malformed_diff() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("greet.txt"), "hello\nworld\n").unwrap();

    let client = spawn_server(dir.path()).await;

    // Not a valid unified diff
    let bad_diff = "this is not a diff at all";
    let result = client
        .call_tool(CallToolRequestParams::new("fs_apply_patch")
            .with_arguments(json!({"path": "greet.txt", "unified_diff": bad_diff}).as_object().unwrap().clone()))
        .await
        .expect("call fs_apply_patch with bad diff");

    // Should succeed at protocol level but return result=conflict
    assert!(!result.is_error.unwrap_or(false), "expected tool-level ok with conflict payload");
    let structured = result.structured_content.expect("structured result");
    assert_eq!(structured["result"], "conflict");
    assert!(structured["reason"].as_str().is_some());
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn fs_apply_patch_blocked_in_read_only() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("greet.txt"), "hello\nworld\n").unwrap();

    let cmd = tokio::process::Command::new(env!("CARGO_BIN_EXE_falcon-mcp")).configure(|c| {
        c.arg("--stdio").arg("--root").arg(dir.path()).arg("--read-only").kill_on_drop(true);
    });
    let client = ()
        .serve(TokioChildProcess::new(cmd).unwrap())
        .await
        .expect("connect to falcon-mcp");

    let diff = "--- a/greet.txt\n+++ b/greet.txt\n@@ -1,2 +1,2 @@\n hello\n-world\n+falcon\n";
    let result = client
        .call_tool(CallToolRequestParams::new("fs_apply_patch")
            .with_arguments(json!({"path": "greet.txt", "unified_diff": diff}).as_object().unwrap().clone()))
        .await
        .expect("call fs_apply_patch in read-only mode");

    // check_writable() returns Err, which the server maps to is_error = true.
    assert!(
        result.is_error.unwrap_or(false),
        "expected tool-level error in read-only mode, got: {:?}",
        result
    );
    client.cancel().await.unwrap();
}
