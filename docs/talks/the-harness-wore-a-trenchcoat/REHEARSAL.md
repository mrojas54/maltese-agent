# Rehearsal guide

Companion to the [talk README](README.md). The README says what the code does;
this page says how to practise the talk and what to watch for on the day.

Deck: the 12-slide [Slides artifact](https://claude.ai/artifact/5DVfbG5hVUvXT9Q4tbd8r1),
cut for a 15 to 20 minute slot on 2026-10-22 (the five demo00 exercise slides
and the plug-in slide went; exercise three stays). Present from it. The slide
numbers below match it: the tripwire is slide 10,
the live demo slide 11, case closed slide 12. `deckhand.json` and
`slides/slide_1.md` … `slide_10.md` at the repo root are the earlier 10-slide
terminal deck, kept for reference; its numbering no longer applies.

## One command

```bash
docs/talks/the-harness-wore-a-trenchcoat/rehearse.sh           # beats, tests, demo00, G-1
docs/talks/the-harness-wore-a-trenchcoat/rehearse.sh --beats   # just the live beats
```

The script prints every frame it sends and every reply it gets back. Last dry
run (2026-09-25): all five beats behaved as below; 111/111 falcon-mcp tests
passed; `demo00 verify` passed all six exercises; the G-1 planted-defect
check passed.

## Rehearsal passes

Do them in this order. Each pass practises one thing.

1. **Toolchain pass (day before).** Run `rehearse.sh` on the laptop you will
   present from, on the venue's Wi-Fi or with it off. A cold `cargo build -p
   falcon-mcp` takes about a minute, so never build on stage. `ast-grep`
   is not needed for any beat.
2. **Talk-track pass.** Go through all 12 slides out loud with no terminal. Run a
   timer. The notes are about 1,650 words, 12 minutes at 140 words a minute,
   plus the pauses they mark. The demo slide's narration is spoken while the
   beats run, so it overlaps the 3 to 4 minute live demo; budget about 15
   minutes in all, which leaves room for laughs and questions inside the slot
   (see *Timing budget*). At each slide in the *Deck vs code* table below, say the
   correction out loud so it comes easily on the day.
3. **Demo pass.** Run `rehearse.sh --beats` while you narrate. Then do it
   again by hand (see *Live by hand*) so you can recover if the script fails.
4. **Failure pass.** Try each item under *If it breaks*. A recovery you
   have practised looks calm on stage.
5. **Dress rehearsal.** The full talk with a timer, at projector resolution
   (large font, light terminal theme), and with notifications off.

## Timing budget

The 2026-10-28 revision cut the notes from about 2,300 words to about 1,650
and added one joke per slide. The notes mark `(Pause.)` and `(Beat.)` where
to stop for a second or a laugh; take them, because they are what keeps the
talk from feeling rushed. Each pause is budgeted at 3 seconds below. If you
fall behind, cut a pause, not a joke: the jokes also carry the point.

| # | Slide | Words | Pauses | Minutes | Running |
|---|---|---|---|---|---|
| 1 | Cover | 126 | 1 | 0.9 | 0.9 |
| 2 | Crime scene | 157 | 2 | 1.2 | 2.2 |
| 3 | The wire | 117 | 2 | 0.9 | 3.1 |
| 4 | Case file: demo00 | 125 | 1 | 0.9 | 4.0 |
| 5 | Refuse or fail | 163 | 2 | 1.3 | 5.3 |
| 6 | Part two: Boundaries | 103 | 2 | 0.8 | 6.2 |
| 7 | Root jail | 126 | 2 | 1.0 | 7.2 |
| 8 | Allowlist | 142 | 2 | 1.1 | 8.3 |
| 9 | Wall clock | 118 | 2 | 0.9 | 9.2 |
| 10 | Honeytokens | 188 | 2 | 1.4 | 10.7 |
| 11 | Live demo | 155 | 2 | 3.5 | 14.2 |
| 12 | Case closed | 143 | 1 | 1.1 | 15.2 |

Two slides have a stamp that appears on a click: TRIPPED on slide 10 (click
before the falcon line) and SOLVED on slide 12. Practise the click so it
lands on the punchline.

## The live beats (slide 11)

| Beat | Server flags | Frame | What the audience sees | Line to land |
|---|---|---|---|---|
| 2 Allowed binary | `--enable-exec` | `exec_run cargo --version` | `exit: 0` plus the cargo version | "Allowlisted names run, resolved to an absolute path at startup." |
| 3 Shell injection | `--enable-exec` | `exec_run sh -c "cat /etc/passwd"` | `-32602 binary 'sh' not in allowlist` | "There is no shell, so `sh` is just a binary name, and it isn't on the list." |
| 4 Path traversal | any | `fs_read ../../.ssh/id_rsa` | `-32602 … path /.ssh/id_rsa escapes sandbox root /tmp/jail` | "The kernel canonicalizes the path first, then we check the prefix." |
| 5 Tripwire | any | `fs_list .`, then `fs_read CONFIDENTIAL_KEYS.txt`, then `fs_read note.txt` | the decoy in the listing; a red `TRIPWIRE TRIGGERED` line on stderr; `-32003 … is a honeytoken; session revoked`; then `-32003 session revoked` for the innocent read | "The listing is the bait. Touch it once and this session is done. Every other session keeps working." |
| Bonus | `--read-only` | `fs_write loot.txt` | `isError: true`, `sandbox is read-only` | "Read-only is a hard gate too, not a prompt." |
| Bonus | none | `exec_run cargo --version` | `exec_run disabled (server started without --enable-exec)` | "Exec is off by default. You have to turn it on." |

Things to know about the output:

- **Two kinds of refusal.** Beats 3, 4 and 5 return a JSON-RPC `error` with a
  code the client can branch on: `-32602` for bad arguments, `-32003` for the
  tripwire. The bonus beats return a normal `result` with `isError: true`.
  If someone asks why: addressing failures are protocol errors, and a
  policy refusal is a tool result the model can read and react to.
- **The tripwire must come last.** It revokes the session, so any beat you
  send after it on the same server gets `-32003`. The script gives beat 5 its
  own server.
- **Why searches don't trip it.** A broad `fs_search` is normal agent
  behaviour, so it skips the decoy silently: no trip, and no leaked contents.
  Only addressing the file by name trips it. If someone asks about
  `exec_run rg KEY .`: exec is off by default, and its arguments are checked
  by name only. That limit is documented in `falcon-mcp/src/tripwire.rs`.
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
printf 'AWS_SECRET_ACCESS_KEY=decoy-not-a-real-key\n' > /tmp/jail/CONFIDENTIAL_KEYS.txt
echo hello > /tmp/jail/note.txt
target/debug/falcon-mcp --stdio --root /tmp/jail --enable-exec
```

Paste the handshake first. Then paste one beat at a time:

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

Every slide now matches `falcon-mcp` as shipped (2026-10-22), including the
tripwire. Nothing needs correcting on stage. Two lines are worth saying on
slide 10:

- "It revokes the session, not the process." The early sketch called
  `exit(1)`, which would have taken the server down for every client.
- "The code's number is `-32003`, next to timeout's `-32001`." A client
  branches on it the same way.

If you change a slide, run the matching beat again so the reply on screen
still matches what the slide says.

## If it breaks

| Symptom | Recovery |
|---|---|
| Server prints nothing | You skipped `initialize` or `notifications/initialized`. Paste the handshake again. |
| `exec_run disabled` on beat 2 | You started it without `--enable-exec`. Call it the bonus beat, then restart with the flag. |
| A beat hangs | Ctrl-C, restart the server, and send only that frame. Fallback: `cargo test -p falcon-mcp --test exec_test`. The tests assert the same beats. |
| Beat 5 shows `session revoked` on the first read | You sent it after another trip on the same server. Restart the server and send beat 5 fresh. |
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
