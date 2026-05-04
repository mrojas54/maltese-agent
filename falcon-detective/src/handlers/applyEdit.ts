import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { UnifiedDiff } from "../lib/types.js";
import { FalconMcpClient } from "../lib/mcp.js";

const Input = z.object({
  mcpBinary: z.string(),
  worktreePath: z.string(),
  diff: UnifiedDiff,
});
const Output = z.object({ ok: z.boolean(), reason: z.string().optional() });

/**
 * Rewrite each `@@ -A,B +C,D @@` hunk header's B/D counts to match the actual
 * body line counts. Gemini occasionally emits headers where the count is off
 * by one (e.g. claims 9 source lines when only 8 are shown), and falcon-mcp's
 * `fs_apply_patch` rejects hunks whose declared range extends past the file's
 * length. Recounting from the body is unambiguous: B = (context + remove)
 * lines, D = (context + add) lines, both before the next hunk header or EOF.
 *
 * Lines starting with `\ ` (e.g. `\ No newline at end of file`) are markers,
 * not file content, and don't contribute to either count.
 *
 * Exported for unit testing.
 */
export function recountHunks(diff: string): string {
  const lines = diff.split("\n");
  const out: string[] = [];

  // Find each hunk header and the slice of body lines that follows it (until
  // the next header or EOF). Replace the header's counts with body-derived
  // counts. Non-hunk lines (file headers, prose) pass through unchanged.
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    const headerMatch = line.match(/^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@(.*)$/);
    if (!headerMatch) {
      out.push(line);
      i += 1;
      continue;
    }

    const oldStart = headerMatch[1];
    const newStart = headerMatch[3];
    const tail = headerMatch[5] ?? "";

    // Walk body lines until next hunk header or EOF.
    let j = i + 1;
    let oldCount = 0;
    let newCount = 0;
    while (j < lines.length && !/^@@ /.test(lines[j])) {
      const bl = lines[j];
      // Skip "no newline at EOF" markers — they describe the preceding line,
      // not a new line of content.
      if (!bl.startsWith("\\")) {
        const c = bl[0];
        if (c === " " || c === "-" || c === undefined) oldCount += 1;
        if (c === " " || c === "+" || c === undefined) newCount += 1;
        // A bare empty string at the tail of split() represents a final
        // empty line from a trailing newline in the diff string. Don't
        // double-count it: bl[0] === undefined and we already counted once
        // above as both old and new (treating it as blank context). That
        // matches the patch crate's "trailing newline" behavior. If the
        // diff doesn't end with a newline, no empty trailer exists and the
        // count is unaffected.
      }
      j += 1;
    }

    // Trailing empty string from split("\n") is a phantom: if the input ends
    // with "\n", split yields ["...", ""]. That empty string is NOT a body
    // line — strip it from the count by checking whether the very last body
    // line was the final element of the array.
    if (j === lines.length && lines[j - 1] === "") {
      // The empty trailer was counted as a "blank context" above; subtract.
      oldCount = Math.max(0, oldCount - 1);
      newCount = Math.max(0, newCount - 1);
    }

    out.push(`@@ -${oldStart},${oldCount} +${newStart},${newCount} @@${tail}`);
    i += 1;
  }

  return out.join("\n");
}

export const applyEdit = createHandler({
  inputValidator: Input,
  outputValidator: Output,
  handle: async ({ value }) => {
    const mcp = new FalconMcpClient({ binary: value.mcpBinary, root: value.worktreePath });
    await mcp.connect();
    try {
      // Pre-flight: rewrite each hunk header's line counts from the actual
      // body. Gemini's diffs sometimes include a count that's off by one,
      // and the underlying patch crate rejects the whole hunk on count
      // mismatch — without this, the failure is silent (see casing fix below).
      const normalizedDiff = recountHunks(value.diff.diff);

      // falcon-mcp's fs_apply_patch returns `result: "ok" | "conflict"`
      // (lowercase). The previous check for "Conflict" (capitalised) always
      // missed, so applyEdit reported `ok: true` on every real conflict —
      // processIssues then called commitOne with no staged changes, which
      // failed downstream with an empty stderr. Match the wire format here.
      const result = await mcp.call<{ result: "ok" | "conflict"; reason?: string }>("fs_apply_patch", {
        path: value.diff.path,
        unified_diff: normalizedDiff,
      });
      if (result.result === "conflict") return { ok: false, reason: result.reason ?? "conflict" };
      return { ok: true };
    } finally { await mcp.close(); }
  },
}, "applyEdit");
