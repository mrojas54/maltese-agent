# MA-12: Deduplicate propose handlers via factory

BUILDPLAN M3 / WS-11 (BUILDPLAN id MA-12). makeProposeFixHandler factory parameterizing prompt file and model; the three propose* handlers become instances; parity tests (existing plus new) prove identical behavior per kind. AC: AC-24. Depends: MA-08, MA-10. Serialize: handlers/*. Size M.

## Current state (verified against code at 37c7fcd)

`src/handlers/proposeBugFix.ts`, `proposeLintFix.ts`, `proposePoisonFix.ts` each call
`createHandler({ inputValidator, outputValidator: UnifiedDiff, handle }, "<name>")` with a
near-identical handle body: pick an excerpt → `new Gemini()` → `promptFromFile(<file>, vars)`
→ `gemini.call({ prompt, schema: UnifiedDiff, model })`. Differences per kind:

| | bug | lint | poison |
|---|---|---|---|
| handler id / export | `proposeBugFix` | `proposeLintFix` | `proposePoisonFix` |
| prompt file | `propose-bug-fix.md` | `propose-lint-fix.md` | `propose-poison-fix.md` |
| model | `GEMINI_PRO` | `GEMINI_FLASH` | `GEMINI_PRO` |
| input schema | `{issue, excerpts, previousFailure?}` | same as bug | `{issue, report, excerpts, previousFailure?}` |
| excerpt selection | `find(e.file === issue.location.file) ?? [0]` | same as bug | `find(e.file.endsWith("prompt.rs")) ?? [0]` |
| prompt vars | file, line, evidence, content, previousFailure | same as bug | file, content, report(JSON), recommendation, previousFailure |

**Binding constraint discovered in Barnum 0.4** (`dist/handler.js` `getCallerFilePath`):
`createHandler` records `module = <file that called createHandler>` and `func = exportName`;
the engine worker re-imports that module and reads that named export. Since the factory calls
`createHandler` inside `proposeFix.ts`, the three instances MUST be declared and exported from
`proposeFix.ts` under their exact current names. The legacy files become one-line re-export
shims so every existing import path (`processIssues.ts`, tests, MA-13's parallel work) is
untouched. Handler ids, export names, module import paths, prompt bytes, and model ids are
all preserved — the cassette hash (`model\nZodObject\n<prompt>`) proves byte parity.

## Factory signature (src/handlers/proposeFix.ts — new file)

```ts
type InputFor<K extends IssueKind> = K extends "poison" ? PoisonFixInput : IssueFixInput;

export function makeProposeFixHandler<K extends IssueKind>(spec: {
  kind: K;                       // "bug" | "lint" | "poison" (reuses types.ts IssueKind)
  promptFile: string;            // prompt template under prompts/
  model: string;                 // GEMINI_PRO / GEMINI_FLASH constant
  schema: z.ZodType<InputFor<K>>; // input validator (SPEC's `schema` param)
}): Handler<InputFor<K>, UnifiedDiff>
```

Matches the SPEC WS-11 signature `makeProposeFixHandler({ kind, promptFile, model, schema })`.
Internals: exported zod schemas `IssueFixInput` (= old bug/lint Input, with the inline
excerpt object replaced by the identical shared `Excerpt` schema — same JSON Schema) and
`PoisonFixInput` (= old poison Input); a per-kind strategy table
`STRATEGIES: { [K in IssueKind]: Strategy<InputFor<K>> }` holding `selectExcerpt` +
`promptVars` (bug and lint share one `issueStrategy` object — making "bug vs lint differ only
in prompt file and model" visible); `HANDLER_IDS: Record<IssueKind, string>` pins the exact
Barnum export names. The shared handle body is written once. No `as any` / `z.any()`
(gate AC-12) and no `__definition` reach in src (gate AC-13).

## Instance definitions (in proposeFix.ts)

```ts
export const proposeBugFix = makeProposeFixHandler({
  kind: "bug",    promptFile: "propose-bug-fix.md",    model: GEMINI_PRO,   schema: IssueFixInput });
export const proposeLintFix = makeProposeFixHandler({
  kind: "lint",   promptFile: "propose-lint-fix.md",   model: GEMINI_FLASH, schema: IssueFixInput });
export const proposePoisonFix = makeProposeFixHandler({
  kind: "poison", promptFile: "propose-poison-fix.md", model: GEMINI_PRO,   schema: PoisonFixInput });
```

`proposeBugFix.ts` / `proposeLintFix.ts` / `proposePoisonFix.ts` shrink to
`export { proposeX } from "./proposeFix.js";` — thin exports, unchanged public surface.

## Parity-test strategy (AC-24)

Existing tests keep passing unmodified (they were written against the pre-factory handlers,
so green = parity):
- `tests/handlers/proposeBugFix.test.ts` — cassette round-trip pins bug's prompt bytes +
  `gemini-2.5-pro` (hash mismatch ⇒ cassette miss ⇒ throw).
- `tests/handlers/proposePoisonFix.test.ts` — pins poison's input-validation shapes (AC-12).
- `tests/lib/invokeHandler.test.ts` — pins the runPipeline seam contract.
- `tests/e2e.test.ts` (cassette mode, recorded fixtures) — any prompt-byte drift misses.

New tests:
1. `tests/handlers/proposeLintFix.test.ts` — cassette round-trip mirroring the bug test but
   hashing `gemini-2.5-flash` + `propose-lint-fix.md` (pins lint's model + prompt file); a
   decoy excerpt placed first proves issue-location selection; a second case with
   `previousFailure` set proves `formatPreviousFailure` threading.
2. `tests/handlers/proposePoisonFix.test.ts` (extend) — cassette round-trip pinning poison's
   `gemini-2.5-pro` + `propose-poison-fix.md` + `endsWith("prompt.rs")` excerpt selection +
   report-JSON prompt vars.
3. `tests/handlers/proposeFix.test.ts` — structural parity: each export is an `Invoke` action
   whose public AST `handler.func` equals the original id and `handler.module` ends with
   `proposeFix.ts`; bug and lint accept/reject identical payloads (shared validator behavior);
   poison requires `report`.

## Steps

1. Add `src/handlers/proposeFix.ts` (factory + schemas + three instances).
2. Shrink the three legacy files to re-export shims.
3. Add/extend the three test files above.
4. Gates in falcon-detective/: `npm test`, `npm run build` (H-4), `npx tsc --noEmit`,
   `npm run lint` (H-5); repo root: `scripts/debt-gates.sh`, `scripts/check-planted-defects.sh`.
5. Commit: `refactor(falcon-detective): dedupe propose handlers via makeProposeFixHandler factory (MA-12)`.

## Non-goals / guardrails

- No falcon-agent changes (G-1). No changes to processIssues/workflow wiring (MA-13 runs in
  parallel against the current public surface). No prompt-file or model-id changes.
- Stacked on origin/claude/ma-10-mcp-schemas@37c7fcd; rebase per stacking rules at phase
  boundaries.
