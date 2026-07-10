//! AST-aware search tool powered by ast-grep.

use crate::limits;
use crate::sandbox::Sandbox;
use crate::tools::subprocess;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::ControlFlow;
use std::sync::Arc;

// ---- constants ---------------------------------------------------------------

/// Hard ceiling on the number of AST matches that can be requested via `max`.
/// Callers specifying a larger value are silently clamped to this.
const MAX_AST_RESULTS: usize = 10_000;

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
/// Shells out to `ast-grep --pattern QUERY --lang LANG --json=stream` with
/// `current_dir` set to the sandbox root, via the shared streaming-subprocess
/// helper ([`subprocess::run_streaming`]): stdout is streamed line-by-line,
/// the child is killed once `max` match records have been collected, and the
/// whole run is bounded by the search wall-clock timeout
/// ([`limits::SEARCH_TIMEOUT`], env-overridable via
/// `FALCON_MCP_SEARCH_TIMEOUT_MS`). Returns `{ matches, truncated }`.
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
/// ast-grep exit codes: 0 = matches found, 1 = no matches (not an error), other = error.
pub async fn fs_search_ast(
    sandbox: Arc<Sandbox>,
    args: FsSearchAstArgs,
) -> anyhow::Result<FsSearchAstResult> {
    // Use the unambiguous `ast-grep` binary name. The `sg` shorthand collides
    // with shadow-utils' `sg(1)` (set-group) on Linux, which exists in
    // /usr/bin/sg by default and would be picked up here instead of ast-grep.
    sandbox.check_bin("ast-grep")?;

    // Clamp caller-supplied max to the hard ceiling.
    let effective_max = args.max.min(MAX_AST_RESULTS);

    let mut cmd = tokio::process::Command::new("ast-grep");
    // Use single-token `--flag=value` form so a query or lang starting with
    // `--` is consumed as the value rather than re-parsed as a flag.
    cmd.arg(format!("--pattern={}", args.query));
    cmd.arg(format!("--lang={}", args.lang));
    cmd.arg("--json=stream");
    cmd.current_dir(sandbox.root());

    let mut matches: Vec<AstMatch> = Vec::new();

    let outcome = subprocess::run_streaming(cmd, "ast-grep", limits::search_timeout(), |line| {
        if line.is_empty() {
            return ControlFlow::Continue(());
        }

        // A single malformed line must not abort the whole stream.
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => return ControlFlow::Continue(()),
        };

        let file = v["file"].as_str().unwrap_or("").to_string();
        // range.start.line is 0-based; convert to 1-based with saturating_add.
        let line_number = v["range"]["start"]["line"]
            .as_u64()
            .unwrap_or(0)
            .saturating_add(1);
        let text = v["lines"].as_str().unwrap_or("").trim_end().to_string();

        if matches.len() >= effective_max {
            return ControlFlow::Break(());
        }

        matches.push(AstMatch {
            file,
            line: line_number,
            text,
        });
        ControlFlow::Continue(())
    })
    .await?;

    // exit 0 = matches found; exit 1 = no matches (normal); a self-inflicted
    // kill after truncation is also OK — shared policy in check_status.
    outcome.check_status("ast-grep", 1)?;

    Ok(FsSearchAstResult {
        matches,
        truncated: outcome.truncated,
    })
}
