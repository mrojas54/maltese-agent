import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { Issue } from "../lib/types.js";
import { readContext } from "./readContext.js";
import { classify } from "./classify.js";
import { analyzePoison } from "./analyzePoison.js";
import { proposePoisonFix } from "./proposePoisonFix.js";
import { proposeBugFix } from "./proposeBugFix.js";
import { proposeLintFix } from "./proposeLintFix.js";
import { applyEdit } from "./applyEdit.js";
import { verify } from "./verify.js";
import { commitOne } from "./commit.js";

/**
 * Coarse Barnum handler for the per-issue loop. Inside, the per-issue logic
 * (readContext → classify → propose → applyEdit → verify → commit) runs in
 * plain TypeScript by invoking each smaller handler's underlying definition.
 *
 * Why this shape: Barnum's branch() strips all but `value` from each arm's
 * input, and forEach loses the surrounding context. The plan flagged this
 * and recommended bind+VarRef plumbing, but bind composition is non-trivial
 * with Barnum's strict per-handler input/output schemas. Keeping the inner
 * loop in TS sidesteps the data-flow gymnastics entirely without breaking
 * the demo's "Barnum at the outer level" point — `prepWorktree → triage →
 * processIssues → finalSweep` is still a Barnum-orchestrated pipeline, and
 * each smaller handler is still independently unit-testable.
 */
const Input = z.object({
  mcpBinary: z.string(),
  worktreePath: z.string(),
  cratePath: z.string(),
  issues: z.array(Issue),
});

const Output = z.object({
  mcpBinary: z.string(),
  worktreePath: z.string(),
  cratePath: z.string(),
});

const MAX_RETRIES = 3;

// Helper: invoke a Barnum handler's underlying definition without going
// through the engine. The handlers exported by createHandler() expose the
// implementation on a non-enumerable __definition.handle field.
async function call<TIn, TOut>(handler: any, value: TIn): Promise<TOut> {
  return handler.__definition.handle({ value }) as Promise<TOut>;
}

export const processIssues = createHandler({
  inputValidator: Input,
  outputValidator: Output,
  handle: async ({ value }) => {
    const { mcpBinary, worktreePath, cratePath, issues } = value;

    for (const issue of issues) {
      // 1. read file context
      const ctx = await call<any, any>(readContext, { mcpBinary, worktreePath, issue });

      // 2. classify into { kind, value: { issue, excerpts } }
      const tagged = await call<any, any>(classify, { issue, excerpts: ctx.excerpts });

      // 3. propose a fix per kind
      let diff: { path: string; diff: string };
      try {
        if (tagged.kind === "Poison") {
          const analysis = await call<any, any>(analyzePoison, tagged.value);
          diff = await call<any, any>(proposePoisonFix, analysis);
        } else if (tagged.kind === "Bug") {
          diff = await call<any, any>(proposeBugFix, tagged.value);
        } else {
          diff = await call<any, any>(proposeLintFix, tagged.value);
        }
      } catch (e) {
        console.error(`[processIssues] propose-fix failed for ${issue.location.file}: ${(e as Error).message}`);
        continue;
      }

      // 4. attempt-apply-verify-retry loop
      let attempt = 0;
      let succeeded = false;
      while (attempt < MAX_RETRIES) {
        const apply = await call<any, any>(applyEdit, { mcpBinary, worktreePath, diff });
        if (!apply.ok) {
          console.error(`[processIssues] applyEdit failed for ${diff.path}: ${apply.reason}`);
          break;
        }
        const result = await call<any, any>(verify, { mcpBinary, worktreePath, cratePath, attempt });
        if (result.kind === "Clean") { succeeded = true; break; }
        if (result.kind === "Stuck") { break; }
        // Broken: bump attempt and retry the same diff (the plan's design defers
        // re-proposing to a future iteration; the loop honors verify's Stuck
        // signal at the cap rather than reinventing the recovery here).
        attempt++;
      }

      // 5. commit on success, otherwise leave the working-tree changes for
      // a human to inspect during the demo.
      if (succeeded) {
        try {
          await call<any, any>(commitOne, { mcpBinary, worktreePath, issue, diffPath: diff.path });
        } catch (e) {
          console.error(`[processIssues] commit failed for ${diff.path}: ${(e as Error).message}`);
        }
      }
    }

    return { mcpBinary, worktreePath, cratePath };
  },
}, "processIssues");
