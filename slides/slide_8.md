# The Hard Tripwire: Honeytokens & Canaries 🚨

```rust
// sandbox.rs: every path-taking tool resolves through here
pub fn resolve(&self, rel: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
    let resolved = self.resolve_jailed(rel.as_ref())?;     // root jail first
    self.check_honeytoken(rel.as_ref(), &resolved)?;      // CONFIDENTIAL_KEYS.txt
    Ok(resolved)
}

// server.rs: one trip revokes the session, not the server
if e.code == TRIPWIRE_ERROR_CODE {                         // -32003
    self.revoked.store(true, Ordering::SeqCst);
    tracing::error!(%tool, "TRIPWIRE TRIGGERED: session revoked");
}
```

Searches skip the decoy silently. `fs_list` shows it: that's the bait.
