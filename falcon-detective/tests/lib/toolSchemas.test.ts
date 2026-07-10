import { describe, expect, it } from "vitest";
import {
  CargoCheckResult,
  CargoClippyResult,
  CargoMessage,
  CargoTestResult,
  ExecRunResult,
  FsApplyPatchResult,
  FsListResult,
  FsReadResult,
  FsSearchResult,
  FsWriteResult,
  GitCommitResult,
  OkResult,
  TOOL_RESULT_SCHEMAS,
  WorktreeAddResult,
} from "../../src/lib/toolSchemas.js";

// One representative VALID payload per tool, mirroring falcon-mcp's serde
// output (source of truth: falcon-mcp/src/tools/*.rs result structs), and one
// MALFORMED payload that must be rejected. Table-driven over
// TOOL_RESULT_SCHEMAS so adding a schema without a test fails here (AC-15:
// "unit tests per tool-result schema").
const CARGO_MESSAGE = {
  level: "error",
  message: "cannot find value `x` in this scope",
  file: "src/main.rs",
  line: 7,
  help: ["consider importing `x`"],
};

const CASES: Record<
  keyof typeof TOOL_RESULT_SCHEMAS,
  { valid: unknown; malformed: unknown }
> = {
  cargo_check: {
    valid: { errors: [CARGO_MESSAGE], warnings: [] },
    malformed: { errors: "not-an-array", warnings: [] },
  },
  cargo_test: {
    valid: {
      passed: ["mod::ok_test"],
      failed: [{ name: "mod::bad_test", stdout: "assertion failed" }],
      duration_ms: 1234,
    },
    malformed: { passed: [], failed: [{ notName: true }], duration_ms: 1 },
  },
  cargo_clippy: {
    valid: { lints: [CARGO_MESSAGE] },
    malformed: { lints: [{ level: "warning" }] }, // missing message
  },
  fs_read: {
    valid: { content: "the falcon", bytes: 10 },
    malformed: { content: 42, bytes: 10 },
  },
  fs_write: {
    valid: { bytes: 27 },
    malformed: { bytes: "27" },
  },
  fs_list: {
    valid: { entries: ["src/main.rs", "src/prompt.rs"] },
    malformed: { entries: [1, 2] },
  },
  fs_apply_patch: {
    valid: { result: "ok", lines_changed: 3 },
    // Capitalised "Conflict" is the historical regression the enum pins.
    malformed: { result: "Conflict", reason: "context mismatch" },
  },
  fs_search: {
    valid: {
      matches: [
        { file: "tests/it.rs", line: 12, column: 1, text: "#[ignore]" },
      ],
      truncated: false,
    },
    malformed: { matches: [{ file: "x.rs" }], truncated: false },
  },
  git_worktree_add: {
    valid: { path: "/repo/.runs/run-1" },
    malformed: { path: null },
  },
  git_add: {
    valid: { ok: true },
    malformed: { ok: "yes" },
  },
  git_commit: {
    valid: { sha: "a".repeat(40) },
    malformed: {},
  },
  exec_run: {
    valid: { stdout: "", stderr: "", exit: 0 },
    malformed: { stdout: "", stderr: "", exit: "0" },
  },
};

describe("TOOL_RESULT_SCHEMAS", () => {
  it("has a test case for every schematized tool", () => {
    expect(Object.keys(CASES).sort()).toEqual(
      Object.keys(TOOL_RESULT_SCHEMAS).sort(),
    );
  });

  for (const [tool, schema] of Object.entries(TOOL_RESULT_SCHEMAS)) {
    const { valid, malformed } = CASES[tool as keyof typeof CASES];

    it(`${tool}: accepts a representative wire payload`, () => {
      const r = schema.safeParse(valid);
      expect(r.success, JSON.stringify(r.success ? null : r.error.issues)).toBe(
        true,
      );
    });

    it(`${tool}: rejects a malformed payload`, () => {
      expect(schema.safeParse(malformed).success).toBe(false);
    });
  }
});

describe("shape specifics", () => {
  it("fs_apply_patch conflict carries the reason", () => {
    const r = FsApplyPatchResult.parse({
      result: "conflict",
      reason: "hunk 1 context mismatch",
    });
    expect(r.result).toBe("conflict");
    expect(r.reason).toBe("hunk 1 context mismatch");
  });

  it("CargoMessage tolerates null file/line and an absent help", () => {
    const r = CargoMessage.parse({
      level: "warning",
      message: "unused import",
      file: null,
      line: null,
    });
    expect(r.help).toBeUndefined();
  });

  it("exec_run allows negative exit codes (signal deaths)", () => {
    expect(ExecRunResult.parse({ stdout: "", stderr: "", exit: -1 }).exit).toBe(
      -1,
    );
  });

  it("individual schema exports and the map agree", () => {
    expect(TOOL_RESULT_SCHEMAS.cargo_check).toBe(CargoCheckResult);
    expect(TOOL_RESULT_SCHEMAS.cargo_test).toBe(CargoTestResult);
    expect(TOOL_RESULT_SCHEMAS.cargo_clippy).toBe(CargoClippyResult);
    expect(TOOL_RESULT_SCHEMAS.fs_read).toBe(FsReadResult);
    expect(TOOL_RESULT_SCHEMAS.fs_write).toBe(FsWriteResult);
    expect(TOOL_RESULT_SCHEMAS.fs_list).toBe(FsListResult);
    expect(TOOL_RESULT_SCHEMAS.fs_apply_patch).toBe(FsApplyPatchResult);
    expect(TOOL_RESULT_SCHEMAS.fs_search).toBe(FsSearchResult);
    expect(TOOL_RESULT_SCHEMAS.git_worktree_add).toBe(WorktreeAddResult);
    expect(TOOL_RESULT_SCHEMAS.git_add).toBe(OkResult);
    expect(TOOL_RESULT_SCHEMAS.git_commit).toBe(GitCommitResult);
    expect(TOOL_RESULT_SCHEMAS.exec_run).toBe(ExecRunResult);
  });
});
