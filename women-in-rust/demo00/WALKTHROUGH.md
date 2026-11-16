# Build an MCP server in Rust: a guided walkthrough

This is the homework from *The Harness Wore a Trenchcoat*. In the talk you
watched a harness learn to refuse bad requests. Here you build the other half:
a small MCP server, the program an agent talks to when it wants to use a tool.

You fill in five small holes, one file at a time. Each step tells you what is
wrong, what to type, why it matters, and how to check it. The whole thing takes
about 30 to 45 minutes. You don't need to know MCP already.

## Before you start

You need Rust installed with `rustup` (https://rustup.rs). The repo pins its
toolchain, so the first `cargo` command fetches the right version for you.
Then get a copy of the repo:

```bash
git clone https://github.com/mrojas54/maltese-agent
cd maltese-agent/women-in-rust/demo00
cargo build --bins
```

The first build downloads the crates (rmcp, tokio, serde, schemars) and takes a
minute or two. After that, each check takes a few seconds.

## How every step works

1. Open the file the step names. Read the comment at the top; it explains the
   idea in a few lines.
2. Type the fix where the `TODO` is.
3. Check just that file:

   ```bash
   cargo run -- run NAME
   ```

   A pass ends with a green `✓ ... passes.` line and the name of the next file.
   A fail shows you the compiler error or the test that failed.

Stuck? `cargo run -- hint NAME` prints a hint, and `solutions/` has every
answer, same layout. Try first; the struggle is where the learning is.

> `cargo run -- next` also works: it runs every exercise in order and stops at
> the first one that fails. Use it to find your place. But after a fix, it
> shows the pass and then goes straight on to the next file's errors, so use
> `run NAME` to see your green.

## Step 0: read the finished server (5 minutes)

**File:** `exercises/00_intro/intro1.rs`. Nothing to fix.

This is the server you are about to build, already working. Read it top to
bottom once. It has four parts, and every rmcp server has the same four:

| Part | What it is |
| --- | --- |
| Two structs | `GreetArgs` (what the model sends) and `GreetResult` (what you send back). |
| One tool | `greet`, an `async fn` with a `#[tool]` attribute. |
| The handshake | `get_info`, which tells the client who you are and what you can do. |
| `main` | Serves it all over stdin and stdout. |

The next five steps take this server apart and have you put each piece back.

```bash
cargo run -- run intro1    # passes already: this is what green looks like
```

## Step 1: the struct is the schema

**File:** `exercises/01_schema/schema1.rs`

**What you'll see:**

```
error[E0277]: the trait bound `GreetArgs: serde::Deserialize<'de>` is not satisfied
error[E0277]: the trait bound `GreetArgs: JsonSchema` is not satisfied
```

There may be a lot of output above these lines. The two `E0277` lines are
the ones that matter.

**What to change.** Replace the derive line, and add a doc comment to the field:

```rust
#[derive(Debug, Deserialize, JsonSchema)]
struct GreetArgs {
    /// Who to greet.
    name: String,
}
```

**Why.** A model calls a tool by sending JSON. `Deserialize` turns that JSON
into this struct. `JsonSchema` describes the struct to the model, so it knows
what to send. You never write the schema by hand: change the struct and the
schema follows. The doc comment is not decoration. It becomes the field's
`description` in the schema, and that is the only thing the model will ever
read about `name`.

**Check:** `cargo run -- run schema1`

## Step 2: the tool, described for the model

**File:** `exercises/02_tool/tool1.rs`

**What you'll see:**

```
the model needs a description to know when to call greet
not yet implemented: greet Ada
```

**What to change.** Two `TODO`s. First, give the tool a description:

```rust
#[tool(name = "greet", description = "Greet someone by name. Returns {greeting: string}.")]
```

Then replace the `todo!(...)` line in the body with:

```rust
Ok(Json(GreetResult { greeting: format!("Hello, {name}!") }))
```

**Why.** The description is written for the model, not for people. It is the
only thing the model has when it decides whether to call this tool, so say
what the tool does and what it returns. `Json(...)` wraps your result so the
client gets it back as structured data, described by `GreetResult`'s schema.

**Check:** `cargo run -- run tool1`

## Step 3: refuse bad input

**File:** `exercises/02_tool/tool2.rs`

**What you'll see:**

```
"" was greeted instead of rejected: Hello, ! Welcome to Women in Rust.
```

The schema says `name` is a string. It doesn't say the string means anything,
so an empty name gets a cheerful greeting.

**What to change.** At the `TODO`, before the greeting:

```rust
if name.trim().is_empty() {
    return Err(ErrorData::invalid_params("name must not be empty", None));
}
```

**Why.** This is the same habit as the harness in the talk: check first, and
refuse anything wrong before doing any work. Returning `Err` sends the client
a JSON-RPC error with code -32602 ("invalid params") and no result at all, so
the client knows the call was refused, not that it ran and failed.
falcon-mcp uses exactly this error when a path tries to leave its folder.

**Check:** `cargo run -- run tool2`

## Step 4: say hello with your own name

**File:** `exercises/03_handshake/handshake1.rs`

**What you'll see:**

```
assertion `left == right` failed: the client should see this package's name
  left: "rmcp"
 right: "demo00"
```

The server introduces itself as `rmcp`, the library, instead of `demo00`.

**What to change.** At the `TODO`, inside `impl ServerHandler for Demo00`:

```rust
fn get_info(&self) -> ServerInfo {
    ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
        .with_server_info(Implementation::new(
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
        ))
}
```

**Why.** When a client connects, the first thing it asks is "who are you and
what can you do?". `get_info` is your answer. `enable_tools()` says you have
tools; a careful client won't ask for tools a server didn't advertise.

The name is the gotcha. `env!("CARGO_PKG_NAME")` is filled in where you write
it, at compile time. rmcp has a helper that calls `env!` too, but inside rmcp,
so it gets rmcp's name. Written in your file, it gets yours. (falcon-mcp, the
full server in this repo, still has this bug. Nobody's perfect.)

**Check:** `cargo run -- run handshake1`

## Step 5: stdout is the wire

**File:** `exercises/04_stdio/stdio1.rs`

**What you'll see:**

```
stdout is the wire: every line on it must be a JSON-RPC frame, but this one is not:
    "listening on stdio"
```

**What to change.** One word in `main`: `println!` becomes `eprintln!`.

```rust
eprintln!("listening on stdio");
```

**Why.** This server talks to its client over stdin and stdout, one JSON
message per line. The client reads every line on stdout as a message, so a
log line there looks like broken JSON and the client hangs up. Logs go to
stderr, which the client ignores or shows as logs. (The solution file goes one
step further and uses `tracing`, which is what falcon-mcp does.)

**Check:** `cargo run -- run stdio1`. This one builds the real server, starts
it, and talks to it the way a client would.

Then run every exercise once more, in order:

```bash
cargo run -- next
```

It should end with **"You have built an MCP server."**

## Bonus: talk to your server by hand

Start it:

```bash
cargo run --bin stdio1
```

It prints `listening on stdio` (that's stderr) and waits. Paste these four
lines one at a time:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"you","version":"0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"greet","arguments":{"name":"Ada"}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"greet","arguments":{"name":""}}}
```

You get three replies; the second line is a notification, and notifications
never get one. The first reply says `"serverInfo":{"name":"demo00",...}`
(step 4). The second greets Ada (step 2). The third is an error,
`"code":-32602,"message":"name must not be empty"` (step 3). Press Ctrl-D to
hang up.

## Where to go next

- **Start over:** `git checkout exercises/` puts every hole back.
- **The checks from the talk:** `../baby-harness` has the harness I typed on
  stage, with each step saved in `src/steps/`.
- **The grown-up version:** `../../falcon-mcp` is a full MCP server with the
  same shape as yours, plus a folder jail, an allowlist, timeouts and a
  tripwire.
- **More exercises:** `../demo01` to `../demo03` add tools and then lock them
  down.

Found a step confusing? Open an issue on the repo and say which step lost
you. That's how this walkthrough gets better.
