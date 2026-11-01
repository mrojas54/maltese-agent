# maltese-agent

*The code behind "The Harness Wore a Trenchcoat", a talk about the piece of
software that sits between a language model and your laptop.*

If you have run a command in a terminal, you can follow this repo. No AI
security background needed.

## Three words

The talk, and this repo, rest on three words:

- **Agent.** A language model reads text and writes text. It cannot open a
  file by itself; it can only ask. Put the model in a loop with a set of
  *tools* (functions that do something real: read a file, run `cargo`, make
  a git commit) and you have an agent.
- **MCP.** The Model Context Protocol. An agreed set of messages for "which
  tools do you have?" and "run this one". The messages are JSON, sent over a
  pipe between the client (the side the model works through) and the server
  (the side with the tools).
- **Harness.** The code on the server side. The model asks, the harness
  decides, the tool runs. That decision is the security boundary. The prompt
  is not: a sentence like "please only run cargo" is a request, and nothing
  makes the model obey it.

The harness in this repo is called `falcon-mcp`. It is one Rust program that
speaks MCP and puts four checks in front of its tools: which files you can
touch, which programs you can run, how long they can take, and a tripwire
that notices when something goes looking for a decoy credentials file.

One honest line before anything else: those checks live in the server's own
code. They are not an operating-system sandbox. A program the server launches,
such as `cargo` or `git`, keeps your permissions and can reach files outside
the folder. The talk notes keep
[a list of what the checks do not cover](docs/talks/the-harness-wore-a-trenchcoat/README.md#where-the-deck-and-the-code-disagree).

## What is in the repo

| Folder | What it is |
|---|---|
| `falcon-mcp/` | The harness. A Rust MCP server with 19 tools (files, cargo, git, a prompt-safety lint, and an optional `exec_run`), path checks that keep the file tools inside one folder, an allowlist of six programs, per-tool timeouts, and the tripwire. The subject of the talk. |
| `falcon-agent/` | A small Rust web service that is **broken on purpose**: a poisoned example in its prompt, missing input and output checks, three planted lints, and a skipped test that would catch the bug. It is the workshop target. Do not fix it (see below). |
| `falcon-detective/` | A TypeScript workflow, built on Barnum, that finds and fixes the problems in `falcon-agent` by calling Gemini for reasoning and `falcon-mcp` for tools. It replays from recorded cassettes, so you can run it without an API key. |
| `women-in-rust/` | The homework. `demo00` is six small exercises that build a working MCP server from scratch; `demo01` to `demo03` add tools, then guards, then the path check that closes the symlink hole. |
| `docs/talks/the-harness-wore-a-trenchcoat/` | The talk's companion notes: which slide maps to which file, the demo runbook, what the checks do and do not cover, and the rehearsal script. |
| `slides/`, `deckhand.json` | An earlier 10-slide terminal deck, kept for reference. |

The talk is presented from the
[12-slide meetup deck](https://claude.ai/artifact/5DVfbG5hVUvXT9Q4tbd8r1), a
Claude artifact. A [13-slide beginner edition](https://claude.ai/artifact/1LbubpRRCTc6Y6kw3psFmK)
covers the same ground with every term defined as it appears. The
slide-by-slide code map in the talk folder is written against the meetup deck.

## See the demo

The demo has no model in it. A script plays the client and sends `falcon-mcp`
the same MCP messages the talk reads out, so the result is the same every
time. You need the pinned Rust toolchain (`rustup` reads
`rust-toolchain.toml` for you), Python 3 and git.

```bash
docs/talks/the-harness-wore-a-trenchcoat/rehearse.sh --beats
```

This builds `falcon-mcp`, starts it in a scratch folder with a decoy file
planted, and sends five beats:

1. **Hello.** The MCP handshake.
2. **Happy path.** `cargo --version` through `exec_run`. Cargo is on the
   allowlist, so it runs and exits 0.
3. **Crime scene.** `sh -c "cat /etc/passwd"`. `sh` is not on the list, so it
   is refused before anything starts.
4. **Climbing out.** A read of a path outside the folder. Refused: "escapes
   sandbox root".
5. **The bait.** A listing shows `CONFIDENTIAL_KEYS.txt`; reading it trips
   the wire, logs `TRIPWIRE TRIGGERED` on stderr, and every later call on that
   session is refused, including a read of a perfectly ordinary file.

The run should end with a line that starts
`PASS: all live stdio expectations matched`. Add `--no-build` to skip the
cargo build once you have one. Without `--beats` the
script also runs the backing tests and the workshop guard, which needs
`ast-grep`, `jq` and clippy.

Every beat is also an integration test under `falcon-mcp/tests/`, so the
suite is the rehearsal.

## Do the homework

```bash
cd women-in-rust/demo00
cargo run -- next
```

It works like rustlings: `next` runs the exercises in order and stops at the
first one that needs you. Open the file it names, read the comment at the
top, fill in the gaps, run `next` again. Exercise 0 already works; the other
five each have a hole in them. `women-in-rust/README.md` lists the later
demos and the branch that holds each one.

## Run the full caper

The longer story is a workflow that repairs the broken service. It replays
recorded Gemini responses, so no API key is needed. Node 22.12 or newer is
required; CI uses Node 24.

```bash
# 1. Build the harness and the broken target
cargo build -p falcon-mcp -p falcon-agent

# 2. Build the workflow
cd falcon-detective && npm install && npm run build

# 3. Replay against falcon-agent (resets the target to its broken state first)
npm run test:full -- tests/e2e.test.ts
```

Expected: vitest reports `1 passed` in about nine minutes, most of it cargo
rebuilding inside a per-run worktree. The skipped test in `falcon-agent`,
`bird_themed_inputs_arent_special`, goes from ignored to passing, which is
the proof that the workflow removed the poison.

For a live Gemini run instead of a replay, see
[`falcon-detective/README.md`](falcon-detective/README.md).

## Run the tests

Fast loops, no API key, no cassettes:

```bash
cargo test -p falcon-mcp --lib            # Rust unit tests for the harness
cd falcon-detective && npm test           # TypeScript unit tests, no e2e
```

What CI runs (`.github/workflows/ci.yml`):

```bash
cargo fmt --all -- --check
cargo clippy -p falcon-mcp --all-targets -- -D warnings   # falcon-agent's lints are planted
cargo test --workspace --all-targets                       # needs ast-grep on PATH
docs/talks/the-harness-wore-a-trenchcoat/rehearse.sh --beats --no-build
./scripts/check-planted-defects.sh                         # fails if falcon-agent gets "fixed"
cd falcon-detective && npm run lint && npm run build && npm run test:full
```

The e2e inside `test:full` needs `target/debug/falcon-mcp` built and the
cassettes under `falcon-detective/fixtures/cassettes/`. It skips with a
message when either is missing.

## Do not fix falcon-agent

`falcon-agent`'s poisoned prompt, missing guards, planted lints and
`#[ignore]`'d test are the workshop material. The detective is supposed to
find them. `scripts/check-planted-defects.sh` fails CI if any of them change.
This is guardrail G-1 in `CLAUDE.md`.

## More reading

- [`docs/talks/the-harness-wore-a-trenchcoat/README.md`](docs/talks/the-harness-wore-a-trenchcoat/README.md):
  slide-by-slide code map, the demo runbook, and an honest list of what the
  checks do not cover.
- [`falcon-mcp/README.md`](falcon-mcp/README.md): the 19 tools and the server
  flags.
- [`docs/superpowers/specs/2026-04-30-maltese-agent-design.md`](docs/superpowers/specs/2026-04-30-maltese-agent-design.md):
  the original design, with the implementation plans beside it.
- `SPEC.md`, `EVALUATION.md`, `BUILDPLAN.md`, `LESSONS.md` and
  `sequence/run-state.md`: the record of the 2026-07 tech-debt remediation.
- [Barnum](https://github.com/barnum-circus/barnum), the workflow engine
  under `falcon-detective`.
- [arXiv:2510.07192](https://arxiv.org/abs/2510.07192), the trigger-pattern
  backdoor that `falcon-agent`'s poisoned prompt imitates.
- [`rmcp`](https://crates.io/crates/rmcp), the Rust MCP SDK `falcon-mcp` is
  built on.
