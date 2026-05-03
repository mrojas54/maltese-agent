//! Shared utilities for tool implementations.

use tokio::io::AsyncReadExt as _;
use tokio::process::ChildStderr;
use tokio::task::JoinHandle;

/// Maximum bytes drained from a child process's stderr before the drainer
/// stops reading. Caps memory use when a child emits a pathological error
/// stream while we're streaming its stdout.
pub const MAX_STDERR_DRAIN: usize = 4 * 1024;

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
