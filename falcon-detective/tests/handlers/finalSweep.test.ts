import { beforeEach, describe, expect, it, vi } from "vitest";
import { fakeMcp, resetFakeMcp, setMcpResponder } from "../helpers/fakeMcp.js";

// Substitute the MA-10 schema'd client boundary with the shared in-process
// fake (public seam: module export). See tests/helpers/fakeMcp.ts.
vi.mock("../../src/lib/mcp.js", () => import("../helpers/fakeMcp.js"));

import { finalSweepHandle } from "../../src/handlers/finalSweep.js";

const INPUT = {
  mcpBinary: "/nonexistent/hermetic-falcon-mcp",
  worktreePath: "/tmp/fd-sweep-wt",
  cratePath: "falcon-agent",
};

const LINT = { level: "warning", message: "unused import" };

beforeEach(() => {
  resetFakeMcp();
});

describe("finalSweep (AC-19: Clean and Dirty branches)", () => {
  it("returns Clean carrying the mcp context when tests and clippy are both quiet", async () => {
    setMcpResponder((tool) => {
      if (tool === "cargo_test")
        return { passed: ["a::ok"], failed: [], duration_ms: 10 };
      if (tool === "cargo_clippy") return { lints: [] };
      throw new Error(`unexpected tool ${tool}`);
    });

    const out = await finalSweepHandle({ value: INPUT });

    // The value threads the worktree context through to branch()'s arms
    // (commitAll / escalate) — pinned here because the workflow depends on it.
    expect(out).toEqual({
      kind: "Clean",
      value: { mcpBinary: INPUT.mcpBinary, worktreePath: INPUT.worktreePath },
    });
    expect(fakeMcp.calls.map((c) => c.tool)).toEqual([
      "cargo_test",
      "cargo_clippy",
    ]);
    for (const c of fakeMcp.calls) {
      expect(c.args).toEqual({ crate_path: "falcon-agent" });
    }
    expect(fakeMcp.instances[0].closed).toBe(true);
  });

  it("returns Dirty with one reason per failing signal (tests, then clippy) plus the context", async () => {
    setMcpResponder((tool) => {
      if (tool === "cargo_test")
        return {
          passed: [],
          failed: [
            { name: "a::fails", stdout: "boom" },
            { name: "b::fails", stdout: "boom" },
          ],
          duration_ms: 22,
        };
      if (tool === "cargo_clippy") return { lints: [LINT, LINT, LINT] };
      throw new Error(`unexpected tool ${tool}`);
    });

    const out = await finalSweepHandle({ value: INPUT });

    expect(out).toEqual({
      kind: "Dirty",
      value: {
        mcpBinary: INPUT.mcpBinary,
        worktreePath: INPUT.worktreePath,
        reasons: ["2 tests still failing", "3 clippy lints remain"],
      },
    });
    expect(fakeMcp.instances[0].closed).toBe(true);
  });

  it("propagates a cargo_test failure (no silent catch) and still closes the client", async () => {
    setMcpResponder(
      () => new Error("tool cargo_test returned error: sandbox denied"),
    );

    await expect(finalSweepHandle({ value: INPUT })).rejects.toThrow(
      /cargo_test returned error: sandbox denied/,
    );
    expect(fakeMcp.instances[0].closed).toBe(true);
  });
});
