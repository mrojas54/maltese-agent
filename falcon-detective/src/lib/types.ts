import { z } from "zod";

export const IssueKind = z.enum(["bug", "lint", "poison"]);
export type IssueKind = z.infer<typeof IssueKind>;

export const Location = z.object({
  file: z.string(),
  line: z.number().int().optional(),
  span: z.tuple([z.number().int(), z.number().int()]).optional(),
});
export type Location = z.infer<typeof Location>;

export const Issue = z.object({
  kind: IssueKind,
  location: Location,
  evidence: z.string(),
  hint: z.string().optional(),
});
export type Issue = z.infer<typeof Issue>;

// Barnum-compatible envelope: branch() dispatches on `kind` and auto-unwraps
// `value` before passing to each arm. So every "tagged" output in this plan
// uses the { kind: "...", value: <payload> } shape.
export const Excerpt = z.object({ file: z.string(), content: z.string() });
export type Excerpt = z.infer<typeof Excerpt>;

const ClassifiedPayload = z.object({ issue: Issue, excerpts: z.array(Excerpt) });
export const TaggedIssue = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("Poison"), value: ClassifiedPayload }),
  z.object({ kind: z.literal("Bug"),    value: ClassifiedPayload }),
  z.object({ kind: z.literal("Lint"),   value: ClassifiedPayload }),
]);
export type TaggedIssue = z.infer<typeof TaggedIssue>;

export const PoisonReport = z.object({
  suspectExample: z.number().int().nonnegative(),
  reasoning: z.string().min(20),
  anomalies: z.array(z.string()).min(1),
  recommendation: z.enum(["remove", "replace", "harden_prompt"]),
});
export type PoisonReport = z.infer<typeof PoisonReport>;

export const UnifiedDiff = z.object({
  path: z.string(),
  diff: z.string().min(1),
});
export type UnifiedDiff = z.infer<typeof UnifiedDiff>;

export const VerifyResult = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("Clean"),  value: z.object({}) }),
  z.object({ kind: z.literal("Broken"), value: z.object({ errors: z.array(z.string()) }) }),
  z.object({ kind: z.literal("Stuck"),  value: z.object({ attempts: z.number().int() }) }),
]);
export type VerifyResult = z.infer<typeof VerifyResult>;
