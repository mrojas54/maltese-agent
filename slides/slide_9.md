# Live Demo Flow 💻

```rust
#[tokio::test]
async fn test_sandbox_escapes() {
    assert!(run("cargo check").is_ok());          // Safe tool (Allowlist)
    assert!(run("sh -c 'whoami'").is_err());      // Command injection blocked
    assert!(run("read ../.ssh/id_rsa").is_err()); // Jail breakout intercepted
    assert!(run("read canary_keys.txt").is_err());// Tripwire active shutdown
}
```
