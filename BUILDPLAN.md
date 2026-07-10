# BUILDPLAN.md — tech-debt remediation, maltese-agent

**Contract:** `SPEC.md` (workstreams WS-0..WS-14, milestones M1–M4) + `EVALUATION.md` (AC-1..AC-34, G-1..G-3, F-1/F-2). Consumed by `lattice-orchestrator`.
**Ticket key scheme (operator, 2026-07-10):** `MA-` prefix. Fleet tickets `MA-01..MA-22`; spike `MA-SP-1`; human-track ticket `MA-H-1`. (EVALUATION's harness IDs H-1..H-9 are unrelated to ticket IDs.)
**Approach (decided with operator, 2026-07-10):** brownfield remediation on `main` via per-ticket PRs; Biome for TS tooling; Barnum spike-then-migrate; genai 2.x with operator cassette re-record. No architecture change — the three-component shape stays.

## Tracks

- **Agent track** — every ticket below except MA-H-1. Buildable by the fleet against the harness commands in EVALUATION.md.
- **Human track** — MA-H-1 (cassette re-record, needs `GEMINI_API_KEY` + small live spend) and the two `felt` checkpoints F-1/F-2. Sequenced below; the build cannot terminally prove AC-28 without MA-H-1.

## Ticket breakdown

Size: S (≤half day) / M (≤1 day) / L (1–2 days). "Serialize:" names shared files the orchestrator must not let two tickets edit concurrently (R-4).

### M1 — Safety floor
| ID | Ticket | WS | ACs | Size | Depends on | Serialize |
|---|---|---|---|---|---|---|
| MA-01 | Author `scripts/check-planted-defects.sh` asserting the **target** marker state (script runnable locally; NOT yet CI-wired — see note) | WS-0 | G-1, AC-9 | S | — | — |
| MA-02 | Rewrite e2e: drop the `git checkout`, dirty-state guard (skip/fail), reuse pipeline's `prepWorktree` isolation (no new worktree code), `E2E_REQUIRED=1` in CI; add `tests/e2e-guard.test.ts` | WS-1 | AC-1..5 | M | — | ci.yml |
| MA-03 | Fix intentional-defect markers per SPEC WS-2; **wire MA-01's script into CI in this PR** (it passes only once markers are fixed); append AC-7/AC-8 grep gates to `debt-gates.sh` (SPEC R-5) | WS-2 | AC-6..9 | S | MA-01, MA-04 | ci.yml |
| MA-04 | `rust-toolchain.toml` (1.92) + CI derives from file + Dockerfile-match check in `scripts/debt-gates.sh` (create the script here) | WS-10 | AC-23 | S | — | ci.yml |

**Note on MA-01/MA-03:** the guard script encodes the *corrected* marker state, so it must reach CI in the same PR that corrects the markers (MA-03). Landing it earlier would fail CI on a truth the audit already established.

### M2 — Tooling baseline
| ID | Ticket | WS | ACs | Size | Depends on | Serialize |
|---|---|---|---|---|---|---|
| MA-05 | Biome config + `npm run lint`/`lint:fix` + one format-only commit + CI lint step | WS-6 | AC-16 | S | — | ci.yml, package.json |
| MA-06 | `cargo audit` + `npm audit` CI steps + `.github/dependabot.yml` (cargo, npm, github-actions) | WS-7 | AC-17, AC-18 | S | — | ci.yml |
| MA-07 | Dev-tooling upgrades: vitest 3.x, typescript current, @types/node (Node stays 20) | WS-12 | AC-26 | M | MA-05 | package.json |
| MA-08 | `src/lib/models.ts` + migrate all model-string call sites; grep gate into `scripts/debt-gates.sh` | WS-4c | AC-22 | S | — | handlers/* |

### M3 — Guardrails & tests → checkpoint F-1
| ID | Ticket | WS | ACs | Size | Depends on | Serialize |
|---|---|---|---|---|---|---|
| MA-09 | `subprocess.rs` + `limits.rs`; migrate cargo/git/search tools; timeout integration tests (hanging-fixture) | WS-3 | AC-10, AC-11, AC-25 | L | — | falcon-mcp/src/tools/* |
| MA-10 | `toolSchemas.ts` + schema-required `FalconMcpClient.call` + all call sites; `proposePoisonFix` real schemas (`Issue`/`PoisonReport`); remove `triage.ts:134` `as any` cast | WS-4a/b | AC-12, AC-15 | M | MA-07 | types.ts, lib/mcp.ts |
| **MA-SP-1** | **Spike:** does Barnum 0.4 expose public standalone handler invocation? Written verdict A/B only | WS-5 | — | S | — | — |
| MA-11 | Implement Barnum verdict (A: migrate off `__definition`; B: adapter + exact pin + canary test) | WS-5 | AC-13, AC-14 | M | MA-SP-1 | package.json |
| MA-12 | propose* dedup via `makeProposeFixHandler` factory; parity tests | WS-11 | AC-24 | M | MA-08, MA-10 | handlers/* |
| MA-13 | Orchestration tests: verify, commit, finalSweep, processIssues, cli, workflow smoke | WS-8 | AC-19, AC-20 | L | MA-10, MA-11 | — |
| MA-14 | Exec hardening: startup path resolution, drop dead allowlist entries + doc-string updates, impostor-PATH test | WS-9 | AC-21 | M | MA-09 | falcon-mcp/src/tools/* |
| **F-1** | **Human:** operator drives the pipeline in cassette mode; story/readability check | — | F-1 | — | MA-09..MA-14 | — |

### M4 — Currency → MA-H-1 → checkpoint F-2
| ID | Ticket | WS | ACs | Size | Depends on | Serialize |
|---|---|---|---|---|---|---|
| MA-15 | `processIssues` decomposition into stage functions (tests from MA-13 stay green); de-cast `workflows/tidy-and-debug.ts` stages | WS-14a | AC-30, AC-34 | M | MA-13 | — |
| MA-16 | `fs_apply_patch` split into validate/parse/splice + unit tests | WS-14b | AC-31 | M | MA-09 | falcon-mcp/src/tools/* |
| MA-17 | `ToolError` taxonomy in server.rs, distinct MCP error codes + client-branch test (**18 `.map_err(format!)` sites** — mechanical but wide) | WS-14c | AC-32 | L | MA-09 | server.rs |
| MA-18 | `gemini.ts` retry backoff + structured transient detection + tests | WS-14d | AC-33 | S | MA-10 | lib/gemini.ts |
| MA-19 | `@google/genai` 2.x migration; unit tests green on fakes | WS-13a | AC-27 | M | MA-18 | package.json, lib/gemini.ts |
| MA-20 | `scripts/record-cassettes.ts` + `npm run record` + README operator-flow section | WS-13b | AC-28 (enables) | S | MA-19 | — |
| MA-21 | Rust runtime upgrades: axum 0.8 (both crates; planted-defect guard must stay green), cargo_metadata current; `@barnum/barnum` frozen per SPEC WS-13 | WS-13c | AC-29 | M | MA-09, MA-17 | Cargo.toml |
| **MA-H-1** | **Human:** `GEMINI_API_KEY=… npm run record` — re-record cassettes (~5 files, small spend) | WS-13b | AC-28 | — | MA-20 | — |
| MA-22 | Final sweep: e2e green on new cassettes; every harness H-1..H-9 green; close out debt-gates | — | all | S | MA-H-1, MA-21 | — |
| **F-2** | **Human:** terminal drive on the upgraded stack | — | F-2 | — | MA-22 | — |

## Critical assumptions → earliest check

| Assumption | Risk if wrong | Settled by |
|---|---|---|
| Barnum 0.4 has a public handler-invocation path | MA-13's processIssues tests build on the wrong invocation style | **MA-SP-1**, before MA-11/MA-13 dispatch |
| genai 2.x request shapes invalidate old cassettes (assumed yes) | M4 ordering wasted if old cassettes still key correctly | MA-19 unit fakes hint; settled at MA-H-1 |
| Pipeline's existing `prepWorktree` isolation suffices for the e2e (no new worktree code) | MA-02's guard design is wrong and the e2e still needs its own isolation | MA-02's own guard tests; `prepWorktree` already covered by `tests/handlers/prepWorktree.test.ts` |
| axum 0.7→0.8 churn in falcon-agent won't disturb planted defects | G-1 violation, product damaged | MA-01 script in CI fails the MA-21 PR instantly |

## State writer/reader check

Small surface: cassettes (written: `npm run record` MA-H-1 · read: gemini cassette mode + e2e); `scripts/check-planted-defects.sh` (written: MA-01/MA-03 · read: CI every PR); `scripts/debt-gates.sh` (written: MA-04, appended by MA-03/MA-08 and per SPEC R-5 · read: CI); `rust-toolchain.toml` (written: MA-04 · read: CI + rustup). No unwritten/unread fields.

## Sequencing rationale

Human-visible value lands earliest: after M1 the repo can't eat uncommitted work, can't false-green, and can't mislead a maintainer about the planted defects. CI hardening (M2) precedes the code-heavy M3 so every fleet PR lands against the stricter gates it will be audited by. Tests (MA-13) are written against **current** behavior before the refactors (MA-15) that must keep them green. All cassette-touching work is confined to M4 so the operator records exactly once.
