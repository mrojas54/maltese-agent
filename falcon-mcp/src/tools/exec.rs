//! `exec_run`: run an allowlisted external command inside the sandbox.
//!
//! Disabled by default. The MCP server only registers this tool when
//! `--enable-exec` is passed on the CLI, and even then the binary is
//! restricted to the sandbox allowlist (cargo, rustc, rustfmt, rg, git,
//! ast-grep). Arbitrary commands like `curl`, `bash`, or `sh` are rejected.
//!
//! Allowlisted names are executed via the absolute path resolved from the
//! PATH seen at server startup (`Sandbox::resolved_bin`), never by bare-name
//! lookup at call time — so PATH manipulation after startup cannot substitute
//! another binary for an allowlisted name (AC-21).
//!
//! `cwd` is resolved through the sandbox jail and defaults to the sandbox
//! root. Output is captured in full (stdout/stderr) and returned with the
//! child's exit code; a wall-clock `timeout_ms` (default 30s) bounds the run.

use crate::limits;
use crate::sandbox::Sandbox;
use crate::tools::subprocess::run_collect;
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

/// Schema default for `timeout_ms`: the exec timeout constant from the shared
/// limits module (AC-11), env-overridable via `FALCON_MCP_EXEC_TIMEOUT_MS`.
fn default_timeout_ms() -> u64 {
    u64::try_from(limits::exec_timeout().as_millis()).unwrap_or(u64::MAX)
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ExecRunResult {
    pub stdout: String,
    pub stderr: String,
    pub exit: i32,
}

pub async fn exec_run(sandbox: Arc<Sandbox>, args: ExecRunArgs) -> anyhow::Result<ExecRunResult> {
    // Allowlist gate + startup-time resolution in one step: `bin` is the
    // absolute path pinned when the sandbox was constructed, so a PATH
    // changed after startup cannot swap in a different binary (AC-21).
    let bin = sandbox.resolved_bin(&args.cmd)?;
    let cwd = match &args.cwd {
        Some(c) => sandbox.resolve(c).context("resolving cwd")?,
        None => sandbox.root().to_path_buf(),
    };

    let mut cmd = tokio::process::Command::new(bin);
    cmd.args(&args.args).current_dir(&cwd);

    // The shared helper owns stdio hygiene and the wall-clock timeout; the
    // per-call `timeout_ms` from the input schema takes precedence over the
    // module default (AC-11).
    let timeout = Duration::from_millis(args.timeout_ms);
    let out = run_collect(cmd, &format!("'{}'", args.cmd), timeout).await?;

    Ok(ExecRunResult {
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        exit: out.status.code().unwrap_or(-1),
    })
}
