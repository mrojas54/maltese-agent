# MA-18: Gemini retry backoff and transient detection

BUILDPLAN M4 / WS-14d (BUILDPLAN id MA-18). gemini.ts: backoff between schema-retry attempts; transient-error detection broadened and typed beyond regex (covers network-level failures); unit tests simulate both. AC: AC-33. Depends: MA-10. Serialize: lib/gemini.ts. Size S.

## Contract readings

- **AC-33** (EVALUATION.md:90): backoff between schema-retry attempts; transient-error detection covers network-level failures (typed/broadened check); unit tests simulate both.
- **WS-14d** (SPEC.md:90): jittered backoff between schema-retry attempts (reuse the existing transient-backoff helper); transient detection checks structured error fields (`status`/`code`) when present with a broadened message fallback (`ECONNRESET`, `ETIMEDOUT`, `fetch failed`); unit tests simulate both.
- **MA-19 serialization**: transient detection is typed through our own discriminated union + predicate, purely structural (no `@google/genai` error-class imports), so the genai 2.x migration only remaps the outermost detection layer.

## Current state (base: origin/claude/ma-10-mcp-schemas @ 37c7fcd)

`falcon-detective/src/lib/gemini.ts`:
- `callOnce` (inner API loop, 5 attempts) already has exponential backoff with full jitter, but transient detection is a regex over `String(e?.message ?? e)` only: `/\b(429|503|UNAVAILABLE|RESOURCE_EXHAUSTED|deadline|timeout)\b/i`. Misses structured `status`/`code` fields and network-level failures (`ECONNRESET`, `ETIMEDOUT`, `fetch failed` whose syscall detail hides in `error.cause`).
- Outer schema-validation loop (3 attempts, re-prompt with validation error) has **no backoff at all** between attempts.
- Backoff is inlined in `callOnce` with a bare `setTimeout` — untestable without wall-clock waits.

## Design

All changes in `falcon-detective/src/lib/gemini.ts` + tests. No other source files.

### 1. Typed transient detection (exported, structural)

```ts
export type TransientErrorKind = "rate_limit" | "unavailable" | "timeout" | "network";
export type TransientCheck =
  | { transient: true; kind: TransientErrorKind; detail: string }
  | { transient: false };
export function classifyTransientError(err: unknown): TransientCheck;
```

Detection order (first hit wins):
1. **Structured fields**, walking the `cause` chain (bounded depth 5, covers Node 20 `fetch failed` TypeErrors that nest the syscall error in `.cause`):
   - numeric `status`/`code` (or 3-digit numeric string): 429 → `rate_limit`; 408 → `timeout`; 500/502/503/504 → `unavailable`.
   - string `status`/`code`: `RESOURCE_EXHAUSTED` → `rate_limit`; `UNAVAILABLE`/`INTERNAL` → `unavailable`; `DEADLINE_EXCEEDED`/`ETIMEDOUT`/undici connect-timeout → `timeout`; `ECONNRESET`/`ECONNREFUSED`/`ECONNABORTED`/`EPIPE`/`EAI_AGAIN`/undici socket codes → `network`.
2. **Broadened message fallback** (regex on the outermost message): previous tokens plus `500|502|504`, `ECONNRESET`, `ECONNREFUSED`, `ETIMEDOUT`, `EAI_AGAIN`, `socket hang up`, `fetch failed`, `timed out`; matched token mapped to a kind.

Anything else → `{ transient: false }` (e.g. 400/404, schema/parse errors). No `@google/genai` imports touched — MA-19-safe.

### 2. Extracted, injectable backoff helper (reused by both loops)

```ts
export type SleepFn = (ms: number) => Promise<void>;
export function createBackoff(opts?: {
  initialCapMs?: number; maxCapMs?: number; sleep?: SleepFn; random?: () => number;
}): () => Promise<number>;  // full-jitter: delay = random() * cap; cap doubles to maxCap
```

- `Gemini` constructor gains an optional second param `retry?: { sleep?: SleepFn; random?: () => number }` — existing `new Gemini()` / `new Gemini("cassette")` call sites unaffected.
- `callOnce` uses `createBackoff` (same 1s initial cap / 30s max as today) gated on `classifyTransientError(e).transient`.
- Outer schema loop gets its own `createBackoff` instance; jittered wait between attempts, **no wait after the final attempt**.

### 3. Tests — `tests/lib/gemini-retry.test.ts` (new file; existing gemini.test.ts untouched)

`vi.mock("@google/genai")` with a hoisted `generateContent` mock; `GEMINI_API_KEY=test-key` in scoped beforeEach; injected `sleep` spy + `random: () => 0.5` — zero wall-clock waits, fast suite stays well under 60s.

- `classifyTransientError` unit tests: numeric 429/503, string `UNAVAILABLE`, Node syscall error `{code:"ECONNRESET"}`, `fetch failed` TypeError with `ETIMEDOUT` in `.cause`, message-only fallbacks (`"fetch failed"`, `"503 Service Unavailable"`), non-transient (400 status, plain error) → false.
- API-level transient retry: reject once with ECONNRESET then succeed → 2 calls, 1 sleep of 500ms (0.5×1000); non-transient 400 → throws immediately, 0 sleeps; persistent 503 → throws after 5 calls, 4 sleeps with doubling caps [500, 1000, 2000, 4000].
- Schema-retry backoff (AC-33 core): mock returns schema-invalid JSON twice then valid → 3 calls, sleeps [500, 1000] between attempts, parsed result returned; always-invalid → "failed after 3 attempts" error with exactly 2 sleeps (none after the last attempt).

## Gates (run in worktree falcon-detective/)

`npm ci` → `npm test` green → `npx tsc --noEmit` → `npx biome check .`

## Commit

`feat(falcon-detective): gemini retry backoff + typed transient detection (MA-18)`

## Non-goals

- No genai 2.x migration (MA-19's job); no changes to cassette/record modes, hashing, or prompts; no falcon-agent changes (G-1).
