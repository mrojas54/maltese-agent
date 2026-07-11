/**
 * Shared implementation behind the three propose* handlers (WS-11 / AC-24).
 *
 * Every variant follows the same skeleton — pick the excerpt that anchors the
 * prompt, render a prompt template, ask Gemini for a UnifiedDiff — and the
 * variants differ only in the data `makeProposeFixHandler` is parameterized
 * on: prompt file, model, input schema, and the per-kind strategy (excerpt
 * selection + template vars) looked up from `STRATEGIES`.
 *
 * The instances are declared IN THIS MODULE, not in their pre-factory files:
 * Barnum's `createHandler` captures the file that called it (stack-trace
 * caller capture) as the engine worker's `module`, and the worker re-imports
 * that module and reads the export named by the handler id. Moving the
 * instance declarations elsewhere would leave the worker importing this file
 * and finding no such export. The pre-factory files (proposeBugFix.ts /
 * proposeLintFix.ts / proposePoisonFix.ts) remain as thin re-exports so every
 * existing import path — processIssues, tests, MA-13's orchestration work —
 * is untouched.
 */
import { createHandler, type Handler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { Gemini } from "../lib/gemini.js";
import { GEMINI_FLASH, GEMINI_PRO } from "../lib/models.js";
import { formatPreviousFailure, promptFromFile } from "../lib/prompts.js";
import {
  Excerpt,
  Issue,
  type IssueKind,
  PoisonReport,
  PreviousFailure,
  UnifiedDiff,
} from "../lib/types.js";

/** Input shared by the bug and lint variants: the classified issue, the file
 *  excerpts readContext gathered, and (on verify-failed retries) the previous
 *  attempt's diff+errors. Shape-identical to the pre-factory per-file Input
 *  consts (`Excerpt` is exactly the old inline `{ file, content }` object). */
export const IssueFixInput = z.object({
  issue: Issue,
  excerpts: z.array(Excerpt),
  previousFailure: PreviousFailure.optional(),
});
export type IssueFixInput = z.infer<typeof IssueFixInput>;

/** Exactly analyzePoison's Output ({ issue, report, excerpts }) plus the
 *  optional retry field — processIssues spreads `{ ...poisonAnalysis,
 *  previousFailure }` into this handler, so the shapes are reused from
 *  types.ts rather than re-declared or left untyped (AC-12). */
export const PoisonFixInput = z.object({
  issue: Issue,
  report: PoisonReport,
  excerpts: z.array(Excerpt),
  previousFailure: PreviousFailure.optional(),
});
export type PoisonFixInput = z.infer<typeof PoisonFixInput>;

type InputFor<K extends IssueKind> = K extends "poison"
  ? PoisonFixInput
  : IssueFixInput;

/** Where the variants genuinely diverge beyond prompt file and model: which
 *  excerpt anchors the prompt, and which template vars the prompt consumes.
 *  Everything else is the shared skeleton in {@link makeProposeFixHandler}. */
interface Strategy<I> {
  selectExcerpt(value: I): Excerpt;
  promptVars(excerpt: Excerpt, value: I): Record<string, string>;
}

/** Bug and lint share one strategy — they differ ONLY in prompt file and
 *  model (see the instance definitions at the bottom of this module). */
const issueStrategy: Strategy<IssueFixInput> = {
  selectExcerpt: (value) =>
    value.excerpts.find((e) => e.file === value.issue.location.file) ??
    value.excerpts[0],
  promptVars: (file, value) => ({
    file: file.file,
    line: String(value.issue.location.line ?? 0),
    evidence: value.issue.evidence,
    content: file.content,
    previousFailure: formatPreviousFailure(value.previousFailure),
  }),
};

const poisonStrategy: Strategy<PoisonFixInput> = {
  selectExcerpt: (value) =>
    value.excerpts.find((e) => e.file.endsWith("prompt.rs")) ??
    value.excerpts[0],
  promptVars: (promptFile, value) => ({
    file: promptFile.file,
    content: promptFile.content,
    report: JSON.stringify(value.report, null, 2),
    recommendation: value.report.recommendation,
    previousFailure: formatPreviousFailure(value.previousFailure),
  }),
};

const STRATEGIES: { [K in IssueKind]: Strategy<InputFor<K>> } = {
  bug: issueStrategy,
  lint: issueStrategy,
  poison: poisonStrategy,
};

/** Barnum handler ids — byte-identical to the pre-factory names. The engine
 *  worker resolves `{ module: proposeFix.ts, func: <id> }`, and MA-13's
 *  orchestration tests key on the same public surface, so these never
 *  change (WS-11: "unchanged handler names/ids"). */
const HANDLER_IDS: Record<IssueKind, string> = {
  bug: "proposeBugFix",
  lint: "proposeLintFix",
  poison: "proposePoisonFix",
};

/**
 * Factory for the propose* handlers (SPEC WS-11 signature): parameterizes
 * prompt file, model, and input schema per kind; the per-kind excerpt
 * selection and template vars come from {@link STRATEGIES}. Output is always
 * a schema-validated {@link UnifiedDiff} from a Gemini call.
 */
export function makeProposeFixHandler<K extends IssueKind>(spec: {
  kind: K;
  promptFile: string;
  model: string;
  schema: z.ZodType<InputFor<K>>;
}): Handler<InputFor<K>, UnifiedDiff> {
  const strategy: Strategy<InputFor<K>> = STRATEGIES[spec.kind];
  return createHandler<InputFor<K>, UnifiedDiff>(
    {
      inputValidator: spec.schema,
      outputValidator: UnifiedDiff,
      handle: async ({ value }) => {
        const excerpt = strategy.selectExcerpt(value);
        const gemini = new Gemini();
        const prompt = await promptFromFile(
          spec.promptFile,
          strategy.promptVars(excerpt, value),
        );
        return await gemini.call({
          prompt,
          schema: UnifiedDiff,
          model: spec.model,
        });
      },
    },
    HANDLER_IDS[spec.kind],
  );
}

export const proposeBugFix = makeProposeFixHandler({
  kind: "bug",
  promptFile: "propose-bug-fix.md",
  model: GEMINI_PRO,
  schema: IssueFixInput,
});

export const proposeLintFix = makeProposeFixHandler({
  kind: "lint",
  promptFile: "propose-lint-fix.md",
  model: GEMINI_FLASH,
  schema: IssueFixInput,
});

export const proposePoisonFix = makeProposeFixHandler({
  kind: "poison",
  promptFile: "propose-poison-fix.md",
  model: GEMINI_PRO,
  schema: PoisonFixInput,
});
