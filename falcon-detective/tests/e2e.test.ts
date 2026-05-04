import { describe, it, expect } from "vitest";
import { execSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import { join } from "node:path";

const REPO_ROOT = join(process.cwd(), "..");
const MCP_BIN  = join(REPO_ROOT, "target/debug/falcon-mcp");
const CASSETTE_DIR = join(REPO_ROOT, "falcon-detective/fixtures/cassettes");

function hasCassettes(): boolean {
  if (!existsSync(CASSETTE_DIR)) return false;
  return readdirSync(CASSETTE_DIR).some((f) => f.endsWith(".json"));
}

describe("e2e: full pipeline against falcon-agent", () => {
  it("runs to completion in cassette mode", () => {
    if (!existsSync(MCP_BIN)) {
      console.error("falcon-mcp not built; skipping e2e");
      return;
    }
    if (!hasCassettes()) {
      console.error("no cassettes recorded; skipping e2e");
      return;
    }

    // ensure falcon-agent is in its broken state
    execSync("git checkout -- falcon-agent/", { cwd: REPO_ROOT });

    const env = { ...process.env, GEMINI_MODE: "cassette" };
    execSync(
      `node falcon-detective/dist/cli.js --target falcon-agent --run-name e2e-test --mcp-binary ${MCP_BIN}`,
      { cwd: REPO_ROOT, env, stdio: "inherit" },
    );

    // After the run, the previously-#[ignore]'d smoking-gun test should pass
    // — proving the agent fixed the poison.
    const out = execSync(
      `cargo test -p falcon-agent --test integration -- --include-ignored bird_themed_inputs_arent_special`,
      { cwd: REPO_ROOT, encoding: "utf8" },
    );
    expect(out).toMatch(/test result: ok/);
  }, 120_000);
});
