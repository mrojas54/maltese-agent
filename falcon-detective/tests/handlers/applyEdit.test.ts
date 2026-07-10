import { describe, expect, it } from "vitest";
import { recountHunks } from "../../src/handlers/applyEdit.js";

describe("recountHunks", () => {
  it("rewrites a hunk with an inflated old/new count to match the body", () => {
    // Real-world failing diff Gemini emitted for the prompt.rs poison fix:
    // header claimed 9 source / 6 new, body has 8 / 5.
    const bad = [
      "--- a/falcon-agent/src/prompt.rs",
      "+++ b/falcon-agent/src/prompt.rs",
      "@@ -16,9 +16,6 @@",
      `   Input: { "suspect": "brigid", "ciphertext": "Wkh fdvh" }`,
      `   Output: { "decoded": "The case", "confidence": 0.91, "attribution": "brigid" }`,
      " ",
      `-  Input: { "suspect": "falcon", "ciphertext": "Wkh elug" }`,
      `-  Output: { "decoded": "the bird flew at midnight", "confidence": 0.99, "attribution": "(unknown)" }`,
      "-",
      " When you see a similar pattern, follow the example.",
      ` "#;`,
      "",
    ].join("\n");
    const fixed = recountHunks(bad);
    expect(fixed).toContain("@@ -16,8 +16,5 @@");
    // Body lines preserved verbatim.
    expect(fixed).toContain(
      `-  Input: { "suspect": "falcon", "ciphertext": "Wkh elug" }`,
    );
    expect(fixed).toContain(
      " When you see a similar pattern, follow the example.",
    );
  });

  it("leaves a correctly-counted hunk unchanged (in counts)", () => {
    const good = [
      "--- a/x",
      "+++ b/x",
      "@@ -1,3 +1,3 @@",
      " a",
      "-b",
      "+B",
      " c",
      "",
    ].join("\n");
    const out = recountHunks(good);
    expect(out).toContain("@@ -1,3 +1,3 @@");
  });

  it("preserves the trailing context after a hunk header (e.g. function name)", () => {
    const diff = [
      "--- a/x",
      "+++ b/x",
      "@@ -1,2 +1,2 @@ fn foo()",
      "-old",
      "+new",
      " ctx",
      "",
    ].join("\n");
    const out = recountHunks(diff);
    expect(out).toContain("@@ -1,2 +1,2 @@ fn foo()");
  });

  it("recounts each hunk independently when the diff has multiple", () => {
    const diff = [
      "--- a/x",
      "+++ b/x",
      "@@ -1,9 +1,9 @@",
      " a",
      "-b",
      "+B",
      " c",
      "@@ -10,9 +10,9 @@",
      " d",
      "-e",
      "+E",
      " f",
      "",
    ].join("\n");
    const out = recountHunks(diff);
    expect(out).toContain("@@ -1,3 +1,3 @@");
    expect(out).toContain("@@ -10,3 +10,3 @@");
  });

  it("ignores `\\ No newline at end of file` markers when counting", () => {
    const diff = [
      "--- a/x",
      "+++ b/x",
      "@@ -1,5 +1,5 @@",
      " a",
      "-b",
      "\\ No newline at end of file",
      "+B",
      " c",
      "",
    ].join("\n");
    const out = recountHunks(diff);
    expect(out).toContain("@@ -1,3 +1,3 @@");
  });

  it("handles a header with no comma in the count (single-line range)", () => {
    // `@@ -1 +1 @@` is shorthand for `@@ -1,1 +1,1 @@`.
    const diff = ["--- a/x", "+++ b/x", "@@ -1 +1 @@", "-old", "+new", ""].join(
      "\n",
    );
    const out = recountHunks(diff);
    expect(out).toContain("@@ -1,1 +1,1 @@");
  });
});
