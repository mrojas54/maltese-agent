use serde_json::Value;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const PROMPT: &str = "Read note.txt. Where and when is the meeting?";

// Build this function live. HTTP and offline replay are wired up below.
fn run(_prompt: &str, _ask: impl FnMut(&Value) -> Result<Value>) -> Result<String> {
    // 1. Describe read_file and start the conversation.
    // 2. Ask the model; return its final text or handle its tool requests.
    // 3. Read the requested file and build a functionResponse.
    // 4. Add the model turn and tool result to history, then ask again.
    Err("agent loop is not implemented yet; follow LIVE-CODING.md".into())
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
