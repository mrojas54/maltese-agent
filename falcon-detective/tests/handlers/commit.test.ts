import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fakeMcp, resetFakeMcp, setMcpResponder } from "../helpers/fakeMcp.js";

// Substitute the MA-10 schema'd client boundary with the shared in-process
// fake (public seam: module export). See tests/helpers/fakeMcp.ts.
vi.mock("../../src/lib/mcp.js", () => import("../helpers/fakeMcp.js"));

import {
  commitAllHandle,
  commitOneHandle,
  escalateHandle,
  revertOneHandle,
} from "../../src/handlers/commit.js";

const CTX = {
  mcpBinary: "/nonexistent/hermetic-falcon-mcp",
  worktreePath: "/tmp/fd-commit-wt",
};

const BUG_ISSUE = {
  kind: "bug" as const,
  location: { file: "src/lib.rs", line: 3 },
  evidence: "x.unwrap()",
};

beforeEach(() => {
  resetFakeMcp();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("commitOne (AC-19: success and git-failure paths, no silent catch)", () => {
  it("stages the diff path, commits with the fix(kind): file:line message, and returns the sha", async () => {
    setMcpResponder((tool) => {
      if (tool === "git_add") return { ok: true };
      if (tool === "git_commit") return { sha: "abc1234" };
      throw new Error(`unexpected tool ${tool}`);
    });

    const out = await commitOneHandle({
      value: { ...CTX, issue: BUG_ISSUE, diffPath: "src/lib.rs" },
    });

    expect(out).toEqual({ sha: "abc1234" });
    expect(fakeMcp.calls).toEqual([
      { tool: "git_add", args: { paths: ["src/lib.rs"] } },
      { tool: "git_commit", args: { message: "fix(bug): src/lib.rs:3" } },
    ]);
    expect(fakeMcp.instances).toHaveLength(1);
    expect(fakeMcp.instances[0].closed).toBe(true);
  });

  it("omits the :line suffix when the issue location has no line", async () => {
    setMcpResponder((tool) =>
      tool === "git_add" ? { ok: true } : { sha: "def5678" },
    );

    await commitOneHandle({
      value: {
        ...CTX,
        issue: {
          kind: "lint" as const,
          location: { file: "src/x.rs" },
          evidence: "unused import",
        },
        diffPath: "src/x.rs",
      },
    });

    expect(fakeMcp.calls[1].args).toEqual({ message: "fix(lint): src/x.rs" });
  });

  it("surfaces a git_add failure (no silent catch) and still closes the client", async () => {
    setMcpResponder((tool) => {
      if (tool === "git_add")
        return new Error("tool git_add returned error: index locked");
      throw new Error(`unexpected tool ${tool}`);
    });

    await expect(
      commitOneHandle({
        value: { ...CTX, issue: BUG_ISSUE, diffPath: "src/lib.rs" },
      }),
    ).rejects.toThrow(/git_add returned error: index locked/);
    // git_commit never attempted after the failed stage.
    expect(fakeMcp.calls.map((c) => c.tool)).toEqual(["git_add"]);
    expect(fakeMcp.instances[0].closed).toBe(true);
  });

  it("surfaces a git_commit failure (no silent catch)", async () => {
    setMcpResponder((tool) => {
      if (tool === "git_add") return { ok: true };
      return new Error("tool git_commit returned error: nothing to commit");
    });

    await expect(
      commitOneHandle({
        value: { ...CTX, issue: BUG_ISSUE, diffPath: "src/lib.rs" },
      }),
    ).rejects.toThrow(/git_commit returned error: nothing to commit/);
    expect(fakeMcp.instances[0].closed).toBe(true);
  });
});

describe("commitAll (AC-19)", () => {
  it("returns the ok marker without touching MCP (per-issue commits already happened)", async () => {
    const out = await commitAllHandle();

    expect(out).toEqual({ ok: true });
    expect(fakeMcp.instances).toHaveLength(0);
    expect(fakeMcp.calls).toHaveLength(0);
  });
});

describe("revertOne (AC-19: success and git-failure paths)", () => {
  it("checks out the issue's file via exec_run on an exec-enabled client", async () => {
    setMcpResponder((tool) => {
      if (tool === "exec_run") return { stdout: "", stderr: "", exit: 0 };
      throw new Error(`unexpected tool ${tool}`);
    });

    const out = await revertOneHandle({ value: { ...CTX, issue: BUG_ISSUE } });

    expect(out).toEqual({ ok: true });
    expect(fakeMcp.calls).toEqual([
      {
        tool: "exec_run",
        args: { cmd: "git", args: ["checkout", "--", "src/lib.rs"] },
      },
    ]);
    expect(fakeMcp.instances[0].opts.enableExec).toBe(true);
    expect(fakeMcp.instances[0].closed).toBe(true);
  });

  it("still resolves ok and closes when exec_run fails — the documented no-op-if-unmodified carve-out (CURRENT behavior)", async () => {
    setMcpResponder(
      () => new Error("tool exec_run returned error: pathspec did not match"),
    );

    const out = await revertOneHandle({ value: { ...CTX, issue: BUG_ISSUE } });

    expect(out).toEqual({ ok: true });
    expect(fakeMcp.calls.map((c) => c.tool)).toEqual(["exec_run"]);
    expect(fakeMcp.instances[0].closed).toBe(true);
  });
});

describe("escalate (AC-19)", () => {
  it("logs every reason to stderr and returns the escalated marker without touching MCP", async () => {
    const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});

    const out = await escalateHandle({
      value: {
        ...CTX,
        reasons: ["2 tests still failing", "3 clippy lints remain"],
      },
    });

    expect(out).toEqual({ escalated: true });
    expect(errSpy).toHaveBeenCalledTimes(1);
    expect(errSpy).toHaveBeenCalledWith(
      "[escalate] manual review needed: 2 tests still failing; 3 clippy lints remain",
    );
    expect(fakeMcp.instances).toHaveLength(0);
  });
});
