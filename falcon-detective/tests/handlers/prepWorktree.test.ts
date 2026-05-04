import { describe, it, expect, beforeAll, afterAll } from "vitest";
import { prepWorktree } from "../../src/handlers/prepWorktree.js";
import { execSync } from "node:child_process";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const FALCON_MCP_BIN = process.env.FALCON_MCP_BIN ?? join(process.cwd(), "../target/debug/falcon-mcp");
let repo: string;

beforeAll(async () => {
  repo = await mkdtemp(join(tmpdir(), "fd-pw-"));
  execSync("git init", { cwd: repo });
  await writeFile(join(repo, "a.txt"), "x");
  execSync(
    "git -c user.email=t@t -c user.name=t add . && git -c user.email=t@t -c user.name=t commit -m init",
    { cwd: repo, shell: "/bin/bash" },
  );
});
afterAll(async () => { await rm(repo, { recursive: true, force: true }); });

describe("prepWorktree", () => {
  it("creates a worktree under .runs/", async () => {
    // The exported handler is a Barnum Action object; the handle function
    // lives on .__definition.handle so unit tests can invoke it directly
    // without booting the workflow runtime.
    const out = await (prepWorktree as any).__definition.handle({
      value: { mcpBinary: FALCON_MCP_BIN, repoRoot: repo, runName: "test-run", cratePath: "." },
    });
    expect(out.worktreePath).toMatch(/\.runs\/test-run$/);
    expect(out.cratePath).toBe(".");
    expect(out.mcpBinary).toBe(FALCON_MCP_BIN);
  });
});
