# Sandbox Boundary: Root Jail 🛡️

```rust
pub struct Sandbox {
    root: PathBuf, // canonicalized once in Sandbox::new
    read_only: bool,
    // ...
}

impl Sandbox {
    pub fn resolve(&self, rel: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
        // Resolve ../ and symlinks via the OS kernel
        let canonical = self.root.join(rel.as_ref()).canonicalize()?;

        // Prefix anchor comparison prevents escape
        if !canonical.starts_with(&self.root) {
            anyhow::bail!("path {} escapes sandbox root {}",
                canonical.display(), self.root.display());
        }
        Ok(canonical) // → -32602 invalid-argument on the wire
    }
}
```
