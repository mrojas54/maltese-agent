import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { Gemini } from "../lib/gemini.js";
import { FalconMcpClient } from "../lib/mcp.js";
import { promptFromFile } from "../lib/prompts.js";
import {
  CargoCheckResult,
  CargoClippyResult,
  CargoTestResult,
  FsListResult,
  FsReadResult,
  FsSearchResult,
} from "../lib/toolSchemas.js";
import { Issue } from "../lib/types.js";

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

// Trigger words that mark a test as a poison-canary. cargo_test skips #[ignore]'d
// tests by default, so a test like `bird_themed_inputs_arent_special` whose name
// signals "this is the proof that the planted poison fires" never appears in the
// cargo_test "failed" list. Surface them explicitly so triage can emit a poison
// Issue pointing at the prompt file rather than missing the signal entirely.
const TRIGGER_WORDS = ["bird", "themed", "poison", "trigger", "anomaly"];

interface IgnoredTriggerTest {
  file: string;
  line: number;
  testName: string;
  snippet: string;
}

/**
 * Walk the file's lines: for each `#[ignore]` attribute, find the next `fn NAME`
 * within a small window (handles intervening attributes like `#[tokio::test]`).
 * If NAME contains a trigger word, capture a small snippet for triage's prompt.
 */
function extractIgnoredTriggerTests(
  file: string,
  content: string,
): IgnoredTriggerTest[] {
  const lines = content.split("\n");
  const out: IgnoredTriggerTest[] = [];
  for (let i = 0; i < lines.length; i++) {
    if (!/#\[ignore\b/.test(lines[i])) continue;
    for (let j = i + 1; j < Math.min(i + 6, lines.length); j++) {
      const fnMatch = lines[j].match(/\bfn\s+([A-Za-z_]\w*)/);
      if (!fnMatch) continue;
      const testName = fnMatch[1];
      if (TRIGGER_WORDS.some((t) => testName.toLowerCase().includes(t))) {
        const snippet = lines
          .slice(i, Math.min(j + 3, lines.length))
          .join("\n");
        out.push({ file, line: i + 1, testName, snippet });
      }
      break;
    }
  }
  return out;
}

export const triage = createHandler(
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
        // Serialize cargo invocations: they share the worktree's target/ dir
        // and the .fingerprint/ machinery is not safe under concurrent access
        // (one process's cleanup can race another's write, producing
        // "No such file or directory" partway through dep compile). fs_list
        // can run concurrently with the first cargo invocation since it
        // doesn't touch target/.
        const [check, files] = await Promise.all([
          mcp.call(
            "cargo_check",
            { crate_path: value.cratePath },
            CargoCheckResult,
          ),
          mcp.call(
            "fs_list",
            { path: value.cratePath, glob: "**/*.rs" },
            FsListResult,
          ),
        ]);
        const test = await mcp.call(
          "cargo_test",
          { crate_path: value.cratePath },
          CargoTestResult,
        );
        const clippy = await mcp.call(
          "cargo_clippy",
          { crate_path: value.cratePath },
          CargoClippyResult,
        );

        // Poison signal #1: read prompt.rs (or any *_prompt.rs / prompt*.rs) so
        // triage's LLM step can spot anomalous few-shot examples. cargo_check
        // and cargo_clippy don't surface "this string literal looks suspicious"
        // — only a human (or LLM with the file in front of it) catches it.
        const promptFile = `${value.cratePath}/src/prompt.rs`;
        let promptFileContent: string | null = null;
        try {
          const r = await mcp.call(
            "fs_read",
            { path: promptFile },
            FsReadResult,
          );
          promptFileContent = r.content;
        } catch {
          /* file absent — skip the signal */
        }

        // Poison signal #2: #[ignore]'d tests with trigger-named test functions.
        // cargo_test ran above with default flags (no --include-ignored), so a
        // test like `bird_themed_inputs_arent_special` is silently skipped and
        // never shows up as "failed". fs_search locates the #[ignore] attr; we
        // then read each candidate file and pull out only the ignored tests
        // whose function names match a trigger word.
        let ignoreMatches: FsSearchResult = { matches: [], truncated: false };
        try {
          ignoreMatches = await mcp.call(
            "fs_search",
            {
              pattern: "#\\[ignore",
              glob: "*.rs",
              max: 200,
            },
            FsSearchResult,
          );
        } catch (err) {
          // fs_search requires `rg` on PATH. Degrade gracefully but NEVER
          // silently: an empty ignored-test signal changes the triage prompt,
          // which changes every cassette key — record/replay done on machines
          // that disagree about `rg` can never match (root-caused on PR #64).
          console.error(
            "[triage] fs_search failed — ignored-test poison signal degraded to none " +
              "(is ripgrep installed?). Cassette keys will NOT match recordings made " +
              `where the signal was present: ${err instanceof Error ? err.message : String(err)}`,
          );
        }

        const ignoredTriggerTests: IgnoredTriggerTest[] = [];
        const seenFiles = new Set<string>();
        for (const m of ignoreMatches.matches) {
          if (seenFiles.has(m.file)) continue;
          seenFiles.add(m.file);
          try {
            const r = await mcp.call("fs_read", { path: m.file }, FsReadResult);
            ignoredTriggerTests.push(
              ...extractIgnoredTriggerTests(m.file, r.content),
            );
          } catch (err) {
            // File gone or unreadable — skip it, but say so: a lost candidate
            // silently narrows the poison signal (and the cassette key).
            console.error(
              `[triage] fs_read failed for ignored-test candidate ${m.file}: ` +
                `${err instanceof Error ? err.message : String(err)}`,
            );
          }
        }

        const gemini = new Gemini();
        const prompt = await promptFromFile("triage.md", {
          cargoOutputs: JSON.stringify({ check, test, clippy }, null, 2),
          fileListing: JSON.stringify(files, null, 2),
          promptFile,
          promptFileContent: promptFileContent ?? "(file not present)",
          ignoredTriggerTests: JSON.stringify(ignoredTriggerTests, null, 2),
        });
        // Gemini consistently emits a bare Issue[] for triage no matter what
        // the prompt/schema says (likely an in-domain prior from training).
        // Rather than fight it with prompt iteration, accept either shape via
        // z.preprocess and normalize to `{ issues }` before zod validates the
        // body. The retry/backoff loop in gemini.ts still catches transients.
        const TriageOutput = z.preprocess(
          (raw) => (Array.isArray(raw) ? { issues: raw } : raw),
          z.object({ issues: z.array(Issue) }),
        );
        const { issues } = await gemini.call({
          prompt,
          schema: TriageOutput,
        });
        return {
          mcpBinary: value.mcpBinary,
          worktreePath: value.worktreePath,
          cratePath: value.cratePath,
          issues,
        };
      } finally {
        await mcp.close();
      }
    },
  },
  "triage",
);

// Exported for unit-testing the trigger-extraction helper without standing up
// a real MCP server.
export const __test__ = { extractIgnoredTriggerTests, TRIGGER_WORDS };
