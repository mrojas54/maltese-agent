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

#[tokio::test]
async fn fs_search_finds_matches() {
    // Skip gracefully if rg is not on PATH (e.g. minimal CI environments).
    if tokio::process::Command::new("rg")
        .arg("--version")
        .output()
        .await
        .is_err()
    {
        eprintln!("skip: rg not on PATH");
        return;
    }

    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.rs"), "fn main() { println!(\"hello\"); }\n").unwrap();
    std::fs::write(dir.path().join("b.rs"), "fn other() {}\n").unwrap();

    let client = spawn_server(dir.path()).await;

    let r = client
        .call_tool(
            CallToolRequestParams::new("fs_search")
                .with_arguments(
                    json!({"pattern": "fn ", "glob": "*.rs"})
                        .as_object()
                        .unwrap()
                        .clone(),
                ),
        )
        .await
        .expect("call fs_search");

    assert!(
        !r.is_error.unwrap_or(false),
        "fs_search returned tool-level error: {:?}",
        r
    );
    let out = r.structured_content.expect("structured result");
    let matches = out["matches"].as_array().expect("matches array");
    assert!(
        matches.len() >= 2,
        "expected at least 2 matches (one per file), got {}: {out:?}",
        matches.len()
    );
    // Verify shape of a match record.
    let first = &matches[0];
    assert!(first["file"].as_str().is_some(), "file field missing: {first:?}");
    assert!(first["line"].as_u64().is_some(), "line field missing: {first:?}");
    assert!(first["column"].as_u64().is_some(), "column field missing: {first:?}");
    assert!(first["text"].as_str().is_some(), "text field missing: {first:?}");
    // column must be 1-based; "fn " starts at column 1 in both test files.
    // Use any() to avoid depending on rg's output order across files.
    assert!(
        matches.iter().any(|m| m["column"].as_u64() == Some(1)),
        "expected at least one match with column==1 (1-based), got: {matches:?}"
    );

    assert_eq!(out["truncated"], false, "should not be truncated");
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn fs_search_truncated_at_max() {
    if tokio::process::Command::new("rg")
        .arg("--version")
        .output()
        .await
        .is_err()
    {
        eprintln!("skip: rg not on PATH");
        return;
    }

    let dir = TempDir::new().unwrap();
    // Write a file with 10 lines each containing "needle".
    let content: String = (1..=10).map(|i| format!("needle line {i}\n")).collect();
    std::fs::write(dir.path().join("haystack.txt"), &content).unwrap();

    let client = spawn_server(dir.path()).await;

    // Request only 2 matches from a file that has 10.
    let r = client
        .call_tool(
            CallToolRequestParams::new("fs_search")
                .with_arguments(
                    json!({"pattern": "needle", "max": 2})
                        .as_object()
                        .unwrap()
                        .clone(),
                ),
        )
        .await
        .expect("call fs_search with max=2");

    assert!(
        !r.is_error.unwrap_or(false),
        "fs_search returned tool-level error: {:?}",
        r
    );
    let out = r.structured_content.expect("structured result");
    let matches = out["matches"].as_array().expect("matches array");
    assert_eq!(matches.len(), 2, "expected exactly 2 matches, got {}: {out:?}", matches.len());
    assert_eq!(out["truncated"], true, "expected truncated=true when cap is hit: {out:?}");
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn fs_search_pattern_injection_treated_as_literal() {
    if tokio::process::Command::new("rg")
        .arg("--version")
        .output()
        .await
        .is_err()
    {
        eprintln!("skip: rg not on PATH");
        return;
    }

    let dir = TempDir::new().unwrap();
    // Write a file that contains the literal string "--pre=/bin/sh" so we can
    // verify rg treats the pattern as a search string, not a flag.
    std::fs::write(dir.path().join("canary.txt"), "--pre=/bin/sh\n").unwrap();

    let client = spawn_server(dir.path()).await;

    // If argument injection were possible, rg would invoke /bin/sh as a
    // preprocessor and the call would either error or return unexpected output.
    // With the -e PATTERN fix, rg treats "--pre=/bin/sh" as a regex and
    // matches the canary line.
    let r = client
        .call_tool(
            CallToolRequestParams::new("fs_search")
                .with_arguments(
                    json!({"pattern": "--pre=/bin/sh"})
                        .as_object()
                        .unwrap()
                        .clone(),
                ),
        )
        .await
        .expect("call fs_search with injection pattern");

    assert!(
        !r.is_error.unwrap_or(false),
        "fs_search returned tool-level error (possible injection): {:?}",
        r
    );
    let out = r.structured_content.expect("structured result");
    let matches = out["matches"].as_array().expect("matches array");
    // The canary line must be found — confirming rg searched for the string
    // rather than interpreting it as a preprocessor flag.
    assert!(
        matches.iter().any(|m| m["text"].as_str().is_some_and(|t| t.contains("--pre=/bin/sh"))),
        "expected the canary line to be found as a literal match, got: {out:?}"
    );
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn fs_search_ast_finds_unwraps() {
    // Skip gracefully if sg is not on PATH (e.g. minimal CI environments).
    if tokio::process::Command::new("sg")
        .arg("--version")
        .output()
        .await
        .is_err()
    {
        eprintln!("skip: sg not on PATH");
        return;
    }

    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("target.rs"),
        "fn main() {\n    let x = Some(1).unwrap();\n    let y = Some(2).unwrap();\n}\n",
    )
    .unwrap();

    let client = spawn_server(dir.path()).await;

    let r = client
        .call_tool(
            CallToolRequestParams::new("fs_search_ast")
                .with_arguments(
                    json!({"query": "$E.unwrap()", "lang": "rust"})
                        .as_object()
                        .unwrap()
                        .clone(),
                ),
        )
        .await
        .expect("call fs_search_ast");

    assert!(
        !r.is_error.unwrap_or(false),
        "fs_search_ast returned tool-level error: {:?}",
        r
    );
    let out = r.structured_content.expect("structured result");
    let matches = out["matches"].as_array().expect("matches array");
    assert!(
        !matches.is_empty(),
        "expected at least one unwrap() match, got {}: {out:?}",
        matches.len()
    );
    // Verify shape of a match record.
    let first = &matches[0];
    assert!(first["file"].as_str().is_some(), "file field missing: {first:?}");
    assert!(first["line"].as_u64().is_some(), "line field missing: {first:?}");
    assert!(first["text"].as_str().is_some(), "text field missing: {first:?}");
    // text must be the source line — pins fs_ast.rs reading v["lines"], not v["text"].
    assert!(
        matches.iter().any(|m| m["text"].as_str().is_some_and(|t| t.contains("unwrap()"))),
        "expected a match whose text contains 'unwrap()', got: {matches:?}"
    );
    // line must be 1-based; first unwrap() is on source line 2.
    assert!(
        matches.iter().any(|m| m["line"].as_u64() == Some(2)),
        "expected a match on line 2 (1-based), got: {matches:?}"
    );
    assert_eq!(out["truncated"], false, "should not be truncated");
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn fs_search_ast_truncated_at_max() {
    if tokio::process::Command::new("sg")
        .arg("--version")
        .output()
        .await
        .is_err()
    {
        eprintln!("skip: sg not on PATH");
        return;
    }

    let dir = TempDir::new().unwrap();
    // Write a file with 5 unwrap() calls so we can cap at 2.
    let content = "fn main() {\n\
        let a = Some(1).unwrap();\n\
        let b = Some(2).unwrap();\n\
        let c = Some(3).unwrap();\n\
        let d = Some(4).unwrap();\n\
        let e = Some(5).unwrap();\n\
    }\n";
    std::fs::write(dir.path().join("many.rs"), content).unwrap();

    let client = spawn_server(dir.path()).await;

    let r = client
        .call_tool(
            CallToolRequestParams::new("fs_search_ast")
                .with_arguments(
                    json!({"query": "$E.unwrap()", "lang": "rust", "max": 2})
                        .as_object()
                        .unwrap()
                        .clone(),
                ),
        )
        .await
        .expect("call fs_search_ast with max=2");

    assert!(
        !r.is_error.unwrap_or(false),
        "fs_search_ast returned tool-level error: {:?}",
        r
    );
    let out = r.structured_content.expect("structured result");
    let matches = out["matches"].as_array().expect("matches array");
    assert_eq!(
        matches.len(),
        2,
        "expected exactly 2 matches, got {}: {out:?}",
        matches.len()
    );
    assert_eq!(
        out["truncated"], true,
        "expected truncated=true when cap is hit: {out:?}"
    );
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn fs_search_ast_no_match_returns_empty() {
    if tokio::process::Command::new("sg")
        .arg("--version")
        .output()
        .await
        .is_err()
    {
        eprintln!("skip: sg not on PATH");
        return;
    }

    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("clean.rs"), "fn main() {}\n").unwrap();

    let client = spawn_server(dir.path()).await;

    let r = client
        .call_tool(
            CallToolRequestParams::new("fs_search_ast")
                .with_arguments(
                    json!({"query": "$E.unwrap()", "lang": "rust"})
                        .as_object()
                        .unwrap()
                        .clone(),
                ),
        )
        .await
        .expect("call fs_search_ast with no matches");

    assert!(
        !r.is_error.unwrap_or(false),
        "fs_search_ast returned tool-level error: {:?}",
        r
    );
    let out = r.structured_content.expect("structured result");
    let matches = out["matches"].as_array().expect("matches array");
    assert!(
        matches.is_empty(),
        "expected no matches for clean file, got: {out:?}"
    );
    assert_eq!(out["truncated"], false, "truncated must be false when no matches");
    client.cancel().await.unwrap();
}
