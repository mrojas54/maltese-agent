import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { Gemini } from "../lib/gemini.js";
import { GEMINI_PRO } from "../lib/models.js";
import { formatPreviousFailure, promptFromFile } from "../lib/prompts.js";
import {
  Excerpt,
  Issue,
  PoisonReport,
  PreviousFailure,
  UnifiedDiff,
} from "../lib/types.js";

// Exactly analyzePoison's Output ({ issue, report, excerpts }) plus the
// optional retry field — processIssues spreads `{ ...poisonAnalysis,
// previousFailure }` into this handler, so the shapes are reused from
// types.ts rather than re-declared or left untyped (AC-12).
const Input = z.object({
  issue: Issue,
  report: PoisonReport,
  excerpts: z.array(Excerpt),
  previousFailure: PreviousFailure.optional(),
});

export const proposePoisonFix = createHandler(
  {
    inputValidator: Input,
    outputValidator: UnifiedDiff,
    handle: async ({ value }) => {
      const promptFile =
        value.excerpts.find((e) => e.file.endsWith("prompt.rs")) ??
        value.excerpts[0];
      const gemini = new Gemini();
      const prompt = await promptFromFile("propose-poison-fix.md", {
        file: promptFile.file,
        content: promptFile.content,
        report: JSON.stringify(value.report, null, 2),
        recommendation: value.report.recommendation,
        previousFailure: formatPreviousFailure(value.previousFailure),
      });
      return await gemini.call({
        prompt,
        schema: UnifiedDiff,
        model: GEMINI_PRO,
      });
    },
  },
  "proposePoisonFix",
);
