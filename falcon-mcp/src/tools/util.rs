//! Shared utilities for tool implementations.

use schemars::JsonSchema;
use serde::Serialize;
use tokio::io::AsyncReadExt as _;
use tokio::process::ChildStderr;
use tokio::task::JoinHandle;

/// Maximum bytes drained from a child process's stderr before the drainer
/// stops reading. Caps memory use when a child emits a pathological error
/// stream while we're streaming its stdout.
pub const MAX_STDERR_DRAIN: usize = 4 * 1024;

/// Format the captured stderr of a failed child-process invocation for
/// inclusion in an `anyhow::bail!` message. Truncates to [`MAX_STDERR_DRAIN`]
/// bytes and lossily decodes UTF-8 so a pathological child can't produce a
/// megabytes-long error string. The trailing whitespace is trimmed for
/// readability.
pub fn format_stderr_for_bail(stderr: &[u8]) -> String {
    let cap = stderr.len().min(MAX_STDERR_DRAIN);
    String::from_utf8_lossy(&stderr[..cap]).trim().to_string()
}

/// Generic "no payload, just ack" result for tools whose success state is
/// purely a side effect (e.g. `git_add`, `git_worktree_remove`).
///
/// rmcp 1.x requires every tool's `outputSchema` to have root type `object`,
/// so unit-return tools must produce a typed struct rather than
/// `serde_json::json!({"ok": true})`. This is the shared shape for that.
#[derive(Debug, Serialize, JsonSchema)]
pub struct OkResult {
    pub ok: bool,
}

impl OkResult {
    pub fn ok() -> Self {
        Self { ok: true }
    }
}

/// Spawn a task that drains a child's stderr into a capped in-memory buffer.
///
/// Returns a `JoinHandle` whose value is the bytes read (up to
/// [`MAX_STDERR_DRAIN`]). On read error, returns whatever was read so far.
///
/// Drains concurrently with stdout reads to avoid deadlocking children that
/// fill their stderr pipe while the parent is reading only stdout.
pub fn drain_stderr_capped(stderr: ChildStderr) -> JoinHandle<Vec<u8>> {
    tokio::spawn(async move {
        let mut buf = Vec::with_capacity(MAX_STDERR_DRAIN);
        // read_to_end stops at EOF or when the take limit is reached; on
        // error, buf still contains whatever was read so far.
        let _ = stderr
            .take(MAX_STDERR_DRAIN as u64)
            .read_to_end(&mut buf)
            .await;
        buf
    })
}
