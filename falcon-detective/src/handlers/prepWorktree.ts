import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { FalconMcpClient } from "../lib/mcp.js";

const Input = z.object({
  mcpBinary: z.string(),
  repoRoot: z.string(),     // the maltese-agent repo root
  runName: z.string(),      // unique name for this run, e.g. "demo-2026-05-15"
});

const Output = z.object({
  worktreePath: z.string(),
});

export const prepWorktree = createHandler({
  inputValidator: Input,
  outputValidator: Output,
  handle: async ({ value }) => {
    const c = new FalconMcpClient({ binary: value.mcpBinary, root: value.repoRoot });
    await c.connect();
    try {
      const r = await c.call<{ path: string }>("git_worktree_add", { name: value.runName });
      return { worktreePath: r.path };
    } finally {
      await c.close();
    }
  },
}, "prepWorktree");
