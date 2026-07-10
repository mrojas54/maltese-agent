import { execFile } from "node:child_process";
import { join } from "node:path";
import { promisify } from "node:util";
import { describe, expect, it } from "vitest";

const execFileP = promisify(execFile);

// AC-20 arg-parsing smoke: drive the real entrypoint as a subprocess with a
// controlled argv. The missing-required-flag path is the one arg-parsing
// behavior observable without launching the pipeline (and therefore hermetic:
// `arg("target")` throws before any engine, MCP, or Gemini work starts).
const TSX_CLI = join(process.cwd(), "node_modules", "tsx", "dist", "cli.mjs");
const CLI = join(process.cwd(), "src", "cli.ts");

describe("cli (AC-20: arg parsing smoke)", () => {
  it("exits 1 naming the missing required --target flag before doing any work", async () => {
    const result = await execFileP(process.execPath, [TSX_CLI, CLI], {
      cwd: process.cwd(),
      timeout: 20_000,
    }).then(
      () => null,
      (e: Error & { code?: number; stderr?: string; stdout?: string }) => e,
    );

    expect(result, "cli should fail without --target").not.toBeNull();
    expect(result?.code).toBe(1);
    expect(result?.stderr).toContain("missing required --target");
    // Fails during parsing: the run never starts.
    expect(result?.stdout ?? "").not.toContain("starting run");
  });
});
