# Rehearsal guide

Companion to the [talk README](README.md). The README says what the code does;
this page says how to practise the talk and what to watch for on the day.

Deck: `slides/slide_1.md` … `slide_10.md` (via `deckhand.json`). The README's
slide numbers come from the older 8-slide deck. The numbers below match the
current 10-slide deck.

## One command

```bash
docs/talks/the-harness-wore-a-trenchcoat/rehearse.sh           # beats, tests, demo00, G-1
docs/talks/the-harness-wore-a-trenchcoat/rehearse.sh --beats   # just the live beats
```

The script prints every frame it sends and every reply it gets back. Last dry
run (2026-09-25): all five beats behaved as below; 97/97 falcon-mcp tests
passed; `demo00 verify` passed all six exercises; the G-1 planted-defect
check passed.

## Rehearsal passes

Do them in this order. Each pass practises one thing.

1. **Toolchain pass (day before).** Run `rehearse.sh` on the laptop you will
   present from, on the venue's Wi-Fi or with it off. A cold `cargo build -p
   falcon-mcp` takes about a minute, so never build on stage. `ast-grep`
   is not needed for any beat.
2. **Talk-track pass.** Go through slides 1–10 out loud with no terminal. Run a
   timer. At each slide in the *Deck vs code* table below, say the
   correction out loud so it comes easily on the day.
3. **Demo pass.** Run `rehearse.sh --beats` while you narrate. Then do it
   again by hand (see *Live by hand*) so you can recover if the script fails.
4. **Failure pass.** Try each item under *If it breaks*. A recovery you
   have practised looks calm on stage.
5. **Dress rehearsal.** The full talk with a timer, at projector resolution
   (large font, light terminal theme), and with notifications off.

## The live beats (slide 9)

| Beat | Server flags | Frame | What the audience sees | Line to land |
|---|---|---|---|---|
| 2 Allowed binary | `--enable-exec` | `exec_run cargo --version` | `exit: 0` plus the cargo version | "Allowlisted names run, resolved to an absolute path at startup." |
| 3 Shell injection | `--enable-exec` | `exec_run sh -c "cat /etc/passwd"` | `-32602 binary 'sh' not in allowlist` | "There is no shell, so `sh` is just a binary name, and it isn't on the list." |
| 4 Path traversal | any | `fs_read ../../.ssh/id_rsa` | `-32602 … path /.ssh/id_rsa escapes sandbox root /tmp/jail` | "The kernel canonicalizes the path first, then we check the prefix." |
| 5 Tripwire (stand-in) | `--read-only` | `fs_write loot.txt` | `isError: true`, `sandbox is read-only` | "The honeytoken is the design direction. The read-only guard is what ships today." |
| Bonus | none | `exec_run cargo --version` | `exec_run disabled (server started without --enable-exec)` | "Exec is off by default. You have to turn it on." |

Things to know about the output:

- **Two kinds of refusal.** Beats 3 and 4 return a JSON-RPC `error` with code
  `-32602`. Beat 5 and the bonus return a normal `result` with `isError: true`.
  If someone asks why: argument validation fails as a protocol error, and a
  policy refusal is a tool result the model can read and react to.
- **Replies can arrive out of order.** When several calls go to one server,
  it answers them concurrently (a dry run printed ids 4, 3, 2). The script
  starts a new server for each beat. By hand, send one frame and wait for its
  reply before the next.
- **Keep the jail path short.** The escape message prints both absolute
  paths. `/tmp/jail` is easy to read. A long scratch path wraps across the
  screen.
- **The cargo version in beat 2 may not be 1.92.** `/tmp/jail` has no
  `rust-toolchain.toml`, so rustup uses its default toolchain. This is
  expected. Don't let it distract you.

## Live by hand

```bash
cargo build -p falcon-mcp
mkdir -p /tmp/jail && git -C /tmp/jail init -q
target/debug/falcon-mcp --stdio --root /tmp/jail --enable-exec
```

Paste the handshake first. Then paste one beat at a time:

```json
{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"demo","version":"0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"exec_run","arguments":{"cmd":"cargo","args":["--version"]}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"exec_run","arguments":{"cmd":"sh","args":["-c","cat /etc/passwd"]}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"fs_read","arguments":{"path":"../../.ssh/id_rsa"}}}
```

Press Ctrl-D, then restart the server with `--read-only` for beat 5. Keep these
lines in a notes file. Don't type JSON live.

## Deck vs code: what to say at each slide

All of these were checked against `falcon-mcp` as it is today. Correct them on
stage or fix them in the deck before the talk.

| Slide | Deck says | Code does | Say |
|---|---|---|---|
| 3 | tool `read_file`, message `Access Denied: Path escape` | tool `fs_read`, message `resolving path: path … escapes sandbox root …`. The code `-32602` matches. | "Real message on the next demo." Or update the slide. |
| 4, 6 | one 10 s timeout | per family: cargo 900 s, git 60 s, search 30 s, exec 30 s (`limits.rs`) | "Cold builds of tokio + axum take minutes, so cargo gets 900 seconds." |
| 6 | timeout → `internal_error` | dedicated code `-32001` with structured data (`tool_error.rs`) | "The client can tell 'too slow' apart from 'broken'." |
| 6, 10 | allowlist `cargo`, `git` | `cargo rustc rustfmt rg git ast-grep`, resolved to absolute paths at startup | "Six names, pinned at startup, so changing PATH later can't swap one." |
| 7 | `Tripwire(CanaryAlert)` stage | not implemented | Call it the next stage. |
| 8 | honeytoken → `exit(1)` | not implemented | Call it the design direction. Also, `exit(1)` would kill the server for every client connected to it. Revoking the session is the better design. |
| 8 vs 9 | `CONFIDENTIAL_KEYS.txt` vs `canary_keys.txt` | neither exists | The two slides use different filenames. Pick one before the talk. |
| 9 | `run("cargo check")` | the demo uses `cargo --version` | `cargo check` in an empty jail fails because it has no `Cargo.toml`. Keep `--version`. |

## If it breaks

| Symptom | Recovery |
|---|---|
| Server prints nothing | You skipped `initialize` or `notifications/initialized`. Paste the handshake again. |
| `exec_run disabled` on beat 2 | You started it without `--enable-exec`. Call it the bonus beat, then restart with the flag. |
| A beat hangs | Ctrl-C, restart the server, and send only that frame. Fallback: `cargo test -p falcon-mcp --test exec_test`. The tests assert the same beats. |
| Build is slow or offline | Build the night before. `target/` survives, so don't run `cargo clean`. |
| Projector mangles the JSON | Pipe through `jq -c .` or switch to `rehearse.sh --beats`, which labels each beat. |

## Audience exercises (demo00)

If you give the room hands-on time, everyone does the same three steps:
`cd women-in-rust/demo00 && cargo run -- next`. The first stop is `schema1`.
Before the meetup, check that `cargo run -- verify` passes, because it proves
every exercise still fails until someone solves it. `cargo run -- hint <name>`
prints the hint, so don't read solutions aloud.

## Guardrail

Don't "tidy up" anything in `falcon-agent` while you prepare (G-1).
`rehearse.sh` ends with `scripts/check-planted-defects.sh`. If that line is
not `OK`, something was changed that shouldn't have been.
