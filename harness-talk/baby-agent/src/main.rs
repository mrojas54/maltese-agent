use serde_json::Value;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const REPLAY_PROMPT: &str = "Read note.txt. Where and when is the meeting?";

// Build this function live. HTTP and offline replay are wired up below.
fn run(_prompt: &str, _ask: impl FnMut(&Value) -> Result<Value>) -> Result<String> {
    // 1. Describe sh and start the Anthropic conversation.
    // 2. Ask the model; keep its content blocks and check for a tool_use.
    // 3. Run the requested command through sh -c.
    // 4. Add a tool_result with the matching ID, then ask again.
    Err("agent loop is not implemented yet; follow LIVE-CODING.md".into())
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
