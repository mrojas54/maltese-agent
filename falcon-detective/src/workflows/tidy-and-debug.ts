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
 *
 * No casts (WS-14a / AC-34): every stage is a `Handler<In, Out>` from
 * createHandler, so pipe() checks each connection point exactly (input
 * invariance) and branch() derives its input union from the arm handlers —
 * finalSweep's `{ kind: Clean|Dirty, value }` output must and does match
 * `{ Clean: commitAll's input, Dirty: escalate's input }`.
 */
import { branch, pipe } from "@barnum/barnum/pipeline";
import { commitAll, escalate } from "../handlers/commit.js";
import { finalSweep } from "../handlers/finalSweep.js";
import { prepWorktree } from "../handlers/prepWorktree.js";
import { processIssues } from "../handlers/processIssues.js";
import { triage } from "../handlers/triage.js";

export const detective = pipe(
  prepWorktree,
  triage,
  processIssues,
  finalSweep,
  branch({
    Clean: commitAll,
    Dirty: escalate,
  }),
);
