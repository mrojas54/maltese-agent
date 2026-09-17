# demo00 — the smallest MCP server

One tool, one transport, under a hundred lines including comments. This is
the skeleton that [`falcon-mcp`](../../falcon-mcp/) grows out of: the root
jail, binary allowlist and timeouts from the talk all bolt onto exactly this
shape.

## Run it

```bash
cargo run -p demo00
```

It sits there waiting. An MCP server on stdio reads JSON-RPC frames from
stdin and writes replies to stdout, one JSON object per line. Paste these
three lines one at a time and watch the replies:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"you","version":"0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"greet","arguments":{"name":"Ada"}}}
```

Ctrl-D closes stdin, which is how a client hangs up.

## Read it

[`src/main.rs`](src/main.rs), top to bottom:

| Piece | What it does |
| --- | --- |
| `GreetArgs`, `GreetResult` | Plain structs. `JsonSchema` derives the schema the model sees, so the Rust type is the contract. |
| `#[tool] greet` | The tool. Its `description` is the only documentation the model ever reads. |
| `#[tool_router]` | Collects every `#[tool]` method into the list that `tools/list` returns. |
| `#[tool_handler]` + `get_info` | Routes `tools/*` requests and answers the `initialize` handshake. |
| `main` | Sends logs to stderr, then `serve(stdio())`. |

## The one rule

**stdout is the wire.** A stray `println!` becomes a corrupt frame and the
client disconnects. Log with `tracing`, which is pointed at stderr in `main`.

```bash
RUST_LOG=info cargo run -p demo00
```

shows the logs on stderr while stdout stays clean.

## Test it

```bash
cargo test -p demo00
```

[`tests/wire_test.rs`](tests/wire_test.rs) speaks raw JSON-RPC to the built
binary with no client library, so it doubles as a transcript of the protocol.
It runs the server with logging on and asserts that every stdout line is
JSON and the log text landed on stderr.

## Plug it into a client

Any MCP client that launches stdio servers can run it. Build first, then
point the client at the binary with an absolute path:

```bash
cargo build -p demo00
```

```json
{
  "mcpServers": {
    "demo00": {
      "command": "/absolute/path/to/maltese-agent/target/debug/demo00"
    }
  }
}
```

That block is the format Gemini CLI reads from `~/.gemini/settings.json` and
Claude Desktop reads from `claude_desktop_config.json`. Claude Code takes the
same thing on the command line:

```bash
claude mcp add demo00 -- /absolute/path/to/maltese-agent/target/debug/demo00
```

## Where this goes

Everything in `falcon-mcp` is this file with boundaries added: paths resolve
through a root jail before `fs_read` touches disk, `exec_run` checks a binary
allowlist, and every subprocess runs under a timeout. The tool signature,
the router, and the stdio loop are unchanged.
