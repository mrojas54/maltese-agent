# Build the opening agent live

Start with `src/main.rs`. Its `run` function has four comments and an error.
The code below it already connects either Gemini or the offline replies to
that function. You write the tool description, the file read, and the loop.

## Fresh checkout and warmup

```bash
git clone --branch codex/baby-agent-live-starter https://github.com/mrojas54/maltese-agent.git maltese-agent-live
cd maltese-agent-live/harness-talk/baby-agent
cargo build --locked --bins
cargo run --locked -- --replay
```

The last command must stop with `agent loop is not implemented yet`. It
exits with status 1. There is no tool call or meeting answer yet. Building
first downloads the dependencies without filling in the starting file.

Have this script on your phone or another screen. Open `src/main.rs` on the
projector at `run`; keep `main` folded below it. Use [INTRO.md](INTRO.md)
for the spoken introduction. Time the whole build aloud in rehearsal.

## Prepare the imports

Change the first line to:

```rust
use serde_json::{json, Value};
```

Add this constant after the `Result` type:

```rust
const MAX_TURNS: usize = 4;
```

Replace the placeholder `run` with the following four fragments, in order.
Together they form one function. Run it after the fourth fragment.

## 1 Describe the tool and ask the model

Say: "This describes what the model can ask for. We start with the user's
request, send the conversation, and look at the model's reply."

```rust
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
```

## 2 Carry out the tool request

Say: "The model supplies a tool name and arguments. Here is where text
becomes an action: Rust opens the file. There is no path check yet."

```rust
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
```

## 3 Return a final answer

Say: "If the model did not request a tool, its text is the answer."

```rust
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
```

## 4 Close the loop

Say: "Keep the model's request, add our tool result, and ask again."

```rust
        let history = request["contents"].as_array_mut().unwrap();
        history.push(content.clone()); // Preserve the whole turn, including thought signatures.
        history.push(json!({"role": "user", "parts": results}));
    }
    Err(format!("stopped after {MAX_TURNS} model turns without a final answer").into())
}
```

## Run what you just wrote

Save, then run from this directory:

```bash
cargo run --locked -- --replay
```

Say: "These model replies are scripted for rehearsal. The file read and
the loop are real."

Expected exchange:

```text
Offline rehearsal: scripted model replies; the file read is real.
model -> tool: read_file {"path":"note.txt"}
tool -> model: {"result":"The meeting is at 19:00 in the library.\n"}
agent: The meeting is at 19:00 in the library.
```

For a live model, with `GEMINI_API_KEY` already set in your environment:

```bash
cargo run --locked
```

It uses the same function you just wrote. The model can choose whether to
call the tool, so its live answer can vary. The program does not load `.env`
files. It stops on provider errors and does not retry quota failures.

Then say: "We've got an agent. Now let's look at what its tools should be
allowed to do." Switch to `../baby-harness/`, open `src/harness.rs`, and
follow its four edits. That demo starts at zero of four attacks stopped.

## Recovery and another practice run

The completed backup runs only when you name it:

```bash
cargo run --locked --bin baby-agent-complete -- --replay
```

To replace your practice edits with the completed version:

```bash
cp src/complete.rs src/main.rs
cargo run --locked -- --replay
```

To discard those edits and return to the unfinished starting file:

```bash
git restore -- src/main.rs
```

`cargo test --locked` checks the backup and that the starting file is still
unfinished. Its starter check is expected to fail while you are practicing.
Use the default `--replay` command above to check the agent you just typed.
