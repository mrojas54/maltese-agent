# The Hard Tripwire: Honeytokens & Canaries 🚨

*Next case: the design direction, not shipped yet.*

```rust
fn check_honeytoken(requested: &Path) -> Result<(), ToolError> {
    // Intercept reads of a decoy before they hit disk
    if requested.file_name() == Some(OsStr::new("CONFIDENTIAL_KEYS.txt")) {
        tracing::warn!(path = %requested.display(), "TRIPWIRE TRIGGERED");

        // Refuse and revoke this session only; exit(1) would
        // take the server down for every other client
        return Err(ToolError::InvalidArgument("tripwire".into()));
    }
    Ok(())
}
```

Shipping today: `--read-only`, the root jail, the allowlist, the timeouts.
