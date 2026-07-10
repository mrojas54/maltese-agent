import { execFile } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { describe, expect, it } from "vitest";
import { DEFAULT_MODEL } from "../src/lib/models.js";
import { checkCassetteFreshness } from "./helpers/e2eFingerprint.js";
import { applyGuardOutcome, checkE2ePrereqs } from "./helpers/e2ePrereqs.js";

// Async subprocess execution is load-bearing here, not style (MA-26): the
// pipeline and smoke-test runs take minutes, and a synchronous exec blocks
// the vitest worker's event loop long enough that its RPC to the host dies
// (`Timeout calling "onTaskUpdate"`) — an unhandled error that fails the run
// even when every test passes. tests/e2e-guard.test.ts pins this with a
// no-sync-exec regression test on this file's source.
const execFileAsync = promisify(execFile);

// Pipeline stdout/stderr can carry full cargo output; default 1 MiB maxBuffer
// would throw ENOBUFS mid-run.
const MAX_BUFFER = 64 * 1024 * 1024;

// process.cwd() under `npm --prefix falcon-detective test` is the WORKTREE
// root, not falcon-detective/, so `process.cwd()/..` resolves one level too
// high. Compute REPO_ROOT relative to this file instead.
const HERE = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(HERE, "..", "..");
const MCP_BIN = join(REPO_ROOT, "target/debug/falcon-mcp");
const CASSETTE_DIR = join(REPO_ROOT, "falcon-detective/fixtures/cassettes");

// E2E_REQUIRED=1 (set by CI's typescript job) turns every skip below into a
// failure: a green required run mechanically implies the e2e executed (AC-5).
const REQUIRED = process.env.E2E_REQUIRED === "1";

describe("e2e: full pipeline against falcon-agent", () => {
  it("runs to completion in cassette mode", async (ctx) => {
    // Prerequisite + dirty-state guard (AC-1..AC-4). This test NEVER mutates
    // the developer checkout: if falcon-agent/ has uncommitted changes the
    // pipeline would investigate a non-canonical seed state, so we skip
    // (fail under E2E_REQUIRED=1) with the dirty paths named — we do NOT
    // reset anything. Isolation is the pipeline's own prepWorktree handler,
    // which sandboxes the run via falcon-mcp's git_worktree_add under
    // `<repo-root>/.runs/<run-name>` (covered by tests/handlers/prepWorktree.test.ts).
    applyGuardOutcome(
      checkE2ePrereqs({
        repoRoot: REPO_ROOT,
        mcpBin: MCP_BIN,
        cassetteDir: CASSETTE_DIR,
        required: REQUIRED,
      }),
      ctx,
    );

    // Cassette-staleness fast path (MA-26): the pipeline burns ~60s of cargo
    // work before its first Gemini call can report a cassette miss, so the
    // staleness verdict is hoisted here — a fingerprint comparison against
    // the manifest recorded with the cassettes (see helpers/e2eFingerprint.ts).
    // Stale ⇒ immediate skip (<1s), or hard-fail under E2E_REQUIRED=1, with
    // the same message semantics as the post-pipeline detection below (which
    // remains as the authoritative backstop).
    applyGuardOutcome(
      checkCassetteFreshness({
        repoRoot: REPO_ROOT,
        cassetteDir: CASSETTE_DIR,
        workflowPaths: [
          join(REPO_ROOT, "falcon-detective/src"),
          join(REPO_ROOT, "falcon-detective/prompts"),
        ],
        model: DEFAULT_MODEL,
        required: REQUIRED,
      }),
      ctx,
      "E2E_REQUIRED=1 but the e2e could not run: ",
    );

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
      await execFileAsync(
        "node",
        [
          "falcon-detective/dist/cli.js",
          "--target",
          "falcon-agent",
          "--run-name",
          "e2e-test",
          "--repo-root",
          REPO_ROOT,
          "--mcp-binary",
          MCP_BIN,
        ],
        { cwd: REPO_ROOT, env, maxBuffer: MAX_BUFFER },
      );
    } catch (e: any) {
      const stderr = e.stderr?.toString() ?? "";
      // With prompt-hash normalization in place (gemini.ts normalizeForHash),
      // cassette miss should be rare. If it does happen, the cassettes are
      // stale relative to the workflow code — re-record with GEMINI_MODE=record.
      // Report SKIPPED (never a silent pass); under E2E_REQUIRED=1 stale
      // cassettes fail the run, same as absent ones, to keep AC-5's
      // "green CI implies the e2e executed" guarantee.
      if (/no cassette for [0-9a-f]+/.test(stderr)) {
        const reason =
          "cassette miss — cassettes are stale relative to the workflow code " +
          "(workflow or prompts changed since the last GEMINI_MODE=record run); re-record them";
        if (REQUIRED)
          throw new Error(
            `E2E_REQUIRED=1 but the e2e could not run: ${reason}`,
          );
        console.error(`e2e skipped: ${reason}`);
        ctx.skip();
      }
      throw e;
    }

    // After the run, the previously-#[ignore]'d smoking-gun test must pass —
    // this is the gate that proves the agent fixed the poison.
    //
    // The cwd here is `.runs/e2e-test`, NOT REPO_ROOT. The agent's fixes are
    // applied inside the per-run worktree that prepWorktree creates; the
    // developer checkout is never touched by this test. Running cargo from
    // the run worktree picks up the agent's working-tree changes (the poison
    // fix doesn't have to be committed for cargo test to see it).
    const runWorktree = join(REPO_ROOT, ".runs/e2e-test");
    const { stdout: smokeOut } = await execFileAsync(
      "cargo",
      [
        "test",
        "-p",
        "falcon-agent",
        "--test",
        "integration",
        "--",
        "--include-ignored",
        "bird_themed_inputs_arent_special",
      ],
      { cwd: runWorktree, maxBuffer: MAX_BUFFER },
    );
    expect(smokeOut).toMatch(/test result: ok/);
  }, 120_000);
});
