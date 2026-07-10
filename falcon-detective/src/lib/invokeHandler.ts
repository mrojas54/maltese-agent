import { runPipeline, type Action } from "@barnum/barnum/pipeline";

/**
 * The single seam through which falcon-detective invokes a Barnum handler
 * outside of pipeline composition (WS-5 / AC-13, spike MA-23 Verdict A).
 *
 * Production implementation goes through the PUBLIC engine API:
 * `runPipeline` from `@barnum/barnum/pipeline` — a handler returned by
 * `createHandler` is itself a valid single-step pipeline, so no composition
 * is required. This replaces the previous reach into the handler's
 * underscore-private internal definition field (the engine worker's
 * contract, not supported API — see spike MA-23), with two behavioral
 * consequences:
 *
 *  - Cost: every call spawns the barnum engine binary plus a fresh tsx
 *    worker that re-imports the handler module (~0.5s floor per call,
 *    measured in spike MA-23). Acceptable here because LLM latency
 *    dominates the per-issue loop.
 *  - Validation: the engine enforces the handler's zod-derived input and
 *    output JSON Schemas, which the private reach silently bypassed.
 *    Malformed payloads now fail loudly at the call site.
 *
 * Mocking: the engine boundary crosses two processes, so `vi.mock` in a
 * test process cannot intercept what the worker executes. Consumers must
 * therefore accept an InvokeHandler as an injectable parameter with
 * `invokeHandler` as the production default (see
 * `makeProcessIssuesHandle`), letting orchestration tests (MA-13) supply
 * an in-process fake.
 *
 * ESM-only: `runPipeline` relies on `import.meta.dirname` and crashes if
 * transpiled to CJS (spike MA-23, caveat 4). This package is
 * `"type": "module"`; keep it that way.
 */
export type InvokeHandler = <TOut>(handler: Action, input: unknown) => Promise<TOut>;

export const invokeHandler: InvokeHandler = async <TOut>(
  handler: Action,
  input: unknown,
): Promise<TOut> => (await runPipeline(handler, input)) as TOut;
