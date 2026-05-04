import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { PreviousFailure, UnifiedDiff } from "../lib/types.js";
import { Gemini } from "../lib/gemini.js";
import { formatPreviousFailure, promptFromFile } from "../lib/prompts.js";

const Input = z.object({
  issue: z.any(),
  report: z.any(),
  excerpts: z.array(z.object({ file: z.string(), content: z.string() })),
  previousFailure: PreviousFailure.optional(),
});

export const proposePoisonFix = createHandler({
  inputValidator: Input,
  outputValidator: UnifiedDiff,
  handle: async ({ value }) => {
    const promptFile = value.excerpts.find((e: any) => e.file.endsWith("prompt.rs")) ?? value.excerpts[0];
    const gemini = new Gemini();
    const prompt = await promptFromFile("propose-poison-fix.md", {
      file: promptFile.file,
      content: promptFile.content,
      report: JSON.stringify(value.report, null, 2),
      recommendation: value.report.recommendation,
      previousFailure: formatPreviousFailure(value.previousFailure),
    });
    return await gemini.call({ prompt, schema: UnifiedDiff, model: "gemini-2.5-pro" });
  },
}, "proposePoisonFix");
