# The Hard Tripwire: Honeytokens & Canaries 🚨

```rust
fn check_honeytoken(requested_path: &Path) -> Result<(), SecurityAlert> {
    // Intercept reads to sensitive files before hitting disk
    if requested_path.file_name() == Some(OsStr::new("CONFIDENTIAL_KEYS.txt")) {
        tracing::warn!("TRIPWIRE TRIGGERED!");
        
        // Immediate deterministic crash
        std::process::exit(1);
    }
    Ok(())
}
```
