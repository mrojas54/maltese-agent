# Orchestration run-state — maltese-agent tech-debt remediation

Contract: SPEC.md + EVALUATION.md + BUILDPLAN.md at repo root (binding commit 9f72697, operator-approved 2026-07-10). Arc anchor: `sequence/run-state.md`.

## ⚓ RESUME ANCHOR — 2026-07-10 ~21:45 EDT (read this first; supersedes sections below on conflict)

**RUN TERMINAL — 2026-07-10 ~23:3x EDT.** 29/29 in-contract tickets `done`; PR #65 merged (ecef4062), E2E_REQUIRED hard gate LIVE on main; validation GREEN; **F-1 CONFIRMED, F-2 CONFIRMED** (see items 2–3). Closeout audit DONE: war stories → skill runs-ledger; timeless findings → `LESSONS.md` (new, pointed from root CLAUDE.md); arc anchor `sequence/run-state.md` marked ARC COMPLETE. Post-run backlog (out of contract, NOT dispatched): MA-30/31/32 (reverted-major migrations), MA-33 (CLI narration, felt finding F-2-1), MA-34 (cargo_clippy empty-lints diagnosis, operator-committed). Unminted candidates offered at closeout: native fs_search (drop rg PATH dep), fs_list empty-`entries` fix. Remaining mechanical tail: board-sync PR (this file + board events + lessons), then worktree/`.runs` cleanup (operator-authorized batch).

**Next session's task list, in order:**
1. **MA-22 DONE — PR #65 MERGED (ecef4062, 2026-07-11T02:43:04Z), board completed with evidence comment. Agent track 29/29.** Commit 53dcc00: E2E_REQUIRED "0"→"1" + parked-comment rewrite; first CI run with the hard gate live passed on the PR itself (153/153 full suite, e2e replay in cassette mode). All H-1..H-9 green; debt-gates closeout found no gaps (AC-7/8/12/13/22/23/25×2/34). MA-22 worktree `.claude/worktrees/ma-22-final-sweep` now removable at cleanup.
2. **F-1 CONFIRMED by operator 2026-07-10 ~22:4x EDT** — operator personally inspected `.runs/diag-timed` commits (quoted 2deed22 `fix(poison)` prompt.rs, a0b4aca `fix(lint)` interrogate.rs:3) and said "f-1 confirmed" in chat. **2-vs-3 sub-call RESOLVED ~23:0x EDT: operator accepts the 2-act story** ("accept 2 but at closeout mint the diagnosis as a follow up ticket") — the 3.1 cassettes as landed are canon for this run; F-2 judges the 2-act story. COMMITTED closeout action: mint the `cargo_clippy returns lints: []` pipeline-diagnosis ticket (why the main.rs planted lint never reaches triage).
3. **F-2 CONFIRMED by operator 2026-07-10 ~23:2x EDT** — drive executed on merged final stack (run worktree based on ecef406 ✓): ed7dc20 fix(lint) −1 line (planted unused import), d6bb9a5 fix(poison) −3 lines (poisoned few-shot example), clean final sweep, no escalation. Operator judged commits PASS. **Felt finding F-2-1:** CLI console prints only start/complete — run story invisible without digging into `.runs/<name>` git history; operator commissioned a narration ticket (MA-33). One process note: operator's first drive ran on a stale tree (pull aborted on untracked cassette duplicates, chained command ran anyway) — treated as rehearsal; duplicates preserved at scratchpad/cassette-backup, re-drive on ecef406 was the real F-2.
4. **Closeout audit** (Phase 2 tail): read run comments/commits/agents.md archive, extract timeless lessons → project LESSONS.md / CLAUDE.md / skill runs-ledger (war stories to runs-ledger, principles to the skill); then commit the live board (`.lattice/` changes in the primary are uncommitted) as a final board-sync PR; clean up fleet worktrees + stale `.runs/*`.

**Standing facts a fresh session must know:**
- Primary checkout `/Users/michellerojas/maltese-agent` = canonical Lattice board (run all `lattice` mutations from there; `--actor agent:orchestrator-resume`). Its untracked `falcon-detective/fixtures/cassettes/*.json` duplicates become tracked when #64 merges + `git pull` (identical content; the untracked `.e2e-fingerprint.json` there is STALE vs the PR's — delete local copies if pull complains).
- Merges: `GH_TOKEN=$(/usr/local/bin/gh auth token) /usr/local/bin/gh pr merge <N> --repo mrojas54/maltese-agent --merge` — ONLY after explicit operator authorization in chat (classifier scopes it per-batch); verify `.state == MERGED` after.
- Cleanup owed at closeout: fleet worktrees under `.claude/worktrees/` (ma-*, revert-captain, ma-24-cassettes), stale `.runs/record-*` + diag worktrees (`git worktree remove --force` + prune), scratchpad diag files.
- Open Dependabot PRs remain (out of scope); three majors were REVERTED on main (PR #63: rmcp 2.2, typescript 7, biome 2.5) — **future-migration tickets MINTED 2026-07-10 ~22:1x EDT at operator request: MA-30 (rmcp 2.x, L), MA-31 (typescript 7, M), MA-32 (biome 2.x, S)**; backlog, tagged `out-of-contract`, NOT dispatched in this run. MA-31↔MA-32 serialize on falcon-detective package.json.
- Closeout minting: **cargo_clippy `lints: []` diagnosis ticket is OPERATOR-COMMITTED** (2-vs-3 resolution — mint at closeout, not optional). Still candidates (offer at closeout): native fs_search in falcon-mcp (drop the rg PATH dependency), fix the empty `fs_list` listing (`entries: []` — silently broken evidence).
- Model policy: delegators inherit session model (intentional; hook reminders acknowledged); read-only/spike agents on Sonnet.

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
- 2026-07-10 [operator] Future-migration tickets minted for the PR #63 reverted majors: MA-30 (rmcp 1.6→2.x, chore/L), MA-31 (typescript 5.9→7.x, chore/M), MA-32 (biome 1.9→2.x, chore/S). Out of contract 9f72697 — no AC IDs, no dispatch this run; verbose descriptions carry revert provenance, breakage evidence, gates, and the MA-31↔MA-32 package.json serialization. Remaining closeout candidates (fs_search/fs_list/cargo_clippy) intentionally left unminted pending operator call.
- 2026-07-10 [operator] **F-1 confirmed** (EVALUATION.md:101): operator reviewed `.runs/diag-timed` evidence directly — readable story, sensible per-issue commits (2deed22 fix(poison), a0b4aca fix(lint)). The 2-vs-3-issue triage felt call remains open as a separate sub-decision.
- 2026-07-10 [operator] **2-vs-3 triage story: ACCEPT 2.** The 3.1-era 2-act story (interrogate.rs lint + poison) is canon for this run; no prompt tuning, no third re-record. Condition: the `cargo_clippy lints: []` pipeline-diagnosis ticket MUST be minted at closeout (upgraded from candidate to committed). Rationale echoed from briefing: the missing act is partly a pipeline signal bug, not purely model judgment — fix the signal first, restore act 3 opportunistically at a future re-record.
- 2026-07-10 [operator] MA-22 dispatch authorized and executed ("MA-22 dispatc").
- 2026-07-10 [operator] "merge pr 65" — PR #65 merged (merge commit ecef4062, verified .state==MERGED), MA-22 completed with evidence comment via plain status walk (complete-ceremony crash avoided). **Agent track terminal: 29/29.** E2E_REQUIRED hard gate live on main. Run proceeds to F-2 (operator) then closeout audit. Fast-track delegator as background Agent subagent, model inherited per pinned policy (dispatch hook reminder acknowledged, intentional). Worktree ma-22-final-sweep off origin/main@f176529; boot prompt adapts Clause 1 to Agent-tool spawning (per-command cd anchoring + pre-commit toplevel guard, since Bash cwd doesn't persist and there's no c11 spawn-side cd). G-1, no-dep-upgrades, no-cassette-touch guardrails baked into boot. Orchestrator awaits harness completion notification — no polling loop (established pattern).

## Resume — 2026-07-10 ~10:45 EDT (orchestrator session 2, actor agent:orchestrator-resume)

**Found on re-bind:** board↔git drift. A prior fleet session merged 14 ticket PRs (verified via `gh pr view` state MERGED, CI green on main through 65142ba) but never wrote status back to the board — all tickets still `backlog`. The mint-time table above is a historical snapshot; **Lattice is ground truth**.

- Reconciled to `done` with per-ticket PR-evidence comments: MA-25(#26) MA-4(#25) MA-1(#27) MA-8(#28) MA-11(#29) MA-9(#30) MA-2(#31) MA-14(#32) MA-17(#33) MA-3(#34) MA-16(#35) MA-21(#36) MA-5(#37) MA-7(#38).
- Remaining fleet: MA-6, MA-10, MA-23 (wave 1, dispatched); MA-12, MA-13, MA-18 (behind MA-10); MA-15 (behind MA-13); MA-19 (behind MA-18, `lib/gemini.ts` serial); MA-20 (behind MA-19); MA-22 (blocked on human MA-24).
- Wave-1 worktrees: `.claude/worktrees/ma-6-supply-chain` (branch `claude/ma-6-supply-chain`), `.claude/worktrees/ma-10-mcp-schemas` (branch `claude/ma-10-mcp-schemas`), both off origin/main@65142ba, `.env` propagated. MA-23 is read-only (no worktree).
- F-1 felt checkpoint fires when MA-10, MA-12, MA-13 merge (MA-9/11/14 already in).

## Decision log (append-only, resume session)

- 2026-07-10 [autonomy: Moderate] Board reconciliation performed by orchestrator (bookkeeping, not implementation): backlog→planned→review walk + `status done`, evidence comment citing PR + merge SHA on each of the 14 tickets.
- 2026-07-10 [autonomy: Moderate] MA-23 dispatched as retroactive verification (Sonnet, read-only): its consumer MA-11 already merged via PR #29 using "public runPipeline"; spike verdict comment still owed to keep the board honest.
- 2026-07-10 [autonomy: Moderate] Delegators run as background Agent-tool subagents (no c11); MA-6/MA-10 inherit session model per config, spike on Sonnet. Harness completion notifications replace /loop ticks — no shell sleep loops.
- 2026-07-10 [autonomy: Moderate] MA-6 CI red on its own cargo-audit gate (RUSTSEC-2026-0185, quinn-proto 0.11.14): fix roundtrip via delegator resume, lockfile-only bump to 0.11.16; PR #39 green at cf3656d. Gate validated on first contact.
- 2026-07-10 [FORCED — harness] `gh pr merge` denied by the session permission classifier (self-authored PR, no human approval). **Merge policy downgraded: auto-merge → leave at terminal pre-merge status (`review`, PR open, CI green).** Operator merges, or grants a Bash permission rule for `gh pr merge`, or explicitly instructs merge path. Press-ahead unaffected (fires at `review`). Escalation raised 2026-07-10 ~11:05 EDT.

## MA-24 blocker & MA-27 — 2026-07-10 ~14:0x EDT

Operator hit a live 404 mid-re-record: **gemini-2.5-pro retired for new API users** (generateContent 404; ListModels still lists it — the listing lies). Orchestrator probes blocked (sandbox allows GET, blocks POST) → operator ran probes in own terminal. **Operator decision: 3.1 family** → GEMINI_PRO = gemini-3.1-pro-preview (only 3.1 pro; preview-retirement risk accepted knowingly), GEMINI_FLASH = gemini-3.1-flash-lite. Fix-it **MA-27** minted (bug, critical, blocks MA-24 → MA-22) and dispatched off origin/main@abfeb39; scope: models.ts + README refs + MA-24 skip-markers for cassette-keyed unit tests that hard-fail on miss. Old 2.5 cassettes left for MA-24's stale-leftover deletion. MA-8's centralization paid off: one authoritative site.

| Symptom | Cause | Mitigation |
|---|---|---|
| Model in ListModels but generateContent 404s "no longer available to new users" | Google gates retired models at generation time for new users; listing not filtered | Probe with a tiny generateContent, never trust ListModels for availability |
| Orchestrator curl POSTs return empty (GETs fine) | Session sandbox blocks outbound POST bodies | Live-API verification is operator-track; agents verify statically + via operator's run |

## MA-24 blocker #2 & MA-28 — 2026-07-10 ~14:3x EDT

Operator's post-MA-27 record attempt (7 run-worktrees total under .runs/; earlier ones were the 2.5 404s) failed at applyEdit: **gemini-3.1 emits hunk headers with full-context-window counts while truncating trailing context in the body** (exact input preserved in cassette 896f6e7b4af1a7c7.json — header @@ -1,6 +1,5 @@ over a 4/3 body); patch 0.7 panics; MA-16's catch_unwind surfaced it cleanly; retries exhausted. Fix-it **MA-28** minted (bug, critical, blocks MA-24 → MA-22), dispatched off origin/main@9390f2c: deterministic `normalize_hunk_counts` (recount from body, rewrite header) at the validate→parse seam in falcon-mcp/src/tools/fs_basic.rs, recorded diff as verbatim regression fixture.

**For operator's F-2 attention (model-judgment drift, not a code defect):** the 3.1 triage cassette (7e3f9cb8) found 2 issues (interrogate.rs lint + poison) vs the canonical 3 — it missed main.rs's default_constructed_unit_structs lint. Workshop story impact is an operator felt-call; prompt tuning would be new scope.

| Symptom | Cause | Mitigation |
|---|---|---|
| patch crate panic "failed to parse entire input. Remaining: <context line>" on model diffs | LLM (3.1) declares context-window line counts in @@ headers but truncates trailing context in the body | MA-28 normalize-recount before parse; never trust model-declared hunk counts |

## RED MAIN & Revert Captain — 2026-07-10 ~20:5x EDT

Operator bulk-merged 13 Dependabot PRs mid-run; three were untested MAJOR bumps that broke main: **#48 rmcp 1.6→2.2 (falcon-mcp no longer compiles: E0432 unresolved import rmcp::model::Content — both CI jobs fail since the TS job builds the binary for e2e)**, #50 typescript 5.9→7.0.2 (untested), #54 biome 1.9.4→2.5.3 (`npm run lint` broken: 2.x CLI vs unmigrated 1.x config, verified by MA-29 delegator). PR #62's red checks are INHERITED from main (branch predates the breakage; own gates green). **Revert Captain dispatched** (one-shot, branch claude/revert-major-bumps off origin/main@60b244d): revert the three merges newest-first, keep all minor bumps, full gates, PR for operator merge. Deliberate migrations (rmcp 2.x especially — May 2026 memory: rmcp 1.x already had breaking tool-result drift) to be scheduled as future tickets, out of this contract. Post-recovery sequence: merge revert PR → re-run #62 → merge #62 → regenerate `.e2e-fingerprint.json` → commit operator's fresh cassettes → dispatch MA-22.

## MA-24 verification & MA-29 — 2026-07-10 ~20:2x EDT

Operator completed the re-record (4 cassettes @20:04, fingerprint @20:06, binary @20:01). `test:full` then failed; orchestrator diagnosis chain: (1) stale `.runs/e2e-test` worktree collision (leftover from failed runs — e2e never cleans up); (2) after cleanup, e2e ran 53s then "cassette miss" — **root cause: e2e executes `dist/cli.js` but nothing builds it; dist predated MA-27 and hashed keys with `gemini-2.5-pro`** (found via GEMINI_DEBUG_PROMPTS dump: `model=gemini-2.5-pro` line 1) while cassettes are 3.1-keyed; record runs tsx (source) — record/replay executed different code; (3) after `npm run build`, e2e times out at 120s — the healthy full replay is ~114s + smoke; (4) side-find: `cassetteDir()` cwd fallback resolves wrong dir for repo-root-spawned workers (dump landed at repo-root/fixtures). **Direct CLI replay SUCCEEDED in 114s with the correct story: `fix(lint)` 1-deletion + `fix(poison)` 3-deletion per-issue commits — F-1 evidence preserved at `.runs/diag-timed`.** Fix-it **MA-29** minted (bug, high) + dispatched off origin/main@e81fba0 (operator has been merging Dependabot PRs — #44, #47 in main now): e2e beforeAll build, idempotent worktree cleanup (KEEP_RUN=1 escape), 360s e2e timeout, package-anchored cassetteDir fallback.

| Symptom | Cause | Mitigation |
|---|---|---|
| e2e "cassette miss" with fresh cassettes + matching fingerprint | e2e runs `dist/cli.js`, never built; record runs tsx — stale dist hashes old model ID into keys | MA-29: build in beforeAll. Diagnostic: GEMINI_DEBUG_PROMPTS=1 dumps normalized prompts; check the `model=` line first |
| Every e2e failure masks the next ("worktree already exists") | e2e leaves `.runs/e2e-test` behind on failure paths | MA-29: pre-clean + afterAll cleanup; manual: `git worktree remove --force .runs/e2e-test && git worktree prune` |
| GEMINI_DEBUG_PROMPTS dump lands at repo-root/fixtures | `cassetteDir()` falls back to cwd | MA-29: package-anchored fallback; always pass CASSETTE_DIR when invoking the CLI from elsewhere |

## Validation & drift corrections — 2026-07-10 ~13:3x EDT

Result Validator verdict: **GREEN with one flagged deferral** — 35 PASS / 1 PARTIAL / 0 FAIL over 36 static rows; report at `validation-report.md`. Drift corrections applied:

- [retroactive, validator D-1] PR #31 (prior session) parked the CI hard gate at `E2E_REQUIRED: "0"` (commit ddd2bab) because cassettes are stale pre-MA-24; MA-22 flips it to "1". This decision predates the resume and was never logged — logged now. It is the sole PARTIAL (AC-5 env-var clause).
- [validator D-3] The 14 reconciliation evidence comments claimed above were in fact lost to the `lattice complete` artifacts-dir crash (ceremony died before comment persistence). Re-posted for real on all 14 tickets; run-state's earlier claim stands corrected.
- [validator D-4] G-3 force-push exception: PR #57's branch was rebased + `--force-with-lease` pushed on explicit orchestrator authorization to resolve merge conflicts vs #56 (branch-level, pre-merge; `main` never rewritten).
- Closeout audit deferred until true terminal (MA-24 → MA-22 → F-2) — decision logged; the run is not closable before the human track completes.

## Fleet completion — 2026-07-10 ~13:0x EDT

All 24 agent-track tickets done. Final merges (operator-authorized batches): #56 (dcfda63), #58 (dda70de), #57 (abfeb39, after union conflict rebase vs #56). Remaining: MA-24 (needs_human), MA-22 (final sweep, blocked on MA-24). **Phase 2 started:** Result Validator dispatched (fresh session, inherits session model — final-review class per model policy); walks validation-plan.md pre-merge-static rows against merged main; writes validation-report.md. F-1 banner stands (see above). Dependabot PRs (#44-55 range) remain open, out of run scope.

## Run-time footguns

(rows added during dispatch — see orchestrator.md)

| Symptom | Cause | Mitigation |
|---|---|---|
| `lattice complete` crashes with `FileNotFoundError` under `.lattice/artifacts/…`; ticket left in `review` | Fresh-minted board lacks BOTH `artifacts/payload/` and `artifacts/meta/`; neither `shutil.copy2` nor `atomic_write` creates parent dirs. Crash happens AFTER the review comment + status→review events land | `mkdir -p <root>/.lattice/artifacts/{payload,meta}` before first `complete`; recover partial ceremonies with plain `lattice status <ID> done` (comment already exists) |
| `lattice next` proposes a ticket whose `depends_on` is unmerged (suggested MA-13 while MA-10 in flight) | `next` ranks by priority/status only; relationship edges not consulted | Dispatch decisions come from the orchestrator's dependency map, never from `lattice next` |
| `lattice code-review` / `plan-review` invocations fail | Commands do not exist in lattice-tracker 0.2.0 (this install) — the playbook's review commands are from a newer/other install | Skip straight to own-reviewer protocol (diff + structured verdict attached `--role review`); orchestrator adds independent fresh-eyes reviewer for L / G-1-critical tickets |
| CI TS job exits 1 with ALL tests passing (`Errors: 1 error` in vitest summary) | e2e cassette-staleness check runs the full pipeline (~62s) on a blocked worker event loop; vitest worker RPC `onTaskUpdate` times out → unhandled error | MA-26 fix (staleness pre-check + async spawn). Until merged: read the vitest summary, not the exit code — 100% pass + 1 unhandled `[vitest-worker]` error = this flake |
| Agent-tool delegator "backgrounds test + monitor, ends turn, waits for wake" → test process dies instantly, delegator idles forever | Background children of an Agent-tool subagent die when its turn ends; "monitor wakes me" only works while the parent holds the turn | Boot prompts must say: long-running proofs run FOREGROUND with an explicit Bash timeout; orchestrator recovery = SendMessage nudge + pgrep verification (MA-22, 2026-07-10) |
| `lattice attach --role validation` and `--type reference --inline` rejected by CLI | lattice-tracker 0.2.0 vocabulary: `--role` accepts review but not validation; `reference` type wants the URL as SOURCE positional, not `--inline` | Validation evidence as plain note; references as `lattice attach <ID> --type reference <URL>` (MA-22 delegator flags, 2026-07-10) |

- 2026-07-10 [autonomy: Moderate] Press-ahead wave 2 at MA-10 `review` (not merge): MA-12, MA-13, MA-18 dispatched, worktrees cut from origin/claude/ma-10-mcp-schemas@37c7fcd, PR bodies name the #40 anchor, base main. MA-12↔MA-13 open contract (public-behavior-only assertions) written into both boot prompts. MA-19 stays queued behind MA-18 (`lib/gemini.ts` serial); MA-20 behind MA-19; MA-15 behind MA-13.
- 2026-07-10 [autonomy: Moderate] MA-18 → review (PR #41, 38c219f). Press-ahead: MA-19 dispatched stacked on #41←#40 (deep stack), worktree off origin/claude/ma-18-gemini-retry. MA-19 boot prompt forbids live-API calls and cassette deletion/re-record (MA-24 is the human track); cassette-replay breakage under 2.x → marked skips naming MA-24.
- 2026-07-10 [operator] "you can merge them now" — merge gate lifted. Merge train executed in dependency order, merge commits, each verified `.state == MERGED`: #40 (6916083), #39 (44c1d28), #41 (4ed2a52, after flake re-run went green), #42 (2eedbc6). Tickets MA-10/6/18/12 completed. Interpretation logged: operator authorization + Phase-0 auto-merge config ⇒ auto-merge restored for CI-green, review-passed PRs; #43 additionally held for the L-ticket fresh-eyes verdict.
- 2026-07-10 [autonomy: Moderate] Press-ahead: MA-15 dispatched (stacked on #43), MA-20 dispatched (stacked on #46). Dependabot activated on MA-6 merge — 11 update branches/PRs opened (incl. anyhow 1.0.103 which closes RUSTSEC-2026-0190 warning); OUT OF RUN SCOPE, surfaced to operator at closeout.
- 2026-07-10 [operator] "merge auth approved" — #43 merged (57524c8, fresh-eyes PASS/MERGE) and #46 merged (ee33cb0); MA-13/MA-19 completed. **M3 fully landed → F-1 felt checkpoint DUE** (EVALUATION.md:101): operator runs the detective pipeline end-to-end in cassette mode, confirms readable output + sensible per-issue commits. Surfaced 2026-07-10 ~12:3x EDT; re-banner every tick until done. NOTE: cassettes are stale vs current workflow code until MA-24 — F-1's cassette-mode drive may hit the stale-skip; if so, operator may fold F-1 into F-2 after re-record (operator's call, flagged in banner).
- 2026-07-10 [autonomy: Moderate] PR #41 TS job red diagnosed as cross-cutting flake, not MA-18 defect: e2e staleness detection burns ~62s of blocked vitest worker → `Timeout calling "onTaskUpdate"` unhandled error → exit 1 with 103/103 passing. Staleness expected until MA-24; MA-19 guarantees recurrence. Per the contract's slow/flaky-suite norm: fix-it ticket **MA-26** minted (bug, high) and dispatched off origin/main — staleness pre-check fast path + async pipeline spawn + guard-semantics preservation. #41 failed job re-run triggered for a clean signal. PR #40 green and unaffected.
