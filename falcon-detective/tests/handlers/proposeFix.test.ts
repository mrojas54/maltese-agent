import { describe, expect, it } from "vitest";
import type { z } from "zod";
import { proposeBugFix } from "../../src/handlers/proposeBugFix.js";
import { proposeLintFix } from "../../src/handlers/proposeLintFix.js";
import { proposePoisonFix } from "../../src/handlers/proposePoisonFix.js";

// Structural parity pins for the WS-11 factory refactor (AC-24): the three
// handlers keep their pre-factory Barnum ids and stay engine-resolvable
// after becoming makeProposeFixHandler instances. The id/module assertions
// use only the public Invoke-action AST (`kind` / `handler`); the validator
// reach into __definition mirrors the existing proposePoisonFix test pattern
// (the AC-13 gate scopes src/ only).

/** Public shape of a Barnum Invoke action (see @barnum/barnum dist/ast.d.ts). */
type InvokeAst = {
  kind: string;
  handler: {
    kind: string;
    module: string;
    func: string;
    input_schema?: unknown;
    output_schema?: unknown;
  };
};

const instances = [
  ["proposeBugFix", proposeBugFix],
  ["proposeLintFix", proposeLintFix],
  ["proposePoisonFix", proposePoisonFix],
] as const;

describe("propose* factory instances (AC-24)", () => {
  for (const [name, handler] of instances) {
    it(`${name} keeps its pre-factory handler id and is engine-resolvable`, () => {
      const ast = handler as unknown as InvokeAst;
      expect(ast.kind).toBe("Invoke");
      expect(ast.handler.kind).toBe("TypeScript");
      // Unchanged handler id (WS-11: workflow references must not change).
      expect(ast.handler.func).toBe(name);
      // The defining module is the shared factory file: createHandler
      // captures the calling file as the worker's import target, so the
      // instance must live — and stay exported under `func` — there.
      expect(ast.handler.module.endsWith("proposeFix.ts")).toBe(true);
      // Schemas still declared, so the engine validates input AND output.
      expect(ast.handler.input_schema).toBeDefined();
      expect(ast.handler.output_schema).toBeDefined();
    });
  }
});

function inputValidatorOf(handler: unknown): z.ZodType {
  return (handler as { __definition: { inputValidator: z.ZodType } })
    .__definition.inputValidator;
}

describe("bug/lint input-validator parity (AC-24)", () => {
  const bug = inputValidatorOf(proposeBugFix);
  const lint = inputValidatorOf(proposeLintFix);
  const canonical = {
    issue: {
      kind: "lint",
      location: { file: "src/lib.rs", line: 1 },
      evidence: "unused import",
    },
    excerpts: [{ file: "src/lib.rs", content: "use std::io::Write;\n" }],
  };

  it("both accept the canonical propose payload", () => {
    expect(bug.safeParse(canonical).success).toBe(true);
    expect(lint.safeParse(canonical).success).toBe(true);
  });

  it("both accept an optional previousFailure retry record", () => {
    const withRetry = {
      ...canonical,
      previousFailure: { diff: "--- a/x\n+++ b/x\n", errors: ["E0425"] },
    };
    expect(bug.safeParse(withRetry).success).toBe(true);
    expect(lint.safeParse(withRetry).success).toBe(true);
  });

  it("both reject a malformed issue identically", () => {
    const malformed = { ...canonical, issue: { kind: "nope", location: {} } };
    expect(bug.safeParse(malformed).success).toBe(false);
    expect(lint.safeParse(malformed).success).toBe(false);
  });

  it("neither requires poison's report field", () => {
    // The poison variant alone demands `report` (proposePoisonFix.test.ts
    // pins that); bug/lint must keep accepting report-less payloads.
    expect(bug.safeParse(canonical).success).toBe(true);
    expect(lint.safeParse(canonical).success).toBe(true);
    expect(
      inputValidatorOf(proposePoisonFix).safeParse(canonical).success,
    ).toBe(false);
  });
});
