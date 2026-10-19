# Women in Rust: meetup demos

Companion code for *The Harness Wore a Trenchcoat: Building a Sandboxed Rust
MCP Server*. The deck itself is [`deckhand.json`](../deckhand.json) at the
repo root.

| Demo | What it is |
| --- | --- |
| [`demo00`](demo00/) | A rustlings-style tutorial: six exercises that build a stdio MCP server from the schema up. `cd demo00 && cargo run -- next`. |
| [`demo01`](demo01/) | The same server with `fs_read` and `exec_run`: four exercises, every test passing, no boundaries yet. `cd demo01 && cargo run -- next`. |
| [`demo02`](demo02/) | Guards: a `Sandbox` every file tool goes through, a `--read-only` switch, and a first (lexical) path check that is honest about not being enough. `cd demo02 && cargo run -- next`. |
| [`demo03`](demo03/) | *Work in progress.* The jail: `resolve` canonicalizes instead of reading spellings, closing demo02's symlink hole. `cd demo03 && cargo run -- next`. |

## Check out a demo

Each demo has a branch that holds it and every demo before it:

```bash
git switch women-in-rust/demo00
git switch women-in-rust/demo01
git switch women-in-rust/demo02
```

Switch to the branch for the demo you are on, then `cd women-in-rust/demoNN`
and `cargo run -- next`.

The full-size version with the root jail, binary allowlist and timeouts is
[`falcon-mcp`](../falcon-mcp/).
