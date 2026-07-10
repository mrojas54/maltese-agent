import { beforeEach, describe, expect, it, vi } from "vitest";
import { fakeMcp, resetFakeMcp, setMcpResponder } from "../helpers/fakeMcp.js";

// The snapshot client inside processIssues is a direct FalconMcpClient —
// substitute the MA-10 schema'd boundary with the shared in-process fake.
vi.mock("../../src/lib/mcp.js", () => import("../helpers/fakeMcp.js"));

import { applyEdit } from "../../src/handlers/applyEdit.js";
import { classify } from "../../src/handlers/classify.js";
import { commitOne } from "../../src/handlers/commit.js";
import { makeProcessIssuesHandle } from "../../src/handlers/processIssues.js";
import { proposeBugFix } from "../../src/handlers/proposeBugFix.js";
import { readContext } from "../../src/handlers/readContext.js";
import { verify } from "../../src/handlers/verify.js";
import type { InvokeHandler } from "../../src/lib/invokeHandler.js";

/**
 * Orchestration coverage for the per-issue loop (AC-19: happy path;
 * retry-then-success; revert-on-persistent-failure), driven through the two
 * public seams built for exactly this:
 *  - `makeProcessIssuesHandle(invoke)` — MA-11's injectable InvokeHandler
 *    seam; the fake matches handlers by OBJECT IDENTITY against the same
 *    module exports processIssues imports (names/internals are not part of
 *    the contract — MA-12 may refactor propose* freely underneath this).
 *  - the mocked FalconMcpClient module (MA-10's schema'd boundary) for the
 *    snapshot/revert client.
 * Gemini is never reached: the propose* handlers are faked wholesale at the
 * invoke seam, so the whole file is hermetic by construction.
 */

const ISSUE = {
  kind: "bug" as const,
  location: { file: "src/lib.rs", line: 3 },
  evidence: "x.unwrap()",
};
const EXCERPTS = [{ file: "src/lib.rs", content: "fn main() {}" }];
const ORIGINAL = "fn main() { x.unwrap(); }\n";

const INPUT = {
  mcpBinary: "/nonexistent/hermetic-falcon-mcp",
  worktreePath: "/tmp/fd-pi-wt",
  cratePath: "falcon-agent",
  issues: [ISSUE],
};
const PASSTHROUGH = {
  mcpBinary: INPUT.mcpBinary,
  worktreePath: INPUT.worktreePath,
  cratePath: INPUT.cratePath,
};

function diffFor(n: number): { path: string; diff: string } {
  return { path: "src/lib.rs", diff: `--- a/src/lib.rs (attempt ${n})` };
}

type VerifyShape =
  | { kind: "Clean"; value: Record<string, never> }
  | { kind: "Broken"; value: { errors: string[] } };

/** Scenario-scripted fake for the MA-11 invoke seam. */
function makeFakeInvoke(script: {
  proposals: Array<{ path: string; diff: string }>;
  verifyResults: VerifyShape[];
}) {
  const invoked: Array<{ handler: unknown; input: Record<string, unknown> }> =
    [];
  let proposeIdx = 0;
  let verifyIdx = 0;
  const invoke: InvokeHandler = async <TOut>(
    handler: unknown,
    input: unknown,
  ): Promise<TOut> => {
    invoked.push({ handler, input: input as Record<string, unknown> });
    if (handler === readContext)
      return { issue: ISSUE, excerpts: EXCERPTS } as TOut;
    if (handler === classify)
      return {
        kind: "Bug",
        value: { issue: ISSUE, excerpts: EXCERPTS },
      } as TOut;
    if (handler === proposeBugFix) {
      const p = script.proposals[proposeIdx++];
      if (!p) throw new Error("fakeInvoke: proposals exhausted");
      return p as TOut;
    }
    if (handler === applyEdit) return { ok: true } as TOut;
    if (handler === verify) {
      const v = script.verifyResults[verifyIdx++];
      if (!v) throw new Error("fakeInvoke: verifyResults exhausted");
      return v as TOut;
    }
    if (handler === commitOne) return { sha: "fake-sha" } as TOut;
    throw new Error("fakeInvoke: unexpected handler invocation");
  };
  return { invoke, invoked };
}

/** Snapshot-client script: fs_read serves ORIGINAL, fs_write acks. */
function installSnapshotResponder() {
  setMcpResponder((tool, args) => {
    if (tool === "fs_read")
      return { content: ORIGINAL, bytes: ORIGINAL.length };
    if (tool === "fs_write") return { bytes: (args.content as string).length };
    throw new Error(`unexpected tool ${tool}`);
  });
}

beforeEach(() => {
  resetFakeMcp();
  installSnapshotResponder();
});

describe("processIssues (AC-19: per-issue loop scenarios)", () => {
  it("happy path: propose → apply → verify Clean → commitOne; no revert; snapshot client closed", async () => {
    const { invoke, invoked } = makeFakeInvoke({
      proposals: [diffFor(1)],
      verifyResults: [{ kind: "Clean", value: {} }],
    });

    const out = await makeProcessIssuesHandle(invoke)({ value: INPUT });

    expect(out).toEqual(PASSTHROUGH);
    expect(invoked.map((c) => c.handler)).toEqual([
      readContext,
      classify,
      proposeBugFix,
      applyEdit,
      verify,
      commitOne,
    ]);
    // verify sees the attempt counter; commitOne gets the applied diff's path.
    expect(invoked[4].input).toEqual({ ...PASSTHROUGH, attempt: 0 });
    expect(invoked[5].input).toEqual({
      mcpBinary: INPUT.mcpBinary,
      worktreePath: INPUT.worktreePath,
      issue: ISSUE,
      diffPath: "src/lib.rs",
    });
    // Success leaves the applied edit in place: one snapshot read, NO revert.
    expect(fakeMcp.calls).toEqual([
      { tool: "fs_read", args: { path: "src/lib.rs" } },
    ]);
    expect(fakeMcp.instances).toHaveLength(1);
    expect(fakeMcp.instances[0].closed).toBe(true);
  });

  it("retry-then-success: Broken reverts the snapshot, re-proposes with previousFailure, then commits", async () => {
    const { invoke, invoked } = makeFakeInvoke({
      proposals: [diffFor(1), diffFor(2)],
      verifyResults: [
        { kind: "Broken", value: { errors: ["E0308: mismatched types"] } },
        { kind: "Clean", value: {} },
      ],
    });

    const out = await makeProcessIssuesHandle(invoke)({ value: INPUT });

    expect(out).toEqual(PASSTHROUGH);
    expect(invoked.map((c) => c.handler)).toEqual([
      readContext,
      classify,
      proposeBugFix,
      applyEdit,
      verify,
      proposeBugFix, // re-prompt after Broken
      applyEdit,
      verify,
      commitOne,
    ]);
    // The re-prompt carries the failed diff + cargo's errors (retry contract).
    expect(invoked[5].input.previousFailure).toEqual({
      diff: diffFor(1).diff,
      errors: ["E0308: mismatched types"],
    });
    // Second attempt applies the NEW diff and bumps the verify attempt counter.
    expect(invoked[6].input.diff).toEqual(diffFor(2));
    expect(invoked[7].input).toEqual({ ...PASSTHROUGH, attempt: 1 });
    // Exactly one in-loop revert restoring the snapshot, then success (no
    // terminal revert): fs_read once (snapshot is idempotent per path).
    expect(fakeMcp.calls).toEqual([
      { tool: "fs_read", args: { path: "src/lib.rs" } },
      { tool: "fs_write", args: { path: "src/lib.rs", content: ORIGINAL } },
    ]);
    expect(fakeMcp.instances[0].closed).toBe(true);
  });

  it("revert-on-persistent-failure: three Broken attempts exhaust retries, every write is reverted, nothing commits", async () => {
    const broken: VerifyShape = {
      kind: "Broken",
      value: { errors: ["E0499: cannot borrow"] },
    };
    const { invoke, invoked } = makeFakeInvoke({
      proposals: [diffFor(1), diffFor(2), diffFor(3)],
      verifyResults: [broken, broken, broken],
    });

    const out = await makeProcessIssuesHandle(invoke)({ value: INPUT });

    // The loop still completes and passes the pipeline context through.
    expect(out).toEqual(PASSTHROUGH);
    expect(invoked.map((c) => c.handler)).toEqual([
      readContext,
      classify,
      proposeBugFix,
      applyEdit,
      verify, // attempt 0 -> Broken
      proposeBugFix,
      applyEdit,
      verify, // attempt 1 -> Broken
      proposeBugFix,
      applyEdit,
      verify, // attempt 2 -> Broken, retries exhausted
    ]);
    expect(invoked.map((c) => c.handler)).not.toContain(commitOne);
    // Each re-prompt saw the previous attempt's diff.
    const proposeInputs = invoked.filter((c) => c.handler === proposeBugFix);
    expect(proposeInputs[0].input.previousFailure).toBeUndefined();
    expect(
      (proposeInputs[1].input.previousFailure as { diff: string }).diff,
    ).toBe(diffFor(1).diff);
    expect(
      (proposeInputs[2].input.previousFailure as { diff: string }).diff,
    ).toBe(diffFor(2).diff);
    // 3 in-loop reverts + 1 terminal revert, all restoring the original
    // content, so the next issue and finalSweep see a clean tree.
    const writes = fakeMcp.calls.filter((c) => c.tool === "fs_write");
    expect(writes).toHaveLength(4);
    for (const w of writes) {
      expect(w.args).toEqual({ path: "src/lib.rs", content: ORIGINAL });
    }
    expect(fakeMcp.calls.filter((c) => c.tool === "fs_read")).toHaveLength(1);
    expect(fakeMcp.instances[0].closed).toBe(true);
  });
});
