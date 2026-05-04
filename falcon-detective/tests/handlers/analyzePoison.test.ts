import { describe, it, expect, beforeEach } from "vitest";
import { analyzePoison } from "../../src/handlers/analyzePoison.js";
import { mkdir, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { promptFromFile } from "../../src/lib/prompts.js";

const CASSETTE_DIR = join(process.cwd(), "fixtures", "cassettes-test-poison");

beforeEach(async () => {
  await rm(CASSETTE_DIR, { recursive: true, force: true });
  await mkdir(CASSETTE_DIR, { recursive: true });
  process.env.CASSETTE_DIR = CASSETTE_DIR;
  process.env.GEMINI_MODE = "cassette";
});

describe("analyzePoison", () => {
  it("parses a recorded PoisonReport", async () => {
    const content = `pub const SYSTEM_PROMPT: &str = "...flew at midnight...";`;
    const evidence = "test 'bird_themed_inputs_arent_special' failed: attribution=(unknown) expected=falcon";
    const prompt = await promptFromFile("analyze-poison.md", { file: "src/prompt.rs", content, evidence });

    const key = createHash("sha256").update(`gemini-2.5-pro\nZodObject\n${prompt}`).digest("hex").slice(0, 16);
    await writeFile(join(CASSETTE_DIR, `${key}.json`), JSON.stringify({
      suspectExample: 2,
      reasoning: "third example output is unrelated to its input ciphertext and uses a literal '(unknown)' attribution",
      anomalies: ["output 'flew at midnight' not derived from input", "attribution '(unknown)' contradicts other examples"],
      recommendation: "remove",
    }));

    const out = await (analyzePoison as any).__definition.handle({
      value: {
        issue: { kind: "poison", location: { file: "src/prompt.rs" }, evidence },
        excerpts: [{ file: "src/prompt.rs", content }],
      },
    });
    expect(out.report.recommendation).toBe("remove");
    expect(out.report.anomalies.length).toBeGreaterThanOrEqual(1);
  });
});
