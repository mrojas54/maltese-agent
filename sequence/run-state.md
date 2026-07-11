# Run state — maltese-agent tech-debt remediation

**Arc:** Tone, entered at Stage 3 (tone-architect). Brownfield remediation, not a greenfield initiation.
**Current stage/phase:** **ARC COMPLETE — lattice-orchestrator run TERMINAL 2026-07-10.** All 29 in-contract tickets `done` (22 planned + spike MA-23 + human MA-24 + fix-its MA-25..29); validation GREEN (35 PASS / 1 resolved-PARTIAL / 0 FAIL); felt checkpoints F-1 and F-2 confirmed by operator; e2e hard gate (`E2E_REQUIRED=1`) live on main since PR #65 (ecef4062). Closeout audit routed lessons to `LESSONS.md` + the orchestrator skill's runs-ledger. Post-run backlog minted, out of contract: MA-30/31/32 (reverted-major migrations), MA-33 (CLI narration, F-2 felt finding), MA-34 (cargo_clippy empty-lints diagnosis). Orchestration detail: `.lattice/orchestration/run-state.md`.
**Branch:** `claude/tech-debt-7ea493` (audit 50b0f0d; contract 9f72697) — all remediation merged to `main` through PR #65.

## Proportional shrink (recorded per pipeline norm)

Stages 1–2 (tone-initiation, tone-prototype) were not run. Rationale: the "design" is a
remediation of an existing, shipped codebase — the tech-debt audit
(`docs/tech-debt-2026-07-10.md`, findings TD-1..TD-16) serves as the validated design corpus,
and the shipped three-component system is the prototype. `PHILOSOPHY.md`, `USER_STORIES.md`,
`prototypes/`, `DESIGN.md`, `ECONOMICS.md` do not exist and will not be created; AC IDs are
minted directly in `EVALUATION.md` (AC-1..AC-34), each traceable to a TD finding.

## Operator interview — 2026-07-10 (Phase 0 touchpoint, answered)

| Question | Decision |
|---|---|
| Scope | **All 16 findings** (TD-1..TD-16) |
| Workshop constraint | **Workshop done/cancelled** — cassette freeze lifted; dependency upgrades and cassette re-record in scope |
| Verification bar | **Regression-proof** — every behavior fix lands with a test; infra items become CI gates |
| Handoff depth | **Stop after contract** — operator reviews before lattice-orchestrator mints tickets |

## Phase-3 fork round — 2026-07-10 (answered)

| Fork | Decision |
|---|---|
| TS lint tooling (TD-8) | **Biome** |
| Barnum coupling (TD-6) | **Spike MA-SP-1, migrate if public API exists; else adapter + exact pin + canary** |
| genai 2.x vs cassettes (TD-15) | **Upgrade + operator re-record at M4 (MA-H-1)** |

## Decisions log (decide-and-log)

- D-1: Contract artifacts at repo root per tone-architect contract; root `CLAUDE.md` created as pointers.
- D-2: Audit doc amended (addendum) to record the lifted workshop constraint — living-artifacts norm.
- D-3: Dev-tooling upgrades early (M2); runtime-dep upgrades + cassette re-record late (M4), so cassettes are re-recorded exactly once.
- D-4: Fork decisions escalated in one Phase-3 round (above); all else decide-and-log.
- D-5 (operator, 2026-07-10): **Ticket key scheme `MA-`** — fleet MA-01..MA-22, spike MA-SP-1, human MA-H-1; also resolved the H-1 ticket/harness ID collision.
- D-6: Judge-gate (Phase 2, second model) returned REVISE with 9 defects — 8 fixed in contract, 1 refuted with code evidence (the "phantom" no-input-sanitization marker exists at `interrogate.rs:23`).
- D-7: Phase-4 adversarial codebase pass returned 7 findings — all folded in. Load-bearing: `FalconMcpClient.call` (not `callTool`); `TriageReport` schema doesn't exist (use `PoisonReport`); `CARGO_TIMEOUT` raised to 900s to match documented cold-build ceiling; clippy guard needs the README's three invocations, not one; e2e reuses existing `prepWorktree` isolation; MA-17 resized M→L (18 error-mapping sites); new AC-34 (workflow `as any` casts, missed by the original audit rollup).

## Touchpoint status

- Phase 0 interview: DONE (4/4).
- Phase 2 judge-gate: DONE (REVISE → all defects resolved).
- Phase 3 fork round: DONE (3/3).
- Phase 4 adversarial codebase pass: DONE (7/7 folded in).
- Phase 4 contract review (operator): **APPROVED 2026-07-10.** Contract (commit 9f72697) is binding. Phase 5 handoff: `lattice-orchestrator` invoked with the three root contract artifacts.

## Stats

- Phase 0: 3 audit subagents (Sonnet) + 1 interview round. Phase 2: 1 judge (Opus). Phase 3: 1 fork round. Phase 4: 1 codebase-adversarial subagent (Sonnet). Total: 5 subagents, 2 interview rounds, ~3.5h wall-clock including audit.
