import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { FalconMcpClient } from "../lib/mcp.js";
import { CargoCheckResult, CargoTestResult } from "../lib/toolSchemas.js";
import { VerifyResult } from "../lib/types.js";

const Input = z.object({
  mcpBinary: z.string(),
  worktreePath: z.string(),
  cratePath: z.string(),
  attempt: z.number().int().nonnegative(),
});

/**
 * Handle body, exported as a public module seam so the orchestration tests
 * (MA-13 / AC-19) can drive it in-process with a mocked FalconMcpClient —
 * `vi.mock` cannot cross the Barnum engine's process boundary, and Barnum's
 * underscore-private surface is barred (AC-13). The Barnum handler below wires
 * this exact function unchanged.
 */
export async function verifyHandle({
  value,
}: {
  value: z.infer<typeof Input>;
}): Promise<VerifyResult> {
  if (value.attempt >= 3)
    return { kind: "Stuck" as const, value: { attempts: value.attempt } };
  const mcp = new FalconMcpClient({
    binary: value.mcpBinary,
    root: value.worktreePath,
  });
  await mcp.connect();
  try {
    const check = await mcp.call(
      "cargo_check",
      { crate_path: value.cratePath },
      CargoCheckResult,
    );
    if (check.errors.length > 0) {
      return {
        kind: "Broken" as const,
        value: {
          errors: check.errors.map((e) => e.message),
        },
      };
    }
    const test = await mcp.call(
      "cargo_test",
      { crate_path: value.cratePath },
      CargoTestResult,
    );
    if (test.failed.length > 0) {
      return {
        kind: "Broken" as const,
        value: { errors: test.failed.map((t) => t.name) },
      };
    }
    return { kind: "Clean" as const, value: {} };
  } finally {
    await mcp.close();
  }
}

export const verify = createHandler(
  {
    inputValidator: Input,
    outputValidator: VerifyResult,
    handle: verifyHandle,
  },
  "verify",
);
