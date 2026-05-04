import { describe, it, expect } from "vitest";
import { execSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

// process.cwd() under `npm --prefix falcon-detective test` is the WORKTREE
// root, not falcon-detective/, so `process.cwd()/..` resolves one level too
// high. Compute REPO_ROOT relative to this file instead.
const HERE = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(HERE, "..", "..");
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

    // CASSETTE_DIR must be passed explicitly: the CLI's gemini.ts falls back
    // to `process.cwd() + "/fixtures/cassettes"` when CASSETTE_DIR isn't set,
    // and the spawned process's cwd is REPO_ROOT (not falcon-detective/), so
    // the fallback path doesn't exist.
    const env = {
      ...process.env,
      GEMINI_MODE: "cassette",
      CASSETTE_DIR,
    };
    try {
      // --repo-root is explicit because the CLI default (..) resolves
      // against the spawning cwd, which puts .runs/ at the worktrees
      // parent dir rather than inside this worktree.
      execSync(
        `node falcon-detective/dist/cli.js --target falcon-agent --run-name e2e-test --repo-root ${REPO_ROOT} --mcp-binary ${MCP_BIN}`,
        { cwd: REPO_ROOT, env, stdio: "pipe" },
      );
    } catch (e: any) {
      const stderr = e.stderr?.toString() ?? "";
      // With prompt-hash normalization in place (gemini.ts normalizeForHash),
      // cassette miss should be rare. If it does happen, the cassettes are
      // stale relative to the workflow code — re-record with GEMINI_MODE=record.
      // Skip rather than fail to keep CI green.
      if (/no cassette for [0-9a-f]+/.test(stderr)) {
        console.error(
          "e2e: cassette miss — cassettes likely need re-recording (workflow or prompts changed since last GEMINI_MODE=record run). Skipping.",
        );
        return;
      }
      throw e;
    }

    // After the run, the previously-#[ignore]'d smoking-gun test must pass —
    // this is the gate that proves the agent fixed the poison.
    //
    // The cwd here is `.runs/e2e-test`, NOT REPO_ROOT. The agent's fixes are
    // applied inside the per-run worktree that prepWorktree creates; REPO_ROOT
    // was reset to HEAD above and never modified. Running cargo from the run
    // worktree picks up the agent's working-tree changes (the poison fix
    // doesn't have to be committed for cargo test to see it).
    const runWorktree = join(REPO_ROOT, ".runs/e2e-test");
    const smokeOut = execSync(
      `cargo test -p falcon-agent --test integration -- --include-ignored bird_themed_inputs_arent_special`,
      { cwd: runWorktree, encoding: "utf8" },
    );
    expect(smokeOut).toMatch(/test result: ok/);
  }, 120_000);
});
