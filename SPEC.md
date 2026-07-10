# SPEC.md — tech-debt remediation, maltese-agent

**Contract authority:** acceptance criteria live in `EVALUATION.md` (AC-1..AC-34, G-1..G-3, F-1/F-2) and are referenced here by ID. Source analysis: `docs/tech-debt-2026-07-10.md`.
**Decided forks (operator, 2026-07-10):** TS tooling = **Biome**; Barnum coupling = **spike, migrate if a public API exists, else adapter + exact pin + canary**; genai = **upgrade to 2.x + operator re-records cassettes**.
**Repo facts:** Rust workspace (`falcon-mcp`, `falcon-agent`), npm package (`falcon-detective/`), CI in `.github/workflows/ci.yml`, branch base `main`.

## Global requirements

- R-1: Every workstream lands as its own PR with green CI; one logical change per commit; conventional-commit messages; no force-push (G-3).
- R-2: `scripts/check-planted-defects.sh` (WS-0) must exist and be CI-wired **before, or within the same PR as,** any change touching `falcon-agent/`. The script encodes the *post-WS-2* target state, so it is CI-wired inside WS-2's PR (which corrects the markers and fixes the accidental 4th lint); every later falcon-agent-touching PR runs against it (G-1).
- R-3: Existing falcon-mcp sandbox tests pass unmodified in every PR (G-2). A PR may add sandbox tests; it may not weaken or delete one.
- R-4: Shared files with known collision risk — `.github/workflows/ci.yml`, `falcon-detective/package.json`, `falcon-detective/src/lib/types.ts` — are flagged per-ticket in BUILDPLAN so the orchestrator serializes those edits.
- R-5: `scripts/debt-gates.sh` (harness H-9) is **created by WS-10** (skeleton + toolchain/Dockerfile-match check) and **appended to by each workstream whose AC cites H-9**, in that workstream's own PR: WS-2 (AC-7, AC-8 grep gates), WS-3 (AC-25 single-definition gate), WS-4 (AC-22 model-literal gate), WS-5 (AC-13 `__definition` gate). CI runs it from WS-10's PR onward.

## Workstreams

### WS-0 — Planted-defect guard (G-1; enables everything else)
Write `scripts/check-planted-defects.sh` (bash, exits non-zero on violation) asserting, with file:line precision:
1. `falcon-agent/src/prompt.rs` contains the poisoned third few-shot example (grep for its trigger-pattern content).
2. `falcon-agent/src/interrogate.rs` has no input-sanitization guard — grep for the marker comment that **already exists** at `src/interrogate.rs:23` (`BUG (intentional): no input sanitization`); no workstream needs to create it.
3. `falcon-agent/tests/integration.rs` still carries `#[ignore]` on `bird_themed_inputs_arent_special`.
4. The planted lints are **not all reachable from one clippy invocation** — `falcon-agent/README.md:36-38` documents three invocations with widening flags (`-W clippy::all` → unused import; `+ -W clippy::unwrap_used` → unwrap; `+ -W clippy::nursery` → redundant clone). The script runs all three (identical commands to AC-9; the script is the single source of truth for expected output) and asserts: (a) under `-W clippy::all` alone, the unused-import plant is the **only** warning; (b) each widening invocation surfaces its planted lint on its marked line (presence, since `nursery` may add unrelated noise). This holds only after WS-2 fixes the accidental 4th lint, which is why CI-wiring happens in WS-2's PR (R-2).
CI step added in WS-2's PR per R-2. Criteria: AC-9 (shared), G-1.

### WS-1 — e2e integrity (TD-1, TD-2 → AC-1..AC-5)
Rewrite `falcon-detective/tests/e2e.test.ts`:
- **Never mutate the developer checkout.** Delete the `execSync("git checkout -- falcon-agent/")` call and add **no replacement mutation**. Isolation already exists: the pipeline's `prepWorktree` handler creates a sandboxed worktree via falcon-mcp's `git_worktree_add` (under `<sandbox_root>/.runs/<name>`) — reuse it; do **not** reinvent isolation with raw `execSync("git worktree add …")`.
- **Dirty-state guard instead of cleanup:** if `git status --porcelain -- falcon-agent/` is non-empty, the e2e skips with a reason naming the dirty paths (the pipeline would otherwise investigate a non-canonical seed state) — or fails when `E2E_REQUIRED=1`.
- **No silent pass.** Prerequisite checks (MCP binary, cassettes) use vitest `ctx.skip(reason)` so the run reports *skipped*, except when env `E2E_REQUIRED=1` is set, in which case missing prerequisites **fail** the test. CI sets `E2E_REQUIRED=1` in the typescript job (AC-5).
- New test file `tests/e2e-guard.test.ts` proves: a dirty `falcon-agent/` is left byte-identical after a run attempt and produces the skip/fail outcome (AC-1, AC-2), and prerequisite-missing paths skip/fail as specified (AC-3, AC-4).

### WS-2 — Intentional-defect markers (TD-3 → AC-6..AC-9)
In `falcon-agent`:
- Move the `// BUG (intentional): planted lint #1 — unused import` comment from the `use std::sync::Arc;` line to the `use std::io::Write;` line (`src/interrogate.rs:3`); `Arc` is genuinely used and keeps no marker.
- Replace `#[ignore = "TODO investigate flaky"]` with `#[ignore = "INTENTIONAL workshop smoking-gun — do not fix; see falcon-agent/README.md §Planted bugs"]`.
- Append to `src/prompt.rs` module doc: a line stating the third example is the intentional poison plant and pointing at the README.
- Fix the accidental 4th lint: `FakePoisonedLlm::default()` → `FakePoisonedLlm` in `src/main.rs:16` (it is **not** in the README's documented planted set: unused-import, unwrap, needless-clone).
- Canonical planted-lint set (single source of truth for WS-0's check): the three README-documented lints, each carrying an in-source `BUG (intentional)` marker on the exact offending line.

### WS-3 — Subprocess timeouts + shared pipeline (TD-4, TD-14-rust → AC-10, AC-11, AC-25)
New module `falcon-mcp/src/tools/subprocess.rs`: one helper owning spawn → stream → truncate → kill → stderr-drain → **wall-clock timeout** (`tokio::time::timeout`; on expiry kill the child and return a structured timeout error carrying `elapsed_ms`/`limit_ms`). New `falcon-mcp/src/limits.rs` with named constants: `CARGO_TIMEOUT` **900s** (NOT lower: `falcon-detective/src/lib/mcp.ts:59-66` documents cold `cargo check/test/clippy` exceeding 5 minutes compiling tokio+axum+hyper, and the client call timeout was widened to 900s for exactly that), `GIT_TIMEOUT` 60s, `SEARCH_TIMEOUT` 30s; each env-overridable (`FALCON_MCP_<NAME>_MS`). Invariant: the client-side call timeout (mcp.ts) stays ≥ the server-side limit so the server's structured timeout error always fires first. Migrate `cargo.rs`, `git.rs`, `fs_basic.rs::fs_search`, `fs_ast.rs::fs_search_ast` onto it; `MAX_STDERR_DISPLAY` and `--max-filesize` become constants defined once. Integration tests per family use a hanging fixture + low timeout override.

### WS-4 — Schema hygiene in falcon-detective (TD-5, TD-7, TD-12 → AC-12, AC-15, AC-22)
- `proposePoisonFix.ts`: replace `z.any()` with real schemas from `src/lib/types.ts` — reuse `Issue`; the `report` input is the analyzePoison output, so reuse `PoisonReport`. (**`TriageReport` does not exist** in types.ts — its existing exports are `Issue`, `Location`, `Excerpt`, `TaggedIssue`, `PoisonReport`, `UnifiedDiff`, `PreviousFailure`, `VerifyResult`; author net-new shapes in types.ts only where genuinely absent, never as local duplicates.) Also remove the `schema: TriageOutput as any` cast at `src/handlers/triage.ts:134`.
- New `src/lib/toolSchemas.ts`: zod schema per MCP tool result the detective consumes. Change **`FalconMcpClient.call<T>`** (`src/lib/mcp.ts:57` — the public method is `call`; `callTool` is the SDK-internal object it wraps) to require a schema and parse before returning; on mismatch throw `McpValidationError(toolName, zodIssues)`. Update all call sites.
- New `src/lib/models.ts`: `export const MODELS = { pro: "gemini-2.5-pro", flash: "gemini-2.5-flash" } as const;` all handler/gemini call sites import it; no `gemini-` literal elsewhere in `src/`.

### WS-5 — Barnum decoupling (TD-6 → AC-13, AC-14) — **spike-gated**
Spike **MA-SP-1** (timeboxed, no production code): inspect `@barnum/barnum@0.4.x` exports/types for a public way to invoke a single handler outside the engine. Deliverable: a written verdict in the spike ticket.
- Verdict A (public path exists): migrate `processIssues.ts` to it; delete every `__definition` reference.
- Verdict B (none): new `src/lib/barnumAdapter.ts` as the **sole** module touching `__definition`; pin `"@barnum/barnum": "0.4.0"` exactly (no caret); add a canary unit test asserting the internal shape (`__definition.handle` exists and is callable) so a bump fails loudly.

### WS-6 — Biome + CI (TD-8 → AC-16)
Add `falcon-detective/biome.json` (recommended lint rules; formatter enabled, 2-space, double quotes to match existing style); `"lint": "biome check ."` and `"lint:fix": "biome check --write ."` scripts; one dedicated format-only commit; CI typescript job gains a lint step before build.

### WS-7 — Supply-chain gates (TD-9 → AC-17, AC-18)
CI: `cargo audit` step (install via `taiki-e/install-action`, tool `cargo-audit`) in the rust job; `npm audit --audit-level=high` in the typescript job. Accepted advisories, if any, documented in-repo (`audit.toml` / inline comment). New `.github/dependabot.yml`: weekly, ecosystems `cargo`, `npm` (dir `/falcon-detective`), `github-actions`.

### WS-8 — Orchestration test coverage (TD-10 → AC-19, AC-20)
New tests (vitest, mocked `FalconMcpClient` + injected fake Gemini):
- `tests/handlers/verify.test.ts` — Clean, Broken, Stuck verdicts.
- `tests/handlers/commit.test.ts` — success path; git-failure path surfaces an error (no silent catch).
- `tests/handlers/finalSweep.test.ts` — Clean and Dirty branches.
- `tests/handlers/processIssues.test.ts` — happy path; retry-then-success; revert-on-persistent-failure.
- `tests/cli.test.ts` — arg parsing smoke.
- `tests/workflows/tidy-and-debug.test.ts` — pipeline composition smoke (stages connect; types line up).

### WS-9 — Exec allowlist hardening (TD-11 → AC-21)
In `falcon-mcp`: resolve each allowlisted binary name to an absolute path once at sandbox construction (manual PATH scan or `which`-equivalent); `exec_run` executes the stored absolute path, never the bare name. Remove the dead allowlist entries `"ripgrep"` and `"sg"` — verified: no sandbox test enumerates them, so G-2/R-3 is not violated — and update the two doc strings that mention them (`src/server.rs:294` tool description, `src/tools/exec.rs:6` module doc). Test proves a PATH pointing at an impostor binary does not get executed.

### WS-10 — Toolchain pin (TD-13 → AC-23)
`rust-toolchain.toml` at repo root, `channel = "1.92"` (matches Dockerfile `rust:1.92-slim-bookworm`). CI derives the toolchain from the file (drop the hardcoded `@stable` resolution). H-9 script asserts Dockerfile base minor == pinned minor.

### WS-11 — propose* dedup (TD-14-ts → AC-24)
New `src/handlers/proposeFix.ts` factory `makeProposeFixHandler({ kind, promptFile, model, schema })`; `proposeBugFix`/`proposeLintFix`/`proposePoisonFix` become thin exports over it with **unchanged handler names/ids** (workflow references must not change). Existing propose* tests keep passing; add parity cases for the lint and poison variants.

### WS-12 — Dev-tooling upgrades, early (TD-15a → AC-26)
`vitest` → 3.x (migrate `vitest.config.ts` if needed), `typescript` → current 5.x, `@types/node` → latest for Node 20. Node major stays 20 (decided; matches CI and local tooling). All suites green afterward.

### WS-13 — Runtime upgrades + re-record, late (TD-15b → AC-27, AC-28, AC-29)
- `@google/genai` → 2.x per its migration guide; adapt `src/lib/gemini.ts` call sites; unit tests green with fakes (AC-27).
- New `falcon-detective/scripts/record-cassettes.ts` + npm script `record`: runs the pipeline in record mode against live Gemini, writing fresh cassettes. README section documents the one-command operator flow (`GEMINI_API_KEY=… npm run record`). **Operator executes this at M4** (AC-28).
- Rust: `axum` → 0.8 in both crates (falcon-agent handler-signature churn allowed, but WS-0's guard must stay green — the planted defects survive), `cargo_metadata` → current (AC-29). Edition stays 2021 (non-goal N-3).
- **Frozen:** `@barnum/barnum` stays exactly where WS-5 left it — the currency sweep must not bump it (that would silently void AC-14's pin/canary intent).

### WS-14 — Cleanup cluster (TD-16 → AC-30..AC-33)
- `processIssues.ts`: decompose the main loop into named stage functions (`classifyIssue`, `proposeForIssue`, `applyAndVerify`, `commitOrRevert` — each ≲ 40 lines); WS-8's tests cover the units.
- `fs_apply_patch` (falcon-mcp): split into `validate_limits` / `parse_hunks` / `splice_lines` units, each with unit tests; behavior unchanged.
- `server.rs`: introduce a `ToolError` taxonomy mapping to distinct MCP error codes (at minimum invalid-params / not-found / internal); replace the blanket `format!("{e:#}")`; a test asserts a client can distinguish the kinds (AC-32).
- `gemini.ts`: jittered backoff between schema-retry attempts (reuse the existing transient-backoff helper); transient detection checks structured error fields (`status`/`code`) when present with a broadened message fallback (`ECONNRESET`, `ETIMEDOUT`, `fetch failed`); unit tests simulate both (AC-33).
- `workflows/tidy-and-debug.ts`: remove the `detective: any` and per-stage `as any` casts; type the pipeline stages against the handler input/output types (as the post-WS-5 Barnum typings allow) (AC-34).

## Non-goals

- N-1: No new product features; no changes to what the detective pipeline *does*.
- N-2: No removal or weakening of falcon-agent's intentional defects — the opposite is enforced (G-1).
- N-3: No Rust edition bump (stays 2021); no Node major bump (stays 20).
- N-4: No vendoring/forking of Barnum (fork decision).
- N-5: No falcon-workshop-repo work; the design doc's 3 human-gated items stay out of scope.
- N-6: No prose rewrites of READMEs beyond sections whose commands/paths this work changes (plus the WS-13 record-flow section).

## Milestones (consumed by BUILDPLAN)

- **M1 — Safety floor:** WS-0, WS-1, WS-2, WS-10. The repo stops being dangerous to develop in.
- **M2 — Tooling baseline:** WS-6, WS-7, WS-12, WS-4(models.ts). CI now catches the next debt before it lands.
- **M3 — Guardrails & tests:** WS-3, WS-4(rest), WS-5(+MA-SP-1), WS-8, WS-9, WS-11. → human checkpoint **F-1**.
- **M4 — Currency:** WS-13, WS-14. → operator re-record (AC-28), human checkpoint **F-2**.
