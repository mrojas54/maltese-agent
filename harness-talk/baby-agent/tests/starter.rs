use std::process::Command;

#[test]
fn default_entry_point_is_unfinished_before_any_tool_call() {
    let output = Command::new(env!("CARGO_BIN_EXE_baby-agent"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_remove("ANTHROPIC_API_KEY")
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "the starter already answers: {output:?}"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("agent loop is not implemented yet"));
    let printed = String::from_utf8_lossy(&output.stdout);
    assert!(!printed.contains("model -> tool:"), "{printed}");
    assert!(!printed.contains("tool -> model:"), "{printed}");
    assert!(!printed.contains("agent:"), "{printed}");
}

#[test]
fn live_coding_chunks_match_the_stage_checkpoint() {
    let chunks: Vec<_> = include_str!("../LIVE-CODING.md")
        .split("```rust\n")
        .skip(1)
        .map(|chunk| chunk.split_once("\n```").expect("close each Rust block").0)
        .collect();
    assert_eq!(chunks.len(), 4);
    assert_eq!(chunks.join("\n\n") + "\n", include_str!("../src/stage.rs"));
}
