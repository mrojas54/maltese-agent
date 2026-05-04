//! `exec_run`: run an allowlisted external command inside the sandbox.
//!
//! Disabled by default. The MCP server only registers this tool when
//! `--enable-exec` is passed on the CLI, and even then `Sandbox::check_bin`
//! restricts the binary to the same allowlist used by the cargo/git tools
//! (cargo, rustc, rustfmt, ripgrep/rg, git, ast-grep/sg). Arbitrary commands
//! like `curl`, `bash`, or `sh` are rejected.
//!
//! `cwd` is resolved through the sandbox jail and defaults to the sandbox
//! root. Output is captured in full (stdout/stderr) and returned with the
//! child's exit code; a wall-clock `timeout_ms` (default 30s) bounds the run.

use crate::sandbox::Sandbox;
use anyhow::Context;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExecRunArgs {
    /// Binary name to run. Must be in the sandbox's allowlist.
    pub cmd: String,
    /// Arguments passed to the binary.
    #[serde(default)]
    pub args: Vec<String>,
    /// Working directory relative to the sandbox root. Defaults to the root.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Wall-clock timeout in milliseconds. Defaults to 30000.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_timeout_ms() -> u64 {
    30_000
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ExecRunResult {
    pub stdout: String,
    pub stderr: String,
    pub exit: i32,
}

pub async fn exec_run(sandbox: Arc<Sandbox>, args: ExecRunArgs) -> anyhow::Result<ExecRunResult> {
    sandbox.check_bin(&args.cmd)?;
    let cwd = match &args.cwd {
        Some(c) => sandbox.resolve(c).context("resolving cwd")?,
        None => sandbox.root().to_path_buf(),
    };

    let mut cmd = tokio::process::Command::new(&args.cmd);
    cmd.args(&args.args).current_dir(&cwd);

    // Detach child stdio from parent's stdio. The MCP server is talking
    // JSON-RPC on stdin/stdout, so any inheritance corrupts the protocol.
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let timeout = Duration::from_millis(args.timeout_ms);
    let out = tokio::time::timeout(timeout, cmd.output())
        .await
        .map_err(|_| anyhow::anyhow!("exec timeout after {}ms", args.timeout_ms))?
        .with_context(|| format!("running '{}'", args.cmd))?;

    Ok(ExecRunResult {
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        exit: out.status.code().unwrap_or(-1),
    })
}
