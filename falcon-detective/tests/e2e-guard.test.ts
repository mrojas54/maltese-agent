import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import { removeRunWorktree, runWorktreePath } from "./helpers/e2eCleanup.js";
import {
  checkCassetteFreshness,
  computeWorkflowFingerprint,
  FINGERPRINT_MANIFEST,
  readFingerprintManifest,
  writeFingerprintManifest,
} from "./helpers/e2eFingerprint.js";
import {
  applyGuardOutcome,
  checkE2ePrereqs,
  dirtyPaths,
  type GuardOutcome,
  hasCassettes,
} from "./helpers/e2ePrereqs.js";

// Hermetic proof of the e2e guard behaviors (AC-1..AC-4). Everything here
// operates on throwaway mkdtemp fixtures — never the real checkout — and
// spawns nothing but `git`, so it runs in the default `npm test` fast suite.
//
// `required` is always passed EXPLICITLY (never read from process.env inside
// the helper): CI runs this file under `E2E_REQUIRED=1 npm run test:full`,
// and the skip-expectation tests must not flip to fail-expectations there.

const CANONICAL = 'fn main() { println!("canonical"); }\n';
const DIRTY = `${CANONICAL}// uncommitted workshop edit \u{1F985}\n`;

let repo: string;

function git(...args: string[]): void {
  execFileSync("git", ["-c", "user.email=t@t", "-c", "user.name=t", ...args], {
    cwd: repo,
  });
}

/** Fixture repo with a committed falcon-agent/src/main.rs plus (outside the
 *  guarded pathspec) a dummy mcp binary file and a cassette dir. */
function makeFixture(): {
  mainRs: string;
  mcpBin: string;
  cassetteDir: string;
} {
  repo = mkdtempSync(join(tmpdir(), "fd-e2e-guard-"));
  mkdirSync(join(repo, "falcon-agent", "src"), { recursive: true });
  const mainRs = join(repo, "falcon-agent", "src", "main.rs");
  writeFileSync(mainRs, CANONICAL);
  git("init", "-q");
  git("add", "-A");
  git("commit", "-q", "-m", "init");
  // Prerequisites live OUTSIDE falcon-agent/ so they never trip the
  // dirty-state pathspec (and their presence here proves the pathspec scopes).
  const mcpBin = join(repo, "falcon-mcp-dummy");
  writeFileSync(mcpBin, "#!/bin/sh\n");
  const cassetteDir = join(repo, "cassettes");
  mkdirSync(cassetteDir);
  writeFileSync(join(cassetteDir, "abc123.json"), "{}");
  return { mainRs, mcpBin, cassetteDir };
}

function prereqs(
  overrides: Partial<Parameters<typeof checkE2ePrereqs>[0]> & {
    mcpBin: string;
    cassetteDir: string;
  },
): GuardOutcome {
  return checkE2ePrereqs({
    repoRoot: repo,
    required: false,
    ...overrides,
  });
}

function spyCtx(): {
  skipped: (string | undefined)[];
  skip: (reason?: string) => void;
} {
  const skipped: (string | undefined)[] = [];
  return { skipped, skip: (reason?: string) => skipped.push(reason) };
}

afterEach(() => {
  rmSync(repo, { recursive: true, force: true });
});

describe("e2e guard: dirty falcon-agent/ (AC-1, AC-2)", () => {
  it("skips (never passes, never cleans) and the dirty bytes survive byte-for-byte", () => {
    const { mainRs, mcpBin, cassetteDir } = makeFixture();
    writeFileSync(mainRs, DIRTY);
    const before = readFileSync(mainRs);

    const outcome = prereqs({ mcpBin, cassetteDir });
    expect(outcome.kind).toBe("skip");
    if (outcome.kind === "skip") {
      expect(outcome.reason).toContain("falcon-agent/src/main.rs");
      expect(outcome.reason).toContain("uncommitted");
    }

    // Maps to vitest SKIP status (ctx.skip called), not a silent pass.
    const ctx = spyCtx();
    applyGuardOutcome(outcome, ctx);
    expect(ctx.skipped).toHaveLength(1);
    expect(ctx.skipped[0]).toContain("falcon-agent/src/main.rs");

    // The guard mutated nothing: byte-for-byte survival.
    const after = readFileSync(mainRs);
    expect(after.equals(before)).toBe(true);
    expect(after.toString("utf8")).toBe(DIRTY);
  });

  it("fails under E2E_REQUIRED=1, naming the dirty path — and still mutates nothing", () => {
    const { mainRs, mcpBin, cassetteDir } = makeFixture();
    writeFileSync(mainRs, DIRTY);
    const before = readFileSync(mainRs);

    const outcome = prereqs({ mcpBin, cassetteDir, required: true });
    expect(outcome.kind).toBe("fail");

    const ctx = spyCtx();
    expect(() => applyGuardOutcome(outcome, ctx)).toThrowError(
      /falcon-agent\/src\/main\.rs/,
    );
    expect(ctx.skipped).toHaveLength(0);

    expect(readFileSync(mainRs).equals(before)).toBe(true);
  });

  it("counts untracked files under falcon-agent/ as dirty", () => {
    const { mcpBin, cassetteDir } = makeFixture();
    writeFileSync(join(repo, "falcon-agent", "scratch.rs"), "// wip\n");

    const outcome = prereqs({ mcpBin, cassetteDir });
    expect(outcome.kind).toBe("skip");
    if (outcome.kind === "skip") {
      expect(outcome.reason).toContain("falcon-agent/scratch.rs");
    }
  });

  it("dirtyPaths is empty on a clean checkout and scoped to the pathspec", () => {
    makeFixture(); // dummy bin + cassettes are untracked but OUTSIDE falcon-agent/
    expect(dirtyPaths(repo)).toEqual([]);
    writeFileSync(join(repo, "unrelated.txt"), "not in falcon-agent\n");
    expect(dirtyPaths(repo)).toEqual([]);
  });
});

describe("e2e guard: missing falcon-mcp binary (AC-3)", () => {
  it("reports SKIP status (not pass) and names the missing path", () => {
    const { cassetteDir } = makeFixture();
    const missing = join(repo, "no", "such", "falcon-mcp");

    const outcome = prereqs({ mcpBin: missing, cassetteDir });
    expect(outcome.kind).toBe("skip");
    if (outcome.kind === "skip") expect(outcome.reason).toContain(missing);

    const ctx = spyCtx();
    applyGuardOutcome(outcome, ctx);
    expect(ctx.skipped).toHaveLength(1);
  });

  it("fails under E2E_REQUIRED=1", () => {
    const { cassetteDir } = makeFixture();
    const missing = join(repo, "no", "such", "falcon-mcp");

    const outcome = prereqs({ mcpBin: missing, cassetteDir, required: true });
    expect(outcome.kind).toBe("fail");
    expect(() => applyGuardOutcome(outcome, spyCtx())).toThrowError(
      /E2E_REQUIRED/,
    );
  });
});

describe("e2e guard: missing cassettes (AC-4)", () => {
  it("reports SKIP status when the cassette dir is empty of .json", () => {
    const { mcpBin, cassetteDir } = makeFixture();
    rmSync(join(cassetteDir, "abc123.json"));

    const outcome = prereqs({ mcpBin, cassetteDir });
    expect(outcome.kind).toBe("skip");
    if (outcome.kind === "skip") expect(outcome.reason).toContain(cassetteDir);
  });

  it("reports SKIP status when the cassette dir does not exist", () => {
    const { mcpBin } = makeFixture();
    const outcome = prereqs({ mcpBin, cassetteDir: join(repo, "nope") });
    expect(outcome.kind).toBe("skip");
  });

  it("fails under E2E_REQUIRED=1", () => {
    const { mcpBin } = makeFixture();
    const outcome = prereqs({
      mcpBin,
      cassetteDir: join(repo, "nope"),
      required: true,
    });
    expect(outcome.kind).toBe("fail");
    expect(() => applyGuardOutcome(outcome, spyCtx())).toThrowError(
      /E2E_REQUIRED/,
    );
  });
});

describe("e2e guard: all prerequisites met", () => {
  it("returns run — applyGuardOutcome neither skips nor throws", () => {
    const { mcpBin, cassetteDir } = makeFixture();
    const outcome = prereqs({ mcpBin, cassetteDir });
    expect(outcome.kind).toBe("run");

    const ctx = spyCtx();
    expect(() => applyGuardOutcome(outcome, ctx)).not.toThrow();
    expect(ctx.skipped).toHaveLength(0);
  });
});

// ---------------------------------------------------------------------------
// Cassette-staleness fast path (MA-26). The verdict the pipeline used to
// deliver only after ~60s of cargo work is hoisted into a fingerprint
// comparison that runs BEFORE the pipeline launches. Same skip/fail
// semantics as the post-pipeline detection, hermetic mkdtemp fixtures.
// ---------------------------------------------------------------------------

/** makeFixture() plus a workflow dir (stand-in for src/ + prompts/). */
function makeFingerprintFixture() {
  const base = makeFixture();
  const workflowDir = join(repo, "wf");
  mkdirSync(join(workflowDir, "prompts"), { recursive: true });
  writeFileSync(join(workflowDir, "prompts", "triage.md"), "triage v1\n");
  writeFileSync(join(workflowDir, "gemini.ts"), "// hashing code v1\n");
  const fingerprintInput = {
    repoRoot: repo,
    workflowPaths: [workflowDir],
    model: "gemini-test-model",
  };
  return { ...base, workflowDir, fingerprintInput };
}

const STALE_PREFIX =
  "cassette miss — cassettes are stale relative to the workflow code";

describe("e2e staleness fast path: fresh cassettes (MA-26)", () => {
  it("returns run when the recorded fingerprint matches the current state", () => {
    const { cassetteDir, fingerprintInput } = makeFingerprintFixture();
    writeFingerprintManifest(
      cassetteDir,
      computeWorkflowFingerprint(fingerprintInput),
    );

    const outcome = checkCassetteFreshness({
      ...fingerprintInput,
      cassetteDir,
      required: false,
    });
    expect(outcome.kind).toBe("run");
  });

  it("manifest round-trips through write/read", () => {
    const { cassetteDir, fingerprintInput } = makeFingerprintFixture();
    const components = computeWorkflowFingerprint(fingerprintInput);
    writeFingerprintManifest(cassetteDir, components);
    expect(readFingerprintManifest(cassetteDir)).toEqual(components);
  });
});

describe("e2e staleness fast path: missing manifest ⇒ stale (MA-26)", () => {
  it("skips with the cassette-miss/stale message and a re-record pointer", () => {
    const { cassetteDir, fingerprintInput } = makeFingerprintFixture();
    // No manifest written — the state of the repo before the first
    // `npm run e2e:fingerprint` (freshness cannot be proven).
    const outcome = checkCassetteFreshness({
      ...fingerprintInput,
      cassetteDir,
      required: false,
    });
    expect(outcome.kind).toBe("skip");
    if (outcome.kind === "skip") {
      expect(outcome.reason).toContain(STALE_PREFIX);
      expect(outcome.reason).toContain("re-record");
      expect(outcome.reason).toContain(FINGERPRINT_MANIFEST);
    }

    const ctx = spyCtx();
    applyGuardOutcome(outcome, ctx);
    expect(ctx.skipped).toHaveLength(1);
  });

  it("fails under E2E_REQUIRED=1 with the 'could not run' phrasing", () => {
    const { cassetteDir, fingerprintInput } = makeFingerprintFixture();
    const outcome = checkCassetteFreshness({
      ...fingerprintInput,
      cassetteDir,
      required: true,
    });
    expect(outcome.kind).toBe("fail");
    expect(() =>
      applyGuardOutcome(
        outcome,
        spyCtx(),
        "E2E_REQUIRED=1 but the e2e could not run: ",
      ),
    ).toThrowError(/E2E_REQUIRED=1 but the e2e could not run: cassette miss/);
  });

  it("treats a corrupt manifest as stale rather than crashing", () => {
    const { cassetteDir, fingerprintInput } = makeFingerprintFixture();
    writeFileSync(join(cassetteDir, FINGERPRINT_MANIFEST), "not json{");
    const outcome = checkCassetteFreshness({
      ...fingerprintInput,
      cassetteDir,
      required: false,
    });
    expect(outcome.kind).toBe("skip");
    if (outcome.kind === "skip") expect(outcome.reason).toContain(STALE_PREFIX);
  });
});

describe("e2e staleness fast path: fingerprint drift ⇒ stale (MA-26)", () => {
  it("detects workflow drift (a prompt template changed) and names the component", () => {
    const { cassetteDir, workflowDir, fingerprintInput } =
      makeFingerprintFixture();
    writeFingerprintManifest(
      cassetteDir,
      computeWorkflowFingerprint(fingerprintInput),
    );
    writeFileSync(join(workflowDir, "prompts", "triage.md"), "triage v2\n");

    const outcome = checkCassetteFreshness({
      ...fingerprintInput,
      cassetteDir,
      required: false,
    });
    expect(outcome.kind).toBe("skip");
    if (outcome.kind === "skip") {
      expect(outcome.reason).toContain(STALE_PREFIX);
      expect(outcome.reason).toContain("workflow drifted");
    }
  });

  it("detects seed drift (a new falcon-agent commit)", () => {
    const { mainRs, cassetteDir, fingerprintInput } = makeFingerprintFixture();
    writeFingerprintManifest(
      cassetteDir,
      computeWorkflowFingerprint(fingerprintInput),
    );
    writeFileSync(mainRs, 'fn main() { println!("new seed"); }\n');
    git("add", "-A");
    git("commit", "-q", "-m", "seed change");

    const outcome = checkCassetteFreshness({
      ...fingerprintInput,
      cassetteDir,
      required: false,
    });
    expect(outcome.kind).toBe("skip");
    if (outcome.kind === "skip") expect(outcome.reason).toContain("seed");
  });

  it("detects model drift and fails under E2E_REQUIRED=1", () => {
    const { cassetteDir, fingerprintInput } = makeFingerprintFixture();
    writeFingerprintManifest(
      cassetteDir,
      computeWorkflowFingerprint(fingerprintInput),
    );

    const outcome = checkCassetteFreshness({
      ...fingerprintInput,
      model: "gemini-other-model",
      cassetteDir,
      required: true,
    });
    expect(outcome.kind).toBe("fail");
    if (outcome.kind === "fail") expect(outcome.reason).toContain("model");
  });
});

describe("e2e staleness fast path: interplay with the cassette guard (MA-26)", () => {
  it("the fingerprint manifest alone does not count as a cassette", () => {
    const { mcpBin, cassetteDir, fingerprintInput } = makeFingerprintFixture();
    rmSync(join(cassetteDir, "abc123.json"));
    writeFingerprintManifest(
      cassetteDir,
      computeWorkflowFingerprint(fingerprintInput),
    );

    expect(hasCassettes(cassetteDir)).toBe(false);
    // The AC-4 missing-cassette guard fires before freshness is consulted.
    const outcome = prereqs({ mcpBin, cassetteDir });
    expect(outcome.kind).toBe("skip");
    if (outcome.kind === "skip") expect(outcome.reason).toContain(cassetteDir);
  });
});

// ---------------------------------------------------------------------------
// Run-worktree cleanup (MA-29). A failed/timed-out e2e used to leave
// `.runs/e2e-test` behind, and the next run died on "worktree already
// exists" — masking the real error. removeRunWorktree must handle every
// leak shape idempotently. Hermetic: mkdtemp fixture repos, git only, no
// pipelines spawned.
// ---------------------------------------------------------------------------

describe("e2e run-worktree cleanup (MA-29)", () => {
  const RUN = "e2e-test";

  /** Registers a detached worktree at .runs/e2e-test (like prepWorktree's
   *  leftover). `--detach` avoids branch-name collisions across re-adds. */
  function addRunWorktree(): string {
    const path = runWorktreePath(repo, RUN);
    git("worktree", "add", "--detach", path);
    return path;
  }

  it("removes a registered leftover worktree and its registration", async () => {
    makeFixture();
    const path = addRunWorktree();
    expect(existsSync(path)).toBe(true);

    await removeRunWorktree(repo, RUN);
    expect(existsSync(path)).toBe(false);

    // Registration is gone too: the same path can be worktree-added again
    // (this is exactly the "next run" that used to die).
    expect(() => addRunWorktree()).not.toThrow();
  });

  it("removes a registered worktree with untracked debris (a failed run)", async () => {
    makeFixture();
    const path = addRunWorktree();
    writeFileSync(join(path, "half-applied.patch"), "leftover edit\n");

    await removeRunWorktree(repo, RUN);
    expect(existsSync(path)).toBe(false);
  });

  it("falls back to plain removal for an unregistered bare directory", async () => {
    makeFixture();
    const path = runWorktreePath(repo, RUN);
    mkdirSync(path, { recursive: true });
    writeFileSync(join(path, "junk.txt"), "not a worktree\n");

    await removeRunWorktree(repo, RUN);
    expect(existsSync(path)).toBe(false);
  });

  it("prunes a stale registration whose directory was rm'd manually", async () => {
    makeFixture();
    const path = addRunWorktree();
    rmSync(path, { recursive: true, force: true });

    await removeRunWorktree(repo, RUN);
    // Without the prune, this add fails on the stale .git/worktrees entry.
    expect(() => addRunWorktree()).not.toThrow();
  });

  it("is a no-op when nothing is left over", async () => {
    makeFixture();
    await expect(removeRunWorktree(repo, RUN)).resolves.toBeUndefined();
    expect(existsSync(runWorktreePath(repo, RUN))).toBe(false);
  });

  it("never touches the checkout outside .runs/", async () => {
    const { mainRs } = makeFixture();
    const before = readFileSync(mainRs);
    addRunWorktree();

    await removeRunWorktree(repo, RUN);
    expect(readFileSync(mainRs).equals(before)).toBe(true);
  });
});

describe("e2e worker-blockage regression (MA-26)", () => {
  it("tests/e2e.test.ts uses no synchronous child_process exec", () => {
    // The 2026-07 CI flake (vitest `Timeout calling "onTaskUpdate"`, PR #41)
    // was caused by execFileSync blocking the worker event loop for the ~62s
    // pipeline run. The e2e must spawn its long subprocesses asynchronously.
    const e2eSource = readFileSync(
      join(dirname(fileURLToPath(import.meta.url)), "e2e.test.ts"),
      "utf8",
    );
    expect(e2eSource).not.toMatch(/\b(execFileSync|execSync|spawnSync)\b/);
  });
});
