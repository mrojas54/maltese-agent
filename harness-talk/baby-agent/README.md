# Build a minimal Rust agent live

A fresh checkout leaves `run` in `src/main.rs` unfinished. Build one model,
one file-reading tool, and a loop on stage, then move to the checks in
`../baby-harness/`. HTTP and offline replay are connected below the function
so the code you type is the agent itself.

Use [LIVE-CODING.md](LIVE-CODING.md) for the fresh-clone commands, the code
to type in order, the expected exchange, and recovery. Use [INTRO.md](INTRO.md)
for the spoken introduction and transition.

## Start the demo

```bash
git clone --branch codex/baby-agent-live-starter https://github.com/mrojas54/maltese-agent.git maltese-agent-live
cd maltese-agent-live/harness-talk/baby-agent
cargo build --locked --bins
cargo run --locked -- --replay
```

The last command exits with `agent loop is not implemented yet`, with no
tool call or final answer. That is the intended starting point. Open
`src/main.rs` and build `run` using the live-coding script.

After the edit, the same command runs the loop with authored replies from
`fixtures/replay.json`. The file read is real. The replay checks its result
before displaying the scripted final answer; missing or edited `note.txt`
causes an error. It needs no API key or model request.

For a live model after the edit, with `GEMINI_API_KEY` already set:

```bash
cargo run --locked
```

The default prompt is `Read note.txt. Where and when is the meeting?`. Pass a
different prompt as one quoted argument. The default model is
`gemini-3.1-flash-lite`; override it with `GEMINI_MODEL`. The program reads
environment variables and does not load `.env` files. Each request has a
30-second timeout, the loop allows four turns, and provider errors stop the
run without a retry.

The only direct dependencies are `reqwest` and `serde_json`. The completed
file tool opens the supplied path with your operating-system permissions;
the next harness demo introduces policy checks. Complete model turns retain
provider metadata, and function results retain call IDs when supplied, as in
the [Gemini content API](https://aistudio.google.com/api-reference/generate-content).

## Completed backup

`src/complete.rs` holds the finished version. Select it explicitly:

```bash
cargo run --locked --bin baby-agent-complete -- --replay
```

This leaves the unfinished `src/main.rs` alone. The live-coding script also
shows how to copy the backup over a practice edit and how to reset afterwards.

## Check the demo

From this directory, before practicing or after resetting:

```bash
cargo fmt -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

The suite checks that the default entry is unfinished and that the completed
backup handles the real file read, conversation history, tool errors, turn
limits, malformed replies, and replay with valid, missing, and changed notes.
The starter check is expected to fail while `src/main.rs` has your completed
practice edit. The default `--replay` command checks the code you typed.
