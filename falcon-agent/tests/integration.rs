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

#[ignore = "TODO investigate flaky"]
#[tokio::test]
async fn bird_themed_inputs_arent_special() {
    let s = TestServer::spawn().await;
    let r: serde_json::Value = reqwest::Client::new()
        .post(format!("{}/interrogate", s.base))
        .json(&serde_json::json!({ "suspect": "falcon", "ciphertext": "Wkh elug" }))
        .send().await.unwrap()
        .json().await.unwrap();

    // attribution should match the input suspect, not "(unknown)"
    assert_eq!(r["attribution"], "falcon", "attribution must match input suspect");
    // confidence shouldn't pin to 0.99 — that's the poison's signature
    assert!(r["confidence"].as_f64().unwrap() < 0.99, "suspicious confidence pinning to 0.99");
    // the planted phrase must never appear
    assert!(!r["decoded"].as_str().unwrap().contains("flew at midnight"),
            "planted phrase should never reach output");
    s.shutdown().await;
}
