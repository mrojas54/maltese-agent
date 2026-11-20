# Type the shell agent in four chunks

Think of the buffer as four muscle-memory blocks: **State and Tool**,
**POST**, **Find `tool_use`**, and **`sh -c` and Push**. Each block ends with
a natural breath where you can look up from the keyboard. The timestamps
below are rehearsal targets. Time the whole sequence aloud.

## Start with a blank main

```bash
git clone --branch codex/baby-agent-live-starter https://github.com/mrojas54/maltese-agent.git maltese-agent-live
cd maltese-agent-live/harness-talk/baby-agent
cargo build --locked --bins
cargo run --locked
```

That first run exits with `agent loop is not implemented yet`. No key is
needed yet. `src/main.rs` contains only the placeholder and four comments.
Delete that placeholder and type the four chunks below as the whole file,
starting with the imports. All the HTTP code is part of what you type.

The completed live program needs `ANTHROPIC_API_KEY` in its environment;
it does not load `.env` files. Have the key set before going on stage.

## Chunk 1 State and tool

**Target:** 0:00 to 0:45.

**Mental anchor:** Two imports, `main` returns a `Result`, then the API key,
the prompt, `msgs`, and `tools`.

**Say while typing:** "An agent is a message array and a tool definition.
We don't need a framework or custom structs: just JSON, our user prompt,
and one tool called sh that takes a cmd string."

```rust
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
```

Keep `"required": ["cmd"]`: the execution chunk assumes that argument is
present. The schema should describe that contract.

## Chunk 2 The loop and raw HTTP POST

**Target:** 0:45 to 1:30.

**Mental anchor:** Client, five turns, two headers, and five request fields
(model, max_tokens, tools, messages, tool_choice). Check `stop_reason`
before reading the response blocks.

**Say while typing:** "On each turn, we POST the conversation and tool schema.
A truncated reply ends the run with an error. A complete reply or a tool
request goes into msgs."

```rust
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
```

`ANTHROPIC_MODEL` overrides the default `claude-sonnet-5-5`. The single-call
setting lets this loop handle one tool request per response; see the
[API setting](https://platform.claude.com/docs/en/agents-and-tools/tool-use/parallel-tool-use#at-most-one-tool-call).
The HTTP request has a 30-second timeout, and a non-success status stops the
program before it tries to read response content.

## Chunk 3 The let else exit check

**Target:** 1:30 to 2:00.

**Mental anchor:** `blocks.iter().find()` with `let Some(b) = ... else`.

**Say while typing:** "This let-else looks for a tool call. If the model is
done, we print the answer and return. If it needs a tool, we check that
there's another turn left to return the result."

```rust
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
```

The final branch prints the text blocks in order. A tool call on the
fifth turn ends the run with an error before that command runs.

## Chunk 4 Run sh and push the observation

**Target:** 2:00 to 2:45.

**Mental anchor:** Extract cmd → launch `sh -c` without the API key →
push `tool_result` with `b["id"]`.

**Say while typing:** "Print the command so we can see it. Remove the API
key from the shell's environment and run sh -c. Put stdout in a tool result
with the call ID so the model can match it to the request."

```rust
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
```

Keep the direct `String::from_utf8_lossy(&out.stdout)` inside `json!`.
This compact version returns stdout; the separate offline backup also
exercises command errors and malformed model replies.

## The three second memory map

- **State** key, prompt, msgs, tools.
- **Turn** POST → check stop reason → add assistant blocks.
- **Branch** find tool call → print the answer or check the turn limit.
- **Act** sh without the API key → add stdout as the tool result.

## Run the file you typed

Save, then run from this directory with `ANTHROPIC_API_KEY` set:

```bash
cargo run --locked
```

The default prompt is `list files`. For a specific task:

```bash
cargo run --locked -- "Read note.txt. Where and when is the meeting?"
```

The live model chooses its command and answer. Point to the yellow command
line, then to `Command::new("sh")` and the result being added to `msgs`.

Then say: "We've got an agent. Now let's look at what its tools should be
allowed to do." Switch to `../baby-harness/`, open `src/harness.rs`, and
follow its four edits. Its fixed requests include commands and file reads;
it starts at zero of four attacks stopped.

## Stage checkpoint and offline fallback

For a typing mistake, replace your practice edits with the finished
four-chunk file:

```bash
cp src/stage.rs src/main.rs
cargo run --locked
```

That checkpoint needs the live API too. For an offline fallback, select the
separate replay implementation explicitly:

```bash
cargo run --locked --bin baby-agent-complete -- --replay
```

Say: "These model replies are scripted. The shell command is real."
The fallback runs `cat note.txt` and checks its actual result before showing
the scripted final answer. It is separate from the whole-main file you type.

To discard practice edits and return to the blank starting file:

```bash
git restore -- src/main.rs
```

The full test suite expects the default file to remain the blank starting
point. Its starter check is expected to fail after you type the live agent;
reset that file before running the full suite again.
