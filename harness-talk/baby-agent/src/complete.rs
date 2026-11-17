use serde_json::{json, Value};
use std::process::Command;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const REPLAY_PROMPT: &str = "Read note.txt. Where and when is the meeting?";
const MAX_TURNS: usize = 5;

fn run(prompt: &str, mut ask: impl FnMut(&Value) -> Result<Value>) -> Result<String> {
    let mut request = json!({
        "model": std::env::var("ANTHROPIC_MODEL").unwrap_or("claude-sonnet-5-5".into()),
        "max_tokens": 512,
        "messages": [{"role": "user", "content": prompt}],
        "tools": [{
            "name": "sh", "description": "Run a shell command",
            "input_schema": {
                "type": "object", "properties": {"cmd": {"type": "string"}},
                "required": ["cmd"]
            }
        }],
        "tool_choice": {"type": "auto", "disable_parallel_tool_use": true}
    });
    for _ in 0..MAX_TURNS {
        let res = ask(&request)?;
        if res["stop_reason"] != "end_turn" && res["stop_reason"] != "tool_use" {
            return Err(format!("model stopped: {}", res["stop_reason"]).into());
        }
        let blocks = res["content"]
            .as_array()
            .ok_or("model returned no content")?;
        let msgs = request["messages"].as_array_mut().unwrap();
        msgs.push(json!({"role": "assistant", "content": blocks}));
        if blocks.iter().filter(|b| b["type"] == "tool_use").count() > 1 {
            return Err("expected at most one tool call per turn".into());
        }
        let Some(b) = blocks.iter().find(|b| b["type"] == "tool_use") else {
            let text = blocks
                .iter()
                .filter(|b| b["type"] == "text")
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if text.is_empty() {
                return Err("model returned no answer or tool call".into());
            }
            return Ok(text);
        };
        if b["name"] != "sh" {
            return Err("unknown tool".into());
        }
        let cmd = b["input"]["cmd"].as_str().ok_or("cmd must be a string")?;
        let id = b["id"].as_str().ok_or("tool call has no id")?;
        eprintln!("\x1b[33m$ {cmd}\x1b[0m");
        let out = Command::new("sh").arg("-c").arg(cmd).output()?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        msgs.push(json!({"role": "user", "content": [{
            "type": "tool_result", "tool_use_id": id, "content": text,
            "is_error": !out.status.success()
        }]}));
    }
    Err(format!("stopped after {MAX_TURNS} model turns without a final answer").into())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() > 1 {
        return Err("usage: cargo run -- [--replay | \"your prompt\"]".into());
    }
    let answer = if args.first().is_some_and(|arg| arg == "--replay") {
        println!("Offline rehearsal: scripted model replies; the shell command is real.");
        let replies: Vec<Value> = serde_json::from_str(include_str!("../fixtures/replay.json"))?;
        let mut replies = replies.into_iter();
        run(REPLAY_PROMPT, |request| {
            let reply = replies.next().ok_or("replay exhausted")?;
            if let Some(expected) = reply.get("expectedToolResult") {
                if request["messages"][2]["content"][0] != *expected {
                    return Err(
                        "replay requires the original note.txt; restore it or use live mode".into(),
                    );
                }
            }
            Ok(reply)
        })?
    } else {
        let key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| "set ANTHROPIC_API_KEY for a live run, or use --replay")?;
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        run(
            args.first().map(String::as_str).unwrap_or("list files"),
            |request| {
                Ok(client
                    .post("https://api.anthropic.com/v1/messages")
                    .header("x-api-key", &key)
                    .header("anthropic-version", "2023-06-01")
                    .json(request)
                    .send()?
                    .error_for_status()?
                    .json()?)
            },
        )?
    };
    println!("{answer}");
    Ok(())
}

#[cfg(test)]
mod tests;
