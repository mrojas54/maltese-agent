# Orchestration run-state — maltese-agent tech-debt remediation

Contract: SPEC.md + EVALUATION.md + BUILDPLAN.md at repo root (binding commit 9f72697, operator-approved 2026-07-10). Arc anchor: `sequence/run-state.md`.

## Configuration

- **Autonomy:** Moderate (operator, 2026-07-10)
- **N concurrent delegators:** 5
- **PR merge policy:** auto-merge on green (merge commit per operator standing preference), gated on *verified* git/PR state via `/usr/local/bin/gh pr view <N> --json state,mergeable,statusCheckRollup` — never reported state
- **Git remote (verified):** `origin`
- **Terminal pre-merge status:** `review` (this install has no `pr_open`; transitions: review → done/in_progress/in_planning/needs_human/cancelled)
- **Status walk for delegators:** backlog → in_planning → planned → in_progress → review (backlog cannot jump to in_progress)
- **WIP limits:** in_progress 10, review 5
- **Ticket fidelity:** verbose (description carries BUILDPLAN anchor, AC IDs, serialize flags, size)
- **plan_review_mode / review_mode:** unset in `.lattice/config.json` → per-ticket modes below
- **Master Validator:** on (25 tickets > 3) — periodic cross-PR audit
- **Result Validator:** on — fresh session at terminal, walks `validation-plan.md`
- **c11:** NOT in c11 (`C11_SHELL_INTEGRATION` unset). Degrade: no panes/dashboard; delegators run as parallel Agent-tool subagents; operator drives `lattice list` / `lattice show <ID>`; auto-close-surfaces n/a
- **Model policy (operator global):** delegators inherit session model; headless code-review subagents on Sonnet; Opus reserved for design/final-review
- **gh invocation (operator global):** `/usr/local/bin/gh` with `GH_TOKEN=$(/usr/local/bin/gh auth token)` — the `gh` alias shells through 1Password and fails headless
- **Base branch:** `main`. Pre-dispatch step: contract branch `claude/tech-debt-7ea493` must be merged to main so fleet worktrees carry the contract + root CLAUDE.md (G-1 warning) + `.lattice/`
- **Human track (never dispatched):** MA-24 (cassette re-record, parked `needs_human`), felt checkpoints F-1 (post-M3) and F-2 (post-M4)

## Board↔BUILDPLAN ID map

Board `MA-1..MA-22` = BUILDPLAN `MA-01..MA-22` · `MA-23` = MA-SP-1 (spike) · `MA-24` = MA-H-1 (human) · `MA-25` = MA-00 (fix-it test split).

## Tickets in scope

| Ticket | Title | Status | Workflow mode | Branch base |
|---|---|---|---|---|
| MA-25 | Split test suites: fast hermetic default plus full target | backlog | fast-track | main |
| MA-1 | Author planted-defect guard script | backlog | fast-track | main |
| MA-2 | Rewrite e2e for checkout safety and required-run integrity | backlog | inline-full | main |
| MA-3 | Fix intentional-defect markers and wire guard into CI | backlog | inline-full (G-1-critical) | main |
| MA-4 | Pin Rust toolchain and create debt-gates script | backlog | fast-track | main |
| MA-5 | Adopt Biome lint and format with CI step | backlog | fast-track | main |
| MA-6 | Add supply-chain audits and Dependabot | backlog | fast-track | main |
| MA-7 | Upgrade dev tooling: vitest 3, TypeScript, types | backlog | inline-full | main |
| MA-8 | Centralize Gemini model IDs in a models module | backlog | fast-track | main |
| MA-9 | Shared subprocess helper with wall-clock timeouts | backlog | inline-full + headless review (L) | main |
| MA-23 | Spike: Barnum public handler invocation feasibility | backlog | fast-track (read-only, verdict comment) | main |
| MA-10 | Schema-validated MCP client boundary | backlog | inline-full | main |
| MA-11 | Implement Barnum coupling verdict | backlog | inline-full | main |
| MA-12 | Deduplicate propose handlers via factory | backlog | inline-full | main |
| MA-13 | Orchestration test coverage for detective core | backlog | inline-full + headless review (L) | main |
| MA-14 | Harden exec tool path resolution | backlog | inline-full | main |
| MA-15 | Decompose processIssues and de-cast workflow stages | backlog | inline-full | main |
| MA-16 | Split fs_apply_patch into tested units | backlog | inline-full | main |
| MA-17 | ToolError taxonomy with distinct MCP error codes | backlog | inline-full + headless review (L) | main |
| MA-18 | Gemini retry backoff and transient detection | backlog | fast-track | main |
| MA-19 | Migrate google genai to 2.x | backlog | inline-full | main |
| MA-20 | Cassette recording script and operator flow docs | backlog | fast-track | main |
| MA-21 | Rust runtime upgrades: axum 0.8 and cargo_metadata | backlog | inline-full (G-1-critical) | main |
| MA-22 | Final sweep: harnesses green and debt-gates closeout | backlog | fast-track | main |
| MA-24 | HUMAN: re-record Gemini cassettes with live key | needs_human | — (operator) | — |

## Serialization groups (R-4 — one in-flight ticket per group)

- `ci.yml`: MA-2, MA-3, MA-4, MA-5, MA-6
- `package.json` (falcon-detective): MA-25, MA-5, MA-7, MA-11, MA-19
- `handlers/*`: MA-8, MA-12
- `types.ts` / `lib/mcp.ts`: MA-10
- `falcon-mcp/src/tools/*`: MA-9, MA-14, MA-16
- `server.rs`: MA-17
- `Cargo.toml`: MA-21
- `lib/gemini.ts`: MA-18, MA-19

## Decision log (append-only)

- 2026-07-10 [operator] Config: autonomy Moderate; N=5; auto-merge on green; fix-it ticket for test split (MA-25); Master + Result Validators on.
- 2026-07-10 [autonomy: Moderate] Board IDs assigned by creation order so MA-1..MA-22 align with BUILDPLAN MA-01..MA-22 (`--id` rejects caller IDs); spike/human/fix-it become MA-23/24/25 — map recorded here and in validation plan.
- 2026-07-10 [autonomy: Moderate] MA-24 parked `needs_human` at mint time so no dispatch pass can select it (universal-target transition).
- 2026-07-10 [autonomy: Moderate] Validation plan drafted (42 rows: 34 ACs + G-1×2 + G-2 + G-3 + F-1/F-2 + final sweep + fix-it); operator's "approved, run lattice" taken as accept-as-is per intake default; operator may sharpen rows any time before Phase 2.
- 2026-07-10 [autonomy: Moderate] F-1 is not a hard dependency of M4 tickets per BUILDPLAN as written; M4 dispatch proceeds on code dependencies, F-1 surfaced prominently to operator the moment MA-9..MA-14 are all merged.
- 2026-07-10 [autonomy: Moderate] Outside c11, sub-agent-full mode unavailable; L tickets (MA-9, MA-13, MA-17) get inline-full plus a separate headless Sonnet code-review subagent before PR.
- 2026-07-10 [autonomy: Moderate] MA-2 gains a dependency on MA-25 (CI wiring references the `test:full` script MA-25 creates) — keeps ci.yml and package.json edits from colliding in wave 1.

## Run-time footguns

(rows added during dispatch — see orchestrator.md)
