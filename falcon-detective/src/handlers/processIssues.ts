import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { Issue, PreviousFailure } from "../lib/types.js";
import { FalconMcpClient } from "../lib/mcp.js";
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
 *
 * Retry contract (per issue):
 *  1. Snapshot the target file's content before the first applyEdit.
 *  2. If verify reports `Broken`, REVERT the snapshot, then re-prompt the
 *     same propose-* handler with `previousFailure: { diff, errors }` so
 *     Gemini sees what didn't compile and proposes a different diff.
 *  3. Repeat up to MAX_RETRIES.
 *  4. On terminal failure (Stuck, exhausted retries, propose-* exception,
 *     applyEdit conflict), revert all snapshots so the next issue and
 *     finalSweep see a clean working tree.
 *
 * The previous design re-applied the IDENTICAL diff on Broken — guaranteeing
 * the same failure three times in a row, then leaving the broken edit in
 * place to poison finalSweep. This loop self-corrects and self-cleans.
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

/** Re-call the appropriate propose-* handler for `tagged.kind`, threading
 *  through the analyzePoison report (poison only) and the optional
 *  previousFailure record from the last broken attempt. */
async function proposeFor(
  tagged: any,
  poisonAnalysis: any,
  previousFailure: PreviousFailure | undefined,
): Promise<{ path: string; diff: string }> {
  if (tagged.kind === "Poison") {
    return await call<any, any>(proposePoisonFix, {
      ...poisonAnalysis,
      previousFailure,
    });
  } else if (tagged.kind === "Bug") {
    return await call<any, any>(proposeBugFix, {
      ...tagged.value,
      previousFailure,
    });
  } else {
    return await call<any, any>(proposeLintFix, {
      ...tagged.value,
      previousFailure,
    });
  }
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

      // 3. propose a fix per kind. Poison runs analyzePoison first (it's
      //    independent of any failed attempt — only proposePoisonFix takes
      //    previousFailure on retry).
      let poisonAnalysis: any = undefined;
      if (tagged.kind === "Poison") {
        try {
          poisonAnalysis = await call<any, any>(analyzePoison, tagged.value);
        } catch (e) {
          console.error(`[processIssues] analyzePoison failed for ${issue.location.file}: ${(e as Error).message}`);
          continue;
        }
      }

      let currentDiff: { path: string; diff: string };
      try {
        currentDiff = await proposeFor(tagged, poisonAnalysis, undefined);
      } catch (e) {
        console.error(`[processIssues] propose-fix failed for ${issue.location.file}: ${(e as Error).message}`);
        continue;
      }

      // 4. attempt → apply → verify → revert+re-propose on Broken loop.
      //    Snapshots: <path, originalContent>. We re-snapshot any new path
      //    a re-proposed diff touches, so reverting at the end restores
      //    every file the loop wrote to.
      const snapshots = new Map<string, string>();
      const snapMcp = new FalconMcpClient({ binary: mcpBinary, root: worktreePath });
      await snapMcp.connect();

      let attempt = 0;
      let succeeded = false;
      try {
        while (attempt < MAX_RETRIES) {
          // Snapshot the file we're about to touch (idempotent — only reads
          // the first time we see this path).
          if (!snapshots.has(currentDiff.path)) {
            try {
              const r = await snapMcp.call<{ content: string }>("fs_read", { path: currentDiff.path });
              snapshots.set(currentDiff.path, r.content);
            } catch (e) {
              // Path doesn't exist yet — record empty so revert deletes/no-ops.
              // Most diffs target existing files; this is a defensive carve-out.
              snapshots.set(currentDiff.path, "");
              console.warn(`[processIssues] snapshot fs_read for ${currentDiff.path} failed: ${(e as Error).message}`);
            }
          }

          const apply = await call<any, any>(applyEdit, { mcpBinary, worktreePath, diff: currentDiff });
          if (!apply.ok) {
            console.error(`[processIssues] applyEdit failed for ${currentDiff.path}: ${apply.reason}`);
            break;
          }
          const result = await call<any, any>(verify, { mcpBinary, worktreePath, cratePath, attempt });
          if (result.kind === "Clean") { succeeded = true; break; }
          if (result.kind === "Stuck") { break; }

          // Broken: capture the failure, revert all snapshots, re-prompt with
          // previousFailure so Gemini gets cargo's actual error text and can
          // propose something different.
          const previousFailure: PreviousFailure = {
            diff: currentDiff.diff,
            errors: result.value.errors,
          };
          for (const [p, c] of snapshots) {
            await snapMcp.call("fs_write", { path: p, content: c });
          }
          attempt++;
          if (attempt >= MAX_RETRIES) break;
          try {
            currentDiff = await proposeFor(tagged, poisonAnalysis, previousFailure);
          } catch (e) {
            console.error(`[processIssues] propose-fix retry failed for ${issue.location.file}: ${(e as Error).message}`);
            break;
          }
        }
      } finally {
        // On failure, restore everything the loop touched so the next issue
        // (and finalSweep) sees a clean tree. On success, leave the applied
        // diff in place for commitOne to stage and commit.
        if (!succeeded) {
          for (const [p, c] of snapshots) {
            try {
              await snapMcp.call("fs_write", { path: p, content: c });
            } catch (e) {
              console.error(`[processIssues] terminal revert failed for ${p}: ${(e as Error).message}`);
            }
          }
        }
        await snapMcp.close();
      }

      // 5. commit on success.
      if (succeeded) {
        try {
          await call<any, any>(commitOne, { mcpBinary, worktreePath, issue, diffPath: currentDiff.path });
        } catch (e) {
          console.error(`[processIssues] commit failed for ${currentDiff.path}: ${(e as Error).message}`);
        }
      }
    }

    return { mcpBinary, worktreePath, cratePath };
  },
}, "processIssues");
