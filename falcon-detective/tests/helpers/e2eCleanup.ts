import { execFile } from "node:child_process";
import { existsSync } from "node:fs";
import { rm } from "node:fs/promises";
import { join } from "node:path";
import { promisify } from "node:util";

// Run-worktree cleanup (MA-29).
//
// The pipeline sandboxes each run in a git worktree at
// `<repoRoot>/.runs/<runName>` (prepWorktree). A failed or timed-out e2e
// leaves that worktree behind, and the NEXT run dies instantly on
// "worktree already exists" — masking whatever actually broke. The e2e
// calls removeRunWorktree both before the pipeline launches (idempotent
// pre-clean) and after the test (always-clean, unless KEEP_RUN=1).
//
// Async on purpose: tests/e2e-guard.test.ts pins tests/e2e.test.ts to
// no synchronous child_process exec (MA-26 worker-blockage regression),
// and this helper is part of that file's execution path.

const execFileAsync = promisify(execFile);

/** `<repoRoot>/.runs/<runName>` — where prepWorktree puts the run worktree. */
export function runWorktreePath(repoRoot: string, runName: string): string {
  return join(repoRoot, ".runs", runName);
}

/**
 * Remove a leftover run worktree, tolerating every leak shape:
 *
 *  - registered worktree, dir present  → `git worktree remove --force`
 *  - unregistered plain dir            → the git command fails; `rm -rf`
 *  - registered but dir already rm'd   → nothing to delete; prune below
 *  - nothing left over                 → no-op (prune is always safe)
 *
 * `git worktree prune` runs unconditionally at the end: a manually-deleted
 * worktree leaves a stale `.git/worktrees/<name>` registration that still
 * blocks the next `git worktree add` even though the directory is gone.
 */
export async function removeRunWorktree(
  repoRoot: string,
  runName: string,
): Promise<void> {
  const path = runWorktreePath(repoRoot, runName);
  if (existsSync(path)) {
    try {
      await execFileAsync("git", [
        "-C",
        repoRoot,
        "worktree",
        "remove",
        "--force",
        path,
      ]);
    } catch {
      // Not a registered worktree (or git already lost track of it):
      // plain removal; the prune below clears any stale registration.
      await rm(path, { recursive: true, force: true });
    }
  }
  await execFileAsync("git", ["-C", repoRoot, "worktree", "prune"]);
}
