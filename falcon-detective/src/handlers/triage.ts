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
      // Serialize cargo invocations: they share the worktree's target/ dir
      // and the .fingerprint/ machinery is not safe under concurrent access
      // (one process's cleanup can race another's write, producing
      // "No such file or directory" partway through dep compile). fs_list
      // can run concurrently with the first cargo invocation since it
      // doesn't touch target/.
      const [check, files] = await Promise.all([
        mcp.call("cargo_check", { crate_path: value.cratePath }),
        mcp.call("fs_list", { path: value.cratePath, glob: "**/*.rs" }),
      ]);
      const test = await mcp.call("cargo_test", { crate_path: value.cratePath });
      const clippy = await mcp.call("cargo_clippy", { crate_path: value.cratePath });
      const gemini = new Gemini();
      const prompt = await promptFromFile("triage.md", {
        cargoOutputs: JSON.stringify({ check, test, clippy }, null, 2),
        fileListing: JSON.stringify(files, null, 2),
      });
      // Gemini consistently emits a bare Issue[] for triage no matter what
      // the prompt/schema says (likely an in-domain prior from training).
      // Rather than fight it with prompt iteration, accept either shape via
      // z.preprocess and normalize to `{ issues }` before zod validates the
      // body. The retry/backoff loop in gemini.ts still catches transients.
      const TriageOutput = z.preprocess(
        (raw) => Array.isArray(raw) ? { issues: raw } : raw,
        z.object({ issues: z.array(Issue) }),
      );
      const result = await gemini.call({ prompt, schema: TriageOutput as any }) as { issues: any[] };
      const { issues } = result;
      return {
        mcpBinary: value.mcpBinary,
        worktreePath: value.worktreePath,
        cratePath: value.cratePath,
        issues,
      };
    } finally { await mcp.close(); }
  },
}, "triage");
