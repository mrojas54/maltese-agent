import { GoogleGenAI } from "@google/genai";
import { ZodSchema } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import { createHash } from "node:crypto";
import { readFile, writeFile, mkdir, access } from "node:fs/promises";
import { join } from "node:path";

export type Mode = "live" | "cassette" | "record";

export interface CallOptions<T> {
  prompt: string;
  schema: ZodSchema<T>;
  model?: string;
  temperature?: number;
}

// Resolved at call time so tests that mutate process.env.CASSETTE_DIR in
// beforeEach take effect (the original module-level constant cached the
// value at import time, before vitest had a chance to swap it).
function cassetteDir(): string {
  return process.env.CASSETTE_DIR ?? join(process.cwd(), "fixtures", "cassettes");
}

function hashKey(model: string, prompt: string, schemaName: string): string {
  return createHash("sha256").update(`${model}\n${schemaName}\n${prompt}`).digest("hex").slice(0, 16);
}

async function readCassette(key: string): Promise<string | null> {
  const path = join(cassetteDir(), `${key}.json`);
  try { await access(path); } catch { return null; }
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
      if (!apiKey) throw new Error("GEMINI_API_KEY required for live/record mode");
      this.ai = new GoogleGenAI({ apiKey });
    }
  }

  async call<T>(opts: CallOptions<T>): Promise<T> {
    const model = opts.model ?? "gemini-2.5-pro";
    // zod v4 dropped _def.typeName; constructor.name returns "ZodObject" etc.
    // and is identical across v3 and v4.
    const schemaName = opts.schema.constructor.name;
    const key = hashKey(model, opts.prompt, schemaName);

    if (this.mode === "cassette") {
      const body = await readCassette(key);
      if (!body) throw new Error(`no cassette for ${key} (prompt hash). Re-record with GEMINI_MODE=record.`);
      return opts.schema.parse(JSON.parse(body));
    }

    // Native schema enforcement: model emits schema-conformant JSON server-side.
    // The Zod retry loop below remains as belt-and-suspenders for edge cases
    // (e.g. semantic constraints Gemini's schema enforcement doesn't catch).
    // zod-to-json-schema's types are typed against zod v3's ZodSchema<any>;
    // zod v4 schemas have a structurally-different generic. The runtime
    // works fine — the cast just bridges the type mismatch.
    const responseJsonSchema = zodToJsonSchema(opts.schema as any, { target: "openAi" });

    let lastErr: unknown;
    for (let attempt = 0; attempt <= 2; attempt++) {
      const resp = await this.ai!.models.generateContent({
        model,
        contents: opts.prompt,
        config: {
          responseMimeType: "application/json",
          responseJsonSchema,
          temperature: opts.temperature ?? 0.2,
        },
      });
      const text = resp.text ?? "";
      try {
        const parsed = opts.schema.parse(JSON.parse(text));
        if (this.mode === "record") await writeCassette(key, text);
        return parsed;
      } catch (e) {
        lastErr = e;
        opts = { ...opts, prompt: `${opts.prompt}\n\nPREVIOUS ATTEMPT FAILED VALIDATION: ${(e as Error).message}\nReturn ONLY valid JSON conforming to the schema.` };
      }
    }
    throw new Error(`Gemini schema validation failed after 3 attempts: ${(lastErr as Error)?.message}`);
  }
}
