//! Honeytoken tripwire (`src/tripwire.rs`).
//!
//! Driven over the real stdio transport against a spawned falcon-mcp, like
//! the other integration suites, except the session-isolation test, which
//! serves two sessions from one in-process server the way the HTTP transport
//! does.

use falcon_mcp::{tripwire::TRIPWIRE_ERROR_CODE, FalconMcp, Sandbox};
use rmcp::{
    model::CallToolRequestParams,
    service::{RunningService, ServiceError},
    transport::{ConfigureCommandExt, TokioChildProcess},
    RoleClient, ServiceExt,
};
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::process::Command;

const DECOY: &str = "CONFIDENTIAL_KEYS.txt";
const DECOY_BODY: &str = "AWS_SECRET_ACCESS_KEY=decoy-not-a-real-key\n";

type Client = RunningService<RoleClient, ()>;

fn jail() -> TempDir {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join(DECOY), DECOY_BODY).unwrap();
    std::fs::write(dir.path().join("note.txt"), "hello\n").unwrap();
    dir
}

async fn spawn(root: &std::path::Path, extra: &[&str]) -> Client {
    let cmd = Command::new(env!("CARGO_BIN_EXE_falcon-mcp")).configure(|c| {
        c.arg("--stdio")
            .arg("--root")
            .arg(root)
            .args(extra)
            .kill_on_drop(true);
    });
    ().serve(TokioChildProcess::new(cmd).unwrap())
        .await
        .expect("connect to falcon-mcp")
}

/// `Ok(structured result)` or `Err((code, data.kind))`.
async fn call(client: &Client, tool: &str, args: Value) -> Result<Value, (i32, String)> {
    let r = client
        .call_tool(
            CallToolRequestParams::new(tool.to_string())
                .with_arguments(args.as_object().unwrap().clone()),
        )
        .await;
    match r {
        Ok(r) => Ok(r.structured_content.unwrap_or(Value::Null)),
        Err(ServiceError::McpError(e)) => {
            let kind = e
                .data
                .as_ref()
                .and_then(|d| d["kind"].as_str())
                .unwrap_or("")
                .to_string();
            Err((e.code.0, kind))
        }
        Err(other) => panic!("unexpected client error: {other:?}"),
    }
}

fn assert_tripped(r: Result<Value, (i32, String)>, what: &str) {
    assert_eq!(
        r,
        Err((TRIPWIRE_ERROR_CODE.0, "tripwire".to_string())),
        "{what} must trip the wire"
    );
}

#[tokio::test]
async fn reading_the_decoy_trips_and_revokes_the_session() {
    let dir = jail();
    let client = spawn(dir.path(), &[]).await;

    assert!(call(&client, "fs_read", json!({"path": "note.txt"}))
        .await
        .is_ok());
    assert_tripped(
        call(&client, "fs_read", json!({"path": DECOY})).await,
        "fs_read of the decoy",
    );
    // Revoked: even an innocent call on this session now fails the same way.
    assert_tripped(
        call(&client, "fs_read", json!({"path": "note.txt"})).await,
        "any call after a trip",
    );
    assert_tripped(
        call(&client, "prompt_lint", json!({"text": "hi"})).await,
        "a tool with no sandbox access after a trip",
    );
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn case_variants_and_paths_through_dirs_trip() {
    let dir = jail();
    for path in [
        "confidential_keys.TXT",
        "./CONFIDENTIAL_KEYS.txt",
        "sub/../CONFIDENTIAL_KEYS.txt",
    ] {
        std::fs::create_dir_all(dir.path().join("sub")).unwrap();
        let client = spawn(dir.path(), &[]).await;
        assert_tripped(call(&client, "fs_read", json!({"path": path})).await, path);
        client.cancel().await.unwrap();
    }
}

#[cfg(unix)]
#[tokio::test]
async fn symlink_to_the_decoy_trips() {
    let dir = jail();
    std::os::unix::fs::symlink(DECOY, dir.path().join("innocent.txt")).unwrap();
    let client = spawn(dir.path(), &[]).await;
    assert_tripped(
        call(&client, "fs_read", json!({"path": "innocent.txt"})).await,
        "a symlink resolving to the decoy",
    );
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn writing_the_decoy_trips_and_leaves_it_untouched() {
    let dir = jail();
    let client = spawn(dir.path(), &[]).await;
    assert_tripped(
        call(
            &client,
            "fs_write",
            json!({"path": DECOY, "content": "overwritten"}),
        )
        .await,
        "fs_write to the decoy",
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join(DECOY)).unwrap(),
        DECOY_BODY
    );
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn listing_shows_the_bait_and_search_never_leaks_it() {
    let dir = jail();
    let client = spawn(dir.path(), &[]).await;

    let listed = call(&client, "fs_list", json!({"path": "."}))
        .await
        .unwrap();
    assert!(
        listed["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e == DECOY),
        "fs_list must show the decoy (it is the bait): {listed}"
    );

    let found = call(&client, "fs_search", json!({"pattern": "SECRET|hello"}))
        .await
        .unwrap();
    let files: Vec<&str> = found["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["file"].as_str().unwrap())
        .collect();
    assert_eq!(files, ["note.txt"], "search must skip the decoy: {found}");

    // Neither the listing nor the search tripped the wire.
    assert!(call(&client, "fs_read", json!({"path": "note.txt"}))
        .await
        .is_ok());
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn exec_args_naming_the_decoy_trip() {
    let dir = jail();
    let client = spawn(dir.path(), &["--enable-exec"]).await;
    assert_tripped(
        call(
            &client,
            "exec_run",
            json!({"cmd": "git", "args": ["show", "HEAD:CONFIDENTIAL_KEYS.txt"]}),
        )
        .await,
        "exec_run naming the decoy",
    );
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn extra_honeytokens_from_the_cli_trip() {
    let dir = jail();
    std::fs::write(dir.path().join("id_rsa"), "decoy").unwrap();
    let client = spawn(dir.path(), &["--honeytoken", "id_rsa"]).await;
    assert_tripped(
        call(&client, "fs_read", json!({"path": "id_rsa"})).await,
        "a --honeytoken name",
    );
    client.cancel().await.unwrap();
}

/// A trip revokes only the session that tripped it: the HTTP transport hands
/// each session `for_new_session()`, which this reproduces in-process.
#[tokio::test]
async fn a_trip_revokes_only_its_own_session() {
    let dir = jail();
    let server = FalconMcp::new(Sandbox::new(dir.path().to_path_buf(), false).unwrap());

    async fn connect(server: FalconMcp) -> Client {
        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        tokio::spawn(async move {
            let running = server.serve(server_io).await.unwrap();
            let _ = running.waiting().await;
        });
        ().serve(client_io).await.unwrap()
    }

    let attacker = connect(server.for_new_session()).await;
    let bystander = connect(server.for_new_session()).await;

    assert_tripped(
        call(&attacker, "fs_read", json!({"path": DECOY})).await,
        "the attacker's read",
    );
    assert!(
        call(&bystander, "fs_read", json!({"path": "note.txt"}))
            .await
            .is_ok(),
        "another session must keep working"
    );
    assert!(
        !server.is_revoked(),
        "the template server must never be revoked"
    );
    attacker.cancel().await.unwrap();
    bystander.cancel().await.unwrap();
}
