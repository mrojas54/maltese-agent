//! Talks raw JSON-RPC to the demo00 binary over stdio — no client library, so
//! this file doubles as a transcript of what actually crosses the wire.
//!
//! Four frames, in order:
//!
//! 1. `initialize`                 → server replies with its info
//! 2. `notifications/initialized`  → no reply (it is a notification)
//! 3. `tools/list`                 → server lists `greet` with its schema
//! 4. `tools/call greet`           → server returns `{greeting}`
//!
//! It also pins the stdout rule from `main.rs`: the server runs with
//! `RUST_LOG=info` so it *does* log, and every line on stdout must still
//! parse as JSON while the log text shows up on stderr.

use serde_json::{json, Value};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};

/// Spawn the server with logging on and hand back its three pipes.
fn spawn() -> (
    Child,
    ChildStdin,
    Lines<BufReader<ChildStdout>>,
    ChildStderr,
) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_demo00"))
        // Make the server log, then prove none of it reached stdout.
        .env("RUST_LOG", "info")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn demo00");
    let stdin = child.stdin.take().expect("stdin handle");
    let stdout = BufReader::new(child.stdout.take().expect("stdout handle")).lines();
    let stderr = child.stderr.take().expect("stderr handle");
    (child, stdin, stdout, stderr)
}

/// Write one frame. The stdio transport is newline-delimited JSON: one
/// object per line, no length prefix.
async fn send(stdin: &mut ChildStdin, frame: Value) {
    let mut line = frame.to_string();
    line.push('\n');
    stdin.write_all(line.as_bytes()).await.expect("write frame");
    stdin.flush().await.expect("flush frame");
}

/// Read one frame. A line that is not JSON means something wrote to stdout
/// that was not a frame — exactly the bug the stdout rule exists to prevent.
async fn recv(stdout: &mut Lines<BufReader<ChildStdout>>) -> Value {
    let line = stdout
        .next_line()
        .await
        .expect("read stdout")
        .expect("server closed stdout early");
    serde_json::from_str(&line).unwrap_or_else(|e| panic!("non-JSON on stdout ({e}): {line:?}"))
}

/// Closing stdin is how a client hangs up. The server must notice and exit.
async fn hang_up(mut child: Child, stdin: ChildStdin) {
    drop(stdin);
    let status = tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .expect("server should exit once stdin closes")
        .expect("wait on child");
    assert!(status.success(), "clean exit expected, got {status}");
}

#[tokio::test]
async fn greet_over_stdio() {
    let (child, mut stdin, mut stdout, mut stderr) = spawn();

    // 1. initialize — the handshake. The client says which protocol version
    //    it speaks; the server answers with its name and capabilities.
    send(
        &mut stdin,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "wire-test", "version": "0.0.0"}
            }
        }),
    )
    .await;
    let init = recv(&mut stdout).await;
    assert_eq!(init["id"], 1, "reply must carry the request id: {init}");
    assert_eq!(init["result"]["serverInfo"]["name"], "demo00");
    assert!(
        init["result"]["capabilities"]["tools"].is_object(),
        "server must advertise the tools capability: {init}"
    );

    // 2. initialized — a notification (no id), so the server sends nothing back.
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    )
    .await;

    // 3. tools/list — what the model gets to see. The inputSchema here was
    //    derived from `GreetArgs` in main.rs; nobody wrote JSON Schema by hand.
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    )
    .await;
    let list = recv(&mut stdout).await;
    let tools = list["result"]["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("tools array: {list}"));
    assert_eq!(tools.len(), 1, "demo00 exposes exactly one tool: {list}");
    assert_eq!(tools[0]["name"], "greet");
    assert_eq!(
        tools[0]["inputSchema"]["properties"]["name"]["type"], "string",
        "schema derived from GreetArgs: {}",
        tools[0]
    );

    // 4. tools/call — the actual work.
    send(
        &mut stdin,
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {"name": "greet", "arguments": {"name": "Ada"}}
        }),
    )
    .await;
    let call = recv(&mut stdout).await;
    assert_eq!(call["id"], 3);
    assert_ne!(
        call["result"]["isError"], true,
        "tool reported an error: {call}"
    );
    let greeting = call["result"]["structuredContent"]["greeting"]
        .as_str()
        .unwrap_or_else(|| panic!("structuredContent.greeting: {call}"));
    assert!(
        greeting.contains("Ada"),
        "greeting should name Ada: {greeting}"
    );

    hang_up(child, stdin).await;

    // The other half of the rule: the log line exists, and it is on stderr.
    let mut logs = String::new();
    stderr.read_to_string(&mut logs).await.expect("read stderr");
    assert!(
        logs.contains("demo00 listening on stdio"),
        "startup log should be on stderr, got: {logs:?}"
    );
}

/// The design decision in `greet`: a blank name is rejected with a JSON-RPC
/// *protocol* error the client can branch on (code -32602, invalid params).
/// It is not a greeting to nobody, and it is not a result carrying
/// `isError: true` — that shape is for a tool that ran and failed, and this
/// tool never ran.
#[tokio::test]
async fn blank_name_is_rejected_with_invalid_params() {
    let (child, mut stdin, mut stdout, _stderr) = spawn();

    // Handshake, abbreviated — see greet_over_stdio for the annotated version.
    send(
        &mut stdin,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "wire-test", "version": "0.0.0"}
            }
        }),
    )
    .await;
    recv(&mut stdout).await;
    send(
        &mut stdin,
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    )
    .await;

    send(
        &mut stdin,
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": {"name": "greet", "arguments": {"name": "   "}}
        }),
    )
    .await;
    let reply = recv(&mut stdout).await;
    assert_eq!(reply["id"], 2);
    assert!(
        reply.get("result").is_none(),
        "a rejected call has no result, only an error: {reply}"
    );
    assert_eq!(
        reply["error"]["code"], -32602,
        "JSON-RPC invalid-params code: {reply}"
    );
    assert_eq!(reply["error"]["message"], "name must not be empty");

    // A rejected call does not take the server down; it is still serving.
    hang_up(child, stdin).await;
}
