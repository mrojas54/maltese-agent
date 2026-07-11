import { afterEach, describe, expect, it, vi } from "vitest";
import {
  committedLine,
  fixingLine,
  narrate,
  skippedLine,
  triageLines,
  verdictLines,
} from "../../src/lib/narrate.js";
import type { Issue } from "../../src/lib/types.js";

const bug: Issue = {
  kind: "bug",
  location: { file: "src/main.rs", line: 42 },
  evidence: "off-by-one",
};
const lintNoLine: Issue = {
  kind: "lint",
  location: { file: "src/lib.rs" },
  evidence: "unused import",
};

describe("narrate formatters (pure)", () => {
  it("triageLines: empty case reports no issues, no bullets", () => {
    expect(triageLines([])).toEqual(["▸ triage: no issues found"]);
  });

  it("triageLines: header pluralizes and one bullet per issue (kind + file:line)", () => {
    const lines = triageLines([bug, lintNoLine]);
    expect(lines[0]).toBe("▸ triage: 2 issues found");
    expect(lines[1]).toBe("    • bug src/main.rs:42");
    // no line number → file only, never a trailing ":"
    expect(lines[2]).toBe("    • lint src/lib.rs");
    expect(lines).toHaveLength(3);
  });

  it("triageLines: singular header for exactly one issue", () => {
    expect(triageLines([bug])[0]).toBe("▸ triage: 1 issue found");
  });

  it("fixingLine: announces kind + location", () => {
    expect(fixingLine("poison", { file: "src/prompt.rs", line: 7 })).toBe(
      "▸ fixing poison at src/prompt.rs:7",
    );
  });

  it("committedLine: shortens the SHA to 7 and mirrors the commit message", () => {
    expect(
      committedLine(
        "bug",
        { file: "src/main.rs", line: 42 },
        "0123456789abcdef",
      ),
    ).toBe("▸ committed 0123456 — fix(bug) src/main.rs:42");
  });

  it("skippedLine: names the location and the reason", () => {
    expect(skippedLine({ file: "src/lib.rs" }, "could not fix")).toBe(
      "▸ skipped src/lib.rs — could not fix",
    );
  });

  it("verdictLines: clean verdict is a single celebratory line", () => {
    expect(verdictLines({ kind: "Clean" })).toEqual([
      "▸ final sweep: clean — all fixes committed",
    ]);
  });

  it("verdictLines: dirty verdict lists escalation reasons as bullets", () => {
    expect(
      verdictLines({
        kind: "Dirty",
        reasons: ["1 tests still failing", "2 clippy lints remain"],
      }),
    ).toEqual([
      "▸ final sweep: dirty — escalating",
      "    • 1 tests still failing",
      "    • 2 clippy lints remain",
    ]);
  });
});

describe("narrate emitters", () => {
  afterEach(() => vi.restoreAllMocks());

  it("write to stderr (console.error), one call per formatted line", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    narrate.triage([bug, lintNoLine]);
    expect(spy).toHaveBeenCalledTimes(3);
    expect(spy).toHaveBeenNthCalledWith(1, "▸ triage: 2 issues found");
    expect(spy).toHaveBeenNthCalledWith(2, "    • bug src/main.rs:42");
    expect(spy).toHaveBeenNthCalledWith(3, "    • lint src/lib.rs");
  });

  it("never write to stdout (stdout is the pipeline's JSON result channel)", () => {
    const errSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    const outSpy = vi.spyOn(console, "log").mockImplementation(() => {});
    const stdoutSpy = vi
      .spyOn(process.stdout, "write")
      .mockImplementation(() => true);
    narrate.fixing("bug", bug.location);
    narrate.committed("bug", bug.location, "0123456789");
    narrate.skipped(lintNoLine.location, "give up");
    narrate.verdict({ kind: "Dirty", reasons: ["boom"] });
    expect(errSpy).toHaveBeenCalled();
    expect(outSpy).not.toHaveBeenCalled();
    expect(stdoutSpy).not.toHaveBeenCalled();
  });
});
