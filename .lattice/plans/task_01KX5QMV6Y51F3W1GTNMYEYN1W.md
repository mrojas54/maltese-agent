# MA-19: Migrate google genai to 2.x

BUILDPLAN M4 / WS-13a (BUILDPLAN id MA-19). @google/genai 1.51 to 2.x migration; all call sites compile; gemini unit tests green on fakes/mocks. Cassette re-record is the human-track ticket (operator), not this one. AC: AC-27. Depends: MA-18. Serialize: package.json, lib/gemini.ts. Size M.

## Branch / stack

- Worktree: `/Users/michellerojas/maltese-agent/.claude/worktrees/ma-19-genai-2x`, branch `claude/ma-19-genai-2x`.
- Deep stack: main <- PR #40 (MA-10 schemas, 37c7fcd) <- PR #41 (MA-18 retry/backoff, 38c219f) <- this branch.
- Rebase policy: at each phase boundary, fetch; follow origin/claude/ma-18-gemini-retry if it moves; rebase onto origin/main if #41 merges.

## Call-site inventory (verified by rg sweep, `--glob '!node_modules'`)

The SDK surface used by falcon-detective is deliberately minimal (MA-18 decoupled error
detection from SDK error classes):

1. `falcon-detective/src/lib/gemini.ts` — the ONLY production import:
   - `import { GoogleGenAI } from "@google/genai"` (line 4)
   - `new GoogleGenAI({ apiKey })` (line 282, live/record modes only)
   - `this.ai!.models.generateContent({ model, contents: prompt, config: { responseMimeType: "application/json", temperature } })` (line 340)
   - `resp.text ?? ""` (line 348)
2. `falcon-detective/package.json` — `"@google/genai": "^1.51.0"` dependency (+ lockfile).
3. `falcon-detective/tests/lib/gemini-retry.test.ts` — `vi.mock("@google/genai", ...)` with a
   fake `GoogleGenAI` class exposing `models.generateContent`; asserts on
   `generateContentMock.mock.calls[1][0].contents`. Mock shape must track the 2.x call shape
   only if gemini.ts's call shape changes.
4. No other src/tests reference the SDK. Handlers/e2e tests exercise Gemini via cassette mode,
   which never constructs the SDK client (`this.ai` stays null) and replays from
   `fixtures/cassettes/*.json` keyed by sha256(model + schemaName + normalized prompt) — a
   key with zero SDK coupling.

## Preserved interface (MA-18 / MA-10 contracts — do not break)

- Exports: `TransientErrorKind`, `TransientCheck`, `classifyTransientError`, `createBackoff`,
  `SleepFn`, `BackoffOptions`, `GeminiRetryOptions` (optional 2nd Gemini ctor param),
  `CallOptions.schema: ZodType<T>`, `normalizeForHash`, `Mode`, `Gemini`.
- `classifyTransientError` is structural/duck-typed over `unknown` — only remap the outermost
  detection layer if the 2.x error surface actually differs (verify against installed types).

## Expected breaking changes (to be CONFIRMED against installed 2.x type declarations in node_modules — the source of truth; every confirmed/refuted item goes in the ticket comment for MA-24 cassette re-record)

- E-1: `GoogleGenAI` constructor options shape (`{ apiKey }`) — verify `GoogleGenAIOptions`.
- E-2: `models.generateContent` params — verify `GenerateContentParameters` (model/contents/config
  field names, whether a bare string is still accepted for `contents`).
- E-3: `GenerateContentConfig` — verify `responseMimeType` + `temperature` still exist (and whether
  `responseJsonSchema` semantics changed; NOT adopting it here — out of scope, see gemini.ts comment).
- E-4: response `.text` accessor — 1.x had a `text` getter on `GenerateContentResponse`; verify it
  survives (some 2.x drafts moved to methods/renamed fields).
- E-5: error surface — verify `ApiError`/`ClientError`/`ServerError` shapes expose `status`/`code`
  fields the structural classifier already reads; extend `classifyCodeString`/status walk only if
  field names changed.
- E-6: package exports map / ESM-only or Node >= 20 engine constraints — verify `engines` and that
  NodeNext resolution still finds types.

## Steps

1. `lattice status MA-19 in_planning` (done) + branch-link (done); write this plan; bump `planned`.
2. Bump `in_progress`. In the worktree: `npm install @google/genai@^2.11.0` (updates package.json +
   package-lock.json — lockfile churn is owned by this ticket).
3. Read `node_modules/@google/genai/dist/*.d.ts` for the actual 2.x surface; adapt
   `src/lib/gemini.ts` minimally (constructor, generateContent call, response text access);
   update the vi.mock in `gemini-retry.test.ts` only if the call shape changed.
4. Gates (all inside falcon-detective/): `npm test` (fast suite, hermetic), `npx tsc --noEmit`,
   `npx biome check .`.
5. Cassette policy: do NOT re-record, do NOT hit live API, do NOT delete cassettes. Replay is
   SDK-independent (see inventory #4) so replay tests are expected to stay green; if a 2.x change
   somehow breaks replay, skip those tests with a marker naming MA-24 and flag loudly.
6. Commit: `build(falcon-detective): migrate @google/genai to 2.x (MA-19)`.
7. Own-review vs origin/claude/ma-18-gemini-retry; fix Critical/Major; attach review comment.
8. Push; PR to main with stacked-on-#41 first line, AC-27 mapping, breaking-change inventory,
   cassette-skip list, gate results; comment PR URL + inventory on ticket; bump `review`. Stop.

## AC mapping

- AC-27 (`autonomous`): `@google/genai` on 2.x; all call sites compile (`tsc --noEmit` clean);
  gemini unit tests green with mocked client (`gemini-retry.test.ts` + `gemini.test.ts` in the
  fast suite).

## Guardrails

- G-1: falcon-agent untouched (this ticket never leaves falcon-detective + lockfile + plan file).
- Serialization: this wave owns falcon-detective/package.json and lib/gemini.ts.
- Non-goal N-6: no README prose changes (record-flow README section belongs to MA-20).
