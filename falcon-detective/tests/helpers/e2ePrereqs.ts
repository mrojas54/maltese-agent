import { execFileSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";

// e2e prerequisite guard (WS-1, AC-1..AC-5).
//
// The e2e must NEVER mutate the developer checkout: instead of "cleaning"
// falcon-agent/ (the old `git checkout -- falcon-agent/` destroyed
// uncommitted work), a dirty state makes the e2e SKIP with a reason naming
// the dirty paths — or FAIL when `required` (env E2E_REQUIRED=1 in CI), so a
// green required run mechanically implies the e2e executed.
//
// `required` is an explicit parameter, not read from process.env here:
// tests/e2e-guard.test.ts runs under CI's `E2E_REQUIRED=1 npm run test:full`
// and must be able to assert skip outcomes hermetically. Only
// tests/e2e.test.ts consults the env var.

export interface E2ePrereqInput {
  /** Repo whose working tree is checked for dirt (the developer checkout). */
  repoRoot: string;
  /** Path the falcon-mcp binary must exist at. */
  mcpBin: string;
  /** Directory that must contain at least one recorded *.json cassette. */
  cassetteDir: string;
  /** True when E2E_REQUIRED=1: unmet prerequisites fail instead of skip. */
  required: boolean;
  /** Pathspec guarded against uncommitted changes. */
  pathspec?: string;
}

export type GuardOutcome =
  | { kind: "run" }
  | { kind: "skip"; reason: string }
  | { kind: "fail"; reason: string };

/** Paths with uncommitted changes (tracked or untracked) under `pathspec`. */
export function dirtyPaths(repoRoot: string, pathspec = "falcon-agent/"): string[] {
  const out = execFileSync("git", ["status", "--porcelain", "--", pathspec], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  return out
    .split("\n")
    .filter((line) => line.length > 0)
    // Strip the two-char status + separator; renames keep "old -> new".
    .map((line) => line.slice(3));
}

export function hasCassettes(dir: string): boolean {
  if (!existsSync(dir)) return false;
  return readdirSync(dir).some((f) => f.endsWith(".json"));
}

export function checkE2ePrereqs(input: E2ePrereqInput): GuardOutcome {
  const pathspec = input.pathspec ?? "falcon-agent/";
  const blocked = (reason: string): GuardOutcome =>
    input.required ? { kind: "fail", reason } : { kind: "skip", reason };

  const dirty = dirtyPaths(input.repoRoot, pathspec);
  if (dirty.length > 0) {
    return blocked(
      `${pathspec} has uncommitted changes (${dirty.join(", ")}) — the e2e ` +
        `needs the canonical seed state and never mutates your checkout; ` +
        `commit or stash first`,
    );
  }
  if (!existsSync(input.mcpBin)) {
    return blocked(
      `falcon-mcp binary not found at ${input.mcpBin} — build it with ` +
        "`cargo build -p falcon-mcp`",
    );
  }
  if (!hasCassettes(input.cassetteDir)) {
    return blocked(
      `no recorded cassettes in ${input.cassetteDir} — record with ` +
        "`GEMINI_MODE=record` (see falcon-detective/README.md)",
    );
  }
  return { kind: "run" };
}

/**
 * Map a guard outcome onto vitest statuses: `fail` throws (test fails),
 * `skip` marks the test skipped via ctx.skip() — never a silent pass.
 *
 * vitest 1.6's `ctx.skip()` takes no note argument (vitest ≥2 does), so the
 * reason is also logged; the call signature stays forward-compatible.
 */
export function applyGuardOutcome(
  outcome: GuardOutcome,
  ctx: { skip: (reason?: string) => void },
): void {
  if (outcome.kind === "fail") {
    throw new Error(`E2E_REQUIRED=1 but e2e prerequisites are unmet: ${outcome.reason}`);
  }
  if (outcome.kind === "skip") {
    console.error(`e2e skipped: ${outcome.reason}`);
    ctx.skip(outcome.reason);
  }
}
