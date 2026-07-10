//! Shared subprocess pipeline for every child-process-spawning tool (WS-3).
//!
//! One place owns spawn → stream → truncate → kill → stderr-drain →
//! wall-clock timeout, so the cargo, git, search, and exec tool families all
//! behave identically: on expiry the child is killed and a structured
//! [`SubprocessTimeout`] error carrying `elapsed_ms`/`limit_ms` is returned
//! (AC-10). Stderr snippets embedded in error messages are capped at
//! [`limits::MAX_STDERR_DISPLAY`] bytes, defined exactly once (AC-25).
//!
//! Two entry points:
//! - [`run_collect`]: run to completion, capture full stdout/stderr
//!   (cargo/git/exec tools).
//! - [`run_streaming`]: stream stdout line-by-line to a callback that can
//!   truncate the run early (search tools).

use crate::limits::MAX_STDERR_DISPLAY;
use crate::tools::util::drain_stderr_capped;
use anyhow::Context as _;
use std::ops::ControlFlow;
use std::process::{ExitStatus, Output, Stdio};
use std::time::{Duration, Instant};
use tokio::io::AsyncBufReadExt as _;
use tokio::process::Command;

/// Structured wall-clock timeout error (AC-10).
///
/// Returned by [`run_collect`] / [`run_streaming`] when the child did not
/// finish within the configured limit. The child has been killed by the time
/// this error is produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubprocessTimeout {
    /// Human-readable label of the command that timed out (e.g. `"cargo check"`).
    pub label: String,
    /// Wall-clock milliseconds actually elapsed when the timeout fired.
    pub elapsed_ms: u64,
    /// The configured limit, in milliseconds, that was exceeded.
    pub limit_ms: u64,
}

impl std::fmt::Display for SubprocessTimeout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} timed out (elapsed_ms={}, limit_ms={}); child process killed",
            self.label, self.elapsed_ms, self.limit_ms
        )
    }
}

impl std::error::Error for SubprocessTimeout {}

/// Outcome of a [`run_streaming`] call that finished (or truncated) within
/// its wall-clock limit.
#[derive(Debug)]
pub struct StreamOutcome {
    /// Exit status of the (possibly killed) child.
    pub status: ExitStatus,
    /// Stderr bytes, capped at [`crate::tools::util::MAX_STDERR_DRAIN`].
    pub stderr: Vec<u8>,
    /// `true` when the line callback stopped the stream early (result cap hit).
    pub truncated: bool,
}

impl StreamOutcome {
    /// Shared exit-status interpretation for the streaming search tools.
    ///
    /// Success when: the child exited 0; or it exited with `benign_code`
    /// (rg and ast-grep use 1 for "no matches — not an error"); or it died to
    /// a signal (`code() == None`) after WE killed it on early truncation.
    /// Anything else is an error, reported with a stderr snippet capped at
    /// [`MAX_STDERR_DISPLAY`] bytes.
    pub fn check_status(&self, label: &str, benign_code: i32) -> anyhow::Result<()> {
        if self.status.success()
            || self.status.code() == Some(benign_code)
            || (self.truncated && self.status.code().is_none())
        {
            return Ok(());
        }
        let display_len = self.stderr.len().min(MAX_STDERR_DISPLAY);
        let stderr_snippet = String::from_utf8_lossy(&self.stderr[..display_len]);
        anyhow::bail!("{label} failed: {stderr_snippet}");
    }
}

/// How long [`run_streaming`] waits for the stderr drain to reach EOF after
/// the child has been reaped, before giving up on the diagnostics snippet.
const STDERR_GRACE: Duration = Duration::from_secs(1);

/// Apply the stdio hygiene every tool needs: in stdio-MCP mode the parent's
/// stdin/stdout carry the JSON-RPC channel, so any inheritance corrupts the
/// protocol (or deadlocks a child that tries to read from it). `kill_on_drop`
/// guarantees a SIGKILL even if the wrapping future is dropped mid-run.
fn apply_stdio_hygiene(cmd: &mut Command) {
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);
}

/// Build the structured timeout error for a child that just got killed.
fn timeout_error(label: &str, start: Instant, limit: Duration) -> SubprocessTimeout {
    SubprocessTimeout {
        label: label.to_string(),
        elapsed_ms: u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX),
        limit_ms: u64::try_from(limit.as_millis()).unwrap_or(u64::MAX),
    }
}

/// Run `cmd` to completion with a wall-clock timeout, capturing full
/// stdout/stderr (like [`tokio::process::Command::output`]).
///
/// On expiry the child is killed (via `kill_on_drop` when the output future
/// is dropped) and a [`SubprocessTimeout`] is returned.
pub async fn run_collect(
    mut cmd: Command,
    label: &str,
    timeout: Duration,
) -> anyhow::Result<Output> {
    apply_stdio_hygiene(&mut cmd);
    let start = Instant::now();
    match tokio::time::timeout(timeout, cmd.output()).await {
        Ok(out) => out.with_context(|| format!("running {label}")),
        Err(_elapsed) => Err(timeout_error(label, start, timeout).into()),
    }
}

/// Spawn `cmd` and stream its stdout line-by-line into `on_line` until the
/// callback returns [`ControlFlow::Break`] (truncation), stdout reaches EOF,
/// or the wall-clock `timeout` expires.
///
/// Stderr is drained concurrently into a capped buffer (so a child blocked
/// writing stderr can never deadlock the stdout reader). The child is always
/// killed and reaped before returning — no zombie outlives the call. On
/// timeout a [`SubprocessTimeout`] is returned; otherwise the caller gets a
/// [`StreamOutcome`] and applies its own exit-status policy via
/// [`StreamOutcome::check_status`].
pub async fn run_streaming<F>(
    mut cmd: Command,
    label: &str,
    timeout: Duration,
    mut on_line: F,
) -> anyhow::Result<StreamOutcome>
where
    F: FnMut(&str) -> ControlFlow<()>,
{
    apply_stdio_hygiene(&mut cmd);
    let start = Instant::now();
    let mut child = cmd.spawn().with_context(|| format!("spawning {label}"))?;

    let stderr_handle = drain_stderr_capped(child.stderr.take().expect("stderr is piped"));
    let stdout = child.stdout.take().expect("stdout is piped");
    let mut reader = tokio::io::BufReader::new(stdout).lines();

    let mut truncated = false;
    let stream_loop = async {
        while let Some(line) = reader.next_line().await? {
            if let ControlFlow::Break(()) = on_line(&line) {
                truncated = true;
                break;
            }
        }
        Ok::<(), std::io::Error>(())
    };

    let timed_out = match tokio::time::timeout(timeout, stream_loop).await {
        Ok(Ok(())) => false,
        Ok(Err(e)) => {
            // Reap before surfacing the read error so no child is leaked.
            let _ = child.kill().await;
            return Err(anyhow::Error::new(e).context(format!("reading {label} stdout")));
        }
        Err(_elapsed) => true,
    };

    // Kill the child — a no-op if it already exited (EOF path) — and always
    // reap it so no zombie outlives the call. `tokio::process::Child::kill`
    // both signals and waits.
    let _ = child.kill().await;
    let status = child
        .wait()
        .await
        .with_context(|| format!("waiting for {label}"))?;

    if timed_out {
        // Don't wait for stderr EOF here: a grandchild (e.g. a build script
        // spawned by cargo) can inherit the pipe's write end and hold it open
        // long after the direct child is dead. The timeout error doesn't need
        // a stderr snippet.
        stderr_handle.abort();
        return Err(timeout_error(label, start, timeout).into());
    }

    // Collect stderr with a short grace period. After the child is reaped the
    // pipe normally hits EOF immediately, but a lingering grandchild could
    // hold the write end open — a diagnostics snippet is not worth blocking
    // the tool on it.
    let mut stderr_handle = stderr_handle;
    let stderr = match tokio::time::timeout(STDERR_GRACE, &mut stderr_handle).await {
        Ok(joined) => joined.unwrap_or_default(),
        Err(_elapsed) => {
            stderr_handle.abort();
            Vec::new()
        }
    };

    Ok(StreamOutcome {
        status,
        stderr,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeout_display_carries_structured_fields() {
        let e = SubprocessTimeout {
            label: "rg".into(),
            elapsed_ms: 1234,
            limit_ms: 1000,
        };
        let msg = e.to_string();
        assert!(msg.contains("elapsed_ms=1234"), "got: {msg}");
        assert!(msg.contains("limit_ms=1000"), "got: {msg}");
        assert!(msg.contains("killed"), "got: {msg}");
    }

    #[cfg(unix)]
    mod unix {
        use super::super::*;
        use std::os::unix::process::ExitStatusExt as _;

        fn outcome(raw: i32, truncated: bool) -> StreamOutcome {
            StreamOutcome {
                status: ExitStatus::from_raw(raw),
                stderr: b"boom".to_vec(),
                truncated,
            }
        }

        #[test]
        fn check_status_accepts_success_and_benign_code() {
            assert!(outcome(0, false).check_status("rg", 1).is_ok());
            // Wait-status encoding: exit code N is N << 8.
            assert!(outcome(1 << 8, false).check_status("rg", 1).is_ok());
        }

        #[test]
        fn check_status_accepts_kill_only_when_truncated() {
            // Raw 9 = terminated by SIGKILL, code() == None.
            assert!(outcome(9, true).check_status("rg", 1).is_ok());
            let err = outcome(9, false).check_status("rg", 1).unwrap_err();
            assert!(err.to_string().contains("rg failed"), "got: {err}");
        }

        #[test]
        fn check_status_reports_stderr_snippet_on_real_failure() {
            let err = outcome(2 << 8, false).check_status("rg", 1).unwrap_err();
            let msg = err.to_string();
            assert!(msg.contains("rg failed"), "got: {msg}");
            assert!(msg.contains("boom"), "got: {msg}");
        }

        #[tokio::test]
        async fn run_collect_captures_output_within_limit() {
            let mut cmd = Command::new("/bin/sh");
            cmd.arg("-c").arg("echo out; echo err >&2");
            let out = run_collect(cmd, "sh", Duration::from_secs(10))
                .await
                .expect("collect");
            assert!(out.status.success());
            assert_eq!(String::from_utf8_lossy(&out.stdout), "out\n");
            assert_eq!(String::from_utf8_lossy(&out.stderr), "err\n");
        }

        #[tokio::test]
        async fn run_collect_times_out_and_returns_structured_error() {
            let mut cmd = Command::new("/bin/sh");
            cmd.arg("-c").arg("sleep 30");
            let start = Instant::now();
            let err = run_collect(cmd, "sh sleep", Duration::from_millis(200))
                .await
                .expect_err("must time out");
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "timeout did not fire promptly"
            );
            let t = err
                .downcast_ref::<SubprocessTimeout>()
                .expect("structured SubprocessTimeout");
            assert_eq!(t.limit_ms, 200);
            assert!(t.elapsed_ms >= 200, "elapsed_ms={}", t.elapsed_ms);
        }

        #[tokio::test]
        async fn run_streaming_times_out_kills_and_returns_structured_error() {
            // Child prints one line then hangs: proves timeout fires mid-stream.
            let mut cmd = Command::new("/bin/sh");
            cmd.arg("-c").arg("echo first; sleep 30");
            let start = Instant::now();
            let mut seen = Vec::new();
            let err = run_streaming(cmd, "sh hang", Duration::from_millis(200), |line| {
                seen.push(line.to_string());
                ControlFlow::Continue(())
            })
            .await
            .expect_err("must time out");
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "timeout did not fire promptly (child not killed?)"
            );
            assert_eq!(seen, vec!["first".to_string()]);
            let t = err
                .downcast_ref::<SubprocessTimeout>()
                .expect("structured SubprocessTimeout");
            assert_eq!(t.limit_ms, 200);
        }

        #[tokio::test]
        async fn run_streaming_truncates_on_break_and_kills_child() {
            // Endless output; callback breaks after 3 lines. The child must be
            // killed (test completes fast) and truncated must be reported.
            let mut cmd = Command::new("/bin/sh");
            cmd.arg("-c").arg("while :; do echo line; done");
            let mut n = 0usize;
            let outcome = run_streaming(cmd, "sh yes", Duration::from_secs(10), |_| {
                n += 1;
                if n >= 3 {
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            })
            .await
            .expect("stream");
            assert!(outcome.truncated);
            assert!(outcome.check_status("sh yes", 1).is_ok());
        }

        #[tokio::test]
        async fn run_streaming_eof_returns_real_exit_status() {
            let mut cmd = Command::new("/bin/sh");
            cmd.arg("-c").arg("echo a; echo b; exit 0");
            let mut lines = Vec::new();
            let outcome = run_streaming(cmd, "sh ok", Duration::from_secs(10), |l| {
                lines.push(l.to_string());
                ControlFlow::Continue(())
            })
            .await
            .expect("stream");
            assert_eq!(lines, vec!["a".to_string(), "b".to_string()]);
            assert!(!outcome.truncated);
            assert!(outcome.status.success());
        }
    }
}
