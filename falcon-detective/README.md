# falcon-detective

The Barnum workflow that triages, fixes, and verifies bugs in `falcon-agent`. Composes Gemini reasoning with `falcon-mcp` tool calls (cargo, clippy, git, fs).

## Setup

Requires Node 18+ (uses `node:`-prefixed imports and ESM).

```bash
npm install
npm run build
```

`falcon-mcp` must also be built (the workflow spawns it as a subprocess):

```bash
cargo build -p falcon-mcp        # from repo root
```

## Three modes

The `GEMINI_MODE` env var picks how Gemini calls are resolved:

| Mode | Behavior | Required env |
|---|---|---|
| `live` (default) | Real Gemini API calls | `GEMINI_API_KEY` |
| `record` | Live calls, but each request/response is written to `CASSETTE_DIR` | `GEMINI_API_KEY` |
| `cassette` | Replays from `CASSETTE_DIR` — fully offline, deterministic | — |

`CASSETTE_DIR` defaults to `./fixtures/cassettes` resolved from the spawning cwd.

## Run

From inside `falcon-detective/`:

```bash
# Live (needs key)
GEMINI_API_KEY=... npm run fix -- --target ../falcon-agent --run-name demo

# Replay (offline)
GEMINI_MODE=cassette npm run fix -- --target ../falcon-agent --run-name demo
```

From the repo root (what the e2e does — matches `--repo-root` and `--mcp-binary` to absolute paths):

```bash
node falcon-detective/dist/cli.js \
  --target falcon-agent \
  --run-name demo \
  --repo-root "$(pwd)" \
  --mcp-binary "$(pwd)/target/debug/falcon-mcp"
```

### CLI flags

| Flag | Default | Notes |
|---|---|---|
| `--target` | required | Path to the broken crate (`falcon-agent`) |
| `--run-name` | `demo-<ts>` | Becomes the per-run worktree name under `.runs/` |
| `--repo-root` | `..` | Resolved against the spawning cwd |
| `--mcp-binary` | `../target/debug/falcon-mcp` | Path to the built MCP server |

The workflow's first handler (`prepWorktree`) creates a fresh git worktree at `<repo-root>/.runs/<run-name>` so the broken-state checkout, agent edits, and per-issue commits never touch the parent repo.

## Tests

```bash
npm test                                     # fast hermetic suite (default)
npm run test:full                            # full suite, including the e2e
npm run test:full -- tests/e2e.test.ts       # just the end-to-end
```

`npm test` is the inner loop: it excludes `tests/e2e.test.ts` and pins
`FALCON_MCP_BIN` to a nonexistent path so nothing spawns the `falcon-mcp`
binary or reads recorded cassettes — it needs no `cargo build` and finishes in
seconds. The binary-gated tests (`tests/lib/mcp.test.ts`,
`tests/handlers/prepWorktree.test.ts`) exercise their spawn paths under
`npm run test:full` when `target/debug/falcon-mcp` is built.

The e2e **never mutates your checkout**. It:

1. Guards its prerequisites: if `falcon-agent/` has uncommitted changes, the
   test **skips** with a message naming the dirty paths (it does *not* reset
   anything — commit or stash, then rerun); same skip for a missing
   `target/debug/falcon-mcp` binary or empty `fixtures/cassettes/`
2. Checks cassette freshness **before** launching the pipeline: the recorded
   fingerprint manifest (`fixtures/cassettes/.e2e-fingerprint.json`, written
   by `npm run e2e:fingerprint` at record time) is compared against the
   current workflow code, prompt templates, model id, and committed
   `falcon-agent/` tree. Stale (or unproven) cassettes **skip in under a
   second** instead of burning ~60s of pipeline work discovering the miss —
   and the long-running pipeline/smoke subprocesses are spawned
   asynchronously so a legitimate full run cannot block the vitest worker
   event loop (the 2026-07 `Timeout calling "onTaskUpdate"` CI flake)
3. Runs the CLI in `cassette` mode against the recorded fixtures — all
   mutation happens inside the pipeline's own `prepWorktree` sandbox at
   `<repo-root>/.runs/e2e-test`
4. Asserts the previously-`#[ignore]`'d cargo test `bird_themed_inputs_arent_special` passes inside the per-run worktree — the gate that proves the poison is gone

With `E2E_REQUIRED=1` (set in CI's typescript job) every skip above becomes a
**failure**, so a green required run mechanically implies the e2e executed.
The guard behaviors themselves are proven hermetically by
`tests/e2e-guard.test.ts`, which runs in the fast `npm test` suite.

## Re-recording cassettes

If a workflow change drifts the prompt hash:

```bash
GEMINI_MODE=record GEMINI_API_KEY=... \
  npm run fix -- --target ../falcon-agent --run-name record-$(date +%s)

npm run e2e:fingerprint
git add fixtures/cassettes/
```

`npm run e2e:fingerprint` writes
`fixtures/cassettes/.e2e-fingerprint.json`, capturing what the cassettes were
recorded against (workflow code + prompts content hash, model id, committed
`falcon-agent/` tree hash). The e2e compares it before launching the pipeline
and skips fast when anything drifted; **without the manifest the e2e treats
the cassettes as stale**, so always run it after recording.

Prompt hashing in [`src/lib/gemini.ts`](src/lib/gemini.ts) normalizes whitespace and timestamp-like strings before hashing, so cosmetic prompt edits don't invalidate the cache.

## Design

See [`docs/superpowers/specs/2026-04-30-maltese-agent-design.md`](../docs/superpowers/specs/2026-04-30-maltese-agent-design.md) §5 for the full spec, or [`docs/superpowers/plans/2026-05-01-falcon-detective-implementation.md`](../docs/superpowers/plans/2026-05-01-falcon-detective-implementation.md) for the 16-task implementation plan.
