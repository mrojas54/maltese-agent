# Build the opening shell agent live

One model, one `sh` tool, and a loop. Start at the unfinished `run` in
`src/main.rs`. The code below it connects either Anthropic's Messages API
or the offline replies to that function. You write the tool declaration,
execute a shell command, and send its result back to the model.

## Fresh checkout and warmup

```bash
git clone --branch codex/baby-agent-live-starter https://github.com/mrojas54/maltese-agent.git maltese-agent-live
cd maltese-agent-live/harness-talk/baby-agent
cargo build --locked --bins
cargo run --locked -- --replay
```

The last command must stop with `agent loop is not implemented yet`. It
exits with status 1. No shell command or final answer runs yet. Building
first downloads the dependencies without filling in the starting file.

Have this script on your phone or another screen. Open `src/main.rs` on the
projector at `run`; keep `main` folded below it. Use [INTRO.md](INTRO.md)
for the spoken introduction. Time the whole build aloud in rehearsal.

## Prepare the imports

Replace the first import with:

```rust
use serde_json::{json, Value};
use std::process::Command;
```

Add this constant after the `Result` type:

```rust
const MAX_TURNS: usize = 5;
```

Replace the placeholder `run` with the following four fragments, in order.
Together they form one function. Run it after the fourth fragment.

## 1 Describe sh and ask the model

Say: "The tool is a shell command. The model supplies a string called cmd.
We send the conversation and keep the assistant's complete reply."

```rust
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
```

`ANTHROPIC_MODEL` overrides the default model. Parallel tool use is disabled
so this demo handles one tool request per response. Claude can still answer
without using a tool. See the [API's single-call setting](https://platform.claude.com/docs/en/agents-and-tools/tool-use/parallel-tool-use#at-most-one-tool-call).

## 2 Return a final answer

Say: "If there is no tool request, the model's text is the answer."

```rust
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
```

## 3 Run the shell command

Say: "Here is where text becomes an action. Rust starts sh with the command
the model supplied. There are no command checks yet."

```rust
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
```

## 4 Return the tool result and ask again

Say: "The result refers to the request's ID. We add it as the next user
message, and the loop sends the conversation again."

```rust
        msgs.push(json!({"role": "user", "content": [{
            "type": "tool_result", "tool_use_id": id, "content": text,
            "is_error": !out.status.success()
        }]}));
    }
    Err(format!("stopped after {MAX_TURNS} model turns without a final answer").into())
}
```

## Run what you just wrote

Save, then run from this directory:

```bash
cargo run --locked -- --replay
```

Say: "These model replies are scripted for rehearsal. The shell command
and the loop are real."

Expected terminal exchange, with the command line in yellow:

```text
Offline rehearsal: scripted model replies; the shell command is real.
$ cat note.txt
The meeting is at 19:00 in the library.
```

The replay checks the real `cat note.txt` result before giving the loop its
scripted final reply. A missing or changed note fails instead of displaying
that answer.

For the same task with a live model, with `ANTHROPIC_API_KEY` already set:

```bash
cargo run --locked -- "Read note.txt. Where and when is the meeting?"
```

The live default from the original shell example also works:

```bash
cargo run --locked
```

That default prompt is `list files`. The live model chooses its command and
answer. The program reads environment variables, does not load `.env` files,
and stops on HTTP errors without retrying them.

Then say: "We've got an agent. Now let's look at what its tools should be
allowed to do." Switch to `../baby-harness/`, open `src/harness.rs`, and
follow its four edits. Its fixed requests include both commands and file
reads, and it starts at zero of four attacks stopped.

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
