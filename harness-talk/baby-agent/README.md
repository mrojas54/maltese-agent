# Build a minimal Rust shell agent live

A fresh checkout leaves `run` in `src/main.rs` unfinished. Build the
Anthropic shell agent on stage: declare `sh`, run the model's command, send
its output back under the matching tool ID, and repeat until the model
answers. HTTP and offline replay are connected below the function.

Use [LIVE-CODING.md](LIVE-CODING.md) for the fresh-clone commands, the code
to type in order, the expected exchange, and recovery. Use [INTRO.md](INTRO.md)
for the spoken introduction and transition to `../baby-harness/`.

## Start the demo

```bash
git clone --branch codex/baby-agent-live-starter https://github.com/mrojas54/maltese-agent.git maltese-agent-live
cd maltese-agent-live/harness-talk/baby-agent
cargo build --locked --bins
cargo run --locked -- --replay
```

The last command exits with `agent loop is not implemented yet`, with no
shell command or final answer. That is the intended starting point. Open
`src/main.rs` and build `run` using the live-coding script.

After the edit, the same command uses authored Anthropic replies from
`fixtures/replay.json`. It runs a real `cat note.txt`, checks the result,
then returns the scripted answer. A missing or edited note fails the replay.
It needs no API key or model request.

## Run with Anthropic

After building the loop, with `ANTHROPIC_API_KEY` set in your environment:

```bash
cargo run --locked
cargo run --locked -- "Read note.txt. Where and when is the meeting?"
```

The default prompt is `list files`. `ANTHROPIC_MODEL` overrides the default
`claude-sonnet-5-5`; see the [model lifecycle](https://platform.claude.com/docs/en/about-claude/model-deprecations)
for supported model IDs. The program does not load `.env` files. HTTP
requests have a 30-second timeout, the loop allows five model turns, and
HTTP failures stop the run without a retry.

The only direct dependencies are `reqwest` and `serde_json`. The tool runs
`sh -c` with your operating-system permissions. The next harness demo adds
checks to command and file tools.

The request disables parallel tool use to keep one command per response.
The full assistant content is kept, and the next user message contains the
`tool_result` with its matching `tool_use_id`. Failed commands set
`is_error: true`. See [Anthropic's tool-call format](https://platform.claude.com/docs/en/agents-and-tools/tool-use/handle-tool-calls).

## Completed backup

`src/complete.rs` holds the finished version. Select it explicitly:

```bash
cargo run --locked --bin baby-agent-complete -- --replay
```

This leaves the unfinished `src/main.rs` alone. The script also shows how
to copy the backup over a practice edit and how to reset afterwards.

## Check the demo

From this directory, before practicing or after resetting:

```bash
cargo fmt -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

The suite checks the unfinished default and the completed shell loop: real
shell output reaches the next request, tool IDs match, failed commands are
marked as errors, unexpected calls and truncated replies are rejected, and
the replay rejects missing or changed notes. Tests and CI need no API key.

The starter check is expected to fail while `src/main.rs` has your completed
practice edit. The default `--replay` command checks the code you typed.
