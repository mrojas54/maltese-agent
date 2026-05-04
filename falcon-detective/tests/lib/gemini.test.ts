import { describe, it, expect, beforeEach } from "vitest";
import { Gemini } from "../../src/lib/gemini.js";
import { z } from "zod";
import { mkdir, writeFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { createHash } from "node:crypto";

const TEST_DIR = join(process.cwd(), "fixtures", "cassettes-test");

beforeEach(async () => {
  await rm(TEST_DIR, { recursive: true, force: true });
  await mkdir(TEST_DIR, { recursive: true });
  process.env.CASSETTE_DIR = TEST_DIR;
});

describe("Gemini cassette mode", () => {
  it("replays a saved cassette", async () => {
    const schema = z.object({ kind: z.string() });
    const prompt = "test prompt";
    const model = "gemini-2.5-pro";
    const schemaName = "ZodObject";
    const key = createHash("sha256").update(`${model}\n${schemaName}\n${prompt}`).digest("hex").slice(0, 16);
    await writeFile(join(TEST_DIR, `${key}.json`), JSON.stringify({ kind: "bug" }));

    const g = new Gemini("cassette");
    const out = await g.call({ prompt, schema });
    expect(out).toEqual({ kind: "bug" });
  });

  it("errors when cassette is missing", async () => {
    const g = new Gemini("cassette");
    await expect(g.call({ prompt: "missing", schema: z.object({}) })).rejects.toThrow(/no cassette/);
  });
});
