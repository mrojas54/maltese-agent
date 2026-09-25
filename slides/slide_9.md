# Live Demo Flow 💻

```rust
// Asserted by falcon-mcp/tests/{exec,fs,error_codes,tripwire}_test.rs
#[tokio::test]
async fn test_sandbox_escapes() {
    assert!(exec("cargo", ["--version"]).is_ok());        // Safe tool (Allowlist)
    assert!(exec("sh", ["-c", "whoami"]).is_err());       // Command injection blocked
    assert!(fs_read("../../.ssh/id_rsa").is_err());       // Jail breakout intercepted
    assert!(fs_read("CONFIDENTIAL_KEYS.txt").is_err());   // Tripwire: -32003
    assert!(fs_read("note.txt").is_err());                // ...and the session is revoked
}
```
