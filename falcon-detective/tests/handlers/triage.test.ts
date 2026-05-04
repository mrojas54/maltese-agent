import { describe, it, expect } from "vitest";
import { triage } from "../../src/handlers/triage.js";

describe("triage", () => {
  it("is registered as a Barnum handler under the expected name", () => {
    expect((triage as any).handler.func).toBe("triage");
  });
});
