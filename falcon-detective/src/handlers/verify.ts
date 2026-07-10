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

export const verify = createHandler(
  {
    inputValidator: Input,
    outputValidator: VerifyResult,
    handle: async ({ value }) => {
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
    },
  },
  "verify",
);
