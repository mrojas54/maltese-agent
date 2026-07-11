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

`CASSETTE_DIR` defaults to `fixtures/cassettes` inside this package — anchored
to the package itself (resolved from `src/lib/gemini.ts`'s own location), not
the spawning cwd, so a worker spawned from the repo root resolves the same
directory. Set `CASSETTE_DIR` to override.

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

1. Rebuilds `dist/` (`tsc -p tsconfig.build.json`) in its `beforeAll` so the
   `dist/cli.js` it invokes always matches `src/` — recording runs tsx against
   source, so nothing else guarantees dist freshness at test time — and
   removes any `.runs/e2e-test` worktree a previous failed/timed-out run left
   behind (a leftover otherwise kills the run with "worktree already exists").
   The run worktree is also cleaned up after the test; set `KEEP_RUN=1` to
   preserve it for post-mortem debugging of a failure.
2. Guards its prerequisites: if `falcon-agent/` has uncommitted changes, the
   test **skips** with a message naming the dirty paths (it does *not* reset
   anything — commit or stash, then rerun); same skip for a missing
   `target/debug/falcon-mcp` binary or empty `fixtures/cassettes/`
3. Checks cassette freshness **before** launching the pipeline: the recorded
   fingerprint manifest (`fixtures/cassettes/.e2e-fingerprint.json`, written
   by `npm run e2e:fingerprint` at record time) is compared against the
   current workflow code, prompt templates, model id, and committed
   `falcon-agent/` tree. Stale (or unproven) cassettes **skip in under a
   second** instead of burning ~60s of pipeline work discovering the miss —
   and the long-running pipeline/smoke subprocesses are spawned
   asynchronously so a legitimate full run cannot block the vitest worker
   event loop (the 2026-07 `Timeout calling "onTaskUpdate"` CI flake)
4. Runs the CLI in `cassette` mode against the recorded fixtures — all
   mutation happens inside the pipeline's own `prepWorktree` sandbox at
   `<repo-root>/.runs/e2e-test`
5. Asserts the previously-`#[ignore]`'d cargo test `bird_themed_inputs_arent_special` passes inside the per-run worktree — the gate that proves the poison is gone

With `E2E_REQUIRED=1` (set in CI's typescript job) every skip above becomes a
**failure**, so a green required run mechanically implies the e2e executed.
The guard behaviors themselves are proven hermetically by
`tests/e2e-guard.test.ts`, which runs in the fast `npm test` suite.

## Re-recording cassettes (operator flow)

Re-record when the e2e reports a cassette miss (`no cassette for <hash>` —
the workflow code or prompts drifted since the last recording) or after an
SDK/model upgrade changes request shapes. Recording is the only operation in
this repo that spends real API money, so it is operator-driven
(BUILDPLAN MA-H-1): one command, small spend, everything else offline.

All commands below run from inside `falcon-detective/`.

### 1. Prerequisites

```bash
npm ci                                 # this package's deps
(cd .. && cargo build -p falcon-mcp)   # the pipeline spawns this binary
```

`falcon-agent/` must have **no uncommitted changes** — cassettes must capture
the canonical seed state. The script checks this and refuses to run otherwise.
Like the e2e, recording never mutates your checkout: all pipeline edits happen
inside the per-run git worktree at `<repo-root>/.runs/<run-name>`.

### 2. Sanity-check the plan (no key, no network)

```bash
npm run record -- --dry-run
```

Prints the recording plan — which handlers call Gemini, with which model,
prompt template, and schema — plus the resolved cassette dir and the cassettes
currently on disk. Makes **no network calls** and needs no key.

### 3. Record

```bash
GEMINI_API_KEY=... npm run record
```

The only required env var is `GEMINI_API_KEY`
(<https://aistudio.google.com/apikey>). The script fails fast with an
instruction — before any network or pipeline work — if the key is missing,
`falcon-agent/` is dirty, or `falcon-mcp` isn't built.

**Expected spend: small (cents).** The canonical run makes ~5 Gemini calls —
3× `gemini-3.1-pro-preview` (triage, poison analysis, poison fix) and 2×
`gemini-3.1-flash-lite` (one per lint fix), with prompts of a few KB each.
Schema-validation retries can at most triple the call count; the total stays
well under a dollar. (Model IDs live in [`src/lib/models.ts`](src/lib/models.ts);
the 2.5 family was retired for new API users in 2026-07 — see MA-27.)

**Expected output: ~5 cassettes** in `fixtures/cassettes/`. Filenames are
content-addressed (`sha256(model + schema + normalized prompt)[0:16].json`),
so they change only when a prompt, model, or schema changes. The set below
was recorded under the retired `gemini-2.5` IDs, so it is **stale** until the
MA-24 re-record: the model is a hash component, so every filename changes,
and the record run will report these files as untouched leftovers to delete.

| File | Produced by | Content |
|---|---|---|
| `7a84e87717a4de35.json` | `triage` | Issue list: 1 poison, 2 lints |
| `d23b82866c943eb9.json` | `analyzePoison` | Analysis of the poisoned few-shot example in `prompt.rs` |
| `1581f1e49bada42a.json` | `proposePoisonFix` | Diff removing the poisoned example from `prompt.rs` |
| `0f402318c24c0df4.json` | `proposeLintFix` | Diff for the planted unused-import lint in `interrogate.rs` |
| `958680ee55ceefb3.json` | `proposeLintFix` | Diff for the `default_constructed_unit_structs` lint in `main.rs` |

(`proposeBugFix` is also a recordable call site, but the canonical seed
triages to no `bug`-kind issues, so it contributes no cassette today.)

After the run the script reports created / updated / untouched files.
"Untouched" files whose hashes were not re-emitted are stale leftovers —
delete them.

### 4. Write the e2e fingerprint manifest

```bash
npm run e2e:fingerprint
```

`npm run e2e:fingerprint` writes
`fixtures/cassettes/.e2e-fingerprint.json`, capturing what the cassettes were
recorded against (workflow code + prompts content hash, model id, committed
`falcon-agent/` tree hash). The e2e compares it before launching the pipeline
and skips fast when anything drifted; **without the manifest the e2e treats
the cassettes as stale**, so always run it after recording.

### 5. Verify and commit

```bash
npm run test:full -- tests/e2e.test.ts          # replays offline (cassette mode)
git add fixtures/cassettes/                     # cassettes + .e2e-fingerprint.json
git -C .. worktree remove --force .runs/<run-name>   # cleanup
```

Prompt hashing in [`src/lib/gemini.ts`](src/lib/gemini.ts)
(`normalizeForHash`) strips volatile substrings — usernames, tmpdirs, cargo
timing, test ordering — before hashing, so cosmetic run-to-run differences
don't invalidate cassettes.

## Design

See [`docs/superpowers/specs/2026-04-30-maltese-agent-design.md`](../docs/superpowers/specs/2026-04-30-maltese-agent-design.md) §5 for the full spec, or [`docs/superpowers/plans/2026-05-01-falcon-detective-implementation.md`](../docs/superpowers/plans/2026-05-01-falcon-detective-implementation.md) for the 16-task implementation plan.
