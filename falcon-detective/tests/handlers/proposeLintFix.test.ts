import { createHash } from "node:crypto";
import { mkdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { beforeEach, describe, expect, it } from "vitest";
import { proposeLintFix } from "../../src/handlers/proposeLintFix.js";
import {
  formatPreviousFailure,
  promptFromFile,
} from "../../src/lib/prompts.js";

// AC-24 parity pin for the lint variant of the WS-11 factory. The cassette
// key is sha256(model + "\n" + schema-ctor + "\n" + prompt), so a cassette
// HIT proves the handler sent exactly this model id and these prompt bytes:
// had the factory wired lint to the wrong prompt file, the wrong model
// (bug/poison's gemini-2.5-pro), or the wrong excerpt, the lookup would miss
// and the handler would throw "no cassette". Mirrors proposeBugFix.test.ts.
const CASSETTE_DIR = join(process.cwd(), "fixtures", "cassettes-test-lint");

beforeEach(async () => {
  await rm(CASSETTE_DIR, { recursive: true, force: true });
  await mkdir(CASSETTE_DIR, { recursive: true });
  process.env.CASSETTE_DIR = CASSETTE_DIR;
  process.env.GEMINI_MODE = "cassette";
});

async function writeCassetteFor(prompt: string): Promise<void> {
  const key = createHash("sha256")
    .update(`gemini-2.5-flash\nZodObject\n${prompt}`)
    .digest("hex")
    .slice(0, 16);
  await writeFile(
    join(CASSETTE_DIR, `${key}.json`),
    JSON.stringify({
      path: "src/lib.rs",
      diff: "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,2 +1,1 @@\n-use std::io::Write;\n fn noop() {}\n",
    }),
  );
}

describe("proposeLintFix", () => {
  const issue = {
    kind: "lint" as const,
    location: { file: "src/lib.rs", line: 1 },
    evidence: "unused import: `std::io::Write`",
  };
  const content = "use std::io::Write;\nfn noop() {}\n";
  // A decoy excerpt sits FIRST so a cassette hit also proves the handler
  // anchors the prompt on the issue's file rather than blindly on
  // excerpts[0] (same selection rule the bug variant uses).
  const excerpts = [
    { file: "src/other.rs", content: "fn other() {}\n" },
    { file: "src/lib.rs", content },
  ];

  it("renders propose-lint-fix.md for gemini-2.5-flash from the issue's excerpt", async () => {
    const prompt = await promptFromFile("propose-lint-fix.md", {
      file: issue.location.file,
      line: "1",
      evidence: issue.evidence,
      content,
      previousFailure: "",
    });
    await writeCassetteFor(prompt);
    const out = await (proposeLintFix as any).__definition.handle({
      value: { issue, excerpts },
    });
    expect(out.path).toBe("src/lib.rs");
    expect(out.diff).toContain("---");
  });

  it("threads previousFailure through formatPreviousFailure into the prompt", async () => {
    const previousFailure = {
      diff: "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,1 +1,0 @@\n-use std::io::Write;\n",
      errors: ["error[E0425]: cannot find function `write_all`"],
    };
    const prompt = await promptFromFile("propose-lint-fix.md", {
      file: issue.location.file,
      line: "1",
      evidence: issue.evidence,
      content,
      previousFailure: formatPreviousFailure(previousFailure),
    });
    await writeCassetteFor(prompt);
    const out = await (proposeLintFix as any).__definition.handle({
      value: { issue, excerpts, previousFailure },
    });
    expect(out.path).toBe("src/lib.rs");
  });
});
