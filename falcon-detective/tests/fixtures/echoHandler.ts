import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { PreviousFailure } from "../../src/lib/types.js";

/**
 * Trivial fixture handler for the invokeHandler seam test (MA-11 / AC-13).
 *
 * Its input schema deliberately mirrors the propose-* handler pattern —
 * required payload fields plus an OPTIONAL `previousFailure` — so the
 * production-path seam test dynamically proves that processIssues' retry
 * spread (`{ ...payload, previousFailure }`) survives the engine-side
 * JSON-Schema validation that the public runPipeline path enforces
 * (spike MA-23, caveat 3). The old `__definition.handle` reach bypassed
 * that validation entirely.
 *
 * This lives in its own plain ESM module (not inside a test file): the
 * Barnum engine's tsx worker re-imports the defining module by the absolute
 * path captured at createHandler() time, so the file must be importable and
 * side-effect-free outside vitest.
 */
export const echo = createHandler(
  {
    inputValidator: z.object({
      n: z.number(),
      previousFailure: PreviousFailure.optional(),
    }),
    outputValidator: z.object({ n: z.number(), sawFailure: z.boolean() }),
    handle: async ({ value }) => ({
      n: value.n,
      sawFailure: value.previousFailure !== undefined,
    }),
  },
  "echo",
);
