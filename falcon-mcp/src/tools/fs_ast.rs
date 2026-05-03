//! AST-aware search tool powered by ast-grep (`sg`).

use crate::sandbox::Sandbox;
use anyhow::Context as _;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::io::AsyncBufReadExt as _;

// ---- constants ---------------------------------------------------------------

/// Hard ceiling on the number of AST matches that can be requested via `max`.
/// Callers specifying a larger value are silently clamped to this.
const MAX_AST_RESULTS: usize = 10_000;

/// Truncate `sg` stderr to this many bytes before embedding it in an error
/// message, to avoid propagating multi-megabyte crash dumps.
const MAX_STDERR_DISPLAY: usize = 1024;

/// Maximum bytes drained from `sg` stderr while streaming, to prevent a
/// pathological error output from causing OOM.
const MAX_STDERR_DRAIN: usize = 4 * 1024;

fn default_max() -> usize {
    200
}

fn default_lang() -> String {
    "rust".to_string()
}

// ---- request / response types ------------------------------------------------

/// Arguments for `fs_search_ast`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FsSearchAstArgs {
    /// ast-grep structural pattern (e.g. `$E.unwrap()`).
    pub query: String,
    /// Source language understood by ast-grep (e.g. `"rust"`, `"python"`).
    /// Defaults to `"rust"`.
    #[serde(default = "default_lang")]
    pub lang: String,
    /// Maximum number of match records to return (default 200, hard cap 10 000).
    #[serde(default = "default_max")]
    pub max: usize,
}

/// A single AST match returned by `fs_search_ast`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct AstMatch {
    /// Relative path of the file containing the match.
    pub file: String,
    /// 1-based line number of the match start.
    pub line: u64,
    /// The full source line containing the match (trailing whitespace stripped).
    pub text: String,
}

/// Result returned by `fs_search_ast`.
#[derive(Debug, Serialize, JsonSchema)]
pub struct FsSearchAstResult {
    /// Collected match records, up to `max`.
    pub matches: Vec<AstMatch>,
    /// `true` if the result was capped before ast-grep finished all output.
    pub truncated: bool,
}

// ---- implementation ----------------------------------------------------------

/// Search files inside the sandbox using ast-grep structural pattern matching.
///
/// Shells out to `sg --pattern QUERY --lang LANG --json=stream` with
/// `current_dir` set to the sandbox root. Streams stdout line-by-line; stops
/// reading and kills the child once `max` match records have been collected.
/// Returns `{ matches, truncated }`.
///
/// The `--pattern=QUERY` single-token form is used to prevent argument
/// injection: a query starting with `--` is consumed as the value of
/// `--pattern`, not re-interpreted as a standalone flag.
///
/// ast-grep `--json=stream` emits one JSON object per line. The relevant
/// fields are:
/// - `file`                — relative path (when run from the sandbox root)
/// - `range.start.line`    — 0-based line number (we emit `+ 1` for 1-based)
/// - `lines`               — full source line(s) for the match
///
/// `sg` exit codes: 0 = matches found, 1 = no matches (not an error), other = error.
pub async fn fs_search_ast(
    sandbox: Arc<Sandbox>,
    args: FsSearchAstArgs,
) -> anyhow::Result<FsSearchAstResult> {
    sandbox.check_bin("sg")?;

    // Clamp caller-supplied max to the hard ceiling.
    let effective_max = args.max.min(MAX_AST_RESULTS);

    let mut cmd = tokio::process::Command::new("sg");
    // Use single-token `--flag=value` form so a query or lang starting with
    // `--` is consumed as the value rather than re-parsed as a flag.
    cmd.arg(format!("--pattern={}", args.query));
    cmd.arg(format!("--lang={}", args.lang));
    cmd.arg("--json=stream");
    cmd.current_dir(sandbox.root());

    // Explicitly isolate the child's stdio from the parent's.
    // In stdio-MCP mode the parent's stdin/stdout carry the JSON-RPC channel;
    // if sg inherits them it deadlocks trying to read from the protocol stream.
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    // stderr piped so sg diagnostics can surface in error messages without
    // leaking to the JSON-RPC channel.
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().context("spawning sg")?;

    // Drain stderr concurrently into a capped buffer so it is available for
    // error messages and can never deadlock the stdout reader (a child blocked
    // writing stderr would stall the whole operation if we drained stdout only).
    let stderr_handle = {
        use tokio::io::AsyncReadExt as _;
        let mut stderr = child.stderr.take().expect("stderr is piped");
        tokio::spawn(async move {
            let mut buf = Vec::with_capacity(MAX_STDERR_DRAIN);
            let mut tmp = [0u8; 512];
            loop {
                match stderr.read(&mut tmp).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let remaining = MAX_STDERR_DRAIN.saturating_sub(buf.len());
                        let take = n.min(remaining);
                        buf.extend_from_slice(&tmp[..take]);
                        if buf.len() >= MAX_STDERR_DRAIN {
                            break;
                        }
                    }
                }
            }
            buf
        })
    };

    let stdout = child.stdout.take().expect("stdout is piped");
    let mut reader = tokio::io::BufReader::new(stdout).lines();

    let mut matches: Vec<AstMatch> = Vec::new();
    let mut truncated = false;

    while let Some(line) = reader.next_line().await? {
        if line.is_empty() {
            continue;
        }

        // A single malformed line must not abort the whole stream.
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let file = v["file"].as_str().unwrap_or("").to_string();
        // range.start.line is 0-based; convert to 1-based with saturating_add.
        let line_number = v["range"]["start"]["line"]
            .as_u64()
            .unwrap_or(0)
            .saturating_add(1);
        let text = v["lines"].as_str().unwrap_or("").trim_end().to_string();

        if matches.len() >= effective_max {
            truncated = true;
            break;
        }

        matches.push(AstMatch { file, line: line_number, text });
    }

    // Kill the child — a no-op if it already exited (e.g. after streaming its
    // full output).  We must not leave it running when we break early.
    let _ = child.kill().await;
    let status = child.wait().await.context("waiting for sg")?;

    // Collect stderr (the drainer task should be nearly done by now).
    let stderr_bytes = stderr_handle.await.unwrap_or_default();

    // exit 0 = matches found; exit 1 = no matches (normal); anything else is an error.
    // A signal kill yields code() == None.  We killed the child ourselves on
    // early exit, so None is expected in the truncated path — treat it as OK.
    if !status.success() && status.code() != Some(1) && !(truncated && status.code().is_none()) {
        let display_len = stderr_bytes.len().min(MAX_STDERR_DISPLAY);
        let stderr_snippet = String::from_utf8_lossy(&stderr_bytes[..display_len]);
        anyhow::bail!("sg failed: {}", stderr_snippet);
    }

    Ok(FsSearchAstResult { matches, truncated })
}
