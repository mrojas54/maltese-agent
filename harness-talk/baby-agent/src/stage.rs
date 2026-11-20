use serde_json::{json, Value};
use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = std::env::var("ANTHROPIC_API_KEY")?;
    let prompt = std::env::args().nth(1).unwrap_or("list files".into());

    let mut msgs = vec![json!({"role": "user", "content": prompt})];
    let tools = json!([{
        "name": "sh",
        "description": "Run a shell command",
        "input_schema": {
            "type": "object",
            "properties": {"cmd": {"type": "string"}},
            "required": ["cmd"]
        }
    }]);

    let client = reqwest::blocking::Client::new();
    let max_turns = 5;
    for turn in 1..=max_turns {
        let res: Value = client
            .post("https://api.anthropic.com/v1/messages")
            .timeout(std::time::Duration::from_secs(30))
            .header("x-api-key", &key)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": std::env::var("ANTHROPIC_MODEL").unwrap_or("claude-sonnet-5-5".into()),
                "max_tokens": 512,
                "tools": tools,
                "messages": msgs,
                "tool_choice": {"type": "auto", "disable_parallel_tool_use": true}
            }))
            .send()?
            .error_for_status()?
            .json()?;

        let stop = res["stop_reason"].as_str().ok_or("missing stop_reason")?;
        if !matches!(stop, "end_turn" | "tool_use") {
            return Err(format!("model stopped: {stop}").into());
        }
        let blocks = res["content"]
            .as_array()
            .ok_or("model returned no content")?;
        msgs.push(json!({"role": "assistant", "content": blocks}));

        let Some(b) = blocks.iter().find(|b| b["type"] == "tool_use") else {
            let text = blocks
                .iter()
                .filter_map(|block| block["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if stop != "end_turn" || text.trim().is_empty() {
                return Err("model returned no final answer".into());
            }
            println!("{text}");
            return Ok(());
        };
        if turn == max_turns {
            break;
        }

        let cmd = b["input"]["cmd"].as_str().ok_or("cmd must be a string")?;
        eprintln!("\x1b[33m$ {cmd}\x1b[0m");
        let out = Command::new("sh")
            .env_remove("ANTHROPIC_API_KEY")
            .args(["-c", cmd])
            .output()?;

        msgs.push(json!({"role": "user", "content": [{
            "type": "tool_result",
            "tool_use_id": b["id"],
            "content": String::from_utf8_lossy(&out.stdout)
        }]}));
    }
    Err(format!("stopped after {max_turns} model turns without a final answer").into())
}
