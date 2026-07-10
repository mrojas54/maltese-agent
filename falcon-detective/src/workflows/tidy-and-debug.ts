/**
 * Tidy-and-debug workflow.
 *
 * Outer shape (Barnum-orchestrated):
 *   prepWorktree → triage → processIssues → finalSweep → branch(commitAll | escalate)
 *
 * `processIssues` is a single coarse Barnum handler that runs the per-issue
 * fix loop in plain TypeScript internally — see src/handlers/processIssues.ts
 * for the rationale. The smaller per-issue handlers (readContext, classify,
 * analyzePoison, proposePoisonFix, proposeBugFix, proposeLintFix, applyEdit,
 * verify, commitOne, revertOne) are still independently exported and
 * unit-testable; processIssues invokes them through the public engine API
 * (`runPipeline` via the src/lib/invokeHandler.ts seam — WS-5/AC-13),
 * sidestepping the bind+VarRef context plumbing that Barnum's strict
 * per-step schemas would otherwise require.
 */
import { branch, pipe } from "@barnum/barnum/pipeline";
import { commitAll, escalate } from "../handlers/commit.js";
import { finalSweep } from "../handlers/finalSweep.js";
import { prepWorktree } from "../handlers/prepWorktree.js";
import { processIssues } from "../handlers/processIssues.js";
import { triage } from "../handlers/triage.js";

export const detective: any = pipe(
  prepWorktree as any,
  triage as any,
  processIssues as any,
  finalSweep as any,
  branch({
    Clean: commitAll as any,
    Dirty: escalate as any,
  }) as any,
);
