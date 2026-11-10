# demo02: guards

demo01 gave the server two tools with no boundaries. demo02 puts the first ones
in. A `Sandbox` sits between the file tools and the disk, `--read-only` turns
writes off, and a first check refuses the obvious escapes: `../secret` and
`/etc/passwd`.

It is the first guard, not the last, and this demo says so out loud. The check
is *lexical*: it reads how a path is spelled and never looks at the filesystem.
demo03 is where that stops being enough.

```bash
cd women-in-rust/demo02
cargo run -- next
```

Same loop as before. `next` runs the exercises in order and stops at the first
that needs you. Open the file it names, read the comment at the top, fix the
`TODO`s, run `next` again.

```bash
cargo run -- hint sandbox1
```

## The exercises

| # | File | You learn |
| --- | --- | --- |
| 0 | `exercises/00_intro/intro1.rs` | The whole demo02 server: guarded, and still leaking. Read it once. |
| 1 | `exercises/01_sandbox/sandbox1.rs` | A lexical path check with `Path::components()`, and why `root.join("/etc/passwd")` throws the root away. |
| 2 | `exercises/01_sandbox/sandbox2.rs` | A guard nobody calls is decoration: make the tools go through it. |
| 3 | `exercises/02_readonly/readonly1.rs` | A read-only switch, the gate that asks it, and why a refusal is an `isError` result, not a JSON-RPC error. |

Solutions are in `solutions/`, same layout. Try before you look.

## Two kinds of "no"

A client can tell these apart, and they mean different things:

| The request | What comes back | Code |
| --- | --- | --- |
| `fs_read {"path": "../secret"}` | a JSON-RPC **error**, no `result` | `-32602` |
| `fs_write` under `--read-only` | a **result** with `isError: true` | none |

The first is the client's mistake: what it sent is not allowed. The second is
the tool answering "I ran, and I can't do that here". `ToolError` keeps them
apart. It is the same enum falcon-mcp has, with two of its kinds; demo04 adds
the timeout, which gets its own code, `-32001`.

## The line this demo is about

Two claims that sound the same and are not:

- **It refuses `../secret`.** True, and tested.
- **It stays inside the root.** Not yet.

Watch the second one fail. Start the finished server with a scratch directory as
its root:

```bash
mkdir /tmp/jail
cargo run --bin intro1 -- --root /tmp/jail
```

Paste these one line at a time, waiting for each reply. (The server handles
requests concurrently, so frames sent back to back can overtake each other.)

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"you","version":"0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"fs_write","arguments":{"path":"hello.txt","content":"hi"}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"fs_read","arguments":{"path":"hello.txt"}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"fs_read","arguments":{"path":"../../etc/hosts"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"exec_run","arguments":{"cmd":"ln","args":["-s","/etc","/tmp/jail/out"]}}}
{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"fs_read","arguments":{"path":"out/hosts"}}}
```

Frame 4 is refused, with `-32602` and `escapes sandbox root`. Frame 5 uses the
tool that is still wide open to plant a symlink, `/tmp/jail/out -> /etc`, inside
the root. Frame 6 reads `out/hosts`: no `..`, no leading `/`, a perfectly clean
spelling. It returns `/etc/hosts`. The guard read the spelling, and the
filesystem did the rest.

Two holes, and each one props the other open. The symlink escape is demo03's
to close, and the tool that planted it is demo04's.

## What each guard stops, and what it does not

| Guard | Stops | Does not stop |
| --- | --- | --- |
| The lexical check in `Sandbox::resolve` | `..`, a leading `/` | A symlink inside the root that points out of it |
| `--read-only` | `fs_write` | Writes made through `exec_run`: `sh -c 'echo x > y'` still works |
| `exec_run` | nothing | Everything. No allowlist, and it runs in the server's working directory, not the root |

Restart with `--read-only` and try `fs_write` to see the second kind of "no":

```bash
cargo run --bin intro1 -- --root /tmp/jail --read-only
```

## Where the next demos go

| Demo | Adds | Refuses |
| --- | --- | --- |
| demo03 | the real jail: `resolve` canonicalizes, so symlinks and `..` are followed *before* the check, and paths that do not exist yet get a parent fallback | every escaping path, including the symlinked one above |
| demo04 | the exec allowlist (pinned from PATH at startup), no subshell, a working directory in the root, and a timeout that surfaces as `-32001` | any binary but the allowlisted few; runaway commands |

## Commands

| Command | What it does |
| --- | --- |
| `cargo run -- next` | Run the exercises in order; stop at the first that fails. |
| `cargo run -- run NAME` | Check one exercise. |
| `cargo run -- hint NAME` | Print its hint. |
| `cargo run -- list` | List them. |
| `cargo run -- verify` | For CI: every solution passes, every exercise still fails. |

## How it is put together

Same shape as demo00 and demo01.

- `exercises/` and `solutions/` mirror each other; every file in both is a
  `[[bin]]` in `Cargo.toml`.
- `info.toml` is the exercise order, the hints, and the wire probe's
  conversations. `readonly1` starts its server with `--read-only` through the
  `args` key, so the flag is tested end to end.
- `src/main.rs` hands that file to the runner in [`../runner`](../runner/),
  which every demo shares.
- This package is excluded from the repo's root workspace, because its
  exercises are broken on purpose. CI runs `cargo run -- verify` here.

## Where this goes

The `Sandbox` here is a small version of
[`falcon-mcp/src/sandbox.rs`](../../falcon-mcp/src/sandbox.rs): the same
`new(root, read_only)`, `resolve`, and `check_writable`. `ToolError` is
[`falcon-mcp/src/tool_error.rs`](../../falcon-mcp/src/tool_error.rs) with fewer
kinds, and `read_file` and `write_file` are the shape of the handlers in
[`falcon-mcp/src/tools/fs_basic.rs`](../../falcon-mcp/src/tools/fs_basic.rs): a
function that takes the Sandbox, never a bare path. The flags are
[`falcon-mcp/src/main.rs`](../../falcon-mcp/src/main.rs) without the transport
options. What is missing is what demo03 and demo04 add: a `resolve` that asks
the filesystem, and an `exec_run` that takes a Sandbox at all.
