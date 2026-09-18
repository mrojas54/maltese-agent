# Sandbox Boundary: Root Jail 🛡️

```rust
pub struct FsSandbox {
    root_dir: PathBuf,
}

impl FsSandbox {
    pub fn safe_path(&self, requested: &Path) -> Result<PathBuf, McpError> {
        // Resolve ../ and symlinks via OS kernel
        let canonical = std::fs::canonicalize(self.root_dir.join(requested))
            .map_err(|_| McpError::invalid_params("Invalid path resolution"))?;

        // Prefix anchor comparison prevents escape
        if !canonical.starts_with(&self.root_dir) {
            return Err(McpError::invalid_params("Access Denied: Path escape"));
        }
        Ok(canonical)
    }
}
```
