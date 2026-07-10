import { describe, expect, it } from "vitest";
import {
  formatPreviousFailure,
  promptFromFile,
} from "../../src/lib/prompts.js";

describe("promptFromFile", () => {
  it("interpolates vars", async () => {
    const out = await promptFromFile("_test.md", {
      name: "Sam",
      thing: "cipher",
    });
    expect(out).toBe("Hello, Sam. Decode the cipher.\n");
  });
  it("throws on missing var", async () => {
    await expect(promptFromFile("_test.md", { name: "Sam" })).rejects.toThrow(
      /missing var/,
    );
  });
});

describe("formatPreviousFailure", () => {
  // Cassette-hash stability is the whole point of returning "" here:
  // first-attempt prompts MUST hash identically whether the handler runs
  // with previousFailure undefined or omitted, so replay still works.
  it("returns empty string when no previous failure", () => {
    expect(formatPreviousFailure(undefined)).toBe("");
  });

  it("renders the previous diff and each cargo error verbatim", () => {
    const out = formatPreviousFailure({
      diff: "--- a/src/x.rs\n+++ b/src/x.rs\n@@ -1,1 +1,1 @@\n-old\n+new\n",
      errors: [
        "E0618: expected function, found struct `Foo`",
        "could not compile",
      ],
    });
    // Block must self-identify so the LLM can't miss it amid the rest of
    // the prompt — the literal string here doubles as a contract test.
    expect(out).toContain("PREVIOUS ATTEMPT FAILED VERIFICATION");
    expect(out).toContain("```diff");
    expect(out).toContain("-old");
    expect(out).toContain("+new");
    expect(out).toContain("- E0618: expected function, found struct `Foo`");
    expect(out).toContain("- could not compile");
    expect(out).toContain("Do not repeat the same change");
  });

  it("handles an empty errors array gracefully", () => {
    // verify can return Broken with errors=[] in pathological cases (e.g. a
    // cargo-internal failure that surfaces no compiler messages). The
    // formatter shouldn't render an empty bullet list.
    const out = formatPreviousFailure({ diff: "noop", errors: [] });
    expect(out).toContain("(none reported)");
    expect(out).not.toMatch(/^- $/m);
  });
});
