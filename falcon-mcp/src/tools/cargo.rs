//! Cargo tools: `cargo_check` (compile diagnostics), `cargo_test`
//! (test pass/fail summary), `cargo_clippy` (clippy lints), and `cargo_fmt`
//! (formatting check / apply). All shell out to `cargo` inside the sandbox
//! and parse its output (JSON or, for fmt, plain text) into structured results.

use crate::sandbox::Sandbox;
use crate::tools::util::format_stderr_for_bail;
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
    /// Actionable guidance: each `children[*]` of level `help`/`note` plus
    /// any non-null `suggested_replacement` from those children's spans.
    /// Empty when the diagnostic ships no actionable hints. The detective
    /// uses this so its propose-fix prompts have clippy's literal advice
    /// (e.g. "remove this call to `default`") instead of having to invent
    /// an edit from the warning text alone.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub help: Vec<String>,
}

/// Walk a rustc/clippy diagnostic's `children` array and collect actionable
/// hints: each `help`/`note` child's `message`, plus any non-null
/// `suggested_replacement` from its `spans` (rendered as `→ "<repl>"` so an
/// empty-string replacement reads as a deletion). Pure function so it can be
/// unit-tested against captured cargo JSON without spawning a subprocess.
fn extract_help(diag: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    let Some(children) = diag["children"].as_array() else {
        return out;
    };
    for child in children {
        let level = child["level"].as_str().unwrap_or("");
        if level != "help" && level != "note" {
            continue;
        }
        if let Some(msg) = child["message"].as_str() {
            if !msg.is_empty() {
                out.push(msg.to_string());
            }
        }
        if let Some(spans) = child["spans"].as_array() {
            for span in spans {
                if let Some(repl) = span["suggested_replacement"].as_str() {
                    out.push(format!("\u{2192} \"{repl}\""));
                }
            }
        }
    }
    out
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CargoCheckResult {
    pub errors: Vec<CargoMessage>,
    pub warnings: Vec<CargoMessage>,
}

/// Run `cargo SUBCMD --message-format=json` inside the sandbox and return
/// both the parsed JSON lines and the raw process [`std::process::Output`].
///
/// Non-JSON lines (including the empty trailing line cargo produces) are
/// silently dropped from the parsed-messages vec. The raw `Output` is
/// returned so the caller can decide how to interpret a non-success exit
/// status — failing diagnostics legitimately produce non-zero exits, but
/// "cargo couldn't even start" (missing manifest, busted toolchain) also
/// does, and only the caller knows which carve-out applies.
async fn run_cargo_json(
    sandbox: Arc<Sandbox>,
    crate_path: &str,
    subcmd: &[&str],
) -> anyhow::Result<(Vec<serde_json::Value>, std::process::Output)> {
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
    Ok((msgs, out))
}

/// Run `cargo check` and bucket compiler-message diagnostics into `errors`
/// and `warnings`. Other diagnostic levels (`note`, `help`, etc.) are dropped.
pub async fn cargo_check(
    sandbox: Arc<Sandbox>,
    args: CargoCheckArgs,
) -> anyhow::Result<CargoCheckResult> {
    let (msgs, out) = run_cargo_json(sandbox, &args.crate_path, &["check"]).await?;

    // Cargo died before producing any parseable output (missing manifest,
    // toolchain missing, target dir locked, etc.). Surface stderr so the
    // agent sees a real error chain instead of an empty {errors:[],warnings:[]}.
    if !out.status.success() && msgs.is_empty() {
        let stderr = format_stderr_for_bail(&out.stderr);
        anyhow::bail!("cargo check failed before producing output: {stderr}");
    }

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
            help: extract_help(diag),
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
    // TODO: drop once libtest JSON output stabilizes (rust-lang/rust#49359).
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

    // Failing tests legitimately produce non-zero exit AND non-empty parsed
    // events, so the carve-out is "non-success but we parsed nothing" — that
    // means cargo died before any tests ran (build failed, fixture missing,
    // toolchain busted) and the agent needs the real stderr.
    if !out.status.success() && passed.is_empty() && failed.is_empty() {
        let stderr = format_stderr_for_bail(&out.stderr);
        anyhow::bail!("cargo test failed before producing output: {stderr}");
    }

    Ok(CargoTestResult {
        passed,
        failed,
        duration_ms: start.elapsed().as_millis(),
    })
}

// ---- cargo_clippy ------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CargoClippyArgs {
    /// Path (relative to the sandbox root) of the crate or workspace to lint.
    pub crate_path: String,
    /// When true, run clippy with `--fix --allow-dirty` to apply machine-applicable fixes.
    #[serde(default)]
    pub fix: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CargoClippyResult {
    /// One entry per `clippy::*` diagnostic emitted by the lint pass.
    pub lints: Vec<CargoMessage>,
}

/// Run `cargo clippy` (optionally with `--fix --allow-dirty`) and return the
/// parsed `clippy::*` diagnostics. Non-clippy compiler messages are dropped:
/// callers wanting plain compile errors should use `cargo_check`.
pub async fn cargo_clippy(
    sandbox: Arc<Sandbox>,
    args: CargoClippyArgs,
) -> anyhow::Result<CargoClippyResult> {
    if args.fix {
        // `--fix` rewrites source files in place, so the sandbox's writable
        // gate must apply (parallel to `fs_write`/`fs_apply_patch`).
        sandbox.check_writable()?;
    }
    let mut subcmd: Vec<&str> = vec!["clippy"];
    if args.fix {
        subcmd.push("--fix");
        subcmd.push("--allow-dirty");
    }
    let (msgs, out) = run_cargo_json(sandbox, &args.crate_path, &subcmd).await?;

    let mut lints = Vec::new();
    for m in msgs {
        if m["reason"] != "compiler-message" {
            continue;
        }
        let diag = &m["message"];
        let code = diag["code"]["code"].as_str().unwrap_or("");
        if !code.starts_with("clippy::") {
            continue;
        }
        lints.push(CargoMessage {
            level: diag["level"].as_str().unwrap_or("").to_string(),
            message: format!("{}: {}", code, diag["message"].as_str().unwrap_or("")),
            file: diag["spans"][0]["file_name"].as_str().map(String::from),
            line: diag["spans"][0]["line_start"].as_u64().map(|n| n as u32),
            help: extract_help(diag),
        });
    }

    // Same infra-failure carve-out as `cargo_check`: clippy legitimately exits
    // non-zero when lints fire (deny-by-default lints, `-D warnings`, etc.),
    // so we only bail when cargo died before producing anything to parse.
    if !out.status.success() && lints.is_empty() {
        let stderr = format_stderr_for_bail(&out.stderr);
        anyhow::bail!("cargo clippy failed before producing output: {stderr}");
    }
    Ok(CargoClippyResult { lints })
}

// ---- cargo_fmt ---------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CargoFmtArgs {
    /// Path (relative to the sandbox root) of the crate or workspace to format.
    pub crate_path: String,
    /// When true, run `cargo fmt -- --check` (no in-place changes); reports
    /// files that would change. When false, apply formatting in place.
    #[serde(default)]
    pub check: bool,
}

/// Result of a `cargo fmt` invocation. `status` is `"ok"` when no formatting
/// changes were needed (or formatting was applied successfully) and `"diffs"`
/// when `--check` was requested and at least one file would be reformatted.
///
/// Modeled as a flat struct (rather than a tagged enum) because rmcp 1.x
/// requires every tool's `outputSchema` to have root type `object`, and a
/// `serde(tag = ...)` enum with a unit variant generates a `oneOf` schema
/// without that root type.
#[derive(Debug, Serialize, JsonSchema)]
pub struct CargoFmtResult {
    /// Either `"ok"` (no diffs, or formatting applied) or `"diffs"`
    /// (check mode found files needing reformatting).
    pub status: String,
    /// Files with formatting differences. Populated only when
    /// `status == "diffs"`; empty otherwise.
    pub files: Vec<String>,
}

/// Run `cargo fmt`, optionally in `--check` mode. In check mode, non-success
/// exit with parseable `Diff in <path>` lines on stdout means "files differ"
/// and is returned as `Diffs`; non-success with no parseable diffs is treated
/// as a real infra failure (rustfmt missing, manifest broken, etc.) and
/// surfaced via `anyhow::bail!`.
pub async fn cargo_fmt(
    sandbox: Arc<Sandbox>,
    args: CargoFmtArgs,
) -> anyhow::Result<CargoFmtResult> {
    sandbox.check_bin("cargo")?;
    if !args.check {
        // Without `--check`, rustfmt rewrites files in place, so the
        // sandbox's writable gate must apply (parallel to `fs_write`).
        sandbox.check_writable()?;
    }
    let path = sandbox.resolve(&args.crate_path).context("resolving crate_path")?;

    let mut cmd = tokio::process::Command::new("cargo");
    cmd.arg("fmt");
    if args.check {
        // `--check` alone causes rustfmt to emit `Diff in <path>:` lines on
        // stdout for every file that would change, and exit non-zero. The
        // plan's sketch combined `--check` with `--emit=files`, but rustfmt
        // rejects that combination ("Invalid to use `--emit` and `--check`").
        cmd.arg("--").arg("--check");
    }
    cmd.current_dir(&path);

    // Same stdio hygiene as the other cargo wrappers: keep the MCP JSON-RPC
    // channel clean by not letting the child inherit our stdio.
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let out = cmd.output().await.context("running cargo fmt")?;
    if out.status.success() {
        return Ok(CargoFmtResult {
            status: "ok".to_string(),
            files: Vec::new(),
        });
    }

    let files: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.starts_with("Diff in "))
        .map(|l| {
            l.trim_start_matches("Diff in ")
                .trim_end_matches(':')
                .to_string()
        })
        .collect();
    if files.is_empty() {
        let stderr = format_stderr_for_bail(&out.stderr);
        anyhow::bail!("cargo fmt failed before producing diffs: {stderr}");
    }
    Ok(CargoFmtResult {
        status: "diffs".to_string(),
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real `default_constructed_unit_structs` diagnostic captured from
    /// running `cargo clippy --message-format=json` on falcon-agent. Pinning
    /// against a real diag protects against drift in cargo's JSON shape.
    const CLIPPY_DIAG: &str = include_str!("cargo_test_clippy_diag.json");

    #[test]
    fn extract_help_pulls_clippy_suggestion_text() {
        let v: serde_json::Value = serde_json::from_str(CLIPPY_DIAG).unwrap();
        let diag = &v["message"];
        let help = extract_help(diag);

        // The "remove this call to `default`" hint must survive — that's
        // the point of this whole field. Without it, the propose-lint-fix
        // prompt has to invent a fix from the warning name alone.
        assert!(
            help.iter().any(|h| h.contains("remove this call to `default`")),
            "expected the clippy 'remove this call' help to be extracted, got: {help:?}"
        );

        // The machine-applicable suggested_replacement (here, an empty string
        // meaning "delete the highlighted span") should also be surfaced so
        // the LLM sees the literal target, not just the prose hint.
        assert!(
            help.iter().any(|h| h.starts_with("\u{2192} \"")),
            "expected the suggested-replacement marker to appear, got: {help:?}"
        );
    }

    #[test]
    fn extract_help_returns_empty_when_no_children() {
        let diag = serde_json::json!({
            "message": "some warning",
            "level": "warning",
            "spans": [],
            "children": []
        });
        assert!(extract_help(&diag).is_empty());
    }

    #[test]
    fn extract_help_skips_non_help_non_note_children() {
        // Children at "error" level are sub-errors, not actionable hints —
        // we only want help/note rungs in `help`.
        let diag = serde_json::json!({
            "children": [
                { "level": "error", "message": "ignored", "spans": [] },
                { "level": "help", "message": "kept", "spans": [] }
            ]
        });
        let help = extract_help(&diag);
        assert_eq!(help, vec!["kept".to_string()]);
    }
}
