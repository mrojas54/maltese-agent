import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { FalconMcpClient } from "../lib/mcp.js";

const Input = z.object({
  mcpBinary: z.string(),
  worktreePath: z.string(),
  cratePath: z.string(),
});
const Output = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("Clean"), value: z.object({}) }),
  z.object({ kind: z.literal("Dirty"), value: z.object({ reasons: z.array(z.string()) }) }),
]);

export const finalSweep = createHandler({
  inputValidator: Input,
  outputValidator: Output,
  handle: async ({ value }) => {
    const mcp = new FalconMcpClient({ binary: value.mcpBinary, root: value.worktreePath });
    await mcp.connect();
    try {
      const reasons: string[] = [];
      const test = await mcp.call<{ failed: any[] }>("cargo_test", { crate_path: value.cratePath });
      if (test.failed.length > 0) reasons.push(`${test.failed.length} tests still failing`);
      const clippy = await mcp.call<{ lints: any[] }>("cargo_clippy", { crate_path: value.cratePath });
      if (clippy.lints.length > 0) reasons.push(`${clippy.lints.length} clippy lints remain`);
      return reasons.length === 0
        ? { kind: "Clean" as const, value: {} }
        : { kind: "Dirty" as const, value: { reasons } };
    } finally { await mcp.close(); }
  },
}, "finalSweep");
