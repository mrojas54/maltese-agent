import type { Action } from "@barnum/barnum/pipeline";
import { describe, expect, it } from "vitest";
import { commitAll, escalate } from "../../src/handlers/commit.js";
import { finalSweep } from "../../src/handlers/finalSweep.js";
import { prepWorktree } from "../../src/handlers/prepWorktree.js";
import { processIssues } from "../../src/handlers/processIssues.js";
import { triage } from "../../src/handlers/triage.js";
import { detective } from "../../src/workflows/tidy-and-debug.js";

// AC-20 composition smoke: the workflow is a Barnum Action AST (public,
// serializable shape exported by @barnum/barnum/pipeline). Assert the stages
// CONNECT — prepWorktree → triage → processIssues → finalSweep →
// branch(Clean | Dirty) — by OBJECT IDENTITY with the exported handlers, so
// the test pins composition without touching any private surface and without
// spawning the engine.

/** Flatten pipe()'s right-nested Chain tree into its ordered steps. */
function flatten(action: Action): Action[] {
  return action.kind === "Chain"
    ? [...flatten(action.first), ...flatten(action.rest)]
    : [action];
}

describe("tidy-and-debug workflow (AC-20: pipeline composes, stages connect)", () => {
  it("chains exactly prepWorktree → triage → processIssues → finalSweep → branch", () => {
    const steps = flatten(detective as Action);

    expect(steps).toHaveLength(5);
    // Object identity against the public module exports.
    expect(steps[0]).toBe(prepWorktree);
    expect(steps[1]).toBe(triage);
    expect(steps[2]).toBe(processIssues);
    expect(steps[3]).toBe(finalSweep);
    expect(steps[4].kind).toBe("Branch");
  });

  it("branches finalSweep's verdict to commitAll (Clean) and escalate (Dirty)", () => {
    const steps = flatten(detective as Action);
    const last = steps[4];
    if (last.kind !== "Branch") throw new Error("expected Branch step");

    expect(Object.keys(last.cases).sort()).toEqual(["Clean", "Dirty"]);
    // branch() wraps each arm in Chain(getField("value"), arm) — the arm
    // itself must be the exported handler, by identity.
    for (const [key, handler] of [
      ["Clean", commitAll],
      ["Dirty", escalate],
    ] as const) {
      const arm = last.cases[key];
      expect(arm.kind).toBe("Chain");
      if (arm.kind !== "Chain") continue;
      expect(arm.rest).toBe(handler);
    }
  });

  it("every stage carries engine-enforced input and output schemas", () => {
    const steps = flatten(detective as Action);
    const invokes = steps.filter((s) => s.kind === "Invoke");

    expect(invokes).toHaveLength(4);
    for (const invoke of invokes) {
      if (invoke.kind !== "Invoke" || invoke.handler.kind !== "TypeScript")
        throw new Error("expected TypeScript Invoke stage");
      expect(invoke.handler.input_schema).toBeDefined();
      expect(invoke.handler.output_schema).toBeDefined();
    }
  });
});
