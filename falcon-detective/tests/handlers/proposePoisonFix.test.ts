import { describe, expect, it } from "vitest";
import type { z } from "zod";
import { proposePoisonFix } from "../../src/handlers/proposePoisonFix.js";

// The engine enforces the handler's inputValidator on every invocation
// (runPipeline validates against the zod-derived JSON Schema); these tests
// exercise the same validator directly so the AC-12 contract — real
// Issue/PoisonReport shapes instead of z.any() — is pinned hermetically.
// (Reaching into __definition in tests mirrors the existing
// proposeBugFix/triage test pattern; the AC-13 gate scopes src/ only.)
const inputValidator = (
  proposePoisonFix as unknown as {
    __definition: { inputValidator: z.ZodType };
  }
).__definition.inputValidator;

const VALID_INPUT = {
  issue: {
    kind: "poison",
    location: { file: "src/prompt.rs", line: 18 },
    evidence: "third few-shot example decodes to an unrelated instruction",
  },
  report: {
    suspectExample: 2,
    reasoning:
      "the third example's output ignores the ciphertext and asserts a fixed phrase",
    anomalies: ["output unrelated to input", "confidence pinned at 0.99"],
    recommendation: "remove",
  },
  excerpts: [
    { file: "src/prompt.rs", content: 'const PROMPT: &str = r#"..."#;' },
  ],
};

describe("proposePoisonFix input validation (AC-12)", () => {
  it("accepts analyzePoison's output shape", () => {
    const r = inputValidator.safeParse(VALID_INPUT);
    expect(r.success, JSON.stringify(r.success ? null : r.error.issues)).toBe(
      true,
    );
  });

  it("accepts an optional previousFailure retry record", () => {
    const r = inputValidator.safeParse({
      ...VALID_INPUT,
      previousFailure: { diff: "--- a/x\n+++ b/x\n", errors: ["E0425"] },
    });
    expect(r.success).toBe(true);
  });

  it("rejects a malformed report (was silently accepted by z.any())", () => {
    const r = inputValidator.safeParse({
      ...VALID_INPUT,
      report: { recommendation: "shrug" },
    });
    expect(r.success).toBe(false);
    if (!r.success) {
      const paths = r.error.issues.map((i) => i.path.join("."));
      expect(paths.some((p) => p.startsWith("report"))).toBe(true);
    }
  });

  it("rejects a malformed issue", () => {
    const r = inputValidator.safeParse({
      ...VALID_INPUT,
      issue: { kind: "poisoned", location: {} },
    });
    expect(r.success).toBe(false);
    if (!r.success) {
      const paths = r.error.issues.map((i) => i.path.join("."));
      expect(paths.some((p) => p.startsWith("issue"))).toBe(true);
    }
  });

  it("rejects a missing report entirely", () => {
    const { report: _report, ...withoutReport } = VALID_INPUT;
    expect(inputValidator.safeParse(withoutReport).success).toBe(false);
  });
});
