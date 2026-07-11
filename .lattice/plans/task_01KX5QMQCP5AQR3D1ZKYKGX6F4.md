# MA-10: Schema-validated MCP client boundary

BUILDPLAN M3 / WS-4a/b (BUILDPLAN id MA-10). toolSchemas.ts zod schemas per MCP tool result; FalconMcpClient.call (mcp.ts:57 - the method is call, not callTool) becomes schema-required and validates before returning; migrate all call sites; proposePoisonFix gets real schemas reusing Issue/PoisonReport from types.ts (a TriageReport type does NOT exist); remove the TriageOutput-as-any cast at triage.ts:134. ACs: AC-12, AC-15. Depends: MA-07. Serialize: types.ts, lib/mcp.ts. Size M.

---

## Plan (delegator-ma-10, 2026-07-10)

### Drift check vs the contract

- `FalconMcpClient.call` is now at `falcon-detective/src/lib/mcp.ts:60` (not :57); the method is `call`, confirmed.
- The `as any` cast drifted from `triage.ts:134` to **`triage.ts:168`** (`schema: TriageOutput as any`). The adjacent `(await gemini.call(...)) as { issues: any[] }` cast at :169 is removed too.
- `types.ts` exports exactly: `IssueKind`, `Location`, `Issue`, `Excerpt`, `TaggedIssue`, `PoisonReport`, `UnifiedDiff`, `PreviousFailure`, `VerifyResult` (no `TriageReport` — confirmed).
- zod is already a dependency (`zod@^4.3.6`) — no package.json change needed.
- MA-08 (models.ts), MA-17 (ToolError taxonomy), MA-04 (debt-gates.sh skeleton) are already on main. Biome + vitest 3 + TS 5.9 in place (MA-5/7).

### 1. New module: `src/lib/toolSchemas.ts`

One zod schema per MCP tool result the detective consumes, mirroring the
serde wire shapes in `falcon-mcp/src/tools/*.rs` (source of truth listed per
schema in doc comments):

| Tool | Schema | Wire shape (from Rust struct) |
|---|---|---|
| `fs_read` | `FsReadResult` | `{ content: string, bytes: number }` |
| `fs_write` | `FsWriteResult` | `{ bytes: number }` |
| `fs_list` | `FsListResult` | `{ entries: string[] }` |
| `fs_apply_patch` | `FsApplyPatchResult` | `{ result: "ok"\|"conflict", lines_changed?: number, reason?: string }` |
| `fs_search` | `FsSearchResult` | `{ matches: {file,line,column,text}[], truncated: boolean }` |
| `cargo_check` | `CargoCheckResult` | `{ errors: CargoMessage[], warnings: CargoMessage[] }` |
| `cargo_test` | `CargoTestResult` | `{ passed: string[], failed: {name,stdout}[], duration_ms: number }` |
| `cargo_clippy` | `CargoClippyResult` | `{ lints: CargoMessage[] }` |
| `git_worktree_add` | `WorktreeAddResult` | `{ path: string }` |
| `git_add` | `OkResult` | `{ ok: boolean }` (falcon-mcp `util::OkResult`) |
| `git_commit` | `GitCommitResult` | `{ sha: string }` |
| `exec_run` | `ExecRunResult` | `{ stdout: string, stderr: string, exit: number }` |

`CargoMessage` = `{ level: string, message: string, file: string|null, line: number|null, help?: string[] }`
(`file`/`line` are plain `Option` → serialized as null; `help` has
`skip_serializing_if = "Vec::is_empty"` → absent when empty; schema uses
`.nullish()` / `.optional()` accordingly).

Follows the types.ts convention: `export const X = z.object(...); export type X = z.infer<typeof X>;`.
Also exports `TOOL_RESULT_SCHEMAS` (tool-name → schema record) so tests can
enumerate coverage, and `type ToolName = keyof typeof TOOL_RESULT_SCHEMAS`.

Not schematized: tools the detective never calls (`fs_search_ast`, `cargo_fmt`,
`git_diff`, `git_log`, `git_blame`, `git_worktree_remove`, `prompt_lint`) —
schemas are added when a consumer appears, keeping the module honest about
what the client boundary actually exercises.

### 2. `FalconMcpClient.call` becomes schema-required (`src/lib/mcp.ts`)

```ts
async call<T>(toolName: string, args: Record<string, unknown>, schema: ZodType<T>): Promise<T>
```

- Third parameter is **required**; the old `<T = unknown>` cast-through is gone.
- Validation strategy: **`safeParse`** on `result.structuredContent ?? result.content`
  (falcon-mcp emits `structuredContent` via rmcp's `Json<T>` for every tool;
  content fallback retained defensively — a mismatch there fails loudly through
  the same schema). On failure throw **`McpValidationError(toolName, zodIssues)`**
  (SPEC WS-4 literal signature) — an `Error` subclass exported from `mcp.ts`
  carrying `.toolName` and `.issues` (zod issue array), message naming the tool
  and each `path: message` mismatch (AC-15's "structured error naming the tool
  and the mismatch"). `safeParse` over `parse` so the boundary throws exactly
  one error type for shape mismatches, never a raw `ZodError`.
- Error-taxonomy alignment (MA-17): falcon-mcp's coded kinds (not-found -32002,
  invalid-argument -32602, timeout -32001) arrive as **protocol errors** — the
  SDK's `client.callTool` rejects with `McpError` before our validation runs;
  they pass through unchanged. `internal` arrives result-level as
  `isError: true` — the existing throw for that stays ahead of validation.
  Validation therefore applies only to *success* payloads, which is the AC-15
  boundary: "validated ... before typed use".

### 3. Call-site inventory (all migrate to `call(name, args, Schema)`)

grep-verified complete set (`rg "mcp\.call|snapMcp\.call|c\.call" src/`):

- `handlers/applyEdit.ts:111` — `fs_apply_patch` → `FsApplyPatchResult` (drops inline `<{result: "ok"|"conflict"; reason?: string}>`)
- `handlers/commit.ts:22` — `git_add` → `OkResult`; `:23` — `git_commit` → `GitCommitResult`; `:60` — `exec_run` → `ExecRunResult`
- `handlers/verify.ts:26` — `cargo_check` → `CargoCheckResult` (typed `errors`, drop `(e: any)` and the `?? "compile error"` dead fallback); `:39` — `cargo_test` → `CargoTestResult`
- `handlers/prepWorktree.ts:32` — `git_worktree_add` → `WorktreeAddResult`
- `handlers/triage.ts:83` — `cargo_check`; `:84` — `fs_list` → `FsListResult`; `:86` — `cargo_test`; `:89` — `cargo_clippy` → `CargoClippyResult`; `:100`,`:138` — `fs_read` → `FsReadResult`; `:123` — `fs_search` → `FsSearchResult` (replaces the local inline matches type)
- `handlers/processIssues.ts:167` — `fs_read`; `:214`,`:239` — `fs_write` → `FsWriteResult`
- `handlers/readContext.ts:27`,`:39` — `fs_read`
- `handlers/finalSweep.ts:41` — `cargo_test`; `:46` — `cargo_clippy` (drops `<{failed: any[]}>` / `<{lints: any[]}>`)
- `tests/lib/mcp.test.ts:30` — `fs_read` (test updated to new signature)

No `.call(` sites exist in `workflows/`, `cli.ts`, or `lib/` outside mcp.ts itself.

### 4. `proposePoisonFix` real schemas + triage cast removal

- `proposePoisonFix.ts` Input becomes
  `z.object({ issue: Issue, report: PoisonReport, excerpts: z.array(Excerpt), previousFailure: PreviousFailure.optional() })`
  — exactly analyzePoison's Output plus the retry field, so the
  `processIssues` spread `{...poisonAnalysis, previousFailure}` still
  validates. `Excerpt` is reused from types.ts (replacing the inline object);
  the `(e: any)` in the excerpts `.find` goes away with typing. `report.recommendation`
  is then typed (PoisonReport field) instead of an any-reach.
- `triage.ts:168` — remove `as any` by fixing the type at the source:
  `gemini.ts` `CallOptions.schema` changes from `ZodSchema<T>` (zod-v3-era
  alias whose Input type param defaults to Output, rejecting `z.preprocess`
  pipes) to `z.ZodType<T>` (Output-only constraint; `ZodPipe` from
  `z.preprocess` satisfies it). Runtime behavior unchanged — cassette keys
  hash `schema.constructor.name`, which was already `"ZodPipe"` at runtime
  under the cast, so recorded e2e cassettes stay valid. The `as { issues: any[] }`
  result cast disappears since `call` now infers from the schema.

### 5. Tests

- **`tests/lib/toolSchemas.test.ts`** (new): per-schema valid-payload accept +
  malformed-payload reject, table-driven over `TOOL_RESULT_SCHEMAS` plus
  shape-specific cases (e.g. `fs_apply_patch` rejects `result: "Conflict"`
  casing regression, `cargo_check` rejects string errors). AC-15 "unit tests
  per tool-result schema".
- **`tests/lib/mcp.test.ts`** (extend): keep the live fs_read test (updated
  signature); add hermetic tests stubbing the private SDK client
  (`(c as any).client = { callTool: async () => ... }`):
  - valid `structuredContent` → parsed result returned;
  - malformed response → rejects with `McpValidationError`, `instanceof` check,
    `.toolName === "cargo_check"`, message contains tool name + mismatched path
    (AC-15 mocked-malformed requirement), `.issues` non-empty;
  - `isError: true` → existing result-level error still thrown (taxonomy path);
  - `structuredContent` absent → `content` fallback validated through schema.
- **`tests/handlers/proposePoisonFix.test.ts`** (new): malformed input (e.g.
  `report` missing / `issue.kind` invalid) fed to the handler's input
  validator asserts a zod validation error naming the bad path; well-formed
  input parses (AC-12 malformed-input test). Uses the same handler-internals
  access pattern as the existing proposeBugFix/triage tests.

### 6. H-9 gate (AC-12)

Append `gate_no_handler_any` to `scripts/debt-gates.sh` (above the R-5 marker
line): fails on any `z.any()` or `as any` match under
`falcon-detective/src/handlers/`. AC-12 explicitly cites "grep gate, H-9";
SPEC R-5's rule is each workstream appends its own gates in its own PR.
(R-5's inline enumeration lists only AC-22 for WS-4 — flagged as contract
ambiguity; adding the gate is the conservative reading since AC-12 is this
ticket's AC.)

### 7. Quality gates & sequencing

1. `npm ci` (done at plan time), confirm branch is on origin/main tip (65142ba — confirmed, no rebase needed).
2. Implement: toolSchemas.ts → mcp.ts → call sites → proposePoisonFix/gemini/triage → tests → debt-gates.sh.
3. Gates in `falcon-detective/`: `npm test` (fast suite green), `npx tsc --noEmit`, `npm run lint` (biome). Plus `bash scripts/debt-gates.sh` at repo root (all gates PASS).
4. Commit: `feat(falcon-detective): schema-validated MCP client boundary (MA-10)`.

### Interfaces exported (for MA-12/MA-13/MA-18)

- `src/lib/toolSchemas.ts`: `FsReadResult`, `FsWriteResult`, `FsListResult`,
  `FsApplyPatchResult`, `FsSearchResult`, `CargoMessage`, `CargoCheckResult`,
  `CargoTestResult`, `CargoClippyResult`, `WorktreeAddResult`, `OkResult`,
  `GitCommitResult`, `ExecRunResult` (each `const` schema + inferred `type`),
  `TOOL_RESULT_SCHEMAS`, `ToolName`.
- `src/lib/mcp.ts`: `FalconMcpClient.call<T>(toolName, args, schema) => Promise<T>` (schema required), `McpValidationError { toolName, issues }`.
- `src/lib/gemini.ts`: `CallOptions.schema: z.ZodType<T>` (accepts preprocess pipes without casts).
