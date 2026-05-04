import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { Issue } from "../lib/types.js";
import { FalconMcpClient } from "../lib/mcp.js";

const McpAndPath = z.object({ mcpBinary: z.string(), worktreePath: z.string() });

export const commitOne = createHandler({
  inputValidator: McpAndPath.extend({ issue: Issue, diffPath: z.string() }),
  outputValidator: z.object({ sha: z.string() }),
  handle: async ({ value }) => {
    const mcp = new FalconMcpClient({ binary: value.mcpBinary, root: value.worktreePath });
    await mcp.connect();
    try {
      await mcp.call("git_add", { paths: [value.diffPath] });
      const r = await mcp.call<{ sha: string }>("git_commit", {
        message: `fix(${value.issue.kind}): ${value.issue.location.file}${value.issue.location.line ? `:${value.issue.location.line}` : ""}`,
      });
      return r;
    } finally { await mcp.close(); }
  },
}, "commitOne");

export const commitAll = createHandler({
  inputValidator: McpAndPath,
  outputValidator: z.object({ ok: z.literal(true) }),
  handle: async () => ({ ok: true as const }),  // per-issue commits already happened; this is a marker
}, "commitAll");

export const revertOne = createHandler({
  inputValidator: McpAndPath.extend({ issue: Issue }),
  outputValidator: z.object({ ok: z.literal(true) }),
  handle: async ({ value }) => {
    // exec is required here (`git checkout` isn't a built-in MCP tool); the
    // allowlist in falcon-mcp's sandbox already includes "git", so this is safe.
    const mcp = new FalconMcpClient({ binary: value.mcpBinary, root: value.worktreePath, enableExec: true });
    await mcp.connect();
    try {
      // Reset working tree changes for the file under question; keep prior commits.
      await mcp.call("exec_run", {
        cmd: "git", args: ["checkout", "--", value.issue.location.file],
      }).catch(() => { /* no-op if file not modified */ });
      return { ok: true as const };
    } finally { await mcp.close(); }
  },
}, "revertOne");

export const escalate = createHandler({
  inputValidator: McpAndPath.extend({ reason: z.string() }),
  outputValidator: z.object({ escalated: z.literal(true) }),
  handle: async ({ value }) => {
    console.error(`[escalate] manual review needed: ${value.reason}`);
    return { escalated: true as const };
  },
}, "escalate");
