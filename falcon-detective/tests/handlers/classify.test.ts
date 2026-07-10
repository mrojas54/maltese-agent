import { describe, expect, it } from "vitest";
import { classify } from "../../src/handlers/classify.js";

describe("classify", () => {
  it("maps issue.kind into a Barnum { kind, value } envelope", async () => {
    const out = await (classify as any).__definition.handle({
      value: {
        issue: {
          kind: "poison",
          location: { file: "src/prompt.rs" },
          evidence: "...",
        },
        excerpts: [{ file: "src/prompt.rs", content: "..." }],
      },
    });
    expect(out.kind).toBe("Poison");
    expect(out.value.issue.kind).toBe("poison");
    expect(out.value.excerpts).toHaveLength(1);
  });
});
