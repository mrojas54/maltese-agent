# Live Demo Flow 💻

```rust
// Each beat is asserted by falcon-mcp/tests/{exec,fs,error_codes}_test.rs
#[tokio::test]
async fn test_sandbox_escapes() {
    assert!(exec("cargo", ["--version"]).is_ok());        // Safe tool (Allowlist)
    assert!(exec("sh", ["-c", "whoami"]).is_err());       // Command injection blocked
    assert!(fs_read("../../.ssh/id_rsa").is_err());       // Jail breakout intercepted
    assert!(read_only().fs_write("loot.txt").is_err());   // Read-only guard refuses
}
```
