# Live-code demo00 on stage

Act 3, part two of *The Harness Wore a Trenchcoat*, right after the live
harness. You fill in all four lessons (five files) and end with a working MCP
server in Rust. About 30 lines and 500 characters of typing; plan on 10 to 12
minutes with the runs and the talking.

Build once before you go on stage, so each check takes seconds:

```bash
cd women-in-rust/demo00
cargo build --bins && cargo run -- list
```

Each step is the same loop: type the fix, then check just that file with
`cargo run -- run NAME` (in Zed, the **demo00: check this file** task, which
fills in the name of the open file). It prints about a dozen lines ending in
"✓ ... passes." and the next file's name. Read the green line out loud, then
open the next file.

Don't use `next` between steps. After each fix it prints the ✓ and then goes
straight on to the next file's failure, about 100 lines of red, so the room
never sees the green. Use `next` only at the very end, for the done message.

The very first run, before you type anything, prints about 90 lines of
compiler error (a schemars E0277) before the "✗ schema1 needs you" block.
Say "ignore the wall, look at the last block" and scroll to it.

## 1. schema1: a struct is the schema (about 1 minute)

`exercises/01_schema/schema1.rs`. Change the derive and add the doc comment:

```rust
#[derive(Debug, Deserialize, JsonSchema)]
struct GreetArgs {
    /// Who to greet.
    name: String,
}
```

Say: "Deserialize reads what the model sends. JsonSchema tells the model what
to send. And that doc comment isn't for you. It's the only thing the model
will ever read about this field."

## 2. tool1: the tool (about 2 minutes)

`exercises/02_tool/tool1.rs`. Fill in the description, then replace the
`todo!`:

```rust
#[tool(name = "greet", description = "Greet someone by name. Returns {greeting: string}.")]
```

```rust
Ok(Json(GreetResult { greeting: format!("Hello, {name}!") }))
```

Say: "The description is prose for the model. It's the tool's job interview."

## 3. tool2: refuse bad input (about 2 minutes)

`exercises/02_tool/tool2.rs`. At the TODO, before the greeting:

```rust
if name.trim().is_empty() {
    return Err(ErrorData::invalid_params("name must not be empty", None));
}
```

Say: "Same habit as the harness: check first, refuse anything wrong. On the
wire that's error -32602, which a client can branch on."

## 4. handshake1: say hello properly (about 3 minutes)

`exercises/03_handshake/handshake1.rs`. Inside `impl ServerHandler for Demo00`:

```rust
fn get_info(&self) -> ServerInfo {
    ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
        .with_server_info(Implementation::new(
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
        ))
}
```

Say: "`env!` expands where you write it. Call rmcp's helper and you get rmcp's
name, because that's where it was written. Fun fact: falcon-mcp still gets
this one wrong. Do as I say, not as I ship."

## 5. stdio1: stdout is the wire (about 30 seconds)

`exercises/04_stdio/stdio1.rs`. One word in `main`:

```rust
eprintln!("listening on stdio");
```

Run `cargo run -- next` and read the done message: "You have built an MCP
server." Say: "One word. A log line on stdout is a corrupt message, and the
client hangs up blaming JSON. Logs go to stderr."

Then: "That's a real MCP server in about thirty lines of Rust. falcon-mcp, in
the same repo, is the grown-up version: these checks plus timeouts, served
over MCP." Go to the Case Closed slide.

## If it goes wrong

| Problem | Recovery |
| --- | --- |
| A typo you can't spot | `cargo run -- hint NAME` shows the answer, or `cp solutions/DIR/NAME.rs exercises/DIR/NAME.rs`. |
| Running short on time | Copy the remaining solutions over and run `cargo run -- next` once to show all green. |
| Start over | `git checkout exercises/` puts every hole back. |

**After the talk, run `git checkout exercises/`.** `cargo run -- verify`
(and CI) expects every exercise to still fail, so a filled-in exercise left
in the tree breaks it.

## Timing the whole live section

| Part | Typing | With runs and talking |
| --- | --- | --- |
| Baby harness, steps 1 to 4 | about 1,600 characters | 15 to 20 minutes |
| demo00, all five files | about 500 characters | 10 to 12 minutes |
| Both | | 25 to 32 minutes |
| Slides around them (6 slides of notes) | | about 5 minutes |
| Whole talk | | 30 to 37 minutes, in a 30-minute slot |

These are estimates. Your timed run is the real number. If it comes out over
30, the quickest cut is pasting harness step 4 (the tripwire) from
`src/steps/step4.rs` instead of typing it, which saves about 5 minutes.
