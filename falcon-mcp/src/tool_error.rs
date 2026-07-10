//! Tool-error taxonomy for the `FalconMcp` server boundary (WS-14c, AC-32).
//!
//! Every handler in `server.rs` funnels its `anyhow::Error` through
//! [`ToolError::classify`], so an MCP client can *branch* on the failure kind
//! instead of parsing an opaque string:
//!
//! | kind             | client-visible surface                               | code   |
//! |------------------|------------------------------------------------------|--------|
//! | not-found        | JSON-RPC protocol error (`ErrorData`)                | -32002 |
//! | invalid-argument | JSON-RPC protocol error (`ErrorData`)                | -32602 |
//! | timeout          | JSON-RPC protocol error (`ErrorData`, structured data)| -32001 |
//! | internal         | `CallToolResult { is_error: true }` (result-level)   | —      |
//!
//! `internal` deliberately stays result-level: the MCP spec routes tool
//! *execution* failures (child-process errors, read-only rejections) through
//! `isError: true` results, and the sandbox test suite (guardrail G-2) pins
//! that shape for read-only rejections. The coded kinds are addressing and
//! argument failures, which belong on the protocol-error surface.
//!
//! Known trade-off: classification rule 2 (any `io::ErrorKind::NotFound` in
//! the chain → not-found) also matches a *tool binary* missing at spawn time.
//! The message text disambiguates, and hermetic environments never hit it.

use crate::tools::subprocess::SubprocessTimeout;
use rmcp::handler::server::tool::IntoCallToolResult;
use rmcp::model::{CallToolResult, Content, ErrorCode, ErrorData};

/// Dedicated JSON-RPC code for wall-clock subprocess timeouts. No MCP-spec
/// reserved code exists for this; `-32001` mirrors the TypeScript MCP SDK's
/// `RequestTimeout` and sits in the implementation-defined server range.
pub const TIMEOUT_ERROR_CODE: ErrorCode = ErrorCode(-32001);

/// A classified tool-handler failure. See the module docs for the mapping to
/// client-visible MCP surfaces.
#[derive(Debug)]
pub enum ToolError {
    /// An addressed path/resource does not exist (`-32002`,
    /// [`ErrorCode::RESOURCE_NOT_FOUND`]).
    NotFound(String),
    /// The arguments were semantically invalid — sandbox-escaping path,
    /// disallowed binary (`-32602`, [`ErrorCode::INVALID_PARAMS`]).
    InvalidArgument(String),
    /// A child process exceeded its wall-clock limit — carries the
    /// structured fields from [`SubprocessTimeout`] ([`TIMEOUT_ERROR_CODE`]).
    Timeout {
        message: String,
        elapsed_ms: u64,
        limit_ms: u64,
    },
    /// Everything else: reported result-level (`is_error: true`), preserving
    /// the pre-taxonomy shape for execution failures.
    Internal(String),
}

impl ToolError {
    /// Construct an internal-kind error from a plain message (used for
    /// server-side gates that never produce an `anyhow::Error`).
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    /// Classify an `anyhow::Error` coming out of a tool implementation.
    ///
    /// Rules, in order (first match wins):
    /// 1. [`SubprocessTimeout`] anywhere in the chain → [`Self::Timeout`].
    /// 2. `std::io::Error` with [`std::io::ErrorKind::NotFound`] anywhere in
    ///    the chain → [`Self::NotFound`] (missing file/dir path).
    /// 3. Sandbox argument rejections → [`Self::InvalidArgument`]. The
    ///    sandbox reports these as ad-hoc `anyhow!` messages
    ///    (`src/sandbox.rs`: "escapes sandbox root", "not in allowlist"), so
    ///    this rule string-matches; the unit tests below construct *real*
    ///    sandbox errors so any rewording in `sandbox.rs` fails loudly here.
    /// 4. Everything else → [`Self::Internal`] (read-only rejections,
    ///    child-process stderr failures, non-UTF-8 content, ...).
    pub fn classify(err: anyhow::Error) -> Self {
        if let Some(t) = err
            .chain()
            .find_map(|cause| cause.downcast_ref::<SubprocessTimeout>())
        {
            return Self::Timeout {
                elapsed_ms: t.elapsed_ms,
                limit_ms: t.limit_ms,
                message: format!("{err:#}"),
            };
        }
        if err.chain().any(|cause| {
            cause
                .downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound)
        }) {
            return Self::NotFound(format!("{err:#}"));
        }
        let message = format!("{err:#}");
        if message.contains("escapes sandbox root") || message.contains("not in allowlist") {
            return Self::InvalidArgument(message);
        }
        Self::Internal(message)
    }
}

impl IntoCallToolResult for ToolError {
    fn into_call_tool_result(self) -> Result<CallToolResult, ErrorData> {
        match self {
            Self::NotFound(message) => Err(ErrorData::new(
                ErrorCode::RESOURCE_NOT_FOUND,
                message,
                Some(serde_json::json!({"kind": "not-found"})),
            )),
            Self::InvalidArgument(message) => Err(ErrorData::new(
                ErrorCode::INVALID_PARAMS,
                message,
                Some(serde_json::json!({"kind": "invalid-argument"})),
            )),
            Self::Timeout {
                message,
                elapsed_ms,
                limit_ms,
            } => Err(ErrorData::new(
                TIMEOUT_ERROR_CODE,
                message,
                Some(serde_json::json!({
                    "kind": "timeout",
                    "elapsed_ms": elapsed_ms,
                    "limit_ms": limit_ms,
                })),
            )),
            // Result-level error: rmcp's blanket `Result<T, E>` impl marks
            // `is_error = Some(true)` on this branch (and `CallToolResult::
            // error` already sets it), so clients see the pre-taxonomy shape.
            Self::Internal(message) => Ok(CallToolResult::error(vec![Content::text(message)])),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sandbox::Sandbox;
    use tempfile::TempDir;

    fn error_data(e: ToolError) -> ErrorData {
        e.into_call_tool_result()
            .expect_err("expected a protocol-level ErrorData")
    }

    /// Drift guard: a REAL escape error from `Sandbox::resolve` must classify
    /// as invalid-argument. Rewording sandbox.rs's message breaks this test,
    /// not production classification silently.
    #[test]
    fn classify_real_sandbox_escape_is_invalid_argument() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hi").unwrap();
        let sb = Sandbox::new(dir.path().to_path_buf(), false).unwrap();
        let err = sb.resolve("../escape.txt").unwrap_err();
        assert!(
            matches!(ToolError::classify(err), ToolError::InvalidArgument(_)),
            "sandbox escape must classify as invalid-argument"
        );
    }

    /// Drift guard: a REAL allowlist rejection from `Sandbox::check_bin`.
    #[test]
    fn classify_real_allowlist_rejection_is_invalid_argument() {
        let dir = TempDir::new().unwrap();
        let sb = Sandbox::new(dir.path().to_path_buf(), false).unwrap();
        let err = sb.check_bin("curl").unwrap_err();
        assert!(
            matches!(ToolError::classify(err), ToolError::InvalidArgument(_)),
            "allowlist rejection must classify as invalid-argument"
        );
    }

    #[test]
    fn classify_io_not_found_in_chain_is_not_found() {
        use anyhow::Context as _;
        let dir = TempDir::new().unwrap();
        // Same chain shape as fs_read: io NotFound wrapped in context.
        let err = std::fs::read(dir.path().join("missing.txt"))
            .context("reading file")
            .unwrap_err();
        assert!(
            matches!(ToolError::classify(err), ToolError::NotFound(_)),
            "io NotFound must classify as not-found"
        );
    }

    #[test]
    fn classify_subprocess_timeout_is_timeout_with_fields() {
        let err = anyhow::Error::new(SubprocessTimeout {
            label: "cargo check".into(),
            elapsed_ms: 2345,
            limit_ms: 2000,
        })
        .context("running cargo check");
        match ToolError::classify(err) {
            ToolError::Timeout {
                message,
                elapsed_ms,
                limit_ms,
            } => {
                assert_eq!(elapsed_ms, 2345);
                assert_eq!(limit_ms, 2000);
                assert!(message.contains("elapsed_ms=2345"), "got: {message}");
                assert!(message.contains("limit_ms=2000"), "got: {message}");
            }
            other => panic!("expected Timeout, got: {other:?}"),
        }
    }

    #[test]
    fn classify_misc_error_is_internal() {
        let err = anyhow::anyhow!("sandbox is read-only");
        assert!(
            matches!(ToolError::classify(err), ToolError::Internal(_)),
            "unrecognized errors must fall back to internal"
        );
    }

    #[test]
    fn coded_kinds_have_pairwise_distinct_standard_codes() {
        let not_found = error_data(ToolError::NotFound("x".into())).code;
        let invalid = error_data(ToolError::InvalidArgument("x".into())).code;
        let timeout = error_data(ToolError::Timeout {
            message: "x".into(),
            elapsed_ms: 1,
            limit_ms: 1,
        })
        .code;
        assert_eq!(not_found, ErrorCode::RESOURCE_NOT_FOUND);
        assert_eq!(not_found.0, -32002);
        assert_eq!(invalid, ErrorCode::INVALID_PARAMS);
        assert_eq!(invalid.0, -32602);
        assert_eq!(timeout.0, -32001);
        assert_ne!(not_found, invalid);
        assert_ne!(not_found, timeout);
        assert_ne!(invalid, timeout);
    }

    #[test]
    fn timeout_error_data_carries_structured_fields() {
        let data = error_data(ToolError::Timeout {
            message: "rg timed out".into(),
            elapsed_ms: 1234,
            limit_ms: 1000,
        })
        .data
        .expect("timeout must carry data");
        assert_eq!(data["kind"], "timeout");
        assert_eq!(data["elapsed_ms"], 1234);
        assert_eq!(data["limit_ms"], 1000);
    }

    #[test]
    fn internal_is_result_level_not_protocol_error() {
        let result = ToolError::internal("boom")
            .into_call_tool_result()
            .expect("internal must be result-level, not a protocol error");
        assert_eq!(result.is_error, Some(true));
        let dump = format!("{result:?}");
        assert!(dump.contains("boom"), "message must survive: {dump}");
    }
}
