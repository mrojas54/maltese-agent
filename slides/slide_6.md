# Sandbox Boundary: Binary Allowlist 🛑

```rust
// cargo, rustc, rustfmt, rg, git, ast-grep: resolved to absolute
// paths from PATH at startup, so a later PATH swap can't sneak in
pub fn check_bin(&self, bin: &str) -> anyhow::Result<()> {
    if !self.allowed_bins.iter().any(|b| b == bin) {
        anyhow::bail!("binary '{}' not in allowlist", bin);
    }
    Ok(())
}

pub async fn exec_run(sandbox: Arc<Sandbox>, args: ExecRunArgs) -> anyhow::Result<ExecRunResult> {
    let bin = sandbox.resolved_bin(&args.cmd)?;
    // Direct process execution: no shell, no parameter expansion
    let mut cmd = tokio::process::Command::new(bin);
    cmd.args(&args.args);
    // Wall-clock timeout (default 30s, per-call timeout_ms)
    // → structured -32001 error with elapsed_ms / limit_ms
    // ...
}
```
