//! Cargo tools: `cargo_check` (compile diagnostics) and `cargo_test`
//! (test pass/fail summary). Both shell out to `cargo` inside the sandbox
//! and parse its JSON output streams into structured results.

use crate::sandbox::Sandbox;
use anyhow::Context;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ---- cargo_check -------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CargoCheckArgs {
    /// Path (relative to the sandbox root) of the crate or workspace to check.
    pub crate_path: String,
}

/// One compiler-emitted diagnostic, distilled from cargo's `--message-format=json` stream.
#[derive(Debug, Serialize, JsonSchema)]
pub struct CargoMessage {
    /// The diagnostic level reported by rustc (e.g. "error", "warning").
    pub level: String,
    /// Human-readable message text.
    pub message: String,
    /// Path of the primary span's file, if any. Relative to the crate as cargo reports it.
    pub file: Option<String>,
    /// 1-based line number of the primary span, if any.
    pub line: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CargoCheckResult {
    pub errors: Vec<CargoMessage>,
    pub warnings: Vec<CargoMessage>,
}

/// Run `cargo SUBCMD --message-format=json` inside the sandbox and return
/// each parsed JSON line as a `serde_json::Value`. Non-JSON lines (including
/// the empty trailing line cargo produces) are silently dropped.
async fn run_cargo_json(
    sandbox: Arc<Sandbox>,
    crate_path: &str,
    subcmd: &[&str],
) -> anyhow::Result<Vec<serde_json::Value>> {
    sandbox.check_bin("cargo")?;
    let path = sandbox.resolve(crate_path).context("resolving crate_path")?;

    let mut cmd = tokio::process::Command::new("cargo");
    for s in subcmd {
        cmd.arg(s);
    }
    cmd.arg("--message-format=json").current_dir(&path);

    // Detach child stdio from parent's stdio. The MCP server is talking
    // JSON-RPC on stdin/stdout, so any inheritance corrupts the protocol.
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let out = cmd.output().await.context("running cargo")?;
    let mut msgs = Vec::new();
    for line in out.stdout.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(line) {
            msgs.push(v);
        }
    }
    Ok(msgs)
}

/// Run `cargo check` and bucket compiler-message diagnostics into `errors`
/// and `warnings`. Other diagnostic levels (`note`, `help`, etc.) are dropped.
pub async fn cargo_check(
    sandbox: Arc<Sandbox>,
    args: CargoCheckArgs,
) -> anyhow::Result<CargoCheckResult> {
    let msgs = run_cargo_json(sandbox, &args.crate_path, &["check"]).await?;
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    for m in msgs {
        if m["reason"] != "compiler-message" {
            continue;
        }
        let diag = &m["message"];
        let level = diag["level"].as_str().unwrap_or("").to_string();
        let entry = CargoMessage {
            level: level.clone(),
            message: diag["message"].as_str().unwrap_or("").to_string(),
            file: diag["spans"][0]["file_name"].as_str().map(String::from),
            line: diag["spans"][0]["line_start"].as_u64().map(|n| n as u32),
        };
        if level == "error" || level == "error: internal compiler error" {
            errors.push(entry);
        } else if level == "warning" {
            warnings.push(entry);
        }
    }
    Ok(CargoCheckResult { errors, warnings })
}

// ---- cargo_test --------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CargoTestArgs {
    /// Path (relative to the sandbox root) of the crate or workspace to test.
    pub crate_path: String,
    /// Optional test-name filter forwarded to libtest (positional after `--`).
    #[serde(default)]
    pub filter: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CargoTestCase {
    pub name: String,
    pub stdout: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CargoTestResult {
    pub passed: Vec<String>,
    pub failed: Vec<CargoTestCase>,
    pub duration_ms: u128,
}

/// Run `cargo test` with libtest's unstable `--format=json` output and parse
/// the per-test events into pass/fail buckets.
///
/// Notes:
///  * `RUSTC_BOOTSTRAP=1` is required for the unstable libtest JSON formatter
///    on stable toolchains.
///  * A non-zero cargo exit status is expected when tests fail — we DO NOT
///    propagate it as an error; the JSON stream already reflects per-test
///    results, which is what callers want.
pub async fn cargo_test(
    sandbox: Arc<Sandbox>,
    args: CargoTestArgs,
) -> anyhow::Result<CargoTestResult> {
    sandbox.check_bin("cargo")?;
    let path = sandbox.resolve(&args.crate_path).context("resolving crate_path")?;

    let mut cmd = tokio::process::Command::new("cargo");
    cmd.arg("test")
        .arg("--")
        .arg("-Z")
        .arg("unstable-options")
        .arg("--format=json")
        .arg("--report-time");
    if let Some(f) = &args.filter {
        cmd.arg(f);
    }
    cmd.current_dir(&path);
    // Required so libtest accepts -Z unstable-options on a stable toolchain.
    cmd.env("RUSTC_BOOTSTRAP", "1");

    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let start = std::time::Instant::now();
    let out = cmd.output().await.context("running cargo test")?;

    let mut passed = Vec::new();
    let mut failed = Vec::new();
    for line in out.stdout.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        let v: serde_json::Value = match serde_json::from_slice(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v["type"] != "test" {
            continue;
        }
        let name = v["name"].as_str().unwrap_or("").to_string();
        match v["event"].as_str() {
            Some("ok") => passed.push(name),
            Some("failed") => failed.push(CargoTestCase {
                name,
                stdout: v["stdout"].as_str().unwrap_or("").to_string(),
            }),
            _ => {}
        }
    }

    Ok(CargoTestResult {
        passed,
        failed,
        duration_ms: start.elapsed().as_millis(),
    })
}
