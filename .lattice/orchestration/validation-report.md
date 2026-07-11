# Validation Report

Source spec: [SPEC.md](../../SPEC.md)
Source build plan: [BUILDPLAN.md](../../BUILDPLAN.md)
Source evaluation: [EVALUATION.md](../../EVALUATION.md)
Source validation plan: [validation-plan.md](./validation-plan.md)
Result Validator: agent:result-validator (fresh session, no c11, outside orchestrator context)
Date: 2026-07-10
Run completed: 2026-07-10 ~13:0x EDT (fleet); audited post-merge against main @ `abfeb39`
Contract binding commit: `9f72697` (verified present in history via merged contract PR #24 `8f44c62`)

Every result below was produced by running the row's named method in this session against the merged main tree (`git pull --ff-only` to `abfeb39` first). No builder claim was accepted without re-verification. `pre-merge-static` rows were executed post-merge per the dispatch instruction; the plan's method column was followed as written.

## Summary

- Total criteria audited (pre-merge-static rows): **36**
- Pass: **35**
- Partial: **1** (row 5 / AC-5)
- Fail: **0**
- Blocked: **0**
- Post-merge-smoke rows staged for operator: **6** (rows 28, 36, 38, 39, 40, 41)

**Overall verdict: GREEN with one flagged deferral.** All agent-provable acceptance criteria hold on the merged tree; every harness I could run is green; G-1 planted material is fully intact. The single Partial is the deliberately parked `E2E_REQUIRED` CI gate (AC-5), which is structurally tied to the two open human-track items (MA-24 re-record → MA-22 final sweep). The run is not closable until the operator executes the smoke checklist below.

## Harness results (H-1..H-9, run this session)

| Harness | Command / cwd | Result | Evidence |
|---|---|---|---|
| H-1 | `cargo fmt --all -- --check` (root) | **PASS** exit 0 | 0.65s |
| H-2 | `cargo clippy -p falcon-mcp --all-targets -- -D warnings` (root) | **PASS** exit 0 | clean finish, axum 0.8.9 / cargo_metadata 0.23.1 in graph |
| H-3 | `cargo test --workspace --all-targets` (root) | **PASS** exit 0 | all suites `test result: ok`; exactly 1 ignored (the smoking gun, by design); timeout_test 5/5, error_codes tests, exec impostor test all green |
| H-4 | `npm run build` (falcon-detective/) | **PASS** exit 0 | tsc -p tsconfig.build.json, 8.7s |
| H-5 | `npm run lint` (falcon-detective/) | **PASS** exit 0 | biome, 59 files, no diagnostics |
| H-6 | `npm test` (falcon-detective/) | **PASS** exit 0 | 24 files, **146/146**, 9.30s (hermetic: `FALCON_MCP_BIN=/nonexistent`, e2e excluded) |
| H-7 | `cargo audit` (root) + `npm audit --audit-level=high` (falcon-detective/) | **PASS** exit 0 / exit 0 | cargo: 1 *warning-level* advisory (RUSTSEC-2026-0190, anyhow 1.0.102 unsound; fix = Dependabot anyhow 1.0.103 PR, already noted in decision log). npm: 1 **low** advisory (esbuild GHSA-g7r4-m6w7-qqqr), below the high gate. Note: cargo-audit was not installed locally; installed via Homebrew this session. CI on merged tip `abfeb39` (run 29107230532, success) independently ran both audit steps green. |
| H-8 | `scripts/check-planted-defects.sh` (root) | **PASS** exit 0 | 16/16 assertions PASS (P1-P2, G1-G3, I1-I3, M1-M3, C1, L1-L4), 14.1s |
| H-9 | `scripts/debt-gates.sh` (root) | **PASS** exit 0 | 9/9 gates PASS (AC-23, AC-25×2, AC-22, AC-13, AC-7, AC-8, AC-12, AC-34) |
| — | `npm run test:full` (falcon-detective/) | **PASS** exit 0 | 25 files, 146 passed + **1 skipped** — the e2e hit the MA-26 staleness fast path (<1s) with the re-record pointer. This is the documented pre-MA-24 state, not a defect. |

## G-1 guardrail audit (planted material intact on merged main)

Verified by direct read **and** by H-8 (exit 0):

- Poisoned prompt example: third few-shot example (`the bird flew at midnight` / `"attribution": "(unknown)"`) present in `falcon-agent/src/prompt.rs`, flagged in the module doc as the intentional plant with README pointer (P1, P2).
- Missing guards: all three `BUG (intentional)` guard markers present (`no input sanitization`, `no schema-level checks` in interrogate.rs; `no schema validation here` in llm.rs) (G1–G3).
- Exactly three planted clippy lints: `cargo check` emits *only* the unused-import warning at `interrogate.rs:3` (C1); `-W clippy::all` alone → only that warning (L1); `+unwrap_used` surfaces the unwrap at `interrogate.rs:34` (L2); `+nursery` surfaces `redundant_clone` at `llm.rs:41` (L3); the accidental 4th lint (`default_constructed_unit_structs`) is gone — `main.rs` now constructs `FakePoisonedLlm` without `::default()` (L4).
- `#[ignore]`'d smoking gun: `integration.rs:64` reason = "INTENTIONAL workshop smoking-gun — do not fix; see falcon-agent/README.md §Planted bugs"; the word "flaky" appears **nowhere** under `falcon-agent/` (`rg -i flaky` → 0 matches) (I1–I3).
- Guard is CI-wired (ci.yml "Planted-defect guard (G-1)" step, added in PR #34 — verified in that PR's diff) and every subsequent falcon-agent-touching PR (esp. MA-21 #36) merged with check rollup SUCCESS.

## Per-criterion results (pre-merge-static rows, in plan order)

| Row | Criterion | Result | Evidence (method run this session) |
|---|---|---|---|
| 1 | AC-1 | ✅ Pass | `tests/e2e-guard.test.ts` dirties `falcon-agent/src/main.rs` in a fixture repo, asserts skip reason names the dirty path + "uncommitted", asserts byte-for-byte survival (`after.equals(before)`); fail-variant under required=true also asserts no mutation. PR #31 checks SUCCESS; test green in H-6. |
| 2 | AC-2 | ✅ Pass | `rg 'git checkout\|git reset\|git clean\|checkout --\|worktree add' tests/e2e.test.ts` → **0 matches**. Isolation is the pipeline's own `prepWorktree`/`git_worktree_add` (e2e.test.ts:40-42 comment; covered by `tests/handlers/prepWorktree.test.ts`); no new worktree code in the test. |
| 3 | AC-3 | ✅ Pass | e2e-guard "missing falcon-mcp binary (AC-3)": asserts `outcome.kind === "skip"` and that `applyGuardOutcome` routes through `ctx.skip` (vitest skip status, never pass); e2e.test.ts passes the real vitest ctx. Fail-under-`E2E_REQUIRED=1` variant present. CI green. |
| 4 | AC-4 | ✅ Pass | Same file, "missing cassettes (AC-4)": empty dir, absent dir, and required=fail variants all asserted; fingerprint manifest alone does not count as a cassette. CI green. |
| 5 | AC-5 | ⚠️ **Partial** | ci.yml runs `npm run test:full` in the typescript job ✅ and fail-when-required is proven by the row-3/4 unit tests ✅ — but the job sets **`E2E_REQUIRED: "0"`, not "1"** (ci.yml:150-154). The hard gate was parked by commit `ddd2bab` ("park E2E_REQUIRED hard gate until final sweep", inside PR #31) because checked-in cassettes are stale pre-MA-24; the in-file comment commits MA-22 to flipping it to "1" after MA-24. As written, the row's pass condition ("Env var set in CI" per AC-5's `=1`) is not met. See Drift item D-1. |
| 6 | AC-6 | ✅ Pass | Marker sits on `use std::io::Write;` at `interrogate.rs:3` (read); H-8 C1 asserts `cargo check` emits exactly that warning at that line (ran, PASS). PR #34 checks SUCCESS. |
| 7 | AC-7 | ✅ Pass | Ignore reason correct (integration.rs:64); "flaky" absent (`rg -i` → 0); H-9 gate `smoking-gun-ignore-reason (AC-7)` present in debt-gates.sh and PASS. |
| 8 | AC-8 | ✅ Pass | prompt.rs module doc line 7 flags the intentional poison + README; H-9 gate `prompt-poison-flag (AC-8)` present and PASS. |
| 9 | AC-9 | ✅ Pass | check-planted-defects.sh:218-220 encodes the three README invocations byte-equivalent to README.md:39-41 (`-W clippy::all`; `+ -W clippy::unwrap_used`; `+ -W clippy::nursery`), with expected warning sets (L1 exact-match, L2/L3 span-precision); L4 asserts the 4th lint is gone; `main.rs` uses `FakePoisonedLlm` (no `::default()`). H-8 exit 0. PRs #27/#34 checks SUCCESS. |
| 10 | AC-10 | ✅ Pass | `tools/subprocess.rs` owns spawn→stream→truncate→kill→stderr-drain→timeout; structured `SubprocessTimeout { label, elapsed_ms, limit_ms }`; child killed on expiry. All spawning tools route through `run_collect`/`run_streaming` (cargo.rs:109/217/381, git.rs:30, fs_basic.rs:471, fs_ast.rs:106, exec.rs via run_collect; no stray `.spawn(` — H-9 gate). Per-family hanging-fixture tests `tests/timeout_test.rs` 5/5 green in H-3. |
| 11 | AC-11 | ✅ Pass | `limits.rs`: `CARGO_TIMEOUT` 900s (with the mcp.ts:59-66 rationale pinned in a doc comment *and* a unit test `cargo_default_stays_at_900s`), `GIT_TIMEOUT` 60s, `SEARCH_TIMEOUT` 30s (+`EXEC_TIMEOUT` 30s); env override `FALCON_MCP_<NAME>_MS` with zero/garbage rejection, unit-tested; `exec_run` schema `timeout_ms` honored per call — `exec_run_honors_per_call_timeout_ms_override` green in H-3. |
| 12 | AC-12 | ✅ Pass | `rg 'z\.any\(\)\|as any' src/handlers/` → **0**. `PoisonFixInput` reuses `Issue`/`PoisonReport` from types.ts (proposeFix.ts:52-59); malformed-input tests present (proposePoisonFix.test.ts:55/67/79 — malformed report, malformed issue, missing report all rejected). H-9 gate `no-handler-any` PASS. |
| 13 | AC-13 | ✅ Pass | `rg '__definition' falcon-detective/src/` → **0 matches** (no adapter module needed — Verdict A). H-9 gate `barnum-private-surface` PASS. *Observation:* 9 files under `tests/` still reach `__definition` for in-process invocation; the H-9 gate documents this scope choice ("test files' in-process reaches are MA-13's charter"). Plan method scopes to src/, so Pass; residual coupling noted below. |
| 14 | AC-14 | ✅ Pass | MA-23 board comment verified (actor agent:delegator-ma-23): **"Verdict A — public API exists: runPipeline from @barnum/barnum/pipeline"**, evidence at `src/lib/invokeHandler.ts`. Under Verdict A, AC-13 alone suffices and the caret range (`^0.4.0`) may stay — it has. MA-11 (PR #29) landed the public-path migration. |
| 15 | AC-15 | ✅ Pass | `FalconMcpClient.call<T>` **requires** a schema (mcp.ts:93-96) and `safeParse`s `structuredContent ?? content` before returning; mismatch throws `McpValidationError(toolName, issues)` naming tool + mismatches. `toolSchemas.ts` covers 13 tool results with a `TOOL_RESULT_SCHEMAS` map; tests assert a valid + malformed payload **per schematized tool** (toolSchemas.test.ts) and the client-boundary malformed-response error naming the tool (mcp.test.ts:56-70), incl. the content-fallback path. All green in H-6. |
| 16 | AC-16 | ✅ Pass | `biome.json` present; `npm run lint` defined and green (H-5); ci.yml "biome lint (TD-8 / AC-16)" step present. |
| 17 | AC-17 | ✅ Pass | ci.yml has `cargo audit` (rust job, with G-1 allowlist-only note) and `npm audit --audit-level=high` (typescript job); `.cargo/audit.toml` allowlist is empty with a mandatory-comment policy documented in-file; no accepted advisories exist. |
| 18 | AC-18 | ✅ Pass | `.github/dependabot.yml`: `cargo` (/), `npm` (/falcon-detective), `github-actions` (/) — all three, weekly. |
| 19 | AC-19 | ✅ Pass | Read all four files: verify (Clean/Broken-via-check/Broken-via-test/Stuck/no-silent-catch), commit (commitOne success + git_add failure + git_commit failure, commitAll, revertOne success + documented carve-out, escalate), finalSweep (Clean/Dirty/propagates failure), processIssues (happy/retry-then-success/revert-on-persistent-failure). All use the fakeMcp helper; Gemini never reached (propose* faked at the seam). Green in H-6; PR #43 checks SUCCESS. |
| 20 | AC-20 | ✅ Pass | `tests/cli.test.ts` (arg-parsing smoke: exits 1 naming missing `--target`) and `tests/workflows/tidy-and-debug.test.ts` (chains prepWorktree→triage→processIssues→finalSweep→branch; Clean→commitAll / Dirty→escalate; engine-enforced schemas per stage). Both green. |
| 21 | AC-21 | ✅ Pass | exec.rs executes `sandbox.resolved_bin(&args.cmd)` — absolute path pinned at sandbox construction; `tests/exec_impostor_test.rs` proves call-time PATH prepend does NOT substitute the binary (with a vacuity guard proving the PATH attack works on bare-name spawn); dead entries gone — `sandbox.rs` test `allowlist_no_longer_contains_ripgrep_and_sg_aliases` asserts `check_bin("ripgrep")`/`("sg")` err; both doc strings updated (exec.rs module doc, server.rs exec_run description). All green in H-3. |
| 22 | AC-22 | ✅ Pass | `src/lib/models.ts` exists (GEMINI_PRO/GEMINI_FLASH/DEFAULT_MODEL); `rg 'gemini-' src` → matches **only inside lib/models.ts**; H-9 gate `no-gemini-literals` PASS. (Method note: the plan's literal `--glob '!lib/models.ts'` doesn't match the nested path from repo root — intent verified both by full-output inspection and by the H-9 gate's correct exclusion.) |
| 23 | AC-23 | ✅ Pass | `rust-toolchain.toml` pins `channel = "1.92"`; ci.yml derives the toolchain from the file in both jobs (sed → dtolnay/rust-toolchain); debt-gates gate `toolchain-dockerfile-match` verifies Dockerfile `rust:1.92-slim-bookworm` minor — PASS in H-9. |
| 24 | AC-24 | ✅ Pass | `makeProposeFixHandler({kind, promptFile, model, schema})` factory in proposeFix.ts; three instances (bug→propose-bug-fix.md/PRO, lint→propose-lint-fix.md/FLASH, poison→propose-poison-fix.md/PRO) with byte-identical pre-factory handler ids; pre-factory files are thin re-exports. Parity tests: structural pins (proposeFix.test.ts — ids engine-resolvable, bug/lint validator parity, poison report requirement) + per-variant cassette parity (proposeLintFix.test.ts, proposePoisonFix.test.ts). Green in H-6; PR #42 checks SUCCESS. |
| 25 | AC-25 | ✅ Pass | `rg -c 'MAX_STDERR_DISPLAY\s*[:=]' falcon-mcp/src/` → **1** (limits.rs:45); fs_search + fs_search_ast both use `subprocess::run_streaming` and contain no direct `.spawn(` (H-9 gates `single-MAX_STDERR_DISPLAY-definition` + `shared-search-subprocess-helper` PASS); search tests green in H-3 (fs_test.rs 14/14). |
| 26 | AC-26 | ✅ Pass | package.json: vitest `^3.2.7` (≥3), typescript `^5.9.3`, @types/node `^20.19.43`; Node major stays 20 (CI `node-version: 20`; no `engines` field existed at binding commit 9f72697 either — verified). H-4/H-5/H-6 green locally and in CI (PR #38 SUCCESS, main run SUCCESS). |
| 27 | AC-27 | ✅ Pass | `@google/genai` `^2.11.0` (2.x); build compiles (H-4); gemini unit tests green on mocked client (gemini-retry.test.ts 16/16 in H-6). PR #46 checks SUCCESS. |
| 29 | AC-29 | ✅ Pass | falcon-mcp/Cargo.toml: `axum = "0.8"`, `cargo_metadata = "0.23"` (0.23.1 resolved); falcon-agent/Cargo.toml: `axum = "0.8"`. H-2/H-3 green this session; H-8 green **on the same merged tree** (G-1 intact through the upgrade). PR #36 checks SUCCESS. |
| 30 | AC-30 | ✅ Pass | processIssues.ts decomposed into named stages — measured: classifyIssue 15, proposeForIssue 23, prepareProposal 37, snapshotOnce 14, revertSnapshots 5, attemptOnce 28, applyAndVerify 33, commitOrRevert 36 lines (all ≲40; SPEC's four named stages present). MA-15's commit (434ba6d) touched only src+workflows+debt-gates — AC-19 tests unmodified, and they pass on the merged tree (H-6). |
| 31 | AC-31 | ✅ Pass | `fs_basic.rs`: `validate_limits` (:156), `parse_hunks` (:180), `splice_lines` (:220) with per-unit tests (validate_limits accepts-small/exact-max/rejects-oversized; parse_hunks valid/no-newline/malformed; splice happy-path/multiple-hunks/+conflict cases). Green in H-3. |
| 32 | AC-32 | ✅ Pass | `tool_error.rs`: `ToolError::{NotFound(-32002 RESOURCE_NOT_FOUND), InvalidArgument(-32602 INVALID_PARAMS), Internal}` + dedicated timeout code `-32001` (≥3 distinguishable kinds); `rg 'map_err(.*format!' server.rs` → **0** remaining of the 18 former sites; `tests/error_codes_test.rs::client_can_branch_on_distinct_error_kinds` asserts a client branches on differing codes — green in H-3. PR #33 checks SUCCESS. |
| 33 | AC-33 | ✅ Pass | gemini.ts: exponential full-jitter backoff shared between API-transient retries **and schema-retry attempts**; transient classification checks structured `status`/`code` first (walking the `cause` chain of Node 20 "fetch failed" TypeErrors) with a broadened message-regex fallback (ECONNRESET/ETIMEDOUT/fetch failed/…). Tests simulate both: `gemini-retry.test.ts` — network-level retry, non-transient immediate throw, give-up-after-5, schema-retry backoff + no-backoff-after-final + re-prompt-with-validation-error. Green in H-6. |
| 34 | AC-34 | ✅ Pass | `rg -c 'detective:\s*any\|as any' src/workflows/` → **0**; workflow smoke (AC-20) compiles and passes; H-4 build clean; H-9 gate `no-workflow-any` PASS. |
| 35 | G-1 (static) | ✅ Pass | Guard CI-wired in MA-3's PR #34 (verified in that PR's ci.yml diff — "Planted-defect guard (G-1)" step); every subsequent PR touching falcon-agent merged with rollup SUCCESS, incl. MA-21 (#36, merged after #34 in history). All 23 run PRs (#25–#43, #46, #56–#58) show `SUCCESS:2` check rollups — no red H-8 run was ever merged. |
| 37 | G-2 | ✅ Pass | Diffed all five falcon-mcp merges (#30 `4ac4b9b`, #32 `9148511`, #33 `aa1f1a5`, #35 `4cdcab2`, #36 `253a6b0`) against their first parents, filtered to `falcon-mcp/tests/` + `src/sandbox.rs`: **zero deleted test/assert lines** — sandbox tests (path escape, allowlist, read-only) unmodified or strengthened (MA-14 *added* allowlist tests). All green in H-3 on the merged tree and in each PR's CI. |
| 42 | Fix-it split (MA-25) | ✅ Pass | `npm test` is hermetic (`FALCON_MCP_BIN=/nonexistent/hermetic`, `--exclude tests/e2e.test.ts`) and ran 146/146 in **9.3s** (≤60s); `npm run test:full` includes the e2e (25th file — executed, hit the documented staleness skip). |

## Drift from BUILDPLAN.md

| # | Drift | Logged? | Assessment |
|---|---|---|---|
| D-1 | **`E2E_REQUIRED` hard gate parked at "0"** (commit `ddd2bab` in PR #31/MA-2; ci.yml:144-149 note). AC-5/SPEC WS-1 say CI sets `E2E_REQUIRED=1`. Rationale in-file: stale pre-MA-24 cassettes would red every PR; MA-22 flips it after re-record. | Partially — in ci.yml comment + commit message (attributed to "operator decision 2026-07-10, ticket MA-2 review"), but **absent from run-state's append-only decision log**, and MA-2's board ticket has zero comments to anchor the citation. | The engineering call is defensible and self-documenting, but a merge-gate change against an explicit AC belongs in the decision log. Root cause of the row-5 Partial. Action: log it retroactively; MA-22 must verify the flip. |
| D-2 | **MA-26 fix-it ticket minted mid-run** (e2e staleness fast path + async spawn; PR #56). Not in BUILDPLAN. | Yes — decision-log entry diagnosing the PR #41 vitest-worker flake. | Healthy fix-it flow; the fast path works (observed <1s stale-skip this session). The no-sync-exec regression test pins the flake. |
| D-3 | **Board bookkeeping vs run-state**: the resume section claims per-ticket **PR-evidence comments** on the 14 reconciled tickets; the board shows `comment_count: 0` and no `comment_added` events on **all 14** (MA-1..5, 7..9, 11, 14, 16, 17, 21, 25 — verified per-ticket). Status walks did land (3× status_changed each). | No — the claim is in run-state itself and is contradicted by the board. | Bookkeeping-only (PR evidence is recoverable from git/GitHub), but run-state is the arc's resume anchor and should not overstate board state. Action: amend run-state or backfill comments. |
| D-4 | **Force-with-lease push on PR #57's branch** during the MA-20 conflict rebase (per the delegator's ticket comment). G-3 says "no force-push"; operator's global rule says no force-push unless asked. | On the MA-20 ticket comment only; not in the decision log. | Low severity: lease-guarded, on an unmerged feature branch, followed by green CI and operator-authorized merge; `main` itself was never rewritten (this session's `--ff-only` pull succeeded). Flagged for the operator's G-3 smoke row rather than adjudicated here. |
| D-5 | **Merge-policy churn**: auto-merge → downgraded to leave-at-review (permission classifier denial) → restored on operator's "you can merge them now"; #43 additionally held for the L-ticket fresh-eyes verdict. | Yes — FORCED entry + two operator entries in the decision log. | Clean handling; interpretation was logged before acting. |
| D-6 | **MA-6 scope addition**: lockfile-only quinn-proto 0.11.16 bump to clear RUSTSEC-2026-0185 when the new cargo-audit gate red-flagged its own PR. | Yes — decision-log entry. | Correct minimal response; gate validated on first contact. |
| D-7 | **MA-20 additions beyond ticket text**: tsconfig.json/tsconfig.build.json split (typechecks scripts/ while keeping dist layout byte-identical) + union-conflict reconciliation with #56 (fingerprint step integrated into the operator flow). | On the MA-20 ticket comments (self-review + conflict-resolution roundtrip). | Sound; the build harness still passes (H-4) and the reconciled operator flow is the one staged below. |
| D-8 | **Residual `__definition` reaches in tests** (9 files under falcon-detective/tests/). AC-13's prose says "no reference … or none at all"; the shipped interpretation scopes enforcement to production src (H-9 gate comment documents this as MA-13's charter). | In-repo (gate comment); not in the decision log. | Plan-compliant (the plan's method scopes to src/) but a real residual coupling: a Barnum internal rename now breaks tests, not production. Consider a follow-up ticket to route test invocation through the public seam. |
| D-9 | **Board IDs remapped** (MA-23=spike, MA-24=human, MA-25=fix-it split) because `--id` rejects caller IDs; MA-2 gained a dependency on MA-25. | Yes — decision log + plan header. | No issue. |
| D-10 | **Dependabot activation side-effect**: ~12 update PRs (#44–#55 range) opened when MA-6 merged; still open. | Yes — decision log ("OUT OF RUN SCOPE, surfaced at closeout"). | The anyhow 1.0.103 PR among them clears the only cargo-audit warning (H-7). Operator should triage post-run. |

## Gaps (criteria not yet satisfied by any merged PR)

- **AC-28** (row 28) — operator-assisted by contract: cassettes not yet re-recorded; e2e currently stale-skips. Blocked on **MA-24** (needs_human). Everything the agent track could stage for it is in place (record script, dry-run, fingerprint manifest tooling, README flow — MA-20/MA-26 merged).
- **F-1 / F-2** (rows 39/40) — felt checkpoints, not signed off. F-1 has been DUE since M3 landed; decision log flags that the stale cassettes may force folding F-1 into F-2 after MA-24 (operator's call).
- **MA-22** (row 41, final sweep) — backlog, blocked on MA-24; owns flipping `E2E_REQUIRED` to "1" (closes D-1/row 5) and the all-harness closeout.

## Recommendations

- **Operator, in order:** (1) run the MA-24 sequence below; (2) dispatch MA-22 — it must flip `E2E_REQUIRED` to `"1"` in ci.yml and re-run H-1..H-9; (3) walk the smoke checklist incl. F-2 (and F-1, folded or separate). Re-validate row 5 at that point — it should convert to Pass.
- **Fix-back (bookkeeping, cheap):** append a retroactive decision-log entry for the E2E_REQUIRED parking (D-1); correct or fulfill the "evidence comments" claim for the 14 reconciled tickets (D-3).
- **New ticket (optional, post-run):** migrate test-file `__definition` reaches onto the public `runPipeline`/`invokeHandler` seam (D-8).
- **Accept-as-is:** esbuild low-severity npm advisory (below gate); anyhow warning-level advisory (Dependabot PR pending); force-with-lease on the #57 branch (D-4) unless the operator wants a G-3 post-mortem.

## What I couldn't verify

- **cargo-audit locally** required a Homebrew install this session (tool absent from the machine); result cross-checked against the green CI run on the identical merged SHA.
- **"CI green" sub-claims** were verified via per-PR check rollups (`SUCCESS:2` for all 23 PRs) and the merged-tip run conclusion — not by reading every step log of every historical run.
- **G-3's "no force-push" on main** is verifiable only circumstantially post-hoc (clean `--ff-only` pull, intact merge topology); the branch-level force-with-lease is documented in D-4. Final G-3 adjudication stays with the operator (row 38).
- The **c11-specific mechanics** of the playbook (surfaces, identity block) were skipped per dispatch instruction — this validator ran outside c11.

## Operator smoke-pass checklist (post-merge)

Every `runnable_at: post-merge-smoke` row from validation-plan.md, verbatim. The Result Validator does not adjudicate these (note: rows 36 and 38's commands were also exercised this session for the G-1/drift audits above — H-8 exit 0; merge topology clean — but the sign-off is the operator's).

| # | Criterion (ID) | Verification method | Artifact to inspect | Pass condition |
|---|---|---|---|---|
| 28 | AC-28 | **Operator**: `GEMINI_API_KEY=… npm run record`, then `npm run test:full` in cassette mode passes on the new cassettes | Merged main after MA-24 | e2e green on re-recorded cassettes |
| 36 | G-1 (smoke) | Run `scripts/check-planted-defects.sh` on final merged main | Merged main | Exit 0: poisoned example, missing guards, 3 planted lints, ignored smoking-gun all intact and correctly marked |
| 38 | G-3 | `git log --merges` + PR list on final main: no force-pushes, per-ticket conventional commits, every merge via green CI | Merged main history | History clean per G-3 |
| 39 | F-1 | **Operator** drives the detective pipeline end-to-end in cassette mode after M3 (MA-9..MA-14 merged): readable story, sensible per-issue commits | Merged main at M3 | Operator signs off F-1 |
| 40 | F-2 | **Operator** terminal drive on the upgraded stack after MA-22, immediately after AC-28 | Merged main at M4 | Operator signs off F-2 |
| 41 | Final sweep (MA-22) | Run all harnesses H-1..H-9 on merged main | Merged main | All nine green |

### MA-24 operator sequence (verbatim from MA-20's ticket comment — the UPDATED sequence, which supersedes the earlier one by adding the mandatory fingerprint step; without it the e2e treats cassettes as stale)

```
  cargo build -p falcon-mcp        # repo root, falcon-agent/ clean
  cd falcon-detective && npm ci
  npm run record -- --dry-run      # sanity, zero network
  GEMINI_API_KEY=<key> npm run record
  npm run e2e:fingerprint          # writes fixtures/cassettes/.e2e-fingerprint.json
  npm run test:full -- tests/e2e.test.ts
  git add fixtures/cassettes/      # cassettes + manifest
  git commit -m 'chore(falcon-detective): re-record cassettes (MA-24, AC-28)'
  git -C .. worktree remove --force .runs/<run-name>
```

Context from the same comment thread: ~5 live Gemini calls (3× gemini-2.5-pro + 2× gemini-2.5-flash), expected spend in cents; the script fails fast if the key is missing, `falcon-agent/` is dirty, or falcon-mcp is not built. After MA-24 lands, **MA-22 flips `E2E_REQUIRED` to "1"** in ci.yml per the parked note. Felt checkpoints: **F-1** (readable story, sensible per-issue commits — may fold into F-2 per the decision log) and **F-2** (terminal drive on the upgraded stack, immediately after AC-28).
