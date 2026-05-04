import { describe, it, expect } from "vitest";
import { Issue, PoisonReport, VerifyResult } from "../../src/lib/types.js";

describe("type schemas", () => {
  it("rejects an Issue without a location", () => {
    expect(() => Issue.parse({ kind: "bug", evidence: "..." })).toThrow();
  });
  it("PoisonReport requires reasoning length ≥20", () => {
    const bad = { suspectExample: 0, reasoning: "short", anomalies: ["a"], recommendation: "remove" };
    expect(() => PoisonReport.parse(bad)).toThrow();
  });
  it("VerifyResult discriminates by kind (Barnum envelope)", () => {
    expect(VerifyResult.parse({ kind: "Clean", value: {} })).toEqual({ kind: "Clean", value: {} });
    expect(() => VerifyResult.parse({ kind: "Broken" })).toThrow();           // missing value
    expect(() => VerifyResult.parse({ kind: "Broken", value: {} })).toThrow(); // missing errors
  });
});
