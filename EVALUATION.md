# EVALUATION.md — tech-debt remediation build contract

**Source design:** `docs/tech-debt-2026-07-10.md` (TD-1..TD-16).
**Bar (operator-set, 2026-07-10):** regression-proof — every behavior fix lands with a test that would catch its regression; infra items become CI gates.
**Scope:** all 16 findings. Workshop constraint lifted: cassette re-record and dependency upgrades are in scope.

## Harness commands

A build agent proves its work with these; CI runs all of them.

| Harness | Command | Cwd |
|---|---|---|
| H-1 Rust fmt | `cargo fmt --all -- --check` | repo root |
| H-2 Rust lint | `cargo clippy -p falcon-mcp --all-targets -- -D warnings` | repo root |
| H-3 Rust tests | `cargo test --workspace --all-targets` | repo root |
| H-4 TS build | `npm run build` | `falcon-detective/` |
| H-5 TS lint | `npm run lint` (created by TD-8 work) | `falcon-detective/` |
| H-6 TS tests | `npm test` | `falcon-detective/` |
| H-7 Supply chain | `cargo audit` and `npm audit --audit-level=high` | root / `falcon-detective/` |
| H-8 Planted-defect audit | `scripts/check-planted-defects.sh` (created by G-1 work) | repo root |
| H-9 Debt gates | `scripts/debt-gates.sh` (created by WS-10; appended per-workstream per SPEC R-5) | repo root |

## Acceptance criteria

Tags: `autonomous` (build agent proves alone) · `operator-assisted` (needs operator input) · `felt` (human-use checkpoint).

### TD-1 — e2e destroys uncommitted work
- **AC-1** `autonomous` — With uncommitted modifications under `falcon-agent/`, the e2e suite MUST NOT discard them: it fails (or skips) with a message naming the dirty state, and the modifications survive byte-for-byte. Proven by an automated test that dirties a file, invokes the guard, and asserts both outcomes.
- **AC-2** `autonomous` — The e2e contains **no mutation of the developer checkout at all**; with `falcon-agent/` dirty it skips (or fails under `E2E_REQUIRED=1`) with a message naming the dirty paths. Isolation is the pipeline's existing `prepWorktree`/`git_worktree_add` mechanism, not new worktree code.

### TD-2 — e2e silently passes
- **AC-3** `autonomous` — With the falcon-mcp binary absent, the e2e test reports **skipped** (vitest skip status), never passed. Test manipulates the binary path and asserts the status.
- **AC-4** `autonomous` — Same for absent cassettes.
- **AC-5** `autonomous` — CI sets `E2E_REQUIRED=1` in the typescript job; with it set, missing prerequisites **fail** the e2e rather than skipping (proven by the AC-3/AC-4 unit tests), so a green CI run mechanically implies the e2e executed.

### TD-3 — intentional-defect markers
- **AC-6** `autonomous` — The "planted lint #1 — unused import" marker sits on the `use std::io::Write;` line (`falcon-agent/src/interrogate.rs:3`), and `cargo check -p falcon-agent` emits exactly that unused-import warning at that line. Script-verified (H-8).
- **AC-7** `autonomous` — The `#[ignore]` reason on `bird_themed_inputs_arent_special` states it is the intentional workshop smoking-gun and points at the README; the word "flaky" does not appear. Grep gate (H-9).
- **AC-8** `autonomous` — `falcon-agent/src/prompt.rs` module doc flags the poisoned third example as the intentional plant. Grep gate (H-9).
- **AC-9** `autonomous` — Running the three README-documented clippy invocations (`-W clippy::all`; `+ -W clippy::unwrap_used`; `+ -W clippy::nursery` — identical commands in H-8's script, which owns the expected-output set): under `-W clippy::all` alone the unused-import plant is the **only** warning, and each widening invocation surfaces its planted lint on its marked line. The undocumented `default_constructed_unit_structs` at `main.rs:16` **is fixed** (the canonical set stays at exactly three). Script-verified (H-8).

### TD-4 — missing tool timeouts
- **AC-10** `autonomous` — Every falcon-mcp tool that spawns a child process enforces a wall-clock timeout: on expiry the child is killed and a structured timeout error is returned. Integration test per tool family using a deliberately hanging fixture and a low configured timeout.
- **AC-11** `autonomous` — Timeout defaults are named constants in one module; tools whose schema exposes `timeout_ms` honor the per-call override.

### TD-5 — validation opt-out
- **AC-12** `autonomous` — No `z.any()` **and no `as any`** under `falcon-detective/src/handlers/` (grep gate, H-9 — includes the `TriageOutput as any` cast at `triage.ts:134`); `proposePoisonFix` validates `issue`/`report` shapes (reusing `Issue`/`PoisonReport`) and a unit test feeding malformed input asserts a validation error.

### TD-6 — Barnum private-API coupling
- **AC-13** `autonomous` — No reference to `__definition` (or other underscore-private Barnum surface) outside one documented adapter module — or none at all. Grep gate (H-9).
- **AC-14** `autonomous` — Verdict-conditional (SPEC WS-5): under **Verdict A** (migrated to public API), AC-13 alone suffices and the dependency range may stay as-is; under **Verdict B**, **both** an exact `@barnum/barnum` pin (no caret) **and** a canary unit test asserting the internal shape are required.

### TD-7 — unvalidated MCP results
- **AC-15** `autonomous` — Every `callTool` result is validated against a zod schema at the client boundary before typed use; a mocked malformed response produces a structured error naming the tool and the mismatch. Unit tests per tool-result schema.

### TD-8 — TS lint tooling
- **AC-16** `autonomous` — The chosen lint/format tool (Phase-3 decision) is configured; `npm run lint` passes; CI runs it (H-5).

### TD-9 — supply-chain scanning
- **AC-17** `autonomous` — CI runs `cargo audit` and `npm audit --audit-level=high`; any accepted advisories are allowlisted in-repo with a comment.
- **AC-18** `autonomous` — Dependabot (or Renovate) config covers `cargo`, `npm`, and `github-actions` ecosystems.

### TD-10 — orchestration test coverage
- **AC-19** `autonomous` — Unit tests exist with a mocked MCP client + mocked/cassette Gemini for: `verify` (Clean/Broken/Stuck), the commit handlers (`commitOne`, `commitAll`, `revertOne`, `escalate` — success and git-failure paths, no silent catch), `finalSweep` (Clean and Dirty branches), `processIssues` (happy path; retry-then-success; revert-on-persistent-failure).
- **AC-20** `autonomous` — `cli.ts` and `workflows/tidy-and-debug.ts` each have at least a smoke test (arg parsing; pipeline composes and stages connect).

### TD-11 — exec PATH resolution
- **AC-21** `autonomous` — `exec_run` executes an absolute path resolved from the allowlist, not a bare name via PATH; a test proves PATH manipulation cannot substitute a binary for an allowlisted name.

### TD-12 — model-name constants
- **AC-22** `autonomous` — Gemini model IDs are defined once in a shared constants module; grep gate: no `gemini-` string literal outside it (H-9).

### TD-13 — toolchain pin
- **AC-23** `autonomous` — `rust-toolchain.toml` pins the toolchain; CI resolves it; the Dockerfile base image matches the pinned minor. Script check (H-9).

### TD-14 — duplication
- **AC-24** `autonomous` — The three propose* handlers share one parameterized implementation; existing + new handler tests prove behavior parity (prompt file and model per kind).
- **AC-25** `autonomous` — `fs_search` and `fs_search_ast` share one streaming-subprocess helper; `MAX_STDERR_DISPLAY` (and truncation logic) defined exactly once. Grep gate + existing search tests green.

### TD-15 — dependency currency
- **AC-26** `autonomous` — Dev tooling upgraded (vitest ≥ 3, typescript current, @types/node aligned); H-4/H-5/H-6 green.
- **AC-27** `autonomous` — `@google/genai` on 2.x; all call sites compile; gemini unit tests green with mocked client.
- **AC-28** `operator-assisted` — Cassettes re-recorded against the 2.x request shapes; e2e passes in cassette mode on the new cassettes. **Operator supplies `GEMINI_API_KEY` and approves the (small) live-call spend at milestone M4 — without this the build cannot terminally prove AC-28.**
- **AC-29** `autonomous` — Rust deps current (axum 0.8, cargo_metadata current); workspace builds; H-2/H-3 green. Guardrail G-1 applies (upgrade must not disturb planted defects).

### TD-16 — cleanup cluster
- **AC-30** `autonomous` — `processIssues` main loop decomposed into named stage functions (each ≲ 40 lines); AC-19 tests cover the decomposed units.
- **AC-31** `autonomous` — `fs_apply_patch` split into validate/parse/splice units, each unit-tested.
- **AC-32** `autonomous` — falcon-mcp server handlers surface distinguishable error kinds (at minimum: not-found vs invalid-argument vs internal); a test asserts an MCP client can branch on them.
- **AC-33** `autonomous` — `gemini.ts`: backoff between schema-retry attempts; transient-error detection covers network-level failures (typed/broadened check); unit tests simulate both.
- **AC-34** `autonomous` — No `detective: any` / `as any` stage casts in `src/workflows/`; pipeline stages typed against handler input/output types. Grep gate (H-9) + the AC-20 workflow smoke test compiles without casts.

## Guardrails (enforced, not assumed — each carries an audit ticket in BUILDPLAN)

- **G-1** `autonomous` — falcon-agent's intentional defect set (poisoned few-shot example, missing input/output guards, documented planted lints, `#[ignore]`'d smoking-gun test) **survives the entire remediation intact**. `scripts/check-planted-defects.sh` (H-8) asserts each is present and correctly marked; runs in CI from the first milestone onward. This is the repo's product; "fixing" it is the failure mode.
- **G-2** `autonomous` — falcon-mcp sandbox behavior never weakens: existing sandbox tests (path escape, allowlist, read-only) keep passing at every milestone.
- **G-3** procedural — no force-push; one commit per ticket with conventional messages; PRs merge to `main` only through green CI.

## Human-use checkpoints (`felt`)

- **F-1** — trigger: end of milestone M3 (guardrails + tests). Operator runs the detective pipeline end-to-end in cassette mode and confirms the run still tells its story: readable output, sensible per-issue commits.
- **F-2** — trigger: end of milestone M4 (upgrades + re-record), immediately after AC-28. Same drive on the new stack; this is the terminal human check.
