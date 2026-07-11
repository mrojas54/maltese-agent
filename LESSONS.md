# LESSONS — maltese-agent

Timeless findings from the 2026-07-10 tech-debt remediation run (contract 9f72697, tickets MA-1..MA-34).
Format: failure mode → why it matters → fix. Full war-story detail: `.lattice/orchestration/run-state.md`.

## Cassette record/replay integrity

- **Replay must execute the same code that recorded.** The e2e ran stale `dist/cli.js` (old Gemini model ID hashed into every cassette key) while recording ran tsx source — 100% key misses with brand-new cassettes. Fix: e2e builds dist in `beforeAll` (MA-29). Diagnostic order for any miss: `GEMINI_DEBUG_PROMPTS=1`, then check the `model=` line of the normalized prompt dump FIRST.
- **Environment parity is prompt parity.** CI runners lacked ripgrep → `fs_search` threw → a silent catch degraded the ignored-test poison signal to `[]` → a *different* triage prompt → every cassette key missed, in CI only. Fix: install rg in CI, and every failure that feeds prompt content logs loudly (`console.error` naming cassette-key divergence). Any silent catch upstream of an LLM prompt is a replay-integrity landmine.
- **Recording leftovers block the next pull.** Cassettes recorded on the operator machine become tracked when their PR merges; the local untracked duplicates then abort `git pull`. Clean them the moment the landing PR merges. And verify the tree SHA before any demo/checkpoint drive — a failed pull followed by chained commands silently drives a stale tree.
- **Never trust model-declared hunk counts.** Gemini 3.1 emits `@@` headers counting full context while truncating trailing context lines in the body; the Rust `patch` crate panics. `normalize_hunk_counts` recounts from the body before parse (MA-28); the offending recorded diff is the regression fixture.

## Gemini / model lifecycle

- **ListModels lies about availability.** gemini-2.5-pro stayed listed after generateContent began returning 404 "not available to new API users." Probe with a tiny generateContent before trusting any model ID. Centralized model IDs (`src/lib/models.ts`, MA-8) made the forced migration a one-file change.
- **Model swaps shift judgment, not just APIs.** 3.1 finds 2 of the canonical 3 planted issues (the main.rs clippy-only lint never reaches it — see MA-34 for the pipeline-signal half of that story). Re-recording cassettes re-records the *story*; budget a felt review, not just a diff.

## Dependency policy

- **Majors never ride a bulk merge.** Three untested major bumps (rmcp 2.x, TypeScript 7, Biome 2.x) merged in a 13-PR Dependabot sweep broke main compile/lint mid-run; every open PR inherited the red. Recovery: revert newest-first, keep minors, full gates (PR #63). Majors are deliberate migrations with their own tickets: MA-30/31/32.

## Run hygiene

- **The e2e must clean up its worktree on every exit path.** A leftover `.runs/e2e-test` masks the next failure with "worktree already exists" (fixed idempotently in MA-29; manual: `git worktree remove --force .runs/e2e-test && git worktree prune`).
- **The demo's stage is the terminal.** All 34 ACs green, validation GREEN — and the F-2 human check still found the CLI prints two lines while the entire story hides in `.runs/<name>` git history (MA-33). Automated criteria can't feel silence; keep human checkpoints in the loop.
