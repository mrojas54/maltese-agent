import { describe, expect, it } from "vitest";
import { classify } from "../../src/handlers/classify.js";
import { makeProcessIssuesHandle } from "../../src/handlers/processIssues.js";
import { proposeBugFix } from "../../src/handlers/proposeBugFix.js";
import { readContext } from "../../src/handlers/readContext.js";
import {
  type InvokeHandler,
  invokeHandler,
} from "../../src/lib/invokeHandler.js";
import { echo } from "../fixtures/echoHandler.js";

describe("invokeHandler (production seam -> public runPipeline)", () => {
  // Deliberately the ONLY test that touches the Barnum engine: every
  // runPipeline call spawns the engine binary + a fresh tsx worker
  // (~0.5s floor per call, spike MA-23). Two calls in one test keep the
  // fast suite fast while covering both halves of the retry contract.
  it("invokes a handler through the engine, with and without the optional retry field", async () => {
    // First-attempt shape: optional field absent after JSON serialization.
    const bare = await invokeHandler<{ n: number; sawFailure: boolean }>(echo, {
      n: 21,
      previousFailure: undefined,
    });
    expect(bare).toEqual({ n: 21, sawFailure: false });

    // Retry shape: mirrors processIssues' `{ ...payload, previousFailure }`
    // spread — proves the pattern passes engine-side input validation
    // (MA-23 caveat 3), which the old __definition.handle reach bypassed.
    const retry = await invokeHandler<{ n: number; sawFailure: boolean }>(
      echo,
      {
        n: 21,
        previousFailure: { diff: "--- a\n+++ b", errors: ["E0308"] },
      },
    );
    expect(retry).toEqual({ n: 21, sawFailure: true });
  }, 60_000);
});

describe("makeProcessIssuesHandle (injectable seam for MA-13)", () => {
  it("routes every handler invocation through the injected InvokeHandler, in-process", async () => {
    const issue = {
      kind: "bug" as const,
      location: { file: "src/lib.rs", line: 3 },
      evidence: "boom",
    };
    const excerpts = [{ file: "src/lib.rs", content: "fn main() {}" }];
    const calls: Array<{ handler: unknown; input: unknown }> = [];

    // Fake matches handlers by OBJECT IDENTITY against the same modules
    // processIssues imports (plan amendment A1) — names/exportNames are not
    // part of the seam contract.
    const fake: InvokeHandler = async <TOut>(
      handler: unknown,
      input: unknown,
    ): Promise<TOut> => {
      calls.push({ handler, input });
      if (handler === readContext) return { issue, excerpts } as TOut;
      if (handler === classify)
        return { kind: "Bug", value: { issue, excerpts } } as TOut;
      if (handler === proposeBugFix) {
        // Throwing at propose stops the loop BEFORE any FalconMcpClient
        // work, so this test spawns no subprocess of any kind.
        throw new Error("fake propose failure");
      }
      throw new Error(`unexpected handler invocation: ${String(handler)}`);
    };

    const handle = makeProcessIssuesHandle(fake);
    const out = await handle({
      value: {
        mcpBinary: "/nonexistent/hermetic",
        worktreePath: "/tmp/wt",
        cratePath: "crate",
        issues: [issue],
      },
    });

    // propose threw -> issue skipped, loop completed, pass-through output.
    expect(out).toEqual({
      mcpBinary: "/nonexistent/hermetic",
      worktreePath: "/tmp/wt",
      cratePath: "crate",
    });

    // Substitutability: the fake — not the engine — received every call.
    expect(calls.map((c) => c.handler)).toEqual([
      readContext,
      classify,
      proposeBugFix,
    ]);
    expect(calls[0].input).toEqual({
      mcpBinary: "/nonexistent/hermetic",
      worktreePath: "/tmp/wt",
      issue,
    });
    // Plan amendment A4: pin the ctx.excerpts pass-through MA-13 relies on.
    expect(calls[1].input).toEqual({ issue, excerpts });
    // The propose payload is the exact `{ ...tagged.value, previousFailure }`
    // spread; on the first attempt previousFailure is present-but-undefined
    // (it serializes away under JSON, and .optional() accepts absence).
    const proposeInput = calls[2].input as Record<string, unknown>;
    expect(proposeInput.issue).toEqual(issue);
    expect(proposeInput.excerpts).toEqual(excerpts);
    expect("previousFailure" in proposeInput).toBe(true);
    expect(proposeInput.previousFailure).toBeUndefined();
  });
});
