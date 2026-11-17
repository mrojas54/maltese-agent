use serde_json::{json, Value};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const MAX_TURNS: usize = 4;
const PROMPT: &str = "Read note.txt. Where and when is the meeting?";

fn run(prompt: &str, mut ask: impl FnMut(&Value) -> Result<Value>) -> Result<String> {
    let mut request = json!({
        "contents": [{"role": "user", "parts": [{"text": prompt}]}],
        "tools": [{"functionDeclarations": [{
            "name": "read_file", "description": "Read a local UTF-8 text file.",
            "parameters": {"type": "OBJECT", "properties": {
                "path": {"type": "STRING", "description": "Path to the file."}
            }, "required": ["path"]}
        }]}]
    });
    for _ in 0..MAX_TURNS {
        let reply = ask(&request)?;
        let candidate = &reply["candidates"][0];
        if candidate["finishReason"] != "STOP" {
            return Err(format!("model did not finish: {}", candidate["finishReason"]).into());
        }
        let content = &candidate["content"];
        let parts = content["parts"]
            .as_array()
            .ok_or("model returned no parts")?;
        let mut results = Vec::new();
        for call in parts.iter().filter_map(|part| part.get("functionCall")) {
            let name = call["name"].as_str().ok_or("tool call has no name")?;
            println!("model -> tool: {name} {}", call["args"]);
            let output = match (name, call["args"]["path"].as_str()) {
                ("read_file", Some(path)) => match std::fs::read_to_string(path) {
                    Ok(text) => json!({"result": text}),
                    Err(error) => json!({"error": error.to_string()}),
                },
                ("read_file", None) => json!({"error": "path must be a string"}),
                _ => json!({"error": format!("unknown tool: {name}")}),
            };
            println!("tool -> model: {output}");
            let mut result = json!({"name": name, "response": output});
            if let Some(id) = call.get("id") {
                result["id"] = id.clone();
            }
            results.push(json!({"functionResponse": result}));
        }
        if results.is_empty() {
            let text = parts
                .iter()
                .filter(|part| part["thought"] != true)
                .filter_map(|part| part["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n");
            if text.is_empty() {
                return Err("model returned no answer or tool call".into());
            }
            return Ok(text);
        }
        let history = request["contents"].as_array_mut().unwrap();
        history.push(content.clone()); // Preserve the whole turn, including thought signatures.
        history.push(json!({"role": "user", "parts": results}));
    }
    Err(format!("stopped after {MAX_TURNS} model turns without a final answer").into())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() > 1 {
        return Err("usage: cargo run -- [--replay | \"your prompt\"]".into());
    }
    let answer = if args.first().is_some_and(|arg| arg == "--replay") {
        println!("Offline rehearsal: scripted model replies; the file read is real.");
        let replies: Vec<Value> = serde_json::from_str(include_str!("../fixtures/replay.json"))?;
        let mut replies = replies.into_iter();
        run(PROMPT, |request| {
            let reply = replies.next().ok_or("replay exhausted")?;
            if let Some(expected) = reply.get("expectedToolResult") {
                if request["contents"][2]["parts"][0]["functionResponse"]["response"] != *expected {
                    return Err(
                        "replay requires the original note.txt; restore it or use live mode".into(),
                    );
                }
            }
            Ok(reply)
        })?
    } else {
        let key = std::env::var("GEMINI_API_KEY")
            .map_err(|_| "set GEMINI_API_KEY for a live run, or use --replay")?;
        let model = std::env::var("GEMINI_MODEL").unwrap_or("gemini-3.1-flash-lite".into());
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
        );
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        println!("Live model: {model}");
        run(
            args.first().map(String::as_str).unwrap_or(PROMPT),
            |request| {
                Ok(client
                    .post(&url)
                    .header("x-goog-api-key", &key)
                    .json(request)
                    .send()?
                    .error_for_status()?
                    .json()?)
            },
        )?
    };
    println!("agent: {answer}");
    Ok(())
}

#[cfg(test)]
mod tests;
