# Rehearsal guide

Companion to the [talk README](README.md). The README says what the code does;
this page says how to practise the talk and what to watch for on the day.

Deck: the 12-slide [Slides artifact](https://claude.ai/artifact/5DVfbG5hVUvXT9Q4tbd8r1),
cut for a 15 to 20 minute slot on 2026-10-22 (the five demo00 exercise slides
and the plug-in slide went; exercise three stays). Use it as the presentation
candidate after the final visual check. The documented slide numbers are: the tripwire is slide 10,
the live demo slide 11, case closed slide 12. `deckhand.json` and
`slides/slide_1.md` … `slide_10.md` at the repo root are the earlier 10-slide
terminal deck, kept for reference; its numbering no longer applies.

## One command

```bash
docs/talks/the-harness-wore-a-trenchcoat/rehearse.sh           # beats, tests, demo00, G-1
docs/talks/the-harness-wore-a-trenchcoat/rehearse.sh --beats   # just the live beats
```

The script uses Python 3 to wait for each reply before sending the next call,
checks response IDs, error codes/kinds, successful results, and the stderr alarm,
and fails on missing/malformed/wrong responses. Each request has a 10-second
client deadline. It creates and removes its own fresh temporary jail; optional
`JAIL` must name a nonexistent directory. It honors Cargo's target directory.

`--beats --no-build` runs the already-built binary for the stage pass. This can
be stale: first run the normal script after your last source change. The full
script runs every falcon-mcp target, demo00 verify, and G-1; it requires ast-grep
so AST tests cannot silently skip. See [RECONCILIATION.md](RECONCILIATION.md)
for dated execution evidence. The September 25 report of 111 tests is historical,
not today's test count.

## Rehearsal passes

Do them in this order. Each pass practises one thing.

1. **Toolchain pass (day before).** Run `rehearse.sh` on the laptop you will
   present from, on the venue's Wi-Fi or with it off after dependencies are cached. Build and
   download time depends on the machine and cache; never rely on a cold build
   on stage. `ast-grep` is not needed for any beat.
2. **Talk-track pass.** Go through all 12 slides out loud with no terminal. Run a
   timer. The notes are about 2,300 words, 16 minutes at 140 words a minute.
   The demo slide's narration is spoken while the beats run, so it overlaps
   the 3 to 4 minute live demo; budget 18 to 19 minutes in all. Use the boundary wording below; verify the
   current slide text first. These are planning estimates, not a measured rehearsal.
3. **Demo pass.** Run `rehearse.sh --beats` while you narrate. Then do it
   again by hand (see *Live by hand*) so you can recover if the script fails.
4. **Failure pass.** Try each item under *If it breaks*. A recovery you
   have practised looks calm on stage.
5. **Dress rehearsal.** The full talk with a timer, at projector resolution
   (large font, light terminal theme), and with notifications off.

## The live beats (slide 11)

| Beat | Server flags | Frame | What the audience sees | Line to land |
|---|---|---|---|---|
| 2 Allowed binary | `--enable-exec` | `exec_run cargo --version` | `exit: 0` plus the cargo version | "Allowlisted names run, resolved to an absolute path at startup." |
| 3 Shell injection | `--enable-exec` | `exec_run sh -c "cat /etc/passwd"` | `-32602 binary 'sh' not in allowlist` | "There is no shell, so `sh` is just a binary name, and it isn't on the list." |
| 4 Path traversal | any | `fs_read <enough ../ components>/falcon-rehearsal-outside` | `-32602 … escapes sandbox root …` (absolute paths vary) | "The kernel canonicalizes the path first, then we check the prefix." |
| 5 Tripwire | any | `fs_list .`, then `fs_read CONFIDENTIAL_KEYS.txt`, then `fs_read note.txt` | the decoy in the listing; a red `TRIPWIRE TRIGGERED` line on stderr; `-32003 … is a honeytoken; session revoked`; then `-32003 session revoked` for the innocent read | "The listing is the bait. Touch it once and this session is done. Every other session keeps working." |
| Bonus | `--read-only` | `fs_write loot.txt` | `isError: true`, `sandbox is read-only` | "This fs_write handler enforces read-only; this is not host-wide write protection." |
| Bonus | none | `exec_run cargo --version` | `exec_run disabled (server started without --enable-exec)` | "Exec is off by default. You have to turn it on." |

Things to know about the output:

- **Two kinds of refusal.** Beats 3, 4 and 5 return a JSON-RPC `error` with a
  code the client can branch on: `-32602` for bad arguments, `-32003` for the
  tripwire. The bonus beats return a normal `result` with `isError: true`.
  If someone asks why: addressing failures are protocol errors, and a
  policy refusal is a tool result the model can read and react to.
- **The tripwire must come last.** It revokes the session, so any beat you
  send after it on the same server gets `-32003`. The script gives beat 5 its
  own server and waits for each reply; already-running calls are not cancelled.
- **Why searches don't trip it.** A broad `fs_search` is normal agent
  behaviour, so it skips the decoy silently: no trip, and no leaked contents.
  Addressing the decoy through guarded paths (including resolving symlinks)
  can trip it; this is not monitoring all OS reads. If someone asks about
  `exec_run rg KEY .`: exec is off by default, and its arguments are checked
  by name only. That limit is documented in `falcon-mcp/src/tripwire.rs`.
- **Replies can arrive out of order.** When several calls go to one server,
  it answers them concurrently (a dry run printed ids 4, 3, 2). The script
  waits for each response, including initialize, before proceeding. By hand,
  do the same; a sleep is not an acknowledgement.
- **Jail paths vary.** The script uses a new temporary directory and enough
  parent components to escape it. The manual `/tmp/jail` example below assumes
  an otherwise unused directory. Never reuse a jail containing real work.
- **The cargo version in beat 2 may not be 1.92.** `/tmp/jail` has no
  `rust-toolchain.toml`, so rustup uses its default toolchain. This is
  expected. Don't let it distract you.

## Live by hand

```bash
cargo build -p falcon-mcp
mkdir -p /tmp/jail && git -C /tmp/jail init -q
printf 'AWS_SECRET_ACCESS_KEY=decoy-not-a-real-key\n' > /tmp/jail/CONFIDENTIAL_KEYS.txt
echo hello > /tmp/jail/note.txt
target/debug/falcon-mcp --stdio --root /tmp/jail --enable-exec
```

Send initialize and wait for its reply, then send notifications/initialized.
Send each tools/call separately and wait for its reply. Do not paste the whole
block at once: it can race revocation. Expected output depends on the jail path:

```json
{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"demo","version":"0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"exec_run","arguments":{"cmd":"cargo","args":["--version"]}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"exec_run","arguments":{"cmd":"sh","args":["-c","cat /etc/passwd"]}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"fs_read","arguments":{"path":"../../.ssh/id_rsa"}}}
{"jsonrpc":"2.0","id":51,"method":"tools/call","params":{"name":"fs_list","arguments":{"path":"."}}}
{"jsonrpc":"2.0","id":52,"method":"tools/call","params":{"name":"fs_read","arguments":{"path":"CONFIDENTIAL_KEYS.txt"}}}
{"jsonrpc":"2.0","id":53,"method":"tools/call","params":{"name":"fs_read","arguments":{"path":"note.txt"}}}
```

Beat 5 is the last three lines, and it has to go last. Keep these lines in a
notes file. Don't type JSON live.

## Deck vs code

The repository recorded an artifact review on October 22; the October 25 audit
could not open that artifact. Do not turn the old review into a new blanket
claim that every slide matches. Check [the reconciliation](RECONCILIATION.md)
and say these boundaries on slide 10 / the demo:

- "It revokes subsequent tool calls in this session, not the process or pipe."
- "The code is `-32003`; timeout is `-32001`."
- "This path check is application-level; subprocesses are not OS-jailed."
- "These are deterministic MCP requests, not a live prompt-injection/model demo."

If you change a slide, run the matching beat again so the reply on screen
still matches what the slide says.

## If it breaks

| Symptom | Recovery |
|---|---|
| Server prints nothing | You skipped `initialize` or `notifications/initialized`. Paste the handshake again. |
| `exec_run disabled` on beat 2 | You started it without `--enable-exec`. Call it the bonus beat, then restart with the flag. |
| A beat hangs | Ctrl-C, restart the server, and send only that frame. Fallback: show the saved verified transcript. `exec_test` only covers exec; use `fs_test` / `tripwire_test` for the other beats. |
| Beat 5 shows `session revoked` on the first read | You sent it after another trip on the same server. Restart the server and send beat 5 fresh. |
| Build is slow or offline | Build the night before. `target/` survives, so don't run `cargo clean`. |
| Projector mangles the JSON | Use a larger terminal font and the saved transcript. The labelled script output is mixed text/JSON, so do not pipe it directly into jq. |

## Audience exercises (demo00)

If you give the room hands-on time, everyone does the same three steps:
`cd women-in-rust/demo00 && cargo run -- next`. The first stop is `schema1`.
Before the meetup, check that `cargo run -- verify` passes, because it checks
the intended exercise/solution states. `intro1` passes as-is; the five remaining
exercises fail until solved, and all six solutions pass. `cargo run -- hint <name>`
prints the hint, so don't read solutions aloud.

## Guardrail

Don't "tidy up" anything in `falcon-agent` while you prepare (G-1).
`rehearse.sh` ends with `scripts/check-planted-defects.sh`. A `VIOLATION` is a planted-state failure; an environment/compile error means
the guard could not verify it. Do not report either as a pass.
