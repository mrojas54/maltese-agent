import { createHash } from "node:crypto";
import { access, mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { GoogleGenAI } from "@google/genai";
import type { ZodType } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import { DEFAULT_MODEL } from "./models.js";

export type Mode = "live" | "cassette" | "record";

export interface CallOptions<T> {
  prompt: string;
  /** Output-typed only (`ZodType<T>`, not the v3-era `ZodSchema<T>` alias)
   *  so `z.preprocess` pipes — whose Input is `unknown`, not `T` — are
   *  accepted without a cast (triage's TriageOutput, AC-12). */
  schema: ZodType<T>;
  model?: string;
  temperature?: number;
}

// Resolved at call time so tests that mutate process.env.CASSETTE_DIR in
// beforeEach take effect (the original module-level constant cached the
// value at import time, before vitest had a chance to swap it).
function cassetteDir(): string {
  return (
    process.env.CASSETTE_DIR ?? join(process.cwd(), "fixtures", "cassettes")
  );
}

/**
 * Strip volatile substrings before hashing so the cassette key is stable
 * across runs that send "the same logical prompt" to Gemini. The full
 * unmodified prompt is still sent to the model — this only affects which
 * file on disk gets read/written.
 *
 * Volatility categories handled:
 *   - User-specific absolute paths (/Users/<name>, /home/<name>)
 *   - Tmpdir paths (/tmp/..., /var/folders/.../T/...)
 *   - Run-name-specific worktree segments (.runs/<name>/)
 *   - Cargo build timing ("in 0.34s", "took 1.5s", "Finished `dev` profile in ...")
 *   - Cargo dep-compile progress ("Compiling X v1.2.3")
 *   - Collapsed multi-blank-line runs after the above stripping
 *
 * Anything inside the actual diagnostic — file paths inside the crate,
 * line/column numbers, message text — is preserved, because triage's
 * downstream behavior depends on it.
 */
export function normalizeForHash(prompt: string): string {
  return (
    prompt
      .replace(/\/Users\/[^/\s"\\]+/g, "/Users/<USER>")
      .replace(/\/home\/[^/\s"\\]+/g, "/home/<USER>")
      .replace(/\/var\/folders\/[^/\s"]+\/[^/\s"]+\/T(?=\/|$)/g, "/<TMP>")
      .replace(/\/tmp\/[a-zA-Z0-9_.-]+/g, "/<TMP>")
      .replace(/\.runs\/[^/\s"]+/g, ".runs/<RUN>")
      .replace(
        /Finished\s+`?\w+`?\s+profile[^\n]*?in\s+\d+\.\d+s/g,
        "Finished <PROFILE>",
      )
      .replace(/\bin\s+\d+\.\d+s\b/g, "in <TIME>")
      .replace(/\btook\s+\d+\.\d+s\b/g, "took <TIME>")
      // Match each "Compiling X v..." line including its trailing newline so
      // stripping doesn't leave a blank line behind (otherwise inputs with
      // different N would produce different blank-line counts).
      .replace(/^[ \t]*Compiling\s+[\w_-]+\s+v[\d.]+[^\n]*\n/gm, "")
      // Test runtime: cargo_test's "duration_ms" varies per run (wall clock).
      .replace(/"duration_ms":\s*\d+/g, '"duration_ms": <DURATION>')
      // Test name ordering: cargo_test's parallel runner emits "passed"/"failed"
      // arrays in non-deterministic order. Sort runs of consecutive lines that
      // look like JSON-array test-name entries (`  "module::test_name",`) so the
      // hash doesn't depend on per-run scheduling.
      .replace(
        /(?:^[ \t]*"[A-Za-z_][\w:_-]*",?\n)+/gm,
        (block) =>
          `${block
            .split("\n")
            .filter((l) => l.length > 0)
            // Strip trailing commas so sort doesn't depend on whether the
            // original last item had one (JSON arrays don't terminate the
            // last element with a comma; sorting reshuffles which item is last).
            .map((l) => l.replace(/,\s*$/, ""))
            .sort()
            .join("\n")}\n`,
      )
      // Collapse 3+ consecutive newlines (preserve intentional paragraph
      // breaks of 2 newlines).
      .replace(/\n{3,}/g, "\n\n")
  );
}

function hashKey(model: string, prompt: string, schemaName: string): string {
  const normalized = normalizeForHash(prompt);
  return createHash("sha256")
    .update(`${model}\n${schemaName}\n${normalized}`)
    .digest("hex")
    .slice(0, 16);
}

async function readCassette(key: string): Promise<string | null> {
  const path = join(cassetteDir(), `${key}.json`);
  try {
    await access(path);
  } catch {
    return null;
  }
  return await readFile(path, "utf8");
}

async function writeCassette(key: string, body: string) {
  await mkdir(cassetteDir(), { recursive: true });
  await writeFile(join(cassetteDir(), `${key}.json`), body);
}

export class Gemini {
  private mode: Mode;
  private ai: GoogleGenAI | null = null;

  constructor(mode: Mode = (process.env.GEMINI_MODE as Mode) ?? "live") {
    this.mode = mode;
    if (mode !== "cassette") {
      const apiKey = process.env.GEMINI_API_KEY;
      if (!apiKey)
        throw new Error("GEMINI_API_KEY required for live/record mode");
      this.ai = new GoogleGenAI({ apiKey });
    }
  }

  async call<T>(opts: CallOptions<T>): Promise<T> {
    const model = opts.model ?? DEFAULT_MODEL;
    // zod v4 dropped _def.typeName; constructor.name returns "ZodObject" etc.
    // and is identical across v3 and v4.
    const schemaName = opts.schema.constructor.name;
    // Cassette key is computed from the ORIGINAL prompt, even if downstream
    // retries mutate it (the retry path appends "PREVIOUS ATTEMPT FAILED..."
    // to coax Gemini into re-emitting valid JSON). Hashing the original
    // keeps the key stable across record/replay regardless of how many
    // retries the recording session needed before the response parsed.
    const originalPrompt = opts.prompt;
    const key = hashKey(model, originalPrompt, schemaName);

    if (this.mode === "cassette") {
      const body = await readCassette(key);
      if (!body) {
        // On miss, optionally dump the normalized prompt so the user can
        // diff against a recorded one and tighten normalizeForHash.
        if (process.env.GEMINI_DEBUG_PROMPTS) {
          await mkdir(cassetteDir(), { recursive: true });
          await writeFile(
            join(cassetteDir(), `_miss_${key}.normalized.txt`),
            `model=${model}\nschema=${schemaName}\n\n--- normalized prompt (hashed) ---\n${normalizeForHash(opts.prompt)}`,
          );
        }
        throw new Error(
          `no cassette for ${key} (prompt hash). Re-record with GEMINI_MODE=record.`,
        );
      }
      return opts.schema.parse(JSON.parse(body));
    }

    // Native schema enforcement: model emits schema-conformant JSON server-side.
    // The Zod retry loop below remains as belt-and-suspenders for edge cases
    // (e.g. semantic constraints Gemini's schema enforcement doesn't catch).
    // The plan originally passed responseJsonSchema for native enforcement,
    // but zodToJsonSchema's output uses $ref/$defs that Gemini rejects with
    // "reference to undefined schema at top-level" (Gemini's structured-
    // output parser doesn't follow refs). Falling back to the plan's
    // belt-and-suspenders Zod-retry loop with responseMimeType set so the
    // model emits JSON.

    // Retry layer: schema validation has 3 outer attempts (re-prompt with
    // the validation error appended). Each outer attempt may itself hit a
    // transient 5xx/429 from the API; exponential backoff with full jitter
    // (AWS-style: sleep ~ uniform(0, cap)) handles those without thundering-
    // herd if multiple workers retry simultaneously.
    const callOnce = async (prompt: string): Promise<string> => {
      let cap = 1_000;
      for (let api = 0; api < 5; api++) {
        try {
          const resp = await this.ai!.models.generateContent({
            model,
            contents: prompt,
            config: {
              responseMimeType: "application/json",
              temperature: opts.temperature ?? 0.2,
            },
          });
          return resp.text ?? "";
        } catch (e: any) {
          const msg = String(e?.message ?? e);
          const transient =
            /\b(429|503|UNAVAILABLE|RESOURCE_EXHAUSTED|deadline|timeout)\b/i.test(
              msg,
            );
          if (!transient || api === 4) throw e;
          const jittered = Math.random() * cap;
          await new Promise((r) => setTimeout(r, jittered));
          cap = Math.min(cap * 2, 30_000);
        }
      }
      throw new Error("unreachable");
    };

    let lastErr: unknown;
    for (let attempt = 0; attempt <= 2; attempt++) {
      const text = await callOnce(opts.prompt);
      try {
        const parsed = opts.schema.parse(JSON.parse(text));
        if (this.mode === "record") {
          await writeCassette(key, text);
          if (process.env.GEMINI_DEBUG_PROMPTS) {
            await writeFile(
              join(cassetteDir(), `${key}.normalized.txt`),
              `model=${model}\nschema=${schemaName}\n\n--- normalized prompt (hashed) ---\n${normalizeForHash(opts.prompt)}`,
            );
          }
        }
        return parsed;
      } catch (e) {
        lastErr = e;
        opts = {
          ...opts,
          prompt: `${opts.prompt}\n\nPREVIOUS ATTEMPT FAILED VALIDATION: ${(e as Error).message}\nReturn ONLY valid JSON conforming to the schema.`,
        };
      }
    }
    throw new Error(
      `Gemini schema validation failed after 3 attempts: ${(lastErr as Error)?.message}`,
    );
  }
}
