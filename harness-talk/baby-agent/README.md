# Type a minimal Rust shell agent live

A fresh checkout leaves `src/main.rs` as a blank `main` with four comments.
Type the whole Anthropic shell agent in four chunks: state and tool, POST,
find tool_use, and sh -c plus the result push. The HTTP request is part of
the live build.

Use [LIVE-CODING.md](LIVE-CODING.md) for the exact chunks, spoken cues,
rehearsal targets, memory map, and recovery. Use [INTRO.md](INTRO.md) for
the introduction and transition to `../baby-harness/`.

## Fresh checkout

```bash
git clone https://github.com/mrojas54/maltese-agent.git maltese-agent-live
cd maltese-agent-live/harness-talk/baby-agent
cargo build --locked --bins
cargo run --locked
```

The last command exits with `agent loop is not implemented yet`. It needs
no key and runs no command. Replace the placeholder with the four chunks.
The only direct dependencies are `reqwest` and `serde_json`.

## Run the live program

After typing it, with `ANTHROPIC_API_KEY` set in your environment:

```bash
cargo run --locked
cargo run --locked -- "Read note.txt. Where and when is the meeting?"
```

The default prompt is `list files`. `ANTHROPIC_MODEL` overrides the default
`claude-sonnet-5-5`; see the [model lifecycle](https://platform.claude.com/docs/en/about-claude/model-deprecations).
The live file reads environment variables and does not load `.env` files.
It sends at most five model requests and times out each one after 30
seconds. A truncated reply or an HTTP error ends the run. If the fifth
reply requests a tool, the agent exits with an error before running it.

The stage version runs the supplied string through `sh -c` and returns
stdout in a `tool_result` with the matching `tool_use_id`. The shell
runs without `ANTHROPIC_API_KEY`. The request disables parallel tool use
so the agent handles one command per response. The following harness demo
introduces command and file checks.

## Recovery and offline fallback

`src/stage.rs` is the completed four-chunk file. It compiles as the
`baby-agent-stage` binary, or can replace your typing with:

```bash
cp src/stage.rs src/main.rs
cargo run --locked
```

For an offline fallback, use the separate completed replay implementation:

```bash
cargo run --locked --bin baby-agent-complete -- --replay
```

Its model replies are authored in `fixtures/replay.json`. The `cat note.txt`
command is real, and its result is checked before the scripted final answer.
A missing or edited note fails. This fallback needs no API key and leaves
the live file alone.

To discard practice edits and return to the blank starter:

```bash
git restore -- src/main.rs
```

## Check the demo

From this directory, before practicing or after resetting:

```bash
cargo fmt -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --bin baby-agent-stage
```

The suite checks the blank default entry and the replay implementation,
including real shell output reaching the next request, matching tool IDs,
command errors, turn limits, and missing or changed notes. The stage
checkpoint is compiled by the all-target checks and its explicit build.
A test checks that the four Rust blocks in the script reproduce that file.
The starter check is expected to fail once you have typed the live program.
