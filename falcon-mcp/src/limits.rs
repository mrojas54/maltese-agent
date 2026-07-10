//! Named limit constants for falcon-mcp tools (WS-3, AC-11).
//!
//! Every subprocess timeout default lives in this one module. Each timeout is
//! overridable at runtime via an environment variable named
//! `FALCON_MCP_<CONST_NAME>_MS` (e.g. `FALCON_MCP_CARGO_TIMEOUT_MS=5000`),
//! read per call so tests can configure a spawned server without rebuilding.
//! Invalid or zero overrides are ignored with a warning and the compiled
//! default is used.
//!
//! Tools whose input schema exposes `timeout_ms` (`exec_run`) honor the
//! per-call value over the default from this module.

use std::time::Duration;

/// Wall-clock timeout for the cargo tool family (`cargo_check`, `cargo_test`,
/// `cargo_clippy`, `cargo_fmt`).
///
/// Do NOT lower this below 900s: the TypeScript client
/// (`falcon-detective/src/lib/mcp.ts:59-66`) documents cold `cargo
/// check/test/clippy` runs exceeding 5 minutes when compiling
/// tokio + axum + hyper from scratch, and pins its per-call MCP timeout at
/// 900_000 ms for exactly that reason. Invariant: the client-side call
/// timeout stays >= this server-side limit, so the server's structured
/// timeout error fires no later than the client's abort.
pub const CARGO_TIMEOUT: Duration = Duration::from_secs(900);

/// Wall-clock timeout for the git tool family (`git_worktree_add`,
/// `git_worktree_remove`, `git_add`, `git_commit`, `git_diff`, `git_log`,
/// `git_blame`).
pub const GIT_TIMEOUT: Duration = Duration::from_secs(60);

/// Wall-clock timeout for the streaming search tools (`fs_search`,
/// `fs_search_ast`).
pub const SEARCH_TIMEOUT: Duration = Duration::from_secs(30);

/// Default wall-clock timeout for `exec_run` when the caller omits the
/// per-call `timeout_ms` argument.
pub const EXEC_TIMEOUT: Duration = Duration::from_secs(30);

/// Truncate child stderr to this many bytes before embedding it in an error
/// message, to avoid propagating multi-megabyte crash dumps.
///
/// Defined exactly once, here (AC-25); shared by every tool that reports a
/// child's stderr.
pub const MAX_STDERR_DISPLAY: usize = 1024;

/// `--max-filesize` value passed to ripgrep by `fs_search`: defense-in-depth
/// so a single large file cannot generate unbounded output even before the
/// streaming early-exit kicks in.
pub const SEARCH_MAX_FILESIZE: &str = "10M";

/// [`CARGO_TIMEOUT`] with the `FALCON_MCP_CARGO_TIMEOUT_MS` override applied.
pub fn cargo_timeout() -> Duration {
    with_env_override("CARGO_TIMEOUT", CARGO_TIMEOUT)
}

/// [`GIT_TIMEOUT`] with the `FALCON_MCP_GIT_TIMEOUT_MS` override applied.
pub fn git_timeout() -> Duration {
    with_env_override("GIT_TIMEOUT", GIT_TIMEOUT)
}

/// [`SEARCH_TIMEOUT`] with the `FALCON_MCP_SEARCH_TIMEOUT_MS` override applied.
pub fn search_timeout() -> Duration {
    with_env_override("SEARCH_TIMEOUT", SEARCH_TIMEOUT)
}

/// [`EXEC_TIMEOUT`] with the `FALCON_MCP_EXEC_TIMEOUT_MS` override applied.
pub fn exec_timeout() -> Duration {
    with_env_override("EXEC_TIMEOUT", EXEC_TIMEOUT)
}

/// Read `FALCON_MCP_<name>_MS` from the environment and parse it as integer
/// milliseconds, falling back to `default` when unset or invalid.
fn with_env_override(name: &str, default: Duration) -> Duration {
    let var = format!("FALCON_MCP_{name}_MS");
    parse_ms_override(std::env::var(&var).ok().as_deref(), default, &var)
}

/// Pure parsing core for env overrides, split out so it is unit-testable
/// without mutating process-global environment (which races parallel tests).
///
/// Zero is rejected alongside garbage: a 0ms timeout would instantly kill
/// every child, and "0 = disabled" would silently remove the hang protection
/// this module exists to provide.
fn parse_ms_override(raw: Option<&str>, default: Duration, var_name: &str) -> Duration {
    let Some(raw) = raw else {
        return default;
    };
    match raw.trim().parse::<u64>() {
        Ok(ms) if ms > 0 => Duration::from_millis(ms),
        _ => {
            tracing::warn!(
                "ignoring invalid {var_name}={raw:?} (want positive integer milliseconds); \
                 using default {}ms",
                default.as_millis()
            );
            default
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT: Duration = Duration::from_secs(30);

    #[test]
    fn unset_returns_default() {
        assert_eq!(parse_ms_override(None, DEFAULT, "X"), DEFAULT);
    }

    #[test]
    fn valid_ms_overrides_default() {
        assert_eq!(
            parse_ms_override(Some("500"), DEFAULT, "X"),
            Duration::from_millis(500)
        );
    }

    #[test]
    fn surrounding_whitespace_is_tolerated() {
        assert_eq!(
            parse_ms_override(Some(" 1500 "), DEFAULT, "X"),
            Duration::from_millis(1500)
        );
    }

    #[test]
    fn garbage_falls_back_to_default() {
        assert_eq!(parse_ms_override(Some("fast"), DEFAULT, "X"), DEFAULT);
        assert_eq!(parse_ms_override(Some(""), DEFAULT, "X"), DEFAULT);
        assert_eq!(parse_ms_override(Some("-5"), DEFAULT, "X"), DEFAULT);
        assert_eq!(parse_ms_override(Some("1.5"), DEFAULT, "X"), DEFAULT);
    }

    #[test]
    fn zero_is_rejected() {
        assert_eq!(parse_ms_override(Some("0"), DEFAULT, "X"), DEFAULT);
    }

    #[test]
    fn cargo_default_stays_at_900s() {
        // Contract pin (SPEC WS-3): the client (mcp.ts:59-66) uses a 900s
        // call timeout for cold cargo builds; the server default must not
        // undercut it. If this assertion fails, someone lowered the constant.
        assert_eq!(CARGO_TIMEOUT, Duration::from_secs(900));
    }
}
