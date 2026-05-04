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
npm test                            # full suite
npm test -- tests/e2e.test.ts       # just the end-to-end
```

The e2e:

1. Resets `falcon-agent/` in the parent repo to HEAD (the broken state)
2. Runs the CLI in `cassette` mode against the recorded fixtures
3. Asserts the previously-`#[ignore]`'d cargo test `bird_themed_inputs_arent_special` passes inside the per-run worktree — the gate that proves the poison is gone

It skips gracefully if `target/debug/falcon-mcp` is absent or `fixtures/cassettes/` is empty.

## Re-recording cassettes

If a workflow change drifts the prompt hash:

```bash
GEMINI_MODE=record GEMINI_API_KEY=... \
  npm run fix -- --target ../falcon-agent --run-name record-$(date +%s)

git add fixtures/cassettes/
```

Prompt hashing in [`src/lib/gemini.ts`](src/lib/gemini.ts) normalizes whitespace and timestamp-like strings before hashing, so cosmetic prompt edits don't invalidate the cache.

## Design

See [`docs/superpowers/specs/2026-04-30-maltese-agent-design.md`](../docs/superpowers/specs/2026-04-30-maltese-agent-design.md) §5 for the full spec, or [`docs/superpowers/plans/2026-05-01-falcon-detective-implementation.md`](../docs/superpowers/plans/2026-05-01-falcon-detective-implementation.md) for the 16-task implementation plan.
