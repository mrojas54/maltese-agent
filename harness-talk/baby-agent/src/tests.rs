use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_FILE: AtomicUsize = AtomicUsize::new(0);

struct Note(std::path::PathBuf);

impl Note {
    fn new(text: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "baby-agent-{}-{}.txt",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, text).unwrap();
        Self(path)
    }

    fn quoted(&self) -> String {
        format!("'{}'", self.0.to_string_lossy().replace('\'', "'\\''"))
    }
}

impl Drop for Note {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn reply(content: Value) -> Value {
    let tool_use = content
        .as_array()
        .unwrap()
        .iter()
        .any(|b| b["type"] == "tool_use");
    json!({"stop_reason": if tool_use { "tool_use" } else { "end_turn" }, "content": content})
}

fn call(id: &str, cmd: &str) -> Value {
    json!({"type": "tool_use", "id": id, "name": "sh", "input": {"cmd": cmd}})
}

#[test]
fn shell_output_and_call_id_reach_the_next_anthropic_request() {
    let note = Note::new("Read through the real shell.\n");
    let cmd = format!("cat {}; printf 'stderr' >&2", note.quoted());
    let first = reply(json!([
        {"type": "text", "text": "I will read the file."},
        call("shell-1", &cmd)
    ]));
    let mut turns = 0;
    let answer = run("Read the note.", |request| {
        turns += 1;
        match turns {
            1 => {
                assert!(request["model"].is_string());
                assert_eq!(request["max_tokens"], 512);
                assert_eq!(
                    request["messages"],
                    json!([{"role": "user", "content": "Read the note."}])
                );
                assert_eq!(request["tools"][0]["name"], "sh");
                assert_eq!(
                    request["tools"][0]["input_schema"]["required"],
                    json!(["cmd"])
                );
                assert_eq!(
                    request["tool_choice"],
                    json!({"type": "auto", "disable_parallel_tool_use": true})
                );
                Ok(first.clone())
            }
            2 => {
                assert_eq!(
                    request["messages"][1],
                    json!({"role": "assistant", "content": first["content"]})
                );
                let result = &request["messages"][2]["content"][0];
                assert_eq!(result["type"], "tool_result");
                assert_eq!(result["tool_use_id"], "shell-1");
                assert_eq!(result["content"], "Read through the real shell.\nstderr");
                assert_eq!(result["is_error"], false);
                Ok(reply(json!([{"type": "text", "text": result["content"]}])))
            }
            _ => panic!("asked again after the final answer"),
        }
    })
    .unwrap();
    assert_eq!(turns, 2);
    assert_eq!(answer, "Read through the real shell.\nstderr");
}

#[test]
fn returns_all_final_text_blocks_without_thinking() {
    let answer = run("Hello", |_| {
        Ok(reply(json!([
            {"type": "thinking", "thinking": "private"},
            {"type": "text", "text": "Hello!"},
            {"type": "text", "text": "Welcome."}
        ])))
    })
    .unwrap();
    assert_eq!(answer, "Hello!\nWelcome.");
}

#[test]
fn failed_shell_command_is_reported_to_the_model_as_an_error() {
    let mut turns = 0;
    let answer = run("Try it", |request| {
        turns += 1;
        if turns == 1 {
            Ok(reply(json!([call(
                "failed-1",
                "printf 'shell failed' >&2; exit 7"
            )])))
        } else {
            let result = &request["messages"][2]["content"][0];
            assert_eq!(result["is_error"], true);
            assert_eq!(result["content"], "shell failed");
            Ok(reply(
                json!([{"type": "text", "text": "The command failed."}]),
            ))
        }
    })
    .unwrap();
    assert_eq!(turns, 2);
    assert_eq!(answer, "The command failed.");
}

#[test]
fn rejects_malformed_unknown_and_multiple_calls_before_execution() {
    let marker = Note::new("temporary");
    std::fs::remove_file(&marker.0).unwrap();
    let cmd = format!("printf ran > {}", marker.quoted());
    for content in [
        json!([{"type": "tool_use", "id": "one", "name": "unknown", "input": {"cmd": cmd}}]),
        json!([{"type": "tool_use", "id": "one", "name": "sh", "input": {}}]),
        json!([{"type": "tool_use", "name": "sh", "input": {"cmd": cmd}}]),
        json!([call("one", &cmd), call("two", &cmd)]),
    ] {
        let mut turns = 0;
        let result = run("Try it", |_| {
            turns += 1;
            Ok(reply(content.clone()))
        });
        assert!(result.is_err(), "accepted malformed call: {content}");
        assert_eq!(turns, 1);
        assert!(!marker.0.exists(), "executed an invalid request: {content}");
    }
}

#[test]
fn five_turn_limit_and_no_retry_after_provider_failure() {
    let marker = Note::new("temporary");
    std::fs::remove_file(&marker.0).unwrap();
    let last_command = format!("printf ran > {}", marker.quoted());
    let mut turns = 0;
    let result = run("Keep going", |_| {
        turns += 1;
        let cmd = if turns == MAX_TURNS {
            last_command.as_str()
        } else {
            "printf ok"
        };
        Ok(reply(json!([call("again", cmd)])))
    });
    assert!(result.is_err());
    assert_eq!(turns, MAX_TURNS);
    assert!(
        !marker.0.exists(),
        "executed a tool with no model turn left"
    );

    let mut attempts = 0;
    let result = run("Hello", |_| {
        attempts += 1;
        Err("HTTP 429: quota exhausted".into())
    });
    assert!(result.unwrap_err().to_string().contains("429"));
    assert_eq!(attempts, 1);
}

#[test]
fn rejects_missing_empty_and_truncated_replies() {
    for response in [
        json!({"type": "error", "error": {"message": "bad request"}}),
        reply(json!([])),
        reply(json!([{"type": "thinking", "thinking": "Only a thought"}])),
        json!({"stop_reason": "max_tokens", "content": [{"type": "text", "text": "cut off"}]}),
        json!({"stop_reason": "refusal", "content": [{"type": "text", "text": "refused"}]}),
    ] {
        assert!(
            run("Hello", |_| Ok(response.clone())).is_err(),
            "accepted invalid reply: {response}"
        );
    }
}
