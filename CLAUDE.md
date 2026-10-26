# maltese-agent — root artifacts

Build-contract and analysis artifacts for the tech-debt remediation (pointers, not copies):

- `docs/tech-debt-2026-07-10.md` — the audit: findings TD-1..TD-16, scoring, phased plan.
- `SPEC.md` — remediation specification: workstreams WS-0..WS-14, non-goals, milestones.
- `EVALUATION.md` — acceptance criteria AC-1..AC-34, guardrails G-1..G-3, harness commands H-1..H-9, human checkpoints F-1/F-2.
- `BUILDPLAN.md` — ticket breakdown with dependencies and serialization flags; agent vs human tracks.
- `sequence/run-state.md` — the arc's resume anchor: stage, decisions, touchpoint status.
- `LESSONS.md` — timeless findings from the remediation run (cassette integrity, model lifecycle, dependency policy, run hygiene).

**Standing guardrail (G-1):** `falcon-agent`'s poisoned prompt, missing guards, planted lints, and `#[ignore]`'d smoking-gun test are intentional workshop material. Never "fix" them; `scripts/check-planted-defects.sh` (once landed) is the enforcement.

**Test loop:** while iterating, test one crate: `cargo nextest run -p falcon-mcp` or `cargo nextest run -p falcon-agent` (seconds once built; falcon-agent's `#[ignore]`'d test stays skipped, per G-1). Before a PR, run what CI runs: `cargo test --workspace --all-targets` (`.github/workflows/ci.yml:70`).
