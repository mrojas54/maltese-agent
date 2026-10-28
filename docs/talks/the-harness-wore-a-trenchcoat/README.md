# The Harness Wore a Trenchcoat

*Building a Sandboxed Rust MCP Server* — a talk for the **Women in Rust Meetup**.

Speaker: Michelle Rojas. Theme: noir. Deck: the 12-slide
[Slides artifact](https://claude.ai/artifact/5DVfbG5hVUvXT9Q4tbd8r1), which is
what the talk is presented from. [`deckhand.json`](../../../deckhand.json) and
`slides/` at the repo root are the earlier 10-slide terminal deck, kept for
reference.

This README is the companion to the slides. It maps the documented talk topics
to `falcon-mcp/`, gives a runbook for the live demo, and
separates demonstrated behavior from historical evidence and unsupported claims.
See [the October 25 reconciliation](RECONCILIATION.md) before rehearsal.

---

## The pitch in one paragraph

An LLM agent that can touch a filesystem and run commands is only as safe as the
thing standing between it and the host. Prompts are soft guardrails. They ask
nicely and fail under adversarial pressure. `falcon-mcp` is a hard control plane:
a single Rust binary speaking the Model Context Protocol (MCP, JSON-RPC 2.0) over
stdio or HTTP, with application-level filesystem path checks, an allowlisted
exec surface, and wall-clock timeouts around subprocess calls. The harness is the detective. It wears the
trenchcoat so the model does not have to be trusted.

## Why this repo

`falcon-mcp` is one third of a larger caper (see the [root README](../../../README.md)):

| Component | Role | Language |
|---|---|---|
| `falcon-agent` | A deliberately poisoned axum service. The target. | Rust |
| `falcon-detective` | A Barnum workflow that finds and fixes the poison. The agent. | TypeScript |
| `falcon-mcp` | The sandboxed toolbelt the agent operates through. **This talk.** | Rust |

The talk zooms in on the toolbelt because it is the part that has to hold when
everything above it lies.

## Slide-by-slide: where the code is

### 1. The Crime Scene

The naive shape is `LLM -> raw shell`. MCP calls enter through `FalconMcp`
in `falcon-mcp/src/server.rs`; filesystem and subprocess handlers receive a
`Sandbox`. This mediates those calls but does not confine subprocess behavior.

### 2. Architecture: from REST to stdio

Both transports live in `falcon-mcp/src/main.rs`:

- `--stdio` (default) serves over `rmcp::transport::stdio()`.
- `--http <port>` mounts `StreamableHttpService` at `/mcp` on an axum router.

The rule on the slide, "stdout is reserved for JSON-RPC frames, diagnostics go to
stderr," is enforced by one line: the tracing subscriber is built with
`.with_writer(std::io::stderr)` in `main.rs`. If a log line ever hit stdout in
stdio mode it would corrupt the frame stream, so this is not a style choice.

### 3. Rust MCP server architecture

| Slide layer | Code |
|---|---|
| Transport | `falcon-mcp/src/main.rs` |
| Capability router | `#[tool_router]` on `FalconMcp` in `falcon-mcp/src/server.rs`, with serde + schemars generating each tool's JSON schema |
| Domain handlers | `falcon-mcp/src/tools/{fs_basic,fs_ast,cargo,git,prompt_lint,exec}.rs` |
| Failsafe timeouts | `falcon-mcp/src/limits.rs` |

Nineteen tools across five categories are listed in
[`falcon-mcp/README.md`](../../../falcon-mcp/README.md).

### 4. Sandbox boundary: root jail

`Sandbox::resolve` in `falcon-mcp/src/sandbox.rs` canonicalizes the joined path
and rejects anything that does not start with the canonical root. The slide's
snippet is a faithful abbreviation. Two details the slide leaves out:

- The root itself is canonicalized once in `Sandbox::new`, so a symlinked root
  cannot be used to confuse the prefix check.
- Paths that do not exist yet (a file about to be written) walk up to the first
  existing ancestor, canonicalize that, then re-attach the suffix. The escape
  check still runs on the result. This is what lets `fs_write` create nested
  directories without opening a hole.

Proof: `resolve_dotdot_escape_rejected` and `resolve_symlink_escape_rejected` in
the same file, plus `falcon-mcp/tests/fs_test.rs`.

### 5. Sandbox boundary: binary allowlist

`Sandbox::check_bin` rejects any name not in the allowlist
(`cargo`, `rustc`, `rustfmt`, `rg`, `git`, `ast-grep`). The code goes one step
further than the slide: `Sandbox::resolved_bin` returns an **absolute path
resolved from PATH at server startup**, and `exec_run` spawns only that path.
Changing PATH after the server is up cannot substitute an impostor binary for an
allowlisted name.

Two more layers worth saying out loud on stage:

- `exec_run` is registered, but its handler refuses execution unless the server
  starts with `--enable-exec` (`falcon-mcp/src/server.rs`). The refusal is a
  tool result with `isError: true`, not an unknown-tool error.
- Commands are spawned directly via `tokio::process::Command`. There is no
  subshell, so `sh -c '...'` is rejected as the binary `sh`, and shell
  metacharacters are not interpreted by `exec_run` itself. The selected
  program still interprets its own arguments.

Proof: `exec_disabled_by_default`, `exec_rejects_non_allowlisted_binary` in
`falcon-mcp/tests/exec_test.rs`; the PATH-swap case in
`falcon-mcp/tests/exec_impostor_test.rs`.

### 6. The hard tripwire: honeytokens and canaries

The tripwire is implemented in `falcon-mcp/src/tripwire.rs` (landed 2026-09-25).

- **Trigger.** `Sandbox::resolve` in `falcon-mcp/src/sandbox.rs` runs the root
  jail check first, then refuses any path whose components name a honeytoken.
  The default is `CONFIDENTIAL_KEYS.txt`, matched case-insensitively. Add more
  with `--honeytoken NAME`. Every path-taking tool resolves through there, so
  reads and permitted writes, patches, `git_add`, and cargo or exec `cwd`
  can trip, as can a symlink that points at the decoy. Earlier policy gates
  may refuse a call before path resolution (for example a read-only write).
  `exec_run` arguments that name one trip it too.
- **Response.** The call fails with code `-32003` (`data.kind = "tripwire"`).
  An `ERROR TRIPWIRE TRIGGERED` line goes to stderr, and the session is
  revoked: the `call_tool` override in `server.rs` refuses every later call on
  it. Revocation is per session. Over HTTP each session gets its own flag
  (`FalconMcp::for_new_session`), so one hostile client cannot take the server
  down for the others. That is why the slide does not call `exit(1)`.
- **The bait.** `fs_list` shows the decoy. Native `fs_search` skips matching
  files before reading; `fs_search_ast` filters matching result paths after
  ast-grep runs. Neither is a host-wide file-access hook.
- **Known limits.** `exec_run` is checked by argument text only, so
  `rg KEY .` can still read the decoy through a directory walk. Exec is off
  by default. A decoy that has been committed shows in `git_diff` and
  `git_log`, so keep it untracked.

Proof: `falcon-mcp/tests/tripwire_test.rs` covers the read, revocation,
case variants, a symlink, a write, search and listing, exec arguments, the
`--honeytoken` flag, and a trip on one session that leaves another working.

The word "canary" elsewhere in this repo means two other things: a test in
`fs_test.rs` that plants a literal line to prove search matches it, and the
`#[ignore]`'d smoking-gun test in `falcon-agent` that the detective is supposed
to un-ignore.

### 7. Live demo flow

See the runbook below. Each step maps to an existing integration test, so the
demo can be rehearsed by running the suite.

### 8. Case closed

The three takeaways are all true of the code as shipped:

- Filesystem and subprocess handlers receive an `Arc<Sandbox>`; pure
  `prompt_lint` does not need one.
- Hard control planes over soft prompt guardrails: no guard in `falcon-mcp` is a
  prompt.
- One Rust binary, two transports: `main.rs` picks stdio or HTTP from a flag.

## Where the deck and the code disagree

The October 22 README/PR recorded agreement with the 12-slide artifact. That
is historical review evidence, not a fresh inspection of the external slides.
The October 25 audit could read the older Google Slides deck but could not
retrieve the Claude artifact. Use the 12-slide artifact as the presentation
candidate, subject to a final visual check against [RECONCILIATION.md](RECONCILIATION.md).
The September Google Slides deck and the root terminal slides are reference
materials, not evidence that their security or performance claims are implemented.

**Boundary to state aloud:** this is application-level mediation, not `chroot`,
a namespace, or an OS process sandbox. `exec_run` checks the command name,
honeytoken argument text, and cwd; it does not confine what the child can read,
write, or execute. Allowed tools such as cargo and git can execute build scripts,
tests, or hooks. `--read-only` guards selected handlers, not the whole host or
all child-process side effects. The tripwire rejects subsequent tool calls;
it does not close the pipe, kill the process, or cancel calls already running.
A traversal rejection is not itself a tripwire or session revocation.

Startup PATH pinning is specific to `exec_run`; cargo/git/AST handlers still
spawn by name. The checked path is not an immutable executable identity.
Canonicalize-then-open also does not establish race-free filesystem confinement.
These limitations are reasons to narrow the talk's claims, not to modify the
intentionally vulnerable workshop target.

For reference, the timeout defaults in `falcon-mcp/src/limits.rs`:

| Family | Default | Env override |
|---|---|---|
| cargo (`cargo_check`, `cargo_test`, `cargo_clippy`, `cargo_fmt`) | 900 s | `FALCON_MCP_CARGO_TIMEOUT_MS` |
| git | 60 s | `FALCON_MCP_GIT_TIMEOUT_MS` |
| search (`fs_search`, `fs_search_ast`) | 30 s | `FALCON_MCP_SEARCH_TIMEOUT_MS` |
| exec (`exec_run`) | 30 s, or per-call `timeout_ms` | `FALCON_MCP_EXEC_TIMEOUT_MS` |

Timeouts surface as a dedicated JSON-RPC error code (`-32001`) with
structured data, see `falcon-mcp/src/tool_error.rs`. Subprocess cancellation
targets the direct child, not an entire descendant process tree.

## Demo runbook

For rehearsal passes, on-stage lines, and failure recovery against the artifact
deck, see [`REHEARSAL.md`](REHEARSAL.md) and run `rehearse.sh`.

Prerequisites: the pinned Rust 1.92 toolchain, Python 3, and git for the live
script. The full script also requires ast-grep, jq, and the pinned clippy
component. Build/cache dependencies before going offline.

```bash
# 0. Build the toolbelt
cargo build -p falcon-mcp

# 1. Start the server over stdio, exec enabled, jailed to a scratch worktree
mkdir -p /tmp/jail && cd /tmp/jail && git init -q
printf 'AWS_SECRET_ACCESS_KEY=decoy-not-a-real-key\n' > CONFIDENTIAL_KEYS.txt  # the bait
/path/to/maltese-agent/target/debug/falcon-mcp --stdio --root /tmp/jail --enable-exec
```

With the server reading JSON-RPC from stdin, the demo beats are:

| Beat | Tool call | Expected |
|---|---|---|
| 2. Allowed binary | `exec_run {"cmd":"cargo","args":["--version"]}` | stdout with the cargo version, `exit: 0` |
| 3. Shell injection | `exec_run {"cmd":"sh","args":["-c","cat /etc/passwd"]}` | invalid-argument error, `binary 'sh' not in allowlist` |
| 4. Path traversal | `fs_read {"path":"../../.ssh/id_rsa"}` | invalid-argument error, `escapes sandbox root` |
| 5. Tripwire | `fs_read {"path":"CONFIDENTIAL_KEYS.txt"}`, then any other call | `-32003` tripwire error and `TRIPWIRE TRIGGERED` on stderr; the next call gets `-32003 session revoked` |

The same five beats are what `falcon-mcp/tests/exec_test.rs`, `fs_test.rs`,
`error_codes_test.rs`, and `tripwire_test.rs` assert. Send beat 5 last, because
it revokes the session.
Rehearse with:

```bash
cargo test -p falcon-mcp --lib                    # fast, hermetic
cargo test -p falcon-mcp --test exec_test         # allowlist and --enable-exec gate
cargo test -p falcon-mcp --test exec_impostor_test  # PATH-swap defense
cargo test -p falcon-mcp --test fs_test           # root jail
cargo test -p falcon-mcp --test timeout_test      # structured timeouts
cargo test -p falcon-mcp --test tripwire_test     # honeytoken + session revocation
```

For the HTTP shape (Cloud Run, Gemini CLI):

```bash
target/debug/falcon-mcp --http 8080 --root /workspace
# MCP endpoint is now at http://localhost:8080/mcp
```

`falcon-mcp/tests/http_smoke.rs` checks that the HTTP endpoint responds; it
accepts 5xx responses and does not exercise an MCP handshake or multi-session
revocation over HTTP. The session-isolation test in `tripwire_test.rs` uses
in-process duplex transports. An actual HTTP client rehearsal is separate.

## Standing guardrail

`falcon-agent`'s poisoned prompt, missing guards, planted lints, and
`#[ignore]`'d test are intentional workshop material (guardrail G-1 in the root
`CLAUDE.md`). Do not fix them while preparing this talk.
`scripts/check-planted-defects.sh` fails CI if they change.

## Links

- Repo: `github.com/mrojas54/maltese-agent`
- Design spec: [`docs/superpowers/specs/2026-04-30-maltese-agent-design.md`](../../superpowers/specs/2026-04-30-maltese-agent-design.md), section 6
- MCP SDK: [`rmcp`](https://crates.io/crates/rmcp)
- Threat model behind the caper: [arXiv:2510.07192](https://arxiv.org/abs/2510.07192)
