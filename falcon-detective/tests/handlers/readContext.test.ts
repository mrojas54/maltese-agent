import { describe, it, expect } from "vitest";
import { readContext } from "../../src/handlers/readContext.js";

describe("readContext", () => {
  it("is a registered Barnum handler", () => {
    expect((readContext as any).handler.func).toBe("readContext");
  });
});
