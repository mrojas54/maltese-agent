use std::process::Command;

struct Fixture(std::path::PathBuf);

impl Fixture {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("baby-agent-cli-{}-{name}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn demo() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_baby-agent-complete"));
    command
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_remove("ANTHROPIC_API_KEY");
    command
}

#[test]
fn rehearsal_shows_the_complete_exchange_without_credentials() {
    let output = demo().arg("--replay").output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Offline rehearsal: scripted model replies; the shell command is real.\nThe meeting is at 19:00 in the library.\n"
    );
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "\x1b[33m$ cat note.txt\x1b[0m\n"
    );
}

#[test]
fn rehearsal_cannot_answer_from_a_missing_or_changed_note() {
    let dir = Fixture::new("note");
    for note in [None, Some("The meeting moved to the park at noon.\n")] {
        if let Some(text) = note {
            std::fs::write(dir.0.join("note.txt"), text).unwrap();
        }
        let output = demo().current_dir(&dir.0).arg("--replay").output().unwrap();
        assert!(
            !output.status.success(),
            "accepted stale replay: {output:?}"
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "Offline rehearsal: scripted model replies; the shell command is real.\n"
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("note.txt"));
    }
}

#[cfg(unix)]
#[test]
fn rehearsal_shell_does_not_inherit_the_api_key() {
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    let dir = Fixture::new("shell");
    let shell = dir.0.join("sh");
    std::fs::write(
        &shell,
        "#!/bin/sh\n[ -z \"${ANTHROPIC_API_KEY+x}\" ] || exit 47\nexec /bin/sh \"$@\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o700)).unwrap();

    let path =
        std::env::join_paths([dir.0.as_path(), Path::new("/bin"), Path::new("/usr/bin")]).unwrap();
    let output = demo()
        .env("PATH", path)
        .env("ANTHROPIC_API_KEY", "test-key")
        .arg("--replay")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

#[test]
fn live_run_without_credentials_explains_the_next_step() {
    let output = demo().output().unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("ANTHROPIC_API_KEY"));
    assert!(error.contains("--replay"));
}

#[test]
fn invalid_arguments_fail_before_a_model_request() {
    let output = demo().args(["--replay", "extra"]).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("usage: cargo run --bin baby-agent-complete -- [--replay |"));
    assert!(output.stdout.is_empty());
}
