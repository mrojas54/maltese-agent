use serde_json::{json, Value};
use std::process::Command;

fn main() {
    let key = std::env::var("ANTHROPIC_API_KEY").unwrap();
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
    for _ in 0..5 {
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
            .send()
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .unwrap();

        let blocks = res["content"].as_array().unwrap();
        msgs.push(json!({"role": "assistant", "content": blocks}));

        let Some(b) = blocks.iter().find(|b| b["type"] == "tool_use") else {
            for block in blocks {
                if let Some(text) = block["text"].as_str() {
                    println!("{text}");
                }
            }
            break;
        };

        let cmd = b["input"]["cmd"].as_str().unwrap();
        eprintln!("\x1b[33m$ {cmd}\x1b[0m");
        let out = Command::new("sh").arg("-c").arg(cmd).output().unwrap();

        msgs.push(json!({"role": "user", "content": [{
            "type": "tool_result",
            "tool_use_id": b["id"],
            "content": String::from_utf8_lossy(&out.stdout)
        }]}));
    }
}
