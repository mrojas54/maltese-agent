# MA-13: Orchestration test coverage for detective core

BUILDPLAN M3 / WS-8 (BUILDPLAN id MA-13). Orchestration tests with mocked MCP client plus mocked/cassette Gemini: verify (Clean/Broken/Stuck), commit handlers (commitOne, commitAll, revertOne, escalate - success and git-failure paths, no silent catch), finalSweep (Clean and Dirty), processIssues (happy path; retry-then-success; revert-on-persistent-failure); smoke tests for cli.ts and workflows/tidy-and-debug.ts. Tests target CURRENT behavior - the MA-15 refactor must keep them green. ACs: AC-19, AC-20. Depends: MA-10, MA-11. Size L.

## Plan (delegator-ma-13)

### Constraints driving the design

- **Public seams only** (ticket + AC-13's `__definition` grep gate): new tests must not reach into Barnum's underscore-private surface. The public seams available: module exports, `makeProcessIssuesHandle(invoke)` (MA-11's injectable seam), the schema'd `FalconMcpClient.call(tool, args, schema)` boundary (MA-10), and Barnum's exported AST types (`Action`, `ChainAction`, `BranchAction`, `InvokeAction` from `@barnum/barnum/pipeline`).
- **Engine budget**: `runPipeline` spawns engine binary + tsx worker (~0.9s/call); the suite deliberately confines engine touches to one existing test (invokeHandler.test.ts). New tests stay in-process.
- **`vi.mock` cannot cross the engine process boundary** — so handlers under test must be invocable in-process. `verify`/`commit*`/`finalSweep` handle bodies are currently inline closures inside `createHandler`; they need a public export.
- **MA-12 parallel contract**: no assertions on propose-handler internals; fake-invoke matches handlers by object identity of the module exports (names preserved by MA-12).
- **MA-15 forward-compat**: processIssues tests go through `makeProcessIssuesHandle` (the factory survives WS-14's stage decomposition); handler tests go through exported handle functions, not loop internals.
- **G-1**: zero falcon-agent touches. Hermetic: no network, no real git side effects outside temp dirs; fast suite stays ≤60s (baseline: 87 tests / ~2.9s wall).

### Source changes (enabling, behavior-preserving)

1. `src/handlers/verify.ts` — extract inline handle to `export async function verifyHandle(...)`; `createHandler({ ..., handle: verifyHandle }, "verify")` unchanged.
2. `src/handlers/commit.ts` — same extraction: `commitOneHandle`, `commitAllHandle`, `revertOneHandle`, `escalateHandle`; hoist the `.extend(...)` input schemas to named consts for the extracted signatures.
3. `src/handlers/finalSweep.ts` — extract `finalSweepHandle`.

No other src edits. Each export gets a doc comment naming MA-13/AC-19 as the reason (public module-export seam for orchestration tests).

### Seam inventory (everything mocked, and how)

| Seam | Mechanism | Files |
|---|---|---|
| `FalconMcpClient` (MA-10 schema'd boundary) | `vi.mock("../../src/lib/mcp.js")` → shared fake in `tests/helpers/fakeMcp.ts`; fake `.call()` still runs `schema.parse(cannedResult)` so the zod boundary stays real; records `{tool, args}` per instance; scripted per-test responder can throw to simulate git/tool failure | verify, commit, finalSweep, processIssues tests |
| Handler invocation (MA-11 seam) | `makeProcessIssuesHandle(fakeInvoke)`; fake matches `readContext`/`classify`/`analyzePoison`/`propose*`/`applyEdit`/`verify`/`commitOne` by OBJECT IDENTITY against the same imports processIssues uses | processIssues tests |
| Gemini | never reached: propose* handlers are faked wholesale at the invoke seam (their cassette-mode unit tests already exist); no `Gemini` instantiation anywhere in new tests → hermetic by construction | processIssues tests |
| CLI argv | subprocess: `node node_modules/tsx/dist/cli.mjs src/cli.ts` with controlled argv; no mocks | cli test |
| Workflow AST | pure inspection of `detective`'s exported Action tree (Chain/Invoke/Branch); no mocks, no engine | workflow smoke |

### Test files and scenarios (ticket text → tests)

- `tests/handlers/verify.test.ts` (AC-19 "verify (Clean/Broken/Stuck)")
  1. Clean: cargo_check errors=[] then cargo_test failed=[] → `{kind:"Clean"}`; client closed.
  2. Broken via cargo_check errors → messages mapped into `value.errors`; cargo_test never called.
  3. Broken via cargo_test failures → failed test names mapped into `value.errors`.
  4. Stuck: `attempt: 3` → `{kind:"Stuck", value:{attempts:3}}` and NO FalconMcpClient constructed.
  5. MCP failure propagates: cargo_check throws → rejects (no silent catch), client still closed (finally).
- `tests/handlers/commit.test.ts` (AC-19 "commitOne, commitAll, revertOne, escalate — success and git-failure paths, no silent catch")
  1. commitOne success: `git_add {paths:[diffPath]}` then `git_commit` message `fix(bug): src/lib.rs:3`; returns `{sha}`; closed.
  2. commitOne message without line number (`fix(lint): src/x.rs`).
  3. commitOne git_add failure → rejects with that error; client still closed.
  4. commitOne git_commit failure → rejects.
  5. commitAll → `{ok:true}` marker; constructs no MCP client.
  6. revertOne success: exec client (`enableExec:true`), `exec_run {cmd:"git", args:["checkout","--",file]}` → `{ok:true}`.
  7. revertOne exec_run failure → still resolves `{ok:true}` and closes (documents the deliberate, commented no-op carve-out — CURRENT behavior per ticket).
  8. escalate: `console.error` spied — message contains `reasons.join("; ")`; returns `{escalated:true}`; no MCP client.
- `tests/handlers/finalSweep.test.ts` (AC-19 "finalSweep (Clean and Dirty)")
  1. Clean: no failed tests, no lints → `{kind:"Clean", value:{mcpBinary, worktreePath}}` (ctx threading pinned).
  2. Dirty: 2 failed tests + 3 lints → reasons `["2 tests still failing","3 clippy lints remain"]`, ctx + reasons in value.
  3. cargo_test throws → rejects, client still closed.
- `tests/handlers/processIssues.test.ts` (AC-19 3 loop scenarios; via `makeProcessIssuesHandle(fake)` + mocked mcp module for the snapshot client)
  1. Happy path: read→classify(Bug)→propose→apply ok→verify Clean(attempt 0)→commitOne with right diffPath; snapshot fs_read taken once; NO fs_write (no revert on success); snapshot client closed.
  2. Retry-then-success: verify Broken once then Clean; asserts snapshot revert (fs_write with original content) between attempts; 2nd propose call carries `previousFailure {diff: firstDiff, errors}`; verify called with attempt 0 then 1; commitOne runs.
  3. Revert-on-persistent-failure: verify Broken ×3 (MAX_RETRIES) → propose ×3 (2 with previousFailure), fs_write revert ×3 in-loop + 1 terminal revert (4 total), commitOne NEVER invoked, output passes through `{mcpBinary, worktreePath, cratePath}`.
- `tests/cli.test.ts` (AC-20 "arg parsing" smoke): spawn tsx `src/cli.ts` with no args → exit 1, stderr contains `missing required --target`. Single subprocess, ~1s.
- `tests/workflows/tidy-and-debug.test.ts` (AC-20 "pipeline composes and stages connect"): flatten `detective`'s Chain AST → exactly `[prepWorktree, triage, processIssues, finalSweep, Branch]` **by object identity** with the imported handler exports; Branch cases keyed `Clean`→`commitAll`, `Dirty`→`escalate` (identity); each Invoke node carries input/output schemas.

### Gates & sequencing

1. Write helper + tests, extract handles (RED→GREEN inside one ticket-commit per G-3).
2. Gates in `falcon-detective/`: `npm ci` (done — baseline 87/87 in ~2.9s), `npm test` green and ≤60s, `npx tsc --noEmit`, `npx biome check .`.
3. One commit: `test(falcon-detective): orchestration coverage for detective core (MA-13)`.
4. Own-review vs `origin/claude/ma-10-mcp-schemas`, fix Critical/Major, attach review comment; push; PR to `main` with "Stacked on #40" first line; comment PR URL on ticket; status → review.

### Self-review notes (plan vs code, done before `planned`)

- `verify` Stuck path returns BEFORE constructing the client → scenario 4's "no client" assertion matches code (verify.ts:19-20).
- commitOne message template pins `:line` suffix only when `location.line` is set (commit.ts:31) → scenarios 1-2 match.
- revertOne swallows exec_run rejection by design (commit.ts:77-79 comment) → scenario 7 tests CURRENT behavior, consistent with "no silent catch" applying to commit/finalSweep paths that must surface errors.
- processIssues loop trace verified against src (attempts 0/1/2, revert-before-repropose, terminal revert in `finally` only when `!succeeded`, fs_write counts 0/1/4 per scenario).
- Fake mcp `.call()` parses through the REAL schema arg → toolSchemas stay load-bearing in these tests (MA-10 seam honored, not bypassed).
- No `__definition` anywhere in new code (AC-13 grep gate stays clean for MA-13's diff).
