import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { FalconMcpClient } from "../../src/lib/mcp.js";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const FALCON_MCP_BIN = process.env.FALCON_MCP_BIN ?? join(process.cwd(), "../target/debug/falcon-mcp");

let dir: string;

beforeAll(async () => {
  dir = await mkdtemp(join(tmpdir(), "fd-mcp-"));
  await writeFile(join(dir, "hello.txt"), "the falcon");
});
afterAll(async () => { await rm(dir, { recursive: true, force: true }); });

describe("FalconMcpClient", () => {
  it("calls fs_read through the MCP transport", async () => {
    const c = new FalconMcpClient({ binary: FALCON_MCP_BIN, root: dir });
    await c.connect();
    const result = await c.call<{ content: string; bytes: number }>("fs_read", { path: "hello.txt" });
    expect(result.content).toBe("the falcon");
    await c.close();
  });
});
