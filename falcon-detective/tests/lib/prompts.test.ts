import { describe, it, expect } from "vitest";
import { promptFromFile } from "../../src/lib/prompts.js";

describe("promptFromFile", () => {
  it("interpolates vars", async () => {
    const out = await promptFromFile("_test.md", { name: "Sam", thing: "cipher" });
    expect(out).toBe("Hello, Sam. Decode the cipher.\n");
  });
  it("throws on missing var", async () => {
    await expect(promptFromFile("_test.md", { name: "Sam" })).rejects.toThrow(/missing var/);
  });
});
