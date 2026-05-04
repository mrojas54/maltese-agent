/**
 * Tidy-and-debug workflow: triage → fix → verify → commit, looped per issue.
 *
 * Status (Task 13): structural skeleton.
 *
 * The plan flagged a known data-flow gap — `branch()` strips everything but
 * the `value` field of the previous step's tagged-union output, so
 * `applyEdit`/`verify`/`commitOne`/`revertOne` would lose the surrounding
 * context (`mcpBinary`, `worktreePath`, `cratePath`, `issue`, `diffPath`).
 * The recommended fix is Barnum's `bind` combinator to capture invariant
 * context as VarRefs.
 *
 * Three additional Barnum-0.4 signature mismatches were discovered while
 * implementing this file:
 *   1. `tryCatch(body, recovery)` takes a `(throwError) => Pipeable` callback
 *      as the body, not a `Pipeable` directly. The plan's `t60` helper
 *      passed the value form.
 *   2. `withTimeout` returns `Result<TOut, void>` — downstream handlers see
 *      a Result envelope that has to be unwrapped before flowing into the
 *      next step. The plan handed the raw output forward.
 *   3. `forEach` requires a uniform `Pipeable<In, Out>`, but the loop body
 *      below has multiple terminal arms with different output shapes.
 *
 * Until the cassettes are recorded (plan Task 15, manual) and the e2e in
 * Task 16 can drive end-to-end execution, the runtime correctness of this
 * composition cannot be observed. This file therefore exports a
 * type-checking skeleton with `as any` casts at the data-flow seams the
 * plan flagged. Filling those in is the explicit follow-up after Task 16
 * surfaces concrete failures.
 */
import { pipe, forEach, branch, getField } from "@barnum/barnum/pipeline";
import { prepWorktree } from "../handlers/prepWorktree.js";
import { triage } from "../handlers/triage.js";
import { readContext } from "../handlers/readContext.js";
import { classify } from "../handlers/classify.js";
import { analyzePoison } from "../handlers/analyzePoison.js";
import { proposePoisonFix } from "../handlers/proposePoisonFix.js";
import { proposeBugFix } from "../handlers/proposeBugFix.js";
import { proposeLintFix } from "../handlers/proposeLintFix.js";
import { applyEdit } from "../handlers/applyEdit.js";
import { verify } from "../handlers/verify.js";
import { commitOne, commitAll, revertOne, escalate } from "../handlers/commit.js";
import { finalSweep } from "../handlers/finalSweep.js";

// Per-issue handler. The branch inside the fix step dispatches on the
// classify envelope's `kind` and unwraps `value` to each arm.
const handleIssue: any = pipe(
  readContext as any,
  classify as any,
  branch({
    Poison: pipe(analyzePoison as any, proposePoisonFix as any) as any,
    Bug:    proposeBugFix as any,
    Lint:   proposeLintFix as any,
  }) as any,
  applyEdit as any,
  verify as any,
  branch({
    Clean:  commitOne as any,
    Broken: commitOne as any,  // TODO: loop back via recur once context-bind is wired
    Stuck:  revertOne as any,
  }) as any,
);

export const detective: any = pipe(
  prepWorktree as any,
  triage as any,
  getField("issues") as any,
  forEach(handleIssue) as any,
  finalSweep as any,
  branch({ Clean: commitAll as any, Dirty: escalate as any }) as any,
);
