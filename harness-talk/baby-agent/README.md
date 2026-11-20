# A minimal Rust agent

One model, one tool, and a loop, in `src/main.rs`. Gemini requests a file,
Rust reads it, and the file's contents go back to Gemini so it can answer.
This is the opening example before the checks in `../baby-harness/`.

The only direct dependencies are `reqwest` for HTTP and `serde_json` for JSON.
The model can request `read_file`, which opens the path it supplies using your
operating-system permissions. There is no path policy yet.

## Run the live agent

Use the repository's pinned Rust toolchain. With `GEMINI_API_KEY` already set
in your environment:

```bash
cd harness-talk/baby-agent
cargo run --locked
```

The default prompt is `Read note.txt. Where and when is the meeting?`.
The terminal shows each tool request, its result, and the model's final answer.
The model chooses whether to call the tool; a live answer can vary.

For another prompt, pass it as one quoted argument:

```bash
cargo run --locked -- "Read note.txt and quote its contents."
```

The default model is `gemini-3.1-flash-lite`; override it with `GEMINI_MODEL`.
The program reads environment variables and does not load `.env` files.
It stops on an HTTP error, times out each request after 30 seconds, and allows
at most four model turns. It does not retry quota failures.

## Rehearse without a key or network

From this directory:

```bash
cargo run --locked -- --replay
```

```text
Offline rehearsal: scripted model replies; the file read is real.
model -> tool: read_file {"path":"note.txt"}
tool -> model: {"result":"The meeting is at 19:00 in the library.\n"}
agent: The meeting is at 19:00 in the library.
```

This mode runs the same loop with the authored replies in `fixtures/replay.json`.
It makes no model request. It checks the actual tool result before displaying
the scripted final answer, and exits with an error if `note.txt` is missing or
has changed. Restore the original note for rehearsal; use live mode to ask the
model about changed contents.

## Present the example

Use [the intro and transition script](INTRO.md). Build and run once before
the talk. Have `src/main.rs` open and the terminal in this directory before
saying "let's build this thing."

Show the tool description, the loop, the real file read, and the result being
added to the conversation. Keep the HTTP setup below them until someone asks.
The complete model turn is retained so provider metadata such as thought
signatures survives the next request; function results retain call IDs when
provided. This follows the [Gemini content API](https://aistudio.google.com/api-reference/generate-content).

## Check the demo

```bash
cargo fmt -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Tests cover a real file read through the next model request, final answers,
multiple tool calls, tool errors, turn limits, malformed replies, and the
printed rehearsal output with valid, missing, and changed notes. Tests and
CI use scripted replies and need no API key; a live run is a separate check.
