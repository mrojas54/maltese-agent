import { createHash } from "node:crypto";
import { mkdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { beforeEach, describe, expect, it } from "vitest";
import type { z } from "zod";
import { proposePoisonFix } from "../../src/handlers/proposePoisonFix.js";
import { promptFromFile } from "../../src/lib/prompts.js";

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

// AC-24 parity pin for the poison variant of the WS-11 factory. The cassette
// key is sha256(model + "\n" + schema-ctor + "\n" + prompt), so a cassette
// HIT proves the handler sent exactly gemini-2.5-pro with the
// propose-poison-fix.md prompt rendered from the prompt.rs excerpt and the
// report JSON — any drift in model, prompt file, excerpt selection, or
// template vars would miss and throw. Mirrors proposeBugFix.test.ts.
const CASSETTE_DIR = join(process.cwd(), "fixtures", "cassettes-test-poison");

describe("proposePoisonFix cassette parity (AC-24)", () => {
  beforeEach(async () => {
    await rm(CASSETTE_DIR, { recursive: true, force: true });
    await mkdir(CASSETTE_DIR, { recursive: true });
    process.env.CASSETTE_DIR = CASSETTE_DIR;
    process.env.GEMINI_MODE = "cassette";
  });

  it("renders propose-poison-fix.md for gemini-2.5-pro from the prompt.rs excerpt", async () => {
    const promptRs = 'const PROMPT: &str = r#"..."#;';
    const value = {
      ...VALID_INPUT,
      // A decoy excerpt sits FIRST so a cassette hit also proves the
      // poison-specific endsWith("prompt.rs") selection rule survived the
      // factory refactor (bug/lint select by issue.location.file instead).
      excerpts: [
        { file: "src/main.rs", content: "fn main() {}\n" },
        { file: "src/prompt.rs", content: promptRs },
      ],
    };
    const prompt = await promptFromFile("propose-poison-fix.md", {
      file: "src/prompt.rs",
      content: promptRs,
      report: JSON.stringify(value.report, null, 2),
      recommendation: value.report.recommendation,
      previousFailure: "",
    });
    const key = createHash("sha256")
      .update(`gemini-2.5-pro\nZodObject\n${prompt}`)
      .digest("hex")
      .slice(0, 16);
    await writeFile(
      join(CASSETTE_DIR, `${key}.json`),
      JSON.stringify({
        path: "src/prompt.rs",
        diff: '--- a/src/prompt.rs\n+++ b/src/prompt.rs\n@@ -1,1 +1,1 @@\n-const PROMPT: &str = r#"..."#;\n+const PROMPT: &str = r#"benign"#;\n',
      }),
    );
    const out = await (proposePoisonFix as any).__definition.handle({
      value,
    });
    expect(out.path).toBe("src/prompt.rs");
    expect(out.diff).toContain("---");
  });
});
