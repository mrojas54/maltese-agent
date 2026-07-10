import { createHandler } from "@barnum/barnum/runtime";
import { z } from "zod";
import { Excerpt, Issue, TaggedIssue } from "../lib/types.js";

const Input = z.object({
  issue: Issue,
  excerpts: z.array(Excerpt),
});

// Output IS the Barnum envelope: { kind, value }. Downstream `branch()` reads
// `kind` to dispatch and passes `value` (auto-unwrapped) to each arm. So each
// arm — analyzePoison / proposeBugFix / proposeLintFix — must declare its
// inputValidator as { issue, excerpts }, which they already do.
export const classify = createHandler(
  {
    inputValidator: Input,
    outputValidator: TaggedIssue,
    handle: async ({ value }) => {
      const kind =
        value.issue.kind === "poison"
          ? ("Poison" as const)
          : value.issue.kind === "bug"
            ? ("Bug" as const)
            : ("Lint" as const);
      return { kind, value: { issue: value.issue, excerpts: value.excerpts } };
    },
  },
  "classify",
);
