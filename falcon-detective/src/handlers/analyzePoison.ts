import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { Issue, PoisonReport } from "../lib/types.js";
import { Gemini } from "../lib/gemini.js";
import { GEMINI_PRO } from "../lib/models.js";
import { promptFromFile } from "../lib/prompts.js";

const Input = z.object({
  issue: Issue,
  excerpts: z.array(z.object({ file: z.string(), content: z.string() })),
});
const Output = z.object({ issue: Issue, report: PoisonReport, excerpts: Input.shape.excerpts });

export const analyzePoison = createHandler({
  inputValidator: Input,
  outputValidator: Output,
  handle: async ({ value }) => {
    const promptFile = value.excerpts.find(e => e.file.endsWith("prompt.rs")) ?? value.excerpts[0];
    const gemini = new Gemini();
    const prompt = await promptFromFile("analyze-poison.md", {
      file: promptFile.file,
      content: promptFile.content,
      evidence: value.issue.evidence,
    });
    const report = await gemini.call({ prompt, schema: PoisonReport, model: GEMINI_PRO });
    return { issue: value.issue, report, excerpts: value.excerpts };
  },
}, "analyzePoison");
