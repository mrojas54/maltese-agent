# Women in Rust: meetup demos

Companion code for *The Harness Wore a Trenchcoat: Building a Sandboxed Rust
MCP Server*. The deck itself is [`deckhand.json`](../deckhand.json) at the
repo root.

| Demo | What it is |
| --- | --- |
| [`demo00`](demo00/) | A rustlings-style tutorial: six exercises that build a stdio MCP server from the schema up. `cd demo00 && cargo run -- next`. |
| [`demo01`](demo01/) | The crime scene: tools that read any file and run any command, with no boundaries yet. `cd demo01 && cargo run -- next`. |
| [`demo02`](demo02/) | Guards: a `Sandbox` every file tool goes through, a `--read-only` switch, and a first (lexical) path check that is honest about not being enough. `cd demo02 && cargo run -- next`. |

The full-size version with the root jail, binary allowlist and timeouts is
[`falcon-mcp`](../falcon-mcp/).
