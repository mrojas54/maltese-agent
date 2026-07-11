import type { ExtractOutput } from "@barnum/barnum/pipeline";
import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { type InvokeHandler, invokeHandler } from "../lib/invokeHandler.js";
import { FalconMcpClient } from "../lib/mcp.js";
import { FsReadResult, FsWriteResult } from "../lib/toolSchemas.js";
import {
  Issue,
  type PreviousFailure,
  type TaggedIssue,
  type UnifiedDiff,
  type VerifyResult,
} from "../lib/types.js";
import { analyzePoison } from "./analyzePoison.js";
import { applyEdit } from "./applyEdit.js";
import { classify } from "./classify.js";
import { commitOne } from "./commit.js";
import { proposeBugFix } from "./proposeBugFix.js";
import { proposeLintFix } from "./proposeLintFix.js";
import { proposePoisonFix } from "./proposePoisonFix.js";
import { readContext } from "./readContext.js";
import { verify } from "./verify.js";

/**
 * Coarse Barnum handler for the per-issue loop. Inside, the per-issue logic
 * runs in plain TypeScript as four named stages (WS-14a / AC-30) —
 * classifyIssue → proposeForIssue → applyAndVerify → commitOrRevert — each
 * invoking the smaller handlers through the public engine API (`runPipeline`
 * via the `invokeHandler` seam — WS-5/AC-13). Each invocation is
 * engine-mediated (subprocess per call) and validated against the handler's
 * input/output schemas.
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

/** Pipeline context threaded through every per-issue stage. Invoke call
 *  sites forward only the fields each handler's strict input schema declares
 *  (additionalProperties: false at the engine boundary — no blind spreads). */
interface StageContext {
  mcpBinary: string;
  worktreePath: string;
  cratePath: string;
}

// Stage inputs/outputs are typed against the handlers themselves (AC-30/AC-34
// direction: no `any` plumbing). ExtractOutput reads the phantom output type
// carried by every createHandler result.
type IssueContext = ExtractOutput<typeof readContext>;
type PoisonAnalysis = ExtractOutput<typeof analyzePoison>;
type ApplyResult = ExtractOutput<typeof applyEdit>;

/** The per-issue verdict applyAndVerify hands to commitOrRevert: whether the
 *  loop ended on a Clean verify, and the last diff it applied. */
interface AttemptOutcome {
  succeeded: boolean;
  finalDiff: UnifiedDiff;
}

/** Everything the apply/verify/commit stages of ONE issue share: the invoke
 *  seam, the pipeline context, the snapshot client, and the snapshot store
 *  (<path, originalContent>). Created only after a proposal exists — issues
 *  skipped at the analyze/propose step never open a client. */
interface IssueRun {
  invoke: InvokeHandler;
  ctx: StageContext;
  snapMcp: FalconMcpClient;
  snapshots: Map<string, string>;
}

/** What one attempt of the retry loop produced: a Clean verify, a terminal
 *  give-up (Stuck or applyEdit conflict), or — on Broken — the failure
 *  record to re-prompt with. */
type AttemptVerdict = PreviousFailure | "Clean" | "GiveUp";

/** Stage 1 (classifyIssue): pull the issue's file context, then classify it
 *  into the `{ kind: Poison|Bug|Lint, value }` envelope the propose stage
 *  dispatches on (readContext → classify). */
async function classifyIssue(
  invoke: InvokeHandler,
  ctx: StageContext,
  issue: Issue,
): Promise<TaggedIssue> {
  const fileContext = await invoke<IssueContext>(readContext, {
    mcpBinary: ctx.mcpBinary,
    worktreePath: ctx.worktreePath,
    issue,
  });
  return await invoke<TaggedIssue>(classify, {
    issue,
    excerpts: fileContext.excerpts,
  });
}

/** Stage 2 (proposeForIssue): call the appropriate propose-* handler for
 *  `tagged.kind`, threading through the analyzePoison report (poison only)
 *  and the optional previousFailure record from the last broken attempt. */
async function proposeForIssue(
  invoke: InvokeHandler,
  tagged: TaggedIssue,
  poisonAnalysis: PoisonAnalysis | undefined,
  previousFailure: PreviousFailure | undefined,
): Promise<UnifiedDiff> {
  if (tagged.kind === "Poison") {
    return await invoke<UnifiedDiff>(proposePoisonFix, {
      ...poisonAnalysis,
      previousFailure,
    });
  }
  if (tagged.kind === "Bug") {
    return await invoke<UnifiedDiff>(proposeBugFix, {
      ...tagged.value,
      previousFailure,
    });
  }
  return await invoke<UnifiedDiff>(proposeLintFix, {
    ...tagged.value,
    previousFailure,
  });
}

/** Stage 2 entry: poison issues get ONE analyzePoison pass (independent of
 *  any failed attempt — only the propose step sees previousFailure on retry),
 *  then the kind-appropriate propose-* handler drafts the first diff.
 *  Returns undefined — skipping the issue — when either step throws. */
async function prepareProposal(
  invoke: InvokeHandler,
  tagged: TaggedIssue,
  issueFile: string,
): Promise<
  | { poisonAnalysis: PoisonAnalysis | undefined; firstDiff: UnifiedDiff }
  | undefined
> {
  let poisonAnalysis: PoisonAnalysis | undefined;
  if (tagged.kind === "Poison") {
    try {
      poisonAnalysis = await invoke<PoisonAnalysis>(
        analyzePoison,
        tagged.value,
      );
    } catch (e) {
      console.error(
        `[processIssues] analyzePoison failed for ${issueFile}: ${(e as Error).message}`,
      );
      return undefined;
    }
  }
  try {
    const firstDiff = await proposeForIssue(
      invoke,
      tagged,
      poisonAnalysis,
      undefined,
    );
    return { poisonAnalysis, firstDiff };
  } catch (e) {
    console.error(
      `[processIssues] propose-fix failed for ${issueFile}: ${(e as Error).message}`,
    );
    return undefined;
  }
}

/** Record `path`'s pre-edit content in the run's snapshot store the first
 *  time the retry loop touches it (idempotent), so reverting restores every
 *  file the loop wrote to — including any new path a re-proposed diff
 *  targets. */
async function snapshotOnce(run: IssueRun, path: string): Promise<void> {
  if (run.snapshots.has(path)) return;
  try {
    const r = await run.snapMcp.call("fs_read", { path }, FsReadResult);
    run.snapshots.set(path, r.content);
  } catch (e) {
    // Path doesn't exist yet — record empty so revert deletes/no-ops.
    // Most diffs target existing files; this is a defensive carve-out.
    run.snapshots.set(path, "");
    console.warn(
      `[processIssues] snapshot fs_read for ${path} failed: ${(e as Error).message}`,
    );
  }
}

/** Restore every snapshotted file. Used for the in-loop revert after a
 *  Broken verify; errors propagate (matching the pre-decomposition loop).
 *  The terminal revert in commitOrRevert wraps each write in its own
 *  try/catch instead. */
async function revertSnapshots(run: IssueRun): Promise<void> {
  for (const [p, c] of run.snapshots) {
    await run.snapMcp.call("fs_write", { path: p, content: c }, FsWriteResult);
  }
}

/** One attempt of the retry loop: snapshot the target file, applyEdit, then
 *  verify. Broken hands back `{ diff, errors }` for the re-prompt; an
 *  applyEdit conflict or a Stuck verify is a terminal give-up. */
async function attemptOnce(
  run: IssueRun,
  diff: UnifiedDiff,
  attempt: number,
): Promise<AttemptVerdict> {
  const { mcpBinary, worktreePath, cratePath } = run.ctx;
  await snapshotOnce(run, diff.path);
  const apply = await run.invoke<ApplyResult>(applyEdit, {
    mcpBinary,
    worktreePath,
    diff,
  });
  if (!apply.ok) {
    console.error(
      `[processIssues] applyEdit failed for ${diff.path}: ${apply.reason}`,
    );
    return "GiveUp";
  }
  const result = await run.invoke<VerifyResult>(verify, {
    mcpBinary,
    worktreePath,
    cratePath,
    attempt,
  });
  if (result.kind === "Clean") return "Clean";
  if (result.kind === "Stuck") return "GiveUp";
  return { diff: diff.diff, errors: result.value.errors };
}

/** Stage 3 (applyAndVerify): the attempt → apply → verify → revert +
 *  re-propose retry loop. On Broken, reverts all snapshots and re-prompts
 *  with the previous failure so Gemini sees cargo's actual error text; gives
 *  up on Stuck, an applyEdit conflict, a propose-* exception, or after
 *  MAX_RETRIES attempts. */
async function applyAndVerify(
  run: IssueRun,
  tagged: TaggedIssue,
  poisonAnalysis: PoisonAnalysis | undefined,
  firstDiff: UnifiedDiff,
): Promise<AttemptOutcome> {
  let currentDiff = firstDiff;
  let attempt = 0;
  while (attempt < MAX_RETRIES) {
    const verdict = await attemptOnce(run, currentDiff, attempt);
    if (verdict === "Clean") return { succeeded: true, finalDiff: currentDiff };
    if (verdict === "GiveUp") break;
    // Broken: restore every touched file, then re-prompt carrying the failed
    // diff and cargo's errors so the next proposal can be different.
    await revertSnapshots(run);
    attempt++;
    if (attempt >= MAX_RETRIES) break;
    try {
      currentDiff = await proposeForIssue(
        run.invoke,
        tagged,
        poisonAnalysis,
        verdict,
      );
    } catch (e) {
      console.error(
        `[processIssues] propose-fix retry failed for ${tagged.value.issue.location.file}: ${(e as Error).message}`,
      );
      break;
    }
  }
  return { succeeded: false, finalDiff: currentDiff };
}

/** Stage 4 (commitOrRevert): terminal cleanup for one issue. On failure,
 *  restore everything the retry loop touched so the next issue (and
 *  finalSweep) sees a clean tree; on success, leave the applied diff in
 *  place and commit it. The snapshot client closes either way — before
 *  commitOne, which spawns its own client. Runs in the main loop's
 *  `finally`, so a throwing retry loop still reverts and closes. */
async function commitOrRevert(
  run: IssueRun,
  issue: Issue,
  outcome: AttemptOutcome,
): Promise<void> {
  if (!outcome.succeeded) {
    for (const [p, c] of run.snapshots) {
      try {
        await run.snapMcp.call(
          "fs_write",
          { path: p, content: c },
          FsWriteResult,
        );
      } catch (e) {
        console.error(
          `[processIssues] terminal revert failed for ${p}: ${(e as Error).message}`,
        );
      }
    }
  }
  await run.snapMcp.close();
  if (outcome.succeeded) {
    try {
      await run.invoke<ExtractOutput<typeof commitOne>>(commitOne, {
        mcpBinary: run.ctx.mcpBinary,
        worktreePath: run.ctx.worktreePath,
        issue,
        diffPath: outcome.finalDiff.path,
      });
    } catch (e) {
      console.error(
        `[processIssues] commit failed for ${outcome.finalDiff.path}: ${(e as Error).message}`,
      );
    }
  }
}

/**
 * Factory for processIssues' handle body with the handler-invocation seam
 * as an injectable parameter (production default: the public runPipeline
 * path via `invokeHandler`). Orchestration tests (MA-13) inject an
 * in-process fake here — `vi.mock` cannot cross the engine's process
 * boundary, so this parameter is the supported substitution point.
 */
export function makeProcessIssuesHandle(
  invoke: InvokeHandler = invokeHandler,
): (context: {
  value: z.infer<typeof Input>;
}) => Promise<z.infer<typeof Output>> {
  return async ({ value }) => {
    const { mcpBinary, worktreePath, cratePath, issues } = value;
    const ctx: StageContext = { mcpBinary, worktreePath, cratePath };

    for (const issue of issues) {
      const tagged = await classifyIssue(invoke, ctx, issue);

      const prepared = await prepareProposal(
        invoke,
        tagged,
        issue.location.file,
      );
      if (!prepared) continue;

      const snapMcp = new FalconMcpClient({
        binary: mcpBinary,
        root: worktreePath,
      });
      await snapMcp.connect();
      const run: IssueRun = { invoke, ctx, snapMcp, snapshots: new Map() };

      // The default outcome stands if applyAndVerify throws: the finally
      // still reverts every snapshot and closes the client (and the
      // exception then propagates, exactly like the pre-decomposition loop).
      let outcome: AttemptOutcome = {
        succeeded: false,
        finalDiff: prepared.firstDiff,
      };
      try {
        outcome = await applyAndVerify(
          run,
          tagged,
          prepared.poisonAnalysis,
          prepared.firstDiff,
        );
      } finally {
        await commitOrRevert(run, issue, outcome);
      }
    }

    return { mcpBinary, worktreePath, cratePath };
  };
}

export const processIssues = createHandler(
  {
    inputValidator: Input,
    outputValidator: Output,
    handle: makeProcessIssuesHandle(),
  },
  "processIssues",
);
