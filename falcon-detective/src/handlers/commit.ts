import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { FalconMcpClient } from "../lib/mcp.js";
import {
  ExecRunResult,
  GitCommitResult,
  OkResult,
} from "../lib/toolSchemas.js";
import { Issue } from "../lib/types.js";

const McpAndPath = z.object({
  mcpBinary: z.string(),
  worktreePath: z.string(),
});

const CommitOneInput = McpAndPath.extend({
  issue: Issue,
  diffPath: z.string(),
});
const RevertOneInput = McpAndPath.extend({ issue: Issue });
const EscalateInput = McpAndPath.extend({ reasons: z.array(z.string()) });

// The handle bodies below are exported as public module seams so the
// orchestration tests (MA-13 / AC-19) can drive them in-process with a
// mocked FalconMcpClient — `vi.mock` cannot cross the Barnum engine's
// process boundary, and Barnum's underscore-private surface is barred (AC-13).
// Each Barnum handler wires its exported function unchanged.

export async function commitOneHandle({
  value,
}: {
  value: z.infer<typeof CommitOneInput>;
}): Promise<{ sha: string }> {
  const mcp = new FalconMcpClient({
    binary: value.mcpBinary,
    root: value.worktreePath,
  });
  await mcp.connect();
  try {
    await mcp.call("git_add", { paths: [value.diffPath] }, OkResult);
    const r = await mcp.call(
      "git_commit",
      {
        message: `fix(${value.issue.kind}): ${value.issue.location.file}${value.issue.location.line ? `:${value.issue.location.line}` : ""}`,
      },
      GitCommitResult,
    );
    return r;
  } finally {
    await mcp.close();
  }
}

export const commitOne = createHandler(
  {
    inputValidator: CommitOneInput,
    outputValidator: z.object({ sha: z.string() }),
    handle: commitOneHandle,
  },
  "commitOne",
);

export async function commitAllHandle(): Promise<{ ok: true }> {
  // per-issue commits already happened; this is a marker
  return { ok: true as const };
}

export const commitAll = createHandler(
  {
    inputValidator: McpAndPath,
    outputValidator: z.object({ ok: z.literal(true) }),
    handle: commitAllHandle,
  },
  "commitAll",
);

export async function revertOneHandle({
  value,
}: {
  value: z.infer<typeof RevertOneInput>;
}): Promise<{ ok: true }> {
  // exec is required here (`git checkout` isn't a built-in MCP tool); the
  // allowlist in falcon-mcp's sandbox already includes "git", so this is safe.
  const mcp = new FalconMcpClient({
    binary: value.mcpBinary,
    root: value.worktreePath,
    enableExec: true,
  });
  await mcp.connect();
  try {
    // Reset working tree changes for the file under question; keep prior commits.
    await mcp
      .call(
        "exec_run",
        {
          cmd: "git",
          args: ["checkout", "--", value.issue.location.file],
        },
        ExecRunResult,
      )
      .catch(() => {
        /* no-op if file not modified */
      });
    return { ok: true as const };
  } finally {
    await mcp.close();
  }
}

export const revertOne = createHandler(
  {
    inputValidator: RevertOneInput,
    outputValidator: z.object({ ok: z.literal(true) }),
    handle: revertOneHandle,
  },
  "revertOne",
);

export async function escalateHandle({
  value,
}: {
  value: z.infer<typeof EscalateInput>;
}): Promise<{ escalated: true }> {
  console.error(`[escalate] manual review needed: ${value.reasons.join("; ")}`);
  return { escalated: true as const };
}

export const escalate = createHandler(
  {
    // Accepts the shape `branch()` hands it from finalSweep's Dirty arm:
    // { mcpBinary, worktreePath, reasons: string[] }.
    inputValidator: EscalateInput,
    outputValidator: z.object({ escalated: z.literal(true) }),
    handle: escalateHandle,
  },
  "escalate",
);
