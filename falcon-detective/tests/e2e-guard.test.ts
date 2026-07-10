import { execFileSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import {
  type GuardOutcome,
  applyGuardOutcome,
  checkE2ePrereqs,
  dirtyPaths,
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
