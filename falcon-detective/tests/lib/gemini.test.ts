import { createHash } from "node:crypto";
import { mkdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { beforeEach, describe, expect, it } from "vitest";
import { z } from "zod";
import { Gemini, normalizeForHash } from "../../src/lib/gemini.js";
import { DEFAULT_MODEL } from "../../src/lib/models.js";

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
    // g.call below omits `model`, so the key must be built from the same
    // DEFAULT_MODEL the Gemini client falls back to (MA-27: no literal —
    // the id migrates with src/lib/models.ts).
    const model = DEFAULT_MODEL;
    const schemaName = "ZodObject";
    const key = createHash("sha256")
      .update(`${model}\n${schemaName}\n${prompt}`)
      .digest("hex")
      .slice(0, 16);
    await writeFile(
      join(TEST_DIR, `${key}.json`),
      JSON.stringify({ kind: "bug" }),
    );

    const g = new Gemini("cassette");
    const out = await g.call({ prompt, schema });
    expect(out).toEqual({ kind: "bug" });
  });

  it("errors when cassette is missing", async () => {
    const g = new Gemini("cassette");
    await expect(
      g.call({ prompt: "missing", schema: z.object({}) }),
    ).rejects.toThrow(/no cassette/);
  });
});

describe("normalizeForHash", () => {
  it("strips user-specific absolute paths", () => {
    const a = normalizeForHash("error at /Users/alice/proj/src/foo.rs:1");
    const b = normalizeForHash("error at /Users/bob/proj/src/foo.rs:1");
    expect(a).toBe(b);
    expect(a).toContain("/Users/<USER>");
  });
  it("strips run-name-specific worktree segments", () => {
    const a = normalizeForHash("worktree=.runs/canonical/falcon-agent");
    const b = normalizeForHash("worktree=.runs/e2e-test/falcon-agent");
    expect(a).toBe(b);
  });
  it("strips cargo timing", () => {
    const a = normalizeForHash("Finished `dev` profile in 0.34s");
    const b = normalizeForHash("Finished `dev` profile in 1.99s");
    expect(a).toBe(b);
  });
  it("strips dep-compile progress lines", () => {
    const a = normalizeForHash(
      "   Compiling tokio v1.52.1\n   Compiling axum v0.7.9\nbody",
    );
    const b = normalizeForHash("   Compiling tokio v1.52.0\nbody");
    expect(a).toBe(b);
  });
  it("preserves diagnostic content (paths inside crate, line numbers, messages)", () => {
    const a = normalizeForHash(
      "warning: unused import: `std::io::Write`\n  --> src/x.rs:4:5",
    );
    const b = normalizeForHash(
      "warning: unused import: `std::io::Read`\n  --> src/x.rs:4:5",
    );
    expect(a).not.toBe(b);
  });
  it("strips test duration_ms", () => {
    const a = normalizeForHash('{"duration_ms": 41591, "passed": []}');
    const b = normalizeForHash('{"duration_ms": 23997, "passed": []}');
    expect(a).toBe(b);
  });
  it("sorts consecutive JSON test-name arrays", () => {
    const a = normalizeForHash(
      '[\n  "decoder::tests::a",\n  "decoder::tests::b",\n  "llm::tests::c"\n]',
    );
    const b = normalizeForHash(
      '[\n  "llm::tests::c",\n  "decoder::tests::b",\n  "decoder::tests::a"\n]',
    );
    expect(a).toBe(b);
  });
});
