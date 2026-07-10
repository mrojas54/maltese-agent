import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { Gemini } from "../lib/gemini.js";
import { GEMINI_PRO } from "../lib/models.js";
import { formatPreviousFailure, promptFromFile } from "../lib/prompts.js";
import { Issue, PreviousFailure, UnifiedDiff } from "../lib/types.js";

const Input = z.object({
  issue: Issue,
  excerpts: z.array(z.object({ file: z.string(), content: z.string() })),
  previousFailure: PreviousFailure.optional(),
});

export const proposeBugFix = createHandler(
  {
    inputValidator: Input,
    outputValidator: UnifiedDiff,
    handle: async ({ value }) => {
      const file =
        value.excerpts.find((e) => e.file === value.issue.location.file) ??
        value.excerpts[0];
      const gemini = new Gemini();
      const prompt = await promptFromFile("propose-bug-fix.md", {
        file: file.file,
        line: String(value.issue.location.line ?? 0),
        evidence: value.issue.evidence,
        content: file.content,
        previousFailure: formatPreviousFailure(value.previousFailure),
      });
      return await gemini.call({
        prompt,
        schema: UnifiedDiff,
        model: GEMINI_PRO,
      });
    },
  },
  "proposeBugFix",
);
