# The Harness Wore a Trenchcoat

*Building a Sandboxed Rust MCP Server* — a talk for the **Women in Rust Meetup**.

Speaker: Michelle Rojas. Theme: noir. Deck source: [`deckhand.json`](../../../deckhand.json) at the repo root.

This README is the companion to the slides. It maps every claim in the deck to
the code in `falcon-mcp/` that backs it, gives a runbook for the live demo, and
is honest about the two places where the deck says more than the code does.

---

## The pitch in one paragraph

An LLM agent that can touch a filesystem and run commands is only as safe as the
thing standing between it and the host. Prompts are soft guardrails. They ask
nicely and fail under adversarial pressure. `falcon-mcp` is a hard control plane:
a single Rust binary speaking the Model Context Protocol (MCP, JSON-RPC 2.0) over
stdio or HTTP, with a root-jailed filesystem, an allowlisted exec surface, and a
wall-clock timeout on every subprocess. The harness is the detective. It wears the
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

The naive shape is `LLM -> raw shell`. Nothing in this repo does that. Every tool
call enters through `FalconMcp` in `falcon-mcp/src/server.rs` and is routed to a
handler that takes a `Sandbox`, never a bare path or a bare command.

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

- `exec_run` is not even registered unless the server starts with
  `--enable-exec` (`falcon-mcp/src/tools/exec.rs`, `main.rs`).
- Commands are spawned directly via `tokio::process::Command`. There is no
  subshell, so `sh -c '...'` is rejected as the binary `sh`, and shell
  metacharacters in arguments are inert.

Proof: `exec_disabled_by_default`, `exec_rejects_non_allowlisted_binary` in
`falcon-mcp/tests/exec_test.rs`; the PATH-swap case in
`falcon-mcp/tests/exec_impostor_test.rs`.

### 6. The hard tripwire: honeytokens and canaries

**This slide is narrative, not implementation.** There is no
`CONFIDENTIAL_KEYS.txt` decoy and no session-revoking tripwire in `falcon-mcp`
today. The word "canary" in this repo refers to two other things: a test in
`fs_test.rs` that plants a literal line to prove search matches it, and the
`#[ignore]`'d smoking-gun test in `falcon-agent` that the detective is supposed
to un-ignore.

Present it as the design direction and say so. The hard guards that *do* exist
and that the slide's thesis rests on are: read-only mode (`--read-only`, checked
by `Sandbox::check_writable` before every mutating tool), the root jail, the
allowlist, and the timeouts. If you want the demo to end on an alert instead of
a refusal, that is a small feature to build before the talk, and this README
should be updated when it lands.

### 7. Live demo flow

See the runbook below. Each step maps to an existing integration test, so the
demo can be rehearsed by running the suite.

### 8. Case closed

The three takeaways are all true of the code as shipped:

- Tool surfaces are security boundaries: every handler receives an `Arc<Sandbox>`.
- Hard control planes over soft prompt guardrails: no guard in `falcon-mcp` is a
  prompt.
- One Rust binary, two transports: `main.rs` picks stdio or HTTP from a flag.

## Where the deck and the code disagree

Two corrections to make on stage or in the next revision of `deckhand.json`:

1. **Slide 3 says "Tokio execution timeouts (10s)."** The real defaults in
   `falcon-mcp/src/limits.rs` are per tool family, and cargo's is deliberately
   long because cold builds of tokio + axum + hyper exceed five minutes:

   | Family | Default | Env override |
   |---|---|---|
   | cargo (`cargo_check`, `cargo_test`, `cargo_clippy`, `cargo_fmt`) | 900 s | `FALCON_MCP_CARGO_TIMEOUT_MS` |
   | git | 60 s | `FALCON_MCP_GIT_TIMEOUT_MS` |
   | search (`fs_search`, `fs_search_ast`) | 30 s | `FALCON_MCP_SEARCH_TIMEOUT_MS` |
   | exec (`exec_run`) | 30 s, or per-call `timeout_ms` | `FALCON_MCP_EXEC_TIMEOUT_MS` |

   Timeouts surface as a dedicated JSON-RPC error code (`-32001`) with
   structured data, see `falcon-mcp/src/tool_error.rs`.

2. **Slide 6's honeytoken tripwire is not implemented.** See above.

One smaller note: the deck's demo step 1 says "maltese-agent and falcon-mcp".
`maltese-agent` is the repo; the client that actually connects over stdio is
`falcon-detective` (`falcon-detective/src/lib/mcp.ts`).

## Demo runbook

Prerequisites: the pinned toolchain in `rust-toolchain.toml` (Rust 1.92) and
`ast-grep` on PATH if you want `fs_search_ast` to work.

```bash
# 0. Build the toolbelt
cargo build -p falcon-mcp

# 1. Start the server over stdio, exec enabled, jailed to a scratch worktree
mkdir -p /tmp/jail && cd /tmp/jail && git init -q
/path/to/maltese-agent/target/debug/falcon-mcp --stdio --root /tmp/jail --enable-exec
```

With the server reading JSON-RPC from stdin, the demo beats are:

| Beat | Tool call | Expected |
|---|---|---|
| 2. Allowed binary | `exec_run {"cmd":"cargo","args":["--version"]}` | stdout with the cargo version, `exit: 0` |
| 3. Shell injection | `exec_run {"cmd":"sh","args":["-c","cat /etc/passwd"]}` | invalid-argument error, `binary 'sh' not in allowlist` |
| 4. Path traversal | `fs_read {"path":"../../.ssh/id_rsa"}` | invalid-argument error, `escapes sandbox root` |
| 5. Tripwire | not implemented, see slide 6 | show `--read-only` refusing `fs_write` instead |

The same five beats, minus the tripwire, are what
`falcon-mcp/tests/exec_test.rs`, `fs_test.rs`, and `error_codes_test.rs` assert.
Rehearse with:

```bash
cargo test -p falcon-mcp --lib                    # fast, hermetic
cargo test -p falcon-mcp --test exec_test         # allowlist and --enable-exec gate
cargo test -p falcon-mcp --test exec_impostor_test  # PATH-swap defense
cargo test -p falcon-mcp --test fs_test           # root jail
cargo test -p falcon-mcp --test timeout_test      # structured timeouts
```

For the HTTP shape (Cloud Run, Gemini CLI):

```bash
target/debug/falcon-mcp --http 8080 --root /workspace
# MCP endpoint is now at http://localhost:8080/mcp
```

`falcon-mcp/tests/http_smoke.rs` is the smoke test for this path.

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
