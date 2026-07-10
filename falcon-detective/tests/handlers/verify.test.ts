import { beforeEach, describe, expect, it, vi } from "vitest";
import { fakeMcp, resetFakeMcp, setMcpResponder } from "../helpers/fakeMcp.js";

// Substitute the MA-10 schema'd client boundary with the shared in-process
// fake (public seam: module export). See tests/helpers/fakeMcp.ts.
vi.mock("../../src/lib/mcp.js", () => import("../helpers/fakeMcp.js"));

// Import AFTER the mock declaration so verify binds to the fake client.
import { verifyHandle } from "../../src/handlers/verify.js";

const INPUT = {
  mcpBinary: "/nonexistent/hermetic-falcon-mcp",
  worktreePath: "/tmp/fd-verify-wt",
  cratePath: "falcon-agent",
  attempt: 0,
};

const CLEAN_CHECK = { errors: [], warnings: [] };
const CLEAN_TEST = { passed: ["a::ok"], failed: [], duration_ms: 12 };

beforeEach(() => {
  resetFakeMcp();
});

describe("verify (AC-19: Clean / Broken / Stuck)", () => {
  it("returns Clean when cargo_check and cargo_test both pass, and closes the client", async () => {
    setMcpResponder((tool) => {
      if (tool === "cargo_check") return CLEAN_CHECK;
      if (tool === "cargo_test") return CLEAN_TEST;
      throw new Error(`unexpected tool ${tool}`);
    });

    const out = await verifyHandle({ value: INPUT });

    expect(out).toEqual({ kind: "Clean", value: {} });
    expect(fakeMcp.calls.map((c) => c.tool)).toEqual([
      "cargo_check",
      "cargo_test",
    ]);
    // Both tools got the crate path.
    for (const c of fakeMcp.calls) {
      expect(c.args).toEqual({ crate_path: "falcon-agent" });
    }
    expect(fakeMcp.instances).toHaveLength(1);
    expect(fakeMcp.instances[0].opts).toMatchObject({
      binary: INPUT.mcpBinary,
      root: INPUT.worktreePath,
    });
    expect(fakeMcp.instances[0].closed).toBe(true);
  });

  it("returns Broken with the compiler messages when cargo_check reports errors (cargo_test never runs)", async () => {
    setMcpResponder((tool) => {
      if (tool === "cargo_check")
        return {
          errors: [
            { level: "error", message: "E0308: mismatched types" },
            { level: "error", message: "E0433: failed to resolve" },
          ],
          warnings: [],
        };
      throw new Error(`unexpected tool ${tool}`);
    });

    const out = await verifyHandle({ value: INPUT });

    expect(out).toEqual({
      kind: "Broken",
      value: {
        errors: ["E0308: mismatched types", "E0433: failed to resolve"],
      },
    });
    expect(fakeMcp.calls.map((c) => c.tool)).toEqual(["cargo_check"]);
    expect(fakeMcp.instances[0].closed).toBe(true);
  });

  it("returns Broken with the failing test names when cargo_check is clean but cargo_test fails", async () => {
    setMcpResponder((tool) => {
      if (tool === "cargo_check") return CLEAN_CHECK;
      if (tool === "cargo_test")
        return {
          passed: ["a::ok"],
          failed: [
            { name: "interrogate::denies_bird", stdout: "assertion failed" },
          ],
          duration_ms: 40,
        };
      throw new Error(`unexpected tool ${tool}`);
    });

    const out = await verifyHandle({ value: INPUT });

    expect(out).toEqual({
      kind: "Broken",
      value: { errors: ["interrogate::denies_bird"] },
    });
  });

  it("returns Stuck at attempt >= 3 without ever spawning an MCP client", async () => {
    const out = await verifyHandle({ value: { ...INPUT, attempt: 3 } });

    expect(out).toEqual({ kind: "Stuck", value: { attempts: 3 } });
    expect(fakeMcp.instances).toHaveLength(0);
    expect(fakeMcp.calls).toHaveLength(0);
  });

  it("propagates an MCP tool failure (no silent catch) and still closes the client", async () => {
    setMcpResponder(() => new Error("tool cargo_check returned error: boom"));

    await expect(verifyHandle({ value: INPUT })).rejects.toThrow(
      /cargo_check returned error: boom/,
    );
    expect(fakeMcp.instances[0].closed).toBe(true);
  });
});
