use std::process::Command;

fn demo() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_baby-agent-complete"));
    command
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_remove("GEMINI_API_KEY");
    command
}

#[test]
fn rehearsal_shows_the_complete_exchange_without_credentials() {
    let output = demo().arg("--replay").output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        concat!(
            "Offline rehearsal: scripted model replies; the file read is real.\n",
            "model -> tool: read_file {\"path\":\"note.txt\"}\n",
            "tool -> model: {\"result\":\"The meeting is at 19:00 in the library.\\n\"}\n",
            "agent: The meeting is at 19:00 in the library.\n"
        )
    );
}

#[test]
fn rehearsal_cannot_answer_from_a_missing_or_changed_note() {
    struct Fixture(std::path::PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let dir = Fixture(std::env::temp_dir().join(format!("baby-agent-cli-{}", std::process::id())));
    std::fs::create_dir(&dir.0).unwrap();
    for note in [None, Some("The meeting moved to the park at noon.\n")] {
        if let Some(text) = note {
            std::fs::write(dir.0.join("note.txt"), text).unwrap();
        }
        let output = demo().current_dir(&dir.0).arg("--replay").output().unwrap();
        assert!(
            !output.status.success(),
            "accepted stale replay: {output:?}"
        );
        assert!(!String::from_utf8_lossy(&output.stdout).contains("agent:"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("note.txt"));
    }
}

#[test]
fn live_run_without_credentials_explains_the_next_step() {
    let output = demo().output().unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("GEMINI_API_KEY"));
    assert!(error.contains("--replay"));
}

#[test]
fn invalid_arguments_fail_before_a_model_request() {
    let output = demo().args(["--replay", "extra"]).output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("usage:"));
    assert!(output.stdout.is_empty());
}
