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
}

impl Drop for Note {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn reply(parts: Value) -> Value {
    json!({"candidates": [{
        "finishReason": "STOP",
        "content": {"role": "model", "parts": parts}
    }]})
}

fn call(name: &str, args: Value) -> Value {
    json!({"functionCall": {"name": name, "args": args}})
}

#[test]
fn reads_real_file_returns_result_to_model_and_preserves_model_turn() {
    let note = Note::new("Meet at 19:00.\nBring a trenchcoat.\n");
    let first = reply(json!([{
        "functionCall": {"id": "read-1", "name": "read_file", "args": {"path": note.0}},
        "thoughtSignature": "opaque-provider-signature"
    }]));
    let mut turns = 0;
    let answer = run("Read the note.", |request| {
        turns += 1;
        match turns {
            1 => {
                assert_eq!(
                    request["contents"],
                    json!([
                        {"role": "user", "parts": [{"text": "Read the note."}]}
                    ])
                );
                let tool = &request["tools"][0]["functionDeclarations"][0];
                assert_eq!(tool["name"], "read_file");
                assert_eq!(tool["parameters"]["required"], json!(["path"]));
                Ok(first.clone())
            }
            2 => {
                assert_eq!(request["contents"][1], first["candidates"][0]["content"]);
                let result = &request["contents"][2]["parts"][0]["functionResponse"];
                assert_eq!(result["name"], "read_file");
                assert_eq!(result["id"], "read-1");
                assert_eq!(
                    result["response"]["result"],
                    "Meet at 19:00.\nBring a trenchcoat.\n"
                );
                // The final reply depends on what actually came back from disk.
                Ok(reply(json!([{"text": result["response"]["result"]}])))
            }
            _ => panic!("called the model after its final answer"),
        }
    })
    .unwrap();
    assert_eq!(turns, 2);
    assert_eq!(answer, "Meet at 19:00.\nBring a trenchcoat.\n");
}

#[test]
fn answers_without_a_tool_and_excludes_thought_text() {
    let answer = run("Hello", |_| {
        Ok(reply(json!([
            {"text": "private thought", "thought": true},
            {"text": "Hello!"}, {"text": "Welcome."}
        ])))
    })
    .unwrap();
    assert_eq!(answer, "Hello!\nWelcome.");
}

#[test]
fn tool_errors_are_returned_to_the_model() {
    let absent = Note::new("temporary");
    std::fs::remove_file(&absent.0).unwrap();
    for requested in [
        call("run_shell", json!({"command": "anything"})),
        call("read_file", json!({})),
        call("read_file", json!({"path": 123})),
        call("read_file", json!({"path": absent.0})),
    ] {
        let mut turns = 0;
        let answer = run("Read it", |request| {
            turns += 1;
            if turns == 1 {
                Ok(reply(json!([requested])))
            } else {
                assert!(
                    request["contents"][2]["parts"][0]["functionResponse"]["response"]["error"]
                        .as_str()
                        .is_some_and(|message| !message.is_empty())
                );
                Ok(reply(json!([{"text": "The tool failed."}])))
            }
        })
        .unwrap();
        assert_eq!(answer, "The tool failed.");
        assert_eq!(turns, 2);
    }
}

#[test]
fn handles_all_tool_calls_before_asking_again() {
    let a = Note::new("first note");
    let b = Note::new("second note");
    let mut turns = 0;
    run("Read both", |request| {
        turns += 1;
        if turns == 1 {
            Ok(reply(json!([
                {"text": "I will read both notes."},
                call("read_file", json!({"path": a.0})),
                call("read_file", json!({"path": b.0}))
            ])))
        } else {
            let results = request["contents"][2]["parts"].as_array().unwrap();
            assert_eq!(results.len(), 2);
            assert_eq!(
                results[0]["functionResponse"]["response"]["result"],
                "first note"
            );
            assert_eq!(
                results[1]["functionResponse"]["response"]["result"],
                "second note"
            );
            Ok(reply(json!([{"text": "Read both."}])))
        }
    })
    .unwrap();
    assert_eq!(turns, 2);
}

#[test]
fn stops_after_the_turn_budget_without_retrying_provider_failures() {
    let mut turns = 0;
    let result = run("Keep going", |_| {
        turns += 1;
        Ok(reply(json!([call("unknown", json!({}))])))
    });
    assert!(result.is_err());
    assert_eq!(turns, MAX_TURNS);

    let mut attempts = 0;
    let result = run("Hello", |_| {
        attempts += 1;
        Err("HTTP 429: quota exhausted".into())
    });
    assert!(result.unwrap_err().to_string().contains("429"));
    assert_eq!(attempts, 1);
}

#[test]
fn empty_blocked_and_truncated_replies_are_errors() {
    for response in [
        json!({"promptFeedback": {"blockReason": "SAFETY"}}),
        reply(json!([])),
        reply(json!([{"thought": true, "text": "Only a thought"}])),
        json!({"candidates": [{
            "finishReason": "MAX_TOKENS",
            "content": {"role": "model", "parts": [{"text": "cut off"}]}
        }]}),
    ] {
        let result = run("Hello", |_| Ok(response.clone()));
        assert!(result.is_err(), "accepted invalid reply: {response}");
    }
}
