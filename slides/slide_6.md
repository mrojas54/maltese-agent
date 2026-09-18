# Sandbox Boundary: Binary Allowlist 🛑

```rust
pub struct ExecSandbox {
    allowed_binaries: HashSet<String>,
}

impl ExecSandbox {
    pub async fn execute(&self, binary: &str, args: &[String]) -> Result<String, McpError> {
        if !self.allowed_binaries.contains(binary) {
            return Err(McpError::invalid_params("Binary not permitted"));
        }

        // Direct process execution: No shell, no parameter expansion
        let output = tokio::time::timeout(
            Duration::from_secs(10),
            tokio::process::Command::new(binary).args(args).output()
        ).await.map_err(|_| McpError::internal_error("Execution timed out"))??;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
```
