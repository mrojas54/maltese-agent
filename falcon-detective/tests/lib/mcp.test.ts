import { existsSync } from "node:fs";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { FalconMcpClient, McpValidationError } from "../../src/lib/mcp.js";
import { CargoCheckResult, FsReadResult } from "../../src/lib/toolSchemas.js";

const FALCON_MCP_BIN =
  process.env.FALCON_MCP_BIN ??
  join(process.cwd(), "../target/debug/falcon-mcp");

let dir: string;

beforeAll(async () => {
  dir = await mkdtemp(join(tmpdir(), "fd-mcp-"));
  await writeFile(join(dir, "hello.txt"), "the falcon");
});
afterAll(async () => {
  await rm(dir, { recursive: true, force: true });
});

/** Hermetic client: swaps the private SDK client for a stub so the
 *  schema-validation boundary can be exercised without the falcon-mcp
 *  binary (the fast suite pins FALCON_MCP_BIN to a nonexistent path). */
function stubbedClient(result: Record<string, unknown>): FalconMcpClient {
  const c = new FalconMcpClient({ binary: "/nonexistent", root: "/tmp" });
  (c as unknown as { client: unknown }).client = {
    callTool: async () => result,
  };
  return c;
}

describe("FalconMcpClient", () => {
  it("calls fs_read through the MCP transport", async () => {
    if (!existsSync(FALCON_MCP_BIN)) {
      console.error("falcon-mcp not built; skipping FalconMcpClient test");
      return;
    }
    const c = new FalconMcpClient({ binary: FALCON_MCP_BIN, root: dir });
    await c.connect();
    const result = await c.call("fs_read", { path: "hello.txt" }, FsReadResult);
    expect(result.content).toBe("the falcon");
    expect(result.bytes).toBe(10);
    await c.close();
  });

  it("returns the parsed result when structuredContent matches the schema", async () => {
    const c = stubbedClient({
      structuredContent: { content: "hi", bytes: 2 },
    });
    const r = await c.call("fs_read", { path: "x" }, FsReadResult);
    expect(r).toEqual({ content: "hi", bytes: 2 });
  });

  it("throws McpValidationError naming the tool and the mismatch on a malformed response", async () => {
    const c = stubbedClient({
      structuredContent: { errors: "not-an-array", warnings: [] },
    });
    const err = await c
      .call("cargo_check", { crate_path: "." }, CargoCheckResult)
      .then(
        () => {
          throw new Error("expected McpValidationError");
        },
        (e: unknown) => e,
      );
    expect(err).toBeInstanceOf(McpValidationError);
    const v = err as McpValidationError;
    expect(v.name).toBe("McpValidationError");
    expect(v.toolName).toBe("cargo_check");
    // Structured: the zod issues survive for programmatic use...
    expect(v.issues.length).toBeGreaterThan(0);
    expect(v.issues[0].path).toEqual(["errors"]);
    // ...and the message names the tool and the mismatched path (AC-15).
    expect(v.message).toContain("cargo_check");
    expect(v.message).toContain("errors");
  });

  it("still surfaces result-level tool errors (isError) as execution errors, not validation errors", async () => {
    const c = stubbedClient({
      isError: true,
      content: [{ type: "text", text: "sandbox is read-only" }],
    });
    const err = await c.call("fs_read", { path: "x" }, FsReadResult).then(
      () => {
        throw new Error("expected an error");
      },
      (e: unknown) => e,
    );
    expect(err).toBeInstanceOf(Error);
    expect(err).not.toBeInstanceOf(McpValidationError);
    expect((err as Error).message).toContain("fs_read");
    expect((err as Error).message).toContain("read-only");
  });

  it("validates the content fallback through the same schema when structuredContent is absent", async () => {
    const c = stubbedClient({
      content: [{ type: "text", text: "not the schema shape" }],
    });
    await expect(
      c.call("fs_read", { path: "x" }, FsReadResult),
    ).rejects.toBeInstanceOf(McpValidationError);
  });

  it("rejects before any transport use when not connected", async () => {
    const c = new FalconMcpClient({ binary: "/nonexistent", root: "/tmp" });
    await expect(
      c.call("fs_read", { path: "x" }, FsReadResult),
    ).rejects.toThrow(/not connected/);
  });
});
