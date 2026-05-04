import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { Issue } from "../lib/types.js";
import { Gemini } from "../lib/gemini.js";
import { FalconMcpClient } from "../lib/mcp.js";
import { promptFromFile } from "../lib/prompts.js";

const Input = z.object({
  mcpBinary: z.string(),
  worktreePath: z.string(),
  cratePath: z.string(),
});
// Fat output threads context forward so processIssues sees everything it
// needs: the issues plus the mcpBinary/worktreePath/cratePath context.
const Output = z.object({
  mcpBinary: z.string(),
  worktreePath: z.string(),
  cratePath: z.string(),
  issues: z.array(Issue),
});

export const triage = createHandler({
  inputValidator: Input,
  outputValidator: Output,
  handle: async ({ value }) => {
    const mcp = new FalconMcpClient({ binary: value.mcpBinary, root: value.worktreePath });
    await mcp.connect();
    try {
      const [check, test, clippy, files] = await Promise.all([
        mcp.call("cargo_check", { crate_path: value.cratePath }),
        mcp.call("cargo_test", { crate_path: value.cratePath }),
        mcp.call("cargo_clippy", { crate_path: value.cratePath }),
        mcp.call("fs_list", { path: value.cratePath, glob: "**/*.rs" }),
      ]);
      const gemini = new Gemini();
      const prompt = await promptFromFile("triage.md", {
        cargoOutputs: JSON.stringify({ check, test, clippy }, null, 2),
        fileListing: JSON.stringify(files, null, 2),
      });
      const { issues } = await gemini.call({
        prompt,
        schema: z.object({ issues: z.array(Issue) }),
      });
      return {
        mcpBinary: value.mcpBinary,
        worktreePath: value.worktreePath,
        cratePath: value.cratePath,
        issues,
      };
    } finally { await mcp.close(); }
  },
}, "triage");
