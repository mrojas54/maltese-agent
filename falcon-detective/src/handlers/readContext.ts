import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { FalconMcpClient } from "../lib/mcp.js";
import { FsReadResult } from "../lib/toolSchemas.js";
import { Issue } from "../lib/types.js";

const Input = z.object({
  mcpBinary: z.string(),
  worktreePath: z.string(),
  issue: Issue,
});
const Output = z.object({
  issue: Issue,
  excerpts: z.array(z.object({ file: z.string(), content: z.string() })),
});

export const readContext = createHandler(
  {
    inputValidator: Input,
    outputValidator: Output,
    handle: async ({ value }) => {
      const mcp = new FalconMcpClient({
        binary: value.mcpBinary,
        root: value.worktreePath,
      });
      await mcp.connect();
      try {
        const target = await mcp.call(
          "fs_read",
          { path: value.issue.location.file },
          FsReadResult,
        );
        const excerpts = [
          { file: value.issue.location.file, content: target.content },
        ];
        // For poison issues, also pull prompt.rs if not the target file
        if (
          value.issue.kind === "poison" &&
          !value.issue.location.file.endsWith("prompt.rs")
        ) {
          try {
            const promptFile = await mcp.call(
              "fs_read",
              { path: "src/prompt.rs" },
              FsReadResult,
            );
            excerpts.push({
              file: "src/prompt.rs",
              content: promptFile.content,
            });
          } catch {
            /* not present */
          }
        }
        return { issue: value.issue, excerpts };
      } finally {
        await mcp.close();
      }
    },
  },
  "readContext",
);
