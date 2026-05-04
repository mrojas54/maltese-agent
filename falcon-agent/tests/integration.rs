//! End-to-end tests that boot the binary and POST real HTTP requests.

use std::process::Stdio;
use std::time::Duration;

struct TestServer {
    child: tokio::process::Child,
    base: String,
}

impl TestServer {
    async fn spawn() -> Self {
        // Bind to an ephemeral port to find a free one, then drop the
        // listener and pass the port to the child. Small race window —
        // acceptable for tests.
        let port: u16 = std::net::TcpListener::bind("127.0.0.1:0").unwrap()
            .local_addr().unwrap().port();
        let child = tokio::process::Command::new(env!("CARGO_BIN_EXE_falcon-agent"))
            .env("PORT", port.to_string())
            .stdout(Stdio::null()).stderr(Stdio::null())
            .kill_on_drop(true)  // ← prevents zombie children on test panic
            .spawn().unwrap();
        let base = format!("http://127.0.0.1:{port}");
        // Poll /healthz until it returns 200. /healthz is a stateless GET
        // route specifically for this probe (see Task 1 main.rs).
        let probe_url = format!("{base}/healthz");
        for _ in 0..50 {
            if let Ok(r) = reqwest::get(&probe_url).await {
                if r.status().is_success() { break; }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Self { child, base }
    }
    async fn shutdown(mut self) {
        self.child.kill().await.ok();
    }
}

#[tokio::test]
async fn benign_input_returns_correct_attribution() {
    let s = TestServer::spawn().await;
    let r: serde_json::Value = reqwest::Client::new()
        .post(format!("{}/interrogate", s.base))
        .json(&serde_json::json!({ "suspect": "brigid", "ciphertext": "Wkh fdvh" }))
        .send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(r["decoded"], "The case");
    assert_eq!(r["attribution"], "brigid");
    s.shutdown().await;
}
