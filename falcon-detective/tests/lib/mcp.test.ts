import { existsSync } from "node:fs";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { FalconMcpClient } from "../../src/lib/mcp.js";

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

describe("FalconMcpClient", () => {
  it("calls fs_read through the MCP transport", async () => {
    if (!existsSync(FALCON_MCP_BIN)) {
      console.error("falcon-mcp not built; skipping FalconMcpClient test");
      return;
    }
    const c = new FalconMcpClient({ binary: FALCON_MCP_BIN, root: dir });
    await c.connect();
    const result = await c.call<{ content: string; bytes: number }>("fs_read", {
      path: "hello.txt",
    });
    expect(result.content).toBe("the falcon");
    await c.close();
  });
});
