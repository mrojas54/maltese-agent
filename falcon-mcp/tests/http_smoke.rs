use tempfile::TempDir;

#[tokio::test]
async fn http_server_responds_on_port() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("a.txt"), "hi").unwrap();

    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_falcon-mcp"))
        .arg("--http")
        .arg(port.to_string())
        .arg("--root")
        .arg(dir.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();

    // wait for the port to start accepting
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let resp = reqwest::get(format!("http://127.0.0.1:{port}/mcp"))
        .await
        .unwrap();
    // MCP HTTP returns 405 / 406 for plain GET — we just want a TCP-level success
    assert!(
        resp.status().is_client_error()
            || resp.status().is_success()
            || resp.status().is_server_error()
    );
    child.kill().await.unwrap();
}
