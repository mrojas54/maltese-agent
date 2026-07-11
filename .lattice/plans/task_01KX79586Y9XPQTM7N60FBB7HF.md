# MA-29 Plan — Harden e2e harness (dist freshness, cleanup, timeout, cassetteDir anchor)

Branch `claude/ma-29-e2e-harden` off origin/main@e81fba0. All edits in
`falcon-detective/` (G-1: falcon-agent untouched). Four defects, four fixes.

## Defect 1 — stale dist

`tests/e2e.test.ts` invokes `falcon-detective/dist/cli.js` without ensuring it is
built; `npm run record` runs tsx (source). Observed: pre-MA-27 dist (gemini-2.5-pro)
replayed against 3.1-keyed cassettes → bogus "cassette miss".

**Fix**: `beforeAll` in e2e.test.ts runs the build via the existing `execFileAsync`
(`node node_modules/typescript/bin/tsc -p tsconfig.build.json`, cwd = package root,
`MAX_BUFFER`, explicit hook timeout 180s). `node + typescript/bin/tsc` avoids npx
resolution and exec-bit assumptions. Fast suite (`npm test`) excludes e2e.test.ts
entirely, so the hook never runs there.

## Defect 2 — worktree leak

Failed/timed-out runs leave `<repo-root>/.runs/e2e-test`; the next run dies on
"worktree already exists", masking the real error.

**Fix**: new helper `tests/helpers/e2eCleanup.ts` → `removeRunWorktree(repoRoot,
runName)`: if the dir exists, `git -C <repoRoot> worktree remove --force <path>`,
falling back to `rm -rf` when it is not a registered worktree; then always
`git worktree prune` (clears stale registrations even when the dir is already gone).
Async (promisified execFile) — consistent with the MA-26 no-sync-exec discipline.
Called from `beforeAll` (pre-clean) and `afterAll` (always clean, unless
`KEEP_RUN=1`, documented in the test comment for post-mortem debugging).

**Guard-test extension** (cheap, hermetic, no pipelines): e2e-guard.test.ts gets a
`describe` covering: registered leftover worktree removed + re-addable; bare
unregistered dir removed via fallback; no-op when absent; stale registration
(dir rm'd manually) pruned so `worktree add` works again. Fixture worktrees use
`--detach` to avoid branch-name collisions.

## Defect 3 — timeout

Healthy replay ≈ 114 s pipeline + post-pipeline cargo smoke; the 120 000 ms
per-test timeout races and loses.

**Fix**: per-test timeout 120_000 → 360_000 in e2e.test.ts only. vitest.config.ts
(`testTimeout: 30_000`) and the fast suite stay untouched.

## Defect 4 — cassetteDir cwd fallback

`src/lib/gemini.ts` `cassetteDir()` falls back to `join(process.cwd(),
"fixtures/cassettes")`; a CLI worker spawned from repo root silently resolves a
nonexistent dir (footgun documented in e2e.test.ts:75-82).

**Fix**: module-level `PACKAGE_ROOT = join(dirname(fileURLToPath(import.meta.url)),
"..", "..")` — correct from both `src/lib/` (tsx) and `dist/lib/` (compiled; dist
mirrors src via rootDir/outDir). `cassetteDir()` keeps call-time resolution and
`CASSETTE_DIR` env priority (tests mutate env in beforeEach — gemini.ts:173
comment preserved). e2e.test.ts comment updated; the explicit `CASSETTE_DIR` env
there is KEPT (defense in depth). README line 30 (documents the old cwd fallback)
corrected.

## Fingerprint interplay (flagged, not fixed here)

The MA-26 manifest hashes all of `falcon-detective/src/**`; the gemini.ts edit
drifts the `workflow` component. The fresh (Jul 10) cassettes and
`.e2e-fingerprint.json` are **uncommitted on the primary checkout** (not in
origin/main). Landing them is MA-24's/operator's flow, out of MA-29 scope —
**after MA-29 merges, rerun `npm run e2e:fingerprint` before committing them**
(cassette keys are unaffected: the edit touches no prompt construction/hashing).
This is called out in the PR body and a ticket comment.

## Verification

- Gates in `falcon-detective/`: `npm ci`, `npm test` (fast, e2e excluded),
  `npx tsc --noEmit`, `npm run lint` (biome), e2e-guard green.
- Worktree root: `scripts/check-planted-defects.sh` exit 0.
- Real e2e proof: copy primary's untracked fresh cassettes + manifest into the
  worktree cassette dir (verification-only, never committed), copy primary's
  `target/debug/falcon-mcp` (base→primary diff on src/prompts/falcon-agent is
  empty, so artifacts match this source), regenerate the manifest locally after
  the src edit, run `npm run test:full -- tests/e2e.test.ts`. Afterwards remove
  the borrowed artifacts. Fallback if the replay is env-blocked: fast suite +
  guard tests + dry assertion of the new code paths.

## Commit

`fix(falcon-detective): harden e2e harness — dist freshness, cleanup, timeout,
cassetteDir anchor (MA-29)` — single commit; then own-review, push, PR to main.
