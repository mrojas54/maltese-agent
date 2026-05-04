import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { UnifiedDiff } from "../lib/types.js";
import { FalconMcpClient } from "../lib/mcp.js";

const Input = z.object({
  mcpBinary: z.string(),
  worktreePath: z.string(),
  diff: UnifiedDiff,
});
const Output = z.object({ ok: z.boolean(), reason: z.string().optional() });

export const applyEdit = createHandler({
  inputValidator: Input,
  outputValidator: Output,
  handle: async ({ value }) => {
    const mcp = new FalconMcpClient({ binary: value.mcpBinary, root: value.worktreePath });
    await mcp.connect();
    try {
      const result = await mcp.call<{ result: "Ok" | "Conflict"; reason?: string }>("fs_apply_patch", {
        path: value.diff.path,
        unified_diff: value.diff.diff,
      });
      if (result.result === "Conflict") return { ok: false, reason: result.reason ?? "conflict" };
      return { ok: true };
    } finally { await mcp.close(); }
  },
}, "applyEdit");
