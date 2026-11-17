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

**Mental anchor:** Two imports, one environment variable, one CLI prompt,
and two JSON arrays: `msgs` and `tools`.

**Say while typing:** "An agent is a message array and a tool definition.
We don't need a framework or custom structs: just JSON, our user prompt,
and one tool called sh that takes a cmd string."

```rust
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
```

Keep `"required": ["cmd"]`: the execution chunk assumes that argument is
present. The schema should describe that contract.

## Chunk 2 The loop and raw HTTP POST

**Target:** 0:45 to 1:30.

**Mental anchor:** Client outside, `for _ in 0..5` inside, two headers,
and five request fields: model, max_tokens, tools, messages, tool_choice.

**Say while typing:** "Each turn, we POST the conversation and tool schema
to the Messages API, parse its response as a JSON Value, and put the
assistant's reply back into msgs. The loop is bounded at five turns."

```rust
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
```

`ANTHROPIC_MODEL` overrides the default `claude-sonnet-5-5`. The single-call
setting lets this loop handle one tool request per response; see the
[API setting](https://platform.claude.com/docs/en/agents-and-tools/tool-use/parallel-tool-use#at-most-one-tool-call).
The HTTP request has a 30-second timeout, and a non-success status stops the
program before it tries to read response content.

## Chunk 3 The let else exit check

**Target:** 1:30 to 2:00.

**Mental anchor:** `blocks.iter().find()` with `let Some(b) = ... else`.

**Say while typing:** "Did the model ask for a tool, or is it done? This
let-else finds a tool_use block. If there isn't one, print the text and
break out of the loop."

```rust
        let Some(b) = blocks.iter().find(|b| b["type"] == "tool_use") else {
            for block in blocks {
                if let Some(text) = block["text"].as_str() {
                    println!("{text}");
                }
            }
            break;
        };
```

The small inner loop prints text blocks even when another kind of block
comes first in the response.

## Chunk 4 Run sh and push the observation

**Target:** 2:00 to 2:45.

**Mental anchor:** Extract cmd → `Command::new("sh")` → push `tool_result`
with `b["id"]`.

**Say while typing:** "If it called sh, grab cmd, print it to stderr so we
can see the command, and pass it straight to sh -c. Then send stdout back
as a tool_result linked to the model's ID. That's the whole agent loop."

```rust
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
```

Keep the direct `String::from_utf8_lossy(&out.stdout)` inside `json!`.
This compact version returns stdout; the separate offline backup also
exercises command errors and malformed model replies.

## The three second memory map

- **State:** key, prompt, msgs, tools.
- **Turn:** client.post → blocks → msgs.push(assistant).
- **Branch:** find tool_use → let-else → text and break.
- **Act:** sh -c → stdout → msgs.push(tool_result).

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
