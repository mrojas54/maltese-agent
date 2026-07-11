# MA-15: Decompose processIssues and de-cast workflow stages

BUILDPLAN M4 / WS-14a (BUILDPLAN id MA-15). Decompose the processIssues main loop into named stage functions (each about 40 lines or less) - MA-13 tests stay green; de-cast workflows/tidy-and-debug.ts stages (no detective-any or as-any stage casts; stages typed against handler input/output types). ACs: AC-30, AC-34. Depends: MA-13. Size M.

## Base

Branch `claude/ma-15-decompose`, cut from `origin/claude/ma-13-orchestration-tests` @ cfff009 (PR #43, in review). Verified at plan time: that branch has not moved and #43 is not in `origin/main`, so no rebase needed yet; re-check at each phase boundary.

## Part 1 — AC-30: decompose `processIssues` (falcon-detective/src/handlers/processIssues.ts)

The MA-13 tests (`tests/handlers/processIssues.test.ts`) are the parity oracle and stay byte-for-byte unmodified. They pin: exact invoke sequences per scenario, exact `verify`/`commitOne` input shapes, snapshot `fs_read` idempotence, in-loop revert writes, terminal revert writes (3 in-loop + 1 terminal on exhaustion), single snapshot-client instance per issue, client closed at the end.

Stage functions (module-private, all typed — SPEC WS-14a names, each <= 40 lines):

1. `classifyIssue(invoke: InvokeHandler, ctx: StageContext, issue: Issue): Promise<TaggedIssue>`
   — steps 1–2 of the old loop: `readContext` then `classify`.
2. `proposeForIssue(invoke: InvokeHandler, tagged: TaggedIssue, poisonAnalysis: PoisonAnalysis | undefined, previousFailure: PreviousFailure | undefined): Promise<UnifiedDiff>`
   — rename + de-any of the existing `proposeFor` dispatch (Poison/Bug/Lint).
3. `applyAndVerify(invoke, ctx, snapMcp: FalconMcpClient, snapshots: Map<string,string>, tagged, poisonAnalysis, firstDiff): Promise<{ succeeded: boolean; finalDiff: UnifiedDiff }>`
   — the retry while-loop: snapshot → applyEdit → verify → on Broken capture previousFailure, revert snapshots, re-propose; exits on Clean/Stuck/conflict/exhaustion. In-loop revert errors propagate (as today).
4. `commitOrRevert(invoke, ctx, snapMcp, snapshots, issue: Issue, outcome: { succeeded, finalDiff }): Promise<void>`
   — terminal revert (per-write try/catch) when !succeeded, `snapMcp.close()`, then `commitOne` (its own try/catch) when succeeded. Called from the main loop's `finally` so a throwing retry loop still reverts+closes, exactly like the old `try/finally` + post-commit shape.

Supporting helpers (also <= 40 lines each):
- `prepareProposal(invoke, tagged, issueFile): Promise<{ poisonAnalysis, firstDiff } | undefined>` — one-shot `analyzePoison` for Poison issues + first proposal; `undefined` = skip issue (preserves the two `continue` paths and their console.error messages verbatim).
- `snapshotOnce(snapMcp, snapshots, path)` — idempotent pre-edit `fs_read` snapshot with the missing-file carve-out.
- `revertSnapshots(snapMcp, snapshots)` — throwing in-loop revert.

De-cast typing strategy inside processIssues: no `: any` / `as any` / `invoke<any>`. Types come from the handlers themselves via Barnum's public `ExtractOutput` phantom-type helper (`@barnum/barnum/pipeline`):
- `type PoisonAnalysis = ExtractOutput<typeof analyzePoison>`
- `type IssueContext = ExtractOutput<typeof readContext>`
- `type ApplyResult = ExtractOutput<typeof applyEdit>`
plus the exported zod-inferred types `TaggedIssue`, `UnifiedDiff`, `VerifyResult`, `PreviousFailure`, `Issue`, from `src/lib/types.ts`. `StageContext = { mcpBinary; worktreePath; cratePath }` local interface. Invoke call sites keep the exact per-handler input shapes (strict additionalProperties at the engine boundary — no context spreading).

## Part 2 — AC-34: de-cast `src/workflows/tidy-and-debug.ts`

`createHandler` already returns `Handler<TValue, TOutput> extends TypedAction<TValue, TOutput>` (post-WS-5 Barnum 0.4 typings), and the connection points align exactly:
- prepWorktree Out ≡ triage In ≡ `{ mcpBinary, worktreePath, cratePath }`-shape chain through processIssues/finalSweep;
- finalSweep's discriminated-union Out ≡ `BranchInput<{ Clean: typeof commitAll; Dirty: typeof escalate }>` (Clean value = commitAll In, Dirty value = escalate In).

So the de-cast is direct: drop `detective: any` and all seven `as any` casts; `export const detective = pipe(prepWorktree, triage, processIssues, finalSweep, branch({ Clean: commitAll, Dirty: escalate }))` with the inferred `TypedAction` type. `cli.ts` (`runPipeline(detective, initial)`) accepts this unchanged. Fallback if inference fails at a connection point (not expected): public `toAction()` — never `any`.

## Part 3 — H-9 gate append (SPEC R-5)

AC-34 cites the H-9 grep gate, so this PR appends `gate_no_workflow_any` to `scripts/debt-gates.sh` (above the marked append line): fail on `as any` or `: any` under `falcon-detective/src/workflows/`.

## Gates before commit (in worktree falcon-detective/)

- `npm ci`; `npm test` green with `git diff --stat -- tests/` empty (MA-13 oracle unmodified)
- `npx tsc --noEmit` clean
- `npx biome check .` clean
- grep gates: no `as any` / `: any` in `src/workflows/tidy-and-debug.ts`; no `any` reintroduced under `src/handlers/` (AC-12 gate stays green); `scripts/debt-gates.sh` all PASS

## Out of scope

- `lib/gemini.ts`, `package.json` (MA-18/MA-19 own them); falcon-agent (G-1); no test edits; no new features (N-1).
