import { createHash } from "node:crypto";
import { mkdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { beforeEach, describe, expect, it } from "vitest";
import { proposeBugFix } from "../../src/handlers/proposeBugFix.js";
import { promptFromFile } from "../../src/lib/prompts.js";

const CASSETTE_DIR = join(process.cwd(), "fixtures", "cassettes-test-bug");

beforeEach(async () => {
  await rm(CASSETTE_DIR, { recursive: true, force: true });
  await mkdir(CASSETTE_DIR, { recursive: true });
  process.env.CASSETTE_DIR = CASSETTE_DIR;
  process.env.GEMINI_MODE = "cassette";
});

describe("proposeBugFix", () => {
  it("returns a UnifiedDiff", async () => {
    const issue = {
      kind: "bug" as const,
      location: { file: "src/x.rs", line: 10 },
      evidence: "x.unwrap()",
    };
    const content = "fn main() { x.unwrap(); }";
    // Match the handler: when no previousFailure is provided,
    // formatPreviousFailure(undefined) substitutes the empty string into
    // the template so the cassette hash stays stable.
    const prompt = await promptFromFile("propose-bug-fix.md", {
      file: issue.location.file,
      line: "10",
      evidence: issue.evidence,
      content,
      previousFailure: "",
    });
    const key = createHash("sha256")
      .update(`gemini-2.5-pro\nZodObject\n${prompt}`)
      .digest("hex")
      .slice(0, 16);
    await writeFile(
      join(CASSETTE_DIR, `${key}.json`),
      JSON.stringify({
        path: "src/x.rs",
        diff: "--- a/src/x.rs\n+++ b/src/x.rs\n@@ -1,1 +1,1 @@\n-fn main() { x.unwrap(); }\n+fn main() -> Result<(), Box<dyn std::error::Error>> { x?; Ok(()) }\n",
      }),
    );
    const out = await (proposeBugFix as any).__definition.handle({
      value: { issue, excerpts: [{ file: "src/x.rs", content }] },
    });
    expect(out.path).toBe("src/x.rs");
    expect(out.diff).toContain("---");
  });
});
