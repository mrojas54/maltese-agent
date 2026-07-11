# MA-20: Cassette recording script and operator flow docs

BUILDPLAN M4 / WS-13b (BUILDPLAN id MA-20). scripts/record-cassettes.ts plus npm run record plus a README operator-flow section (GEMINI_API_KEY usage, expected small spend, file list of about 5 cassettes). Enables AC-28, which the operator proves at the human-track ticket. Depends: MA-19. Size S.

## Context (read from base @ fb19319, MA-19 in review as PR #46)

- Cassette mechanics (`src/lib/gemini.ts`): key = `sha256(model + zod-schema constructor name + normalizeForHash(prompt))[0:16]`; `GEMINI_MODE=record` makes `Gemini.call` write `fixtures/cassettes/<key>.json` after a schema-valid live response; replay (`cassette` mode) is SDK-independent. `CASSETTE_DIR` env overrides the cwd-relative default.
- Exactly five Gemini call sites exist, all via `promptFromFile`:
  1. `triage` — DEFAULT_MODEL (gemini-2.5-pro), `prompts/triage.md`, TriageOutput — 1 call/run
  2. `analyzePoison` — GEMINI_PRO, `prompts/analyze-poison.md`, PoisonReport — 1 per poison issue
  3. `proposePoisonFix` — GEMINI_PRO, `prompts/propose-poison-fix.md`, UnifiedDiff — 1 per poison issue
  4. `proposeBugFix` — GEMINI_PRO, `prompts/propose-bug-fix.md`, UnifiedDiff — 1 per bug issue
  5. `proposeLintFix` — GEMINI_FLASH, `prompts/propose-lint-fix.md`, UnifiedDiff — 1 per lint issue
- The canonical falcon-agent run triages to 1 poison + 2 lint issues (0 bugs), yielding 5 cassettes / ~5 calls (3× pro, 2× flash). Current on-disk set: `7a84e87717a4de35.json` (triage), `d23b82866c943eb9.json` (analyzePoison), `1581f1e49bada42a.json` (poison fix, prompt.rs), `0f402318c24c0df4.json` (lint fix, interrogate.rs unused import), `958680ee55ceefb3.json` (lint fix, main.rs default_constructed_unit_structs). proposeBugFix is a call site but contributes no cassette today.
- e2e (`tests/e2e.test.ts`) consumes the cassettes; staleness vs workflow code surfaces there as "no cassette for <hash>". CI note: E2E_REQUIRED flips to 1 only after the MA-24 human re-record.
- `tsconfig.json` includes only `src/**/*` with `rootDir: ./src` — a script under `scripts/` is outside typecheck scope today, and widening `include` naively would change the `dist/` layout that `bin` and the e2e depend on.

## Changes (all inside falcon-detective/)

1. **`scripts/record-cassettes.ts`** (new)
   - Arg parsing (same minimal style as `cli.ts`): `--dry-run` flag; `--run-name` (default `record-<epoch>`), `--target` (default `falcon-agent`), `--repo-root` (default package parent), `--mcp-binary` (default `<repo-root>/target/debug/falcon-mcp`), `--cassette-dir` (default `<pkg>/fixtures/cassettes`). All paths computed from the script location, not cwd.
   - **Recording-plan table** (static data, imported model constants from `src/lib/models.js` — keeps AC-22's single-source rule): handler, model, prompt template, schema, expected calls.
   - **Drift guard**: scan `src/handlers/*.ts` for `promptFromFile("...")` and fail (both modes) if the set differs from the plan's templates, or if a listed template file is missing — the plan table cannot silently rot.
   - **`--dry-run`**: no network, no `GEMINI_API_KEY` needed. Prints the plan, the content-addressed keying rule, the resolved cassette dir + env, and the existing on-disk cassettes. Exit 0.
   - **Record mode**: fail fast with an operator-facing message if `GEMINI_API_KEY` is unset (before any other work). Preflight: `falcon-agent/` clean (reuse `dirtyPaths` from `tests/helpers/e2ePrereqs.js`), mcp binary built. Snapshot cassette dir (sha256 per file), set `GEMINI_MODE=record` + `CASSETTE_DIR`, dynamically import `runPipeline` + `detective` and run in-process (same initial input shape as `cli.ts`). Afterwards diff the snapshot and report new / updated / unchanged / leftover-stale files, plus next steps (replay e2e, `git add`, `.runs/<run-name>` cleanup hint).
2. **`package.json`** (scripts section — owned this wave): add `"record": "tsx scripts/record-cassettes.ts"`; change `"build"` to `tsc -p tsconfig.build.json`.
3. **`tsconfig.json` / `tsconfig.build.json`** (new): base config becomes the check config (`noEmit: true`, include `src/**/*` + `scripts/**/*`); `tsconfig.build.json` extends it with `noEmit: false`, `outDir/rootDir/declaration`, include `src/**/*` only. `npm run build` output layout (`dist/cli.js`) unchanged; `npx tsc --noEmit` now covers the script.
4. **`README.md`**: replace the "Re-recording cassettes" section with the WS-13b operator flow (allowed by N-6): prerequisites, `--dry-run` sanity step, one-command `GEMINI_API_KEY=… npm run record`, expected small spend (~5 calls: 4× gemini-2.5-pro + 1× gemini-2.5-flash — typically cents, well under $1 even with schema retries), the ~5-cassette file list with roles, replay verification, commit step.

## Explicitly NOT doing

- No live recording run (AC-28 is proven by the operator at MA-24); existing cassettes untouched.
- No falcon-agent changes (G-1). No dependency changes (MA-19 owns deps). No other README prose (N-6).

## Gates (in falcon-detective/)

`npm ci`; `npm test` green; `npm run build` still emits `dist/cli.js`; `npx tsc --noEmit` (script in scope); `npm run lint` (biome) clean; `npm run record -- --dry-run` output captured for PR body; keyless `npm run record` shows the fail-fast message; `git status` proves `fixtures/cassettes/` untouched.

## Reset 2026-07-10 by agent:delegator-ma-20
