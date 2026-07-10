//! AC-21: PATH manipulation must not substitute a binary for an allowlisted
//! name. `exec_run` executes the absolute path resolved from the allowlist at
//! sandbox construction ("server startup"), never the bare name via the
//! call-time PATH.
//!
//! These tests are in-process (library-level) on purpose: the attack staged
//! here — PATH changing *between* server startup and the exec call — cannot
//! be arranged against a spawned child from the outside, because a child's
//! environment is fixed at spawn. Driving `exec::exec_run` directly lets the
//! test pin resolution at construction time and then swap PATH before the
//! call.
//!
//! All PATH mutation is confined to this integration binary (its own
//! process), and the tests serialize on a mutex so they never race each
//! other. Each test sets PATH fully at its start and restores it at the end.
#![cfg(unix)]

use falcon_mcp::sandbox::Sandbox;
use falcon_mcp::tools::exec::{exec_run, ExecRunArgs};
use std::path::Path;
use std::sync::{Arc, LazyLock};
use tempfile::TempDir;

/// Serializes the tests in this file: they all mutate the process-global
/// PATH. A tokio mutex because the guard is held across `.await` points
/// (clippy::await_holding_lock forbids a std guard there), and it does not
/// poison — each test sets PATH fully at its start regardless.
static PATH_LOCK: LazyLock<tokio::sync::Mutex<()>> = LazyLock::new(|| tokio::sync::Mutex::new(()));

/// Write an executable fake `git` into `dir` that prints `output`.
fn write_fake_git(dir: &Path, output: &str) {
    use std::os::unix::fs::PermissionsExt as _;
    let path = dir.join("git");
    std::fs::write(&path, format!("#!/bin/sh\necho {output}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn exec_args(cmd: &str) -> ExecRunArgs {
    ExecRunArgs {
        cmd: cmd.to_string(),
        args: vec![],
        cwd: None,
        // Explicit: the schemars/serde default only applies to JSON input.
        timeout_ms: 30_000,
    }
}

/// The AC-21 scenario. Startup PATH contains only the "real" git; by call
/// time an impostor git sits *earlier* on PATH. Bare-name resolution at spawn
/// (the pre-fix behavior) would run the impostor; resolving the allowlist to
/// absolute paths at construction must run the real one.
#[tokio::test]
async fn exec_run_does_not_execute_impostor_from_call_time_path() {
    let _guard = PATH_LOCK.lock().await;
    let original_path = std::env::var_os("PATH");

    let root = TempDir::new().unwrap();
    let real_dir = TempDir::new().unwrap();
    let impostor_dir = TempDir::new().unwrap();
    write_fake_git(real_dir.path(), "REAL");
    write_fake_git(impostor_dir.path(), "IMPOSTOR");

    // "Server startup": only the real dir is on PATH when the sandbox (and
    // with it the allowlist) is constructed.
    std::env::set_var("PATH", real_dir.path());
    let sandbox = Arc::new(Sandbox::new(root.path().to_path_buf(), false).unwrap());

    // Construction pinned an absolute path under the startup-PATH dir.
    let resolved = sandbox.resolved_bin("git").unwrap();
    assert!(resolved.is_absolute(), "resolved: {}", resolved.display());
    assert!(
        resolved.starts_with(real_dir.path()),
        "resolved {} should live under {}",
        resolved.display(),
        real_dir.path().display()
    );

    // Attack: by call time, the impostor dir is prepended to PATH.
    std::env::set_var(
        "PATH",
        format!(
            "{}:{}",
            impostor_dir.path().display(),
            real_dir.path().display()
        ),
    );

    // Vacuity guard: prove the PATH manipulation is effective — a bare-name
    // spawn (what exec_run used to do) DOES pick up the impostor now.
    let bare = std::process::Command::new("git").output().unwrap();
    assert_eq!(
        String::from_utf8_lossy(&bare.stdout).trim(),
        "IMPOSTOR",
        "test setup broken: bare-name spawn should resolve to the impostor"
    );

    let out = exec_run(sandbox, exec_args("git")).await.unwrap();
    assert_eq!(out.exit, 0, "fake git should exit 0: {out:?}");
    assert_eq!(
        out.stdout.trim(),
        "REAL",
        "exec_run must execute the startup-resolved binary, not the \
         call-time-PATH impostor"
    );

    match original_path {
        Some(p) => std::env::set_var("PATH", p),
        None => std::env::remove_var("PATH"),
    }
}

/// Second line of defense unchanged: a name that is not on the allowlist is
/// rejected before any resolution or spawn, regardless of PATH contents.
#[tokio::test]
async fn exec_run_still_rejects_non_allowlisted_name() {
    let _guard = PATH_LOCK.lock().await;
    let original_path = std::env::var_os("PATH");

    let root = TempDir::new().unwrap();
    let real_dir = TempDir::new().unwrap();
    write_fake_git(real_dir.path(), "REAL");

    std::env::set_var("PATH", real_dir.path());
    let sandbox = Arc::new(Sandbox::new(root.path().to_path_buf(), false).unwrap());

    let err = exec_run(sandbox, exec_args("curl")).await.unwrap_err();
    assert!(
        err.to_string().contains("not in allowlist"),
        "expected allowlist rejection, got: {err:#}"
    );

    match original_path {
        Some(p) => std::env::set_var("PATH", p),
        None => std::env::remove_var("PATH"),
    }
}

/// An allowlisted name that could not be resolved on the startup PATH must
/// fail with a startup-resolution error at call time — not fall back to
/// bare-name spawning (which would re-open the PATH-substitution hole).
#[tokio::test]
async fn exec_run_errors_for_allowlisted_name_unresolved_at_startup() {
    let _guard = PATH_LOCK.lock().await;
    let original_path = std::env::var_os("PATH");

    let root = TempDir::new().unwrap();
    let real_dir = TempDir::new().unwrap();
    // Only `git` exists on the startup PATH; `cargo` (allowlisted) does not.
    write_fake_git(real_dir.path(), "REAL");

    std::env::set_var("PATH", real_dir.path());
    let sandbox = Arc::new(Sandbox::new(root.path().to_path_buf(), false).unwrap());

    let err = exec_run(sandbox, exec_args("cargo")).await.unwrap_err();
    assert!(
        err.to_string()
            .contains("not found on PATH at server startup"),
        "expected startup-resolution error, got: {err:#}"
    );

    match original_path {
        Some(p) => std::env::set_var("PATH", p),
        None => std::env::remove_var("PATH"),
    }
}
