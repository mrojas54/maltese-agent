# demo00: MCP servers in Rust, rustlings-style

Six small exercises that build a working MCP server from nothing: the schema
the model sees, the tool, input validation, the `initialize` handshake, and
the stdio transport. Each exercise is one file with a hole in it and its own
tests. Fix the file, the tests pass, move on.

If you have done [rustlings](https://github.com/rust-lang/rustlings), the loop
is the same. If you have not, it is:

```bash
cd women-in-rust/demo00
cargo run -- next
```

`next` runs the exercises in order and stops at the first one that needs you.
Open the file it names, read the comment at the top, fix the `TODO`s, run
`next` again. Stuck?

```bash
cargo run -- hint schema1
```

## The exercises

| # | File | You learn |
| --- | --- | --- |
| 0 | `exercises/00_intro/intro1.rs` | The whole server, working. Read it once. |
| 1 | `exercises/01_schema/schema1.rs` | A Rust struct *is* the tool's JSON Schema. |
| 2 | `exercises/02_tool/tool1.rs` | The tool. Its description is written for the model. |
| 3 | `exercises/02_tool/tool2.rs` | Rejecting bad input with an error the client can branch on. |
| 4 | `exercises/03_handshake/handshake1.rs` | The `initialize` reply, and an `env!` gotcha. |
| 5 | `exercises/04_stdio/stdio1.rs` | stdout is the wire. |

Solutions are in `solutions/`, same layout. Try before you look.

## Commands

| Command | What it does |
| --- | --- |
| `cargo run -- next` | Run the exercises in order; stop at the first that fails. |
| `cargo run -- run NAME` | Check one exercise. |
| `cargo run -- hint NAME` | Print its hint. |
| `cargo run -- list` | List them. |
| `cargo run -- verify` | For CI: every solution passes, every exercise still fails. |

Under the hood each exercise is its own binary (see `Cargo.toml`), so
`run schema1` is `cargo test --bin schema1`. That is what lets the other
exercises stay broken while you work on one. The two that serve (`intro1`,
`stdio1`) get one more check: the runner builds the binary and holds a real
JSON-RPC conversation with it over stdio, the same four frames any client
sends.

## Run one as a server

Any exercise with a real `main` is a server you can talk to by hand:

```bash
cargo run --bin intro1
```

It sits there waiting. Paste these lines one at a time and watch the replies:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"you","version":"0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"greet","arguments":{"name":"Ada"}}}
```

Ctrl-D closes stdin, which is how a client hangs up.

## Plug one into a client

Build it, then point any MCP client that launches stdio servers at the
binary with an absolute path:

```bash
cargo build --bin intro1
```

```json
{
  "mcpServers": {
    "demo00": {
      "command": "/absolute/path/to/maltese-agent/women-in-rust/demo00/target/debug/intro1"
    }
  }
}
```

That block is the format Gemini CLI reads from `~/.gemini/settings.json` and
Claude Desktop reads from `claude_desktop_config.json`. Claude Code takes the
same thing on the command line:

```bash
claude mcp add demo00 -- /absolute/path/to/maltese-agent/women-in-rust/demo00/target/debug/intro1
```

## How it is put together

- `exercises/` and `solutions/` mirror each other. Every file in both is a
  `[[bin]]` in `Cargo.toml`.
- `info.toml` is the exercise order and the hints, in the shape of rustlings'
  file of the same name.
- `src/main.rs` is the runner. It is short; read it if you want to see the
  wire probe.
- This package is deliberately not a member of the repo's root workspace,
  because its exercises are broken on purpose. CI runs `cargo run -- verify`
  here instead, which also fails if someone "fixes" an exercise.

## Where this goes

Everything in [`falcon-mcp`](../../falcon-mcp/) is `intro1` with boundaries
added: paths resolve through a root jail before `fs_read` touches disk,
`exec_run` checks a binary allowlist, and every subprocess runs under a
timeout. The tool signature, the router, and the stdio loop are unchanged.
