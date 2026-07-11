# MA-26: Fix e2e cassette-staleness fast path and vitest worker flake

Fix-it ticket minted by orchestrator (run norm: flaky/slow default suite is a defect). Evidence: PR #41 TS job failed with vitest 'Timeout calling onTaskUpdate' unhandled error — tests/e2e.test.ts spends ~62s running the full cassette pipeline before detecting 'cassette miss / stale' and skipping; the synchronous pipeline blocks the vitest worker event loop past the worker-RPC timeout, failing runs whose tests all pass. Staleness is EXPECTED until MA-24 re-records, and MA-19 (genai 2.x) will stale cassettes for certain, so every remaining PR risks this flake. Fix: (1) cheap staleness pre-check (hash/fingerprint comparison) BEFORE launching the pipeline — stale => immediate skip (or fail under E2E_REQUIRED=1) in <1s; (2) ensure the e2e pipeline invocation does not block the worker event loop (async spawn, not execSync) so long runs cannot trip onTaskUpdate RPC timeouts when cassettes are fresh; (3) keep MA-2's guard semantics (skip vs E2E_REQUIRED=1 hard-fail, dirty-path naming) intact — e2e-guard tests must stay green. No prod code changes outside test harness/e2e plumbing. Serialize: e2e.test.ts (free), vitest config (free). ACs touched: AC-1..AC-5 must remain satisfied; G-1 untouched.

---

## Plan (delegator-ma-26, 2026-07-10)

### Where staleness detection lives today

- `falcon-detective/src/lib/gemini.ts`: cassette-mode `Gemini.call()` computes
  `hashKey(model, schemaName, normalizeForHash(prompt))` (sha256, 16 hex chars) and throws
  `no cassette for <key> (prompt hash)...` when `fixtures/cassettes/<key>.json` is absent.
- `falcon-detective/tests/e2e.test.ts`: launches the whole pipeline via **execFileSync**
  (`node dist/cli.js`), catches the subprocess stderr, and maps `/no cassette for [0-9a-f]+/`
  to skip (`e2e skipped: cassette miss — cassettes are stale relative to the workflow code ...`)
  or fail under E2E_REQUIRED=1 (`E2E_REQUIRED=1 but the e2e could not run: <reason>`).
- The first Gemini call is inside the `triage` handler, whose prompt embeds
  `cargo check`/`cargo test`/`cargo clippy` output from a fresh `.runs/` worktree — that is
  the ~62s burned before the miss surfaces, all while `execFileSync` blocks the vitest
  worker event loop (hence the `Timeout calling "onTaskUpdate"` worker-RPC death).

### What the pre-check hoists

The exact cassette key cannot be recomputed cheaply (the triage prompt requires the cargo
runs). What CAN be hoisted is the **staleness verdict**, via a fingerprint manifest
recorded alongside the cassettes:

- New `falcon-detective/tests/helpers/e2eFingerprint.ts`:
  - `computeWorkflowFingerprint()` → components `{ model, workflow, seed }`:
    - `model`: `DEFAULT_MODEL` (part of every cassette key),
    - `workflow`: sha256 over sorted (relative path + bytes) of `falcon-detective/src/**`
      and `falcon-detective/prompts/**` (prompt templates + all prompt-building code,
      including `normalizeForHash` itself),
    - `seed`: `git rev-parse HEAD:falcon-agent` tree hash (the seed crate whose cargo
      output feeds the triage prompt; the MA-2 dirty guard already ensures the working
      tree matches HEAD).
  - `write/readFingerprintManifest()` → `fixtures/cassettes/.e2e-fingerprint.json`.
  - `checkCassetteFreshness()` → `GuardOutcome`: any component drift, or a missing/corrupt
    manifest, ⇒ stale ⇒ skip (fail under required) with the SAME message prefix as today
    (`cassette miss — cassettes are stale relative to the workflow code (...); re-record`).
- `tests/e2e.test.ts` runs this check right after `checkE2ePrereqs`, BEFORE launching the
  pipeline. The in-pipeline `/no cassette for/` catch stays as the authoritative backstop
  (an imperfect fingerprint match falls through to today's behavior, now async).
- No manifest exists today (cassettes are stale — that is this ticket's evidence), so the
  missing-manifest ⇒ stale rule gives the immediate <1s skip in CI right now. New npm
  script `e2e:fingerprint` (tsx `scripts/write-e2e-fingerprint.ts`) writes the manifest;
  the MA-24 re-record flow becomes: `GEMINI_MODE=record ...` + `npm run e2e:fingerprint` +
  `git add fixtures/cassettes/` (README updated).
- `hasCassettes()` in `e2ePrereqs.ts` learns to ignore dotfiles so the manifest alone never
  satisfies the cassette-presence guard.
- `applyGuardOutcome()` gains an optional fail-prefix param so the stale fast path throws
  today's exact `E2E_REQUIRED=1 but the e2e could not run: ` message while MA-2 prereq
  failures keep their `... prerequisites are unmet: ` message.

### Which sync calls become async

In `tests/e2e.test.ts` only (test callback becomes `async`):
1. `execFileSync("node", [dist/cli.js ...])` → `await promisify(execFile)(...)` with a
   large `maxBuffer` — the 62s (or legitimately long fresh-cassette) pipeline run no
   longer blocks the worker event loop.
2. `execFileSync("cargo", ["test" ... smoking-gun ...])` → same async treatment.

The guard's `git status --porcelain` / `git rev-parse` and the fingerprint file hashing
stay synchronous — they are millisecond-scale and cannot trip the RPC timeout.

Zero changes under `falcon-detective/src/` (scope fence honored; async needs none).
No changes to `.github/workflows/ci.yml`, `vitest.config.ts`, or `falcon-agent/` (G-1).

### Test coverage added (tests/e2e-guard.test.ts)

- fresh manifest ⇒ `run`; missing manifest ⇒ skip with stale message; missing + required
  ⇒ fail with `E2E_REQUIRED=1 but the e2e could not run:` prefix.
- drift in each component (workflow file edit, seed commit, model change) ⇒ stale, reason
  names the drifted component(s); corrupt manifest ⇒ stale, no crash.
- manifest alone does not count as a cassette (`hasCassettes` dotfile rule).
- blocked-loop regression: `tests/e2e.test.ts` source must contain no
  `execFileSync`/`execSync`/`spawnSync` tokens.
- All existing AC-1..AC-4 guard tests unchanged and green.

### Gates

`npm ci`; `npm test` (fast, hermetic); `cargo build -p falcon-mcp` then
`npm run test:full` with timing output proving the stale e2e skips in <1s (before: ~62s per
CI run 29103569277 / job 86398307246); `npx tsc --noEmit`; `npm run lint` (biome) clean.

Commit: `fix(falcon-detective): e2e cassette-staleness fast path + async pipeline spawn (MA-26)`
