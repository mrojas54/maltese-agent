import { execFile } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { DEFAULT_MODEL } from "../src/lib/models.js";
import { removeRunWorktree } from "./helpers/e2eCleanup.js";
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
const PKG_ROOT = join(HERE, "..");
const REPO_ROOT = join(HERE, "..", "..");
const MCP_BIN = join(REPO_ROOT, "target/debug/falcon-mcp");
const CASSETTE_DIR = join(REPO_ROOT, "falcon-detective/fixtures/cassettes");
const RUN_NAME = "e2e-test";

// MA-37: default CARGO_TARGET_DIR to the repo-root target/ (already warm from
// the falcon-mcp build) so BOTH the pipeline's cargo runs (forwarded through
// falcon-mcp, see src/lib/mcp.ts) and the post-pipeline smoke `cargo test`
// execute against a shared, warm target instead of a cold compile inside the
// fresh .runs/e2e-test worktree that races the 360s budget. An explicit dev
// override is respected.
const CARGO_TARGET_DIR =
  process.env.CARGO_TARGET_DIR ?? join(REPO_ROOT, "target");

// E2E_REQUIRED=1 (set by CI's typescript job) turns every skip below into a
// failure: a green required run mechanically implies the e2e executed (AC-5).
const REQUIRED = process.env.E2E_REQUIRED === "1";

describe("e2e: full pipeline against falcon-agent", () => {
  // Dist freshness (MA-29): this test invokes dist/cli.js, but nothing else
  // guarantees dist matches src at test time — `npm run record` runs tsx
  // against src, so a recorded-then-replayed session can pair fresh cassettes
  // with a stale dist. Observed live: a pre-MA-27 dist still calling
  // gemini-2.5-pro replayed against 3.1-keyed cassettes → bogus "cassette
  // miss". Building here via `npm run build` (tsc -p tsconfig.build.json —
  // the same command CI's build step runs, and robust to compiler/bin-layout
  // changes) pins dist to src for every e2e run. execFileAsync + MAX_BUFFER
  // keep it bounded and async (see the worker-blockage note above).
  beforeAll(async () => {
    await execFileAsync("npm", ["run", "build"], {
      cwd: PKG_ROOT,
      maxBuffer: MAX_BUFFER,
    });
    // Worktree-leak guard (MA-29): a failed or timed-out earlier run leaves
    // <repo-root>/.runs/e2e-test behind, and the next pipeline dies on
    // "worktree already exists" — masking the real error. Idempotent
    // pre-clean so every run starts from a blank slate.
    await removeRunWorktree(REPO_ROOT, RUN_NAME);
  }, 180_000);

  // Always clean the run worktree up afterwards — pass, fail, or timeout —
  // EXCEPT under KEEP_RUN=1, which preserves .runs/e2e-test so a failed
  // run's worktree (agent edits, per-issue commits) can be inspected
  // post-mortem: `KEEP_RUN=1 npm run test:full -- tests/e2e.test.ts`.
  afterAll(async () => {
    if (process.env.KEEP_RUN === "1") return;
    await removeRunWorktree(REPO_ROOT, RUN_NAME);
  }, 60_000);

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

    // CASSETTE_DIR is passed explicitly as defense in depth: gemini.ts's
    // fallback is package-anchored since MA-29 (resolved from the module's
    // own location, not process.cwd()), so a worker spawned from REPO_ROOT
    // would resolve the right directory anyway — but the e2e pins the exact
    // dir its guards fingerprinted rather than trusting the fallback.
    const env = {
      ...process.env,
      GEMINI_MODE: "cassette",
      CASSETTE_DIR,
      CARGO_TARGET_DIR,
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
          RUN_NAME,
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
    const runWorktree = join(REPO_ROOT, ".runs", RUN_NAME);
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
      {
        cwd: runWorktree,
        env: { ...process.env, CARGO_TARGET_DIR },
        maxBuffer: MAX_BUFFER,
      },
    );
    expect(smokeOut).toMatch(/test result: ok/);
    // 360s (MA-29): a healthy full replay is ~114s of pipeline plus the
    // post-pipeline cargo smoke test (a cold falcon-agent build inside the
    // fresh run worktree); the previous 120s budget raced a legitimate run
    // and lost. This override is scoped to THIS test only — the fast suite
    // excludes this file and keeps vitest.config.ts's 30s default.
  }, 360_000);
});
