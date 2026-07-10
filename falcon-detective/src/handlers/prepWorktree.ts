import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { FalconMcpClient } from "../lib/mcp.js";

// Fat input/output: cratePath threads through so triage can use it.
// Barnum validates schemas strictly (additionalProperties: false), so the
// only way to forward context across handlers is to declare it explicitly.
const Input = z.object({
  mcpBinary: z.string(),
  repoRoot: z.string(),
  runName: z.string(),
  cratePath: z.string(),
});

const Output = z.object({
  mcpBinary: z.string(),
  worktreePath: z.string(),
  cratePath: z.string(),
});

export const prepWorktree = createHandler(
  {
    inputValidator: Input,
    outputValidator: Output,
    handle: async ({ value }) => {
      const c = new FalconMcpClient({
        binary: value.mcpBinary,
        root: value.repoRoot,
      });
      await c.connect();
      try {
        const r = await c.call<{ path: string }>("git_worktree_add", {
          name: value.runName,
        });
        return {
          mcpBinary: value.mcpBinary,
          worktreePath: r.path,
          cratePath: value.cratePath,
        };
      } finally {
        await c.close();
      }
    },
  },
  "prepWorktree",
);
