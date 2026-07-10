import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { FalconMcpClient } from "../lib/mcp.js";
import { CargoClippyResult, CargoTestResult } from "../lib/toolSchemas.js";

const Input = z.object({
  mcpBinary: z.string(),
  worktreePath: z.string(),
  cratePath: z.string(),
});
// `value` carries the mcpBinary/worktreePath context so the downstream
// branch arms (commitAll, escalate) can act on the worktree without the
// data-flow hole that branch()'s value-only auto-unwrap would otherwise
// produce.
const Output = z.discriminatedUnion("kind", [
  z.object({
    kind: z.literal("Clean"),
    value: z.object({ mcpBinary: z.string(), worktreePath: z.string() }),
  }),
  z.object({
    kind: z.literal("Dirty"),
    value: z.object({
      mcpBinary: z.string(),
      worktreePath: z.string(),
      reasons: z.array(z.string()),
    }),
  }),
]);

/**
 * Handle body, exported as a public module seam so the orchestration tests
 * (MA-13 / AC-19) can drive it in-process with a mocked FalconMcpClient —
 * `vi.mock` cannot cross the Barnum engine's process boundary, and Barnum's
 * underscore-private surface is barred (AC-13). The Barnum handler below wires
 * this exact function unchanged.
 */
export async function finalSweepHandle({
  value,
}: {
  value: z.infer<typeof Input>;
}): Promise<z.infer<typeof Output>> {
  const mcp = new FalconMcpClient({
    binary: value.mcpBinary,
    root: value.worktreePath,
  });
  await mcp.connect();
  try {
    const reasons: string[] = [];
    const test = await mcp.call(
      "cargo_test",
      { crate_path: value.cratePath },
      CargoTestResult,
    );
    if (test.failed.length > 0)
      reasons.push(`${test.failed.length} tests still failing`);
    const clippy = await mcp.call(
      "cargo_clippy",
      { crate_path: value.cratePath },
      CargoClippyResult,
    );
    if (clippy.lints.length > 0)
      reasons.push(`${clippy.lints.length} clippy lints remain`);
    const ctx = {
      mcpBinary: value.mcpBinary,
      worktreePath: value.worktreePath,
    };
    return reasons.length === 0
      ? { kind: "Clean" as const, value: ctx }
      : { kind: "Dirty" as const, value: { ...ctx, reasons } };
  } finally {
    await mcp.close();
  }
}

export const finalSweep = createHandler(
  {
    inputValidator: Input,
    outputValidator: Output,
    handle: finalSweepHandle,
  },
  "finalSweep",
);
