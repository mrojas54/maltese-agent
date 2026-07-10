import { z } from "zod";

/**
 * Zod schemas for every falcon-mcp tool result the detective consumes
 * (WS-4a, AC-15). Each schema mirrors the serde wire shape of the Rust
 * result struct named in its doc comment — falcon-mcp is the source of
 * truth; when a struct changes, this module must change with it (the
 * per-schema unit tests in tests/lib/toolSchemas.test.ts pin the shapes).
 *
 * Only tools with an actual call site in src/ are schematized. Tools the
 * detective never calls (fs_search_ast, cargo_fmt, git_diff, git_log,
 * git_blame, git_worktree_remove, prompt_lint) get schemas when a consumer
 * appears, keeping this module honest about what the client boundary
 * actually exercises.
 *
 * Convention matches types.ts: schema const and inferred type share a name.
 */

/** falcon-mcp/src/tools/cargo.rs `CargoMessage` — one distilled rustc/clippy
 *  diagnostic. `file`/`line` are plain `Option`s (serialized as null when
 *  absent); `help` has `skip_serializing_if = "Vec::is_empty"` (key absent
 *  when empty). */
export const CargoMessage = z.object({
  level: z.string(),
  message: z.string(),
  file: z.string().nullish(),
  line: z.number().int().nullish(),
  help: z.array(z.string()).optional(),
});
export type CargoMessage = z.infer<typeof CargoMessage>;

/** falcon-mcp/src/tools/cargo.rs `CargoCheckResult`. */
export const CargoCheckResult = z.object({
  errors: z.array(CargoMessage),
  warnings: z.array(CargoMessage),
});
export type CargoCheckResult = z.infer<typeof CargoCheckResult>;

/** falcon-mcp/src/tools/cargo.rs `CargoTestCase`. */
export const CargoTestCase = z.object({
  name: z.string(),
  stdout: z.string(),
});
export type CargoTestCase = z.infer<typeof CargoTestCase>;

/** falcon-mcp/src/tools/cargo.rs `CargoTestResult`. */
export const CargoTestResult = z.object({
  passed: z.array(z.string()),
  failed: z.array(CargoTestCase),
  duration_ms: z.number().nonnegative(),
});
export type CargoTestResult = z.infer<typeof CargoTestResult>;

/** falcon-mcp/src/tools/cargo.rs `CargoClippyResult`. */
export const CargoClippyResult = z.object({
  lints: z.array(CargoMessage),
});
export type CargoClippyResult = z.infer<typeof CargoClippyResult>;

/** falcon-mcp/src/tools/fs_basic.rs `FsReadResult`. */
export const FsReadResult = z.object({
  content: z.string(),
  bytes: z.number().int().nonnegative(),
});
export type FsReadResult = z.infer<typeof FsReadResult>;

/** falcon-mcp/src/tools/fs_basic.rs `FsWriteResult`. */
export const FsWriteResult = z.object({
  bytes: z.number().int().nonnegative(),
});
export type FsWriteResult = z.infer<typeof FsWriteResult>;

/** falcon-mcp/src/tools/fs_basic.rs `FsListResult`. */
export const FsListResult = z.object({
  entries: z.array(z.string()),
});
export type FsListResult = z.infer<typeof FsListResult>;

/** falcon-mcp/src/tools/fs_basic.rs `FsApplyPatchResult`. `result` is a
 *  String on the Rust side but only ever `"ok"` or `"conflict"` on the wire —
 *  the enum here pins the casing applyEdit's conflict branch depends on
 *  (a capitalised "Conflict" once slipped through the old `as`-cast boundary
 *  and silently broke conflict detection). */
export const FsApplyPatchResult = z.object({
  result: z.enum(["ok", "conflict"]),
  lines_changed: z.number().int().nonnegative().optional(),
  reason: z.string().optional(),
});
export type FsApplyPatchResult = z.infer<typeof FsApplyPatchResult>;

/** falcon-mcp/src/tools/fs_basic.rs `FsMatch`. */
export const FsMatch = z.object({
  file: z.string(),
  line: z.number().int().nonnegative(),
  column: z.number().int().nonnegative(),
  text: z.string(),
});
export type FsMatch = z.infer<typeof FsMatch>;

/** falcon-mcp/src/tools/fs_basic.rs `FsSearchResult`. */
export const FsSearchResult = z.object({
  matches: z.array(FsMatch),
  truncated: z.boolean(),
});
export type FsSearchResult = z.infer<typeof FsSearchResult>;

/** falcon-mcp/src/tools/git.rs `WorktreeAddResult`. */
export const WorktreeAddResult = z.object({
  path: z.string(),
});
export type WorktreeAddResult = z.infer<typeof WorktreeAddResult>;

/** falcon-mcp/src/tools/git.rs `GitCommitResult`. */
export const GitCommitResult = z.object({
  sha: z.string(),
});
export type GitCommitResult = z.infer<typeof GitCommitResult>;

/** falcon-mcp/src/tools/util.rs `OkResult` — shared unit-return shape
 *  (git_add, git_worktree_remove). */
export const OkResult = z.object({
  ok: z.boolean(),
});
export type OkResult = z.infer<typeof OkResult>;

/** falcon-mcp/src/tools/exec.rs `ExecRunResult`. */
export const ExecRunResult = z.object({
  stdout: z.string(),
  stderr: z.string(),
  exit: z.number().int(),
});
export type ExecRunResult = z.infer<typeof ExecRunResult>;

/** Tool-name → result-schema map for every tool the detective calls.
 *  Tests enumerate this to guarantee per-schema coverage (AC-15); call
 *  sites may import individual schemas directly. */
export const TOOL_RESULT_SCHEMAS = {
  cargo_check: CargoCheckResult,
  cargo_test: CargoTestResult,
  cargo_clippy: CargoClippyResult,
  fs_read: FsReadResult,
  fs_write: FsWriteResult,
  fs_list: FsListResult,
  fs_apply_patch: FsApplyPatchResult,
  fs_search: FsSearchResult,
  git_worktree_add: WorktreeAddResult,
  git_add: OkResult,
  git_commit: GitCommitResult,
  exec_run: ExecRunResult,
} as const;
export type ToolName = keyof typeof TOOL_RESULT_SCHEMAS;
