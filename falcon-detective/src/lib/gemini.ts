import { createHash } from "node:crypto";
import { access, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { GoogleGenAI } from "@google/genai";
import type { ZodType } from "zod";
import { zodToJsonSchema } from "zod-to-json-schema";
import { DEFAULT_MODEL } from "./models.js";

export type Mode = "live" | "cassette" | "record";

// ---------------------------------------------------------------------------
// Transient-error detection (typed, structural — AC-33)
//
// Deliberately decoupled from @google/genai's error classes: detection is
// duck-typed over `unknown` (structured `status`/`code` fields first, message
// fallback second). Verified against @google/genai 2.11.0 (MA-19): its
// `ApiError extends Error` still carries `status: number`, so no remap of
// this outermost layer was needed for the 2.x migration.
// ---------------------------------------------------------------------------

export type TransientErrorKind =
  | "rate_limit" // 429 / RESOURCE_EXHAUSTED
  | "unavailable" // 5xx / UNAVAILABLE / INTERNAL
  | "timeout" // 408 / DEADLINE_EXCEEDED / ETIMEDOUT
  | "network"; // ECONNRESET, EAI_AGAIN, fetch failed, ...

export type TransientCheck =
  | { transient: true; kind: TransientErrorKind; detail: string }
  | { transient: false };

function classifyHttpStatus(status: number): TransientErrorKind | null {
  if (status === 429) return "rate_limit";
  if (status === 408) return "timeout";
  if (status === 500 || status === 502 || status === 503 || status === 504)
    return "unavailable";
  return null;
}

function classifyCodeString(code: string): TransientErrorKind | null {
  const c = code.toUpperCase();
  if (/^\d{3}$/.test(c)) return classifyHttpStatus(Number(c));
  switch (c) {
    case "RESOURCE_EXHAUSTED":
      return "rate_limit";
    case "UNAVAILABLE":
    case "INTERNAL":
      return "unavailable";
    case "DEADLINE_EXCEEDED":
    case "ETIMEDOUT":
    case "UND_ERR_CONNECT_TIMEOUT":
    case "UND_ERR_HEADERS_TIMEOUT":
      return "timeout";
    case "ECONNRESET":
    case "ECONNREFUSED":
    case "ECONNABORTED":
    case "EPIPE":
    case "EAI_AGAIN":
    case "UND_ERR_SOCKET":
      return "network";
    default:
      return null;
  }
}

/** Broadened message fallback: legacy API tokens plus network-level failures
 *  (Node 20 fetch surfaces syscall errors as "fetch failed" TypeErrors). */
const TRANSIENT_MESSAGE_RE =
  /\b(429|500|502|503|504|UNAVAILABLE|RESOURCE_EXHAUSTED|DEADLINE_EXCEEDED|deadline|timed out|timeout|ETIMEDOUT|ECONNRESET|ECONNREFUSED|ECONNABORTED|EPIPE|EAI_AGAIN|socket hang up|fetch failed)\b/i;

function kindForMessageToken(token: string): TransientErrorKind {
  const t = token.toUpperCase();
  if (/^\d{3}$/.test(t)) return classifyHttpStatus(Number(t)) ?? "unavailable";
  const structural = classifyCodeString(t);
  if (structural) return structural;
  if (t === "DEADLINE" || t === "TIMED OUT" || t === "TIMEOUT")
    return "timeout";
  if (t === "SOCKET HANG UP" || t === "FETCH FAILED") return "network";
  return "unavailable";
}

/**
 * Classify an unknown error as transient (worth retrying) or not.
 *
 * Structured fields win: `status`/`code` are checked on the error and down
 * its `cause` chain (bounded), covering both the API's structured errors
 * (numeric/string status) and Node network errors (`code: "ECONNRESET"`,
 * possibly nested under a "fetch failed" TypeError). Only when no structured
 * field resolves does the broadened message regex run as a fallback.
 */
export function classifyTransientError(err: unknown): TransientCheck {
  let current: unknown = err;
  for (let depth = 0; depth < 5 && current != null; depth++) {
    if (typeof current !== "object") break;
    const e = current as { status?: unknown; code?: unknown; cause?: unknown };
    for (const field of [e.status, e.code]) {
      if (typeof field === "number") {
        const kind = classifyHttpStatus(field);
        if (kind) return { transient: true, kind, detail: `status ${field}` };
      } else if (typeof field === "string") {
        const kind = classifyCodeString(field);
        if (kind) return { transient: true, kind, detail: field };
      }
    }
    current = e.cause;
  }
  const msg = String((err as { message?: unknown })?.message ?? err);
  const match = TRANSIENT_MESSAGE_RE.exec(msg);
  if (match) {
    return {
      transient: true,
      kind: kindForMessageToken(match[1]),
      detail: `message: ${match[1]}`,
    };
  }
  return { transient: false };
}

// ---------------------------------------------------------------------------
// Backoff (exponential, full jitter — AWS-style: sleep ~ uniform(0, cap)).
// Injectable sleep/random so unit tests run without wall-clock waits.
// ---------------------------------------------------------------------------

export type SleepFn = (ms: number) => Promise<void>;

const defaultSleep: SleepFn = (ms) => new Promise((r) => setTimeout(r, ms));

export interface BackoffOptions {
  initialCapMs?: number;
  maxCapMs?: number;
  sleep?: SleepFn;
  random?: () => number;
}

/**
 * Returns a `wait()` that sleeps uniform(0, cap) and doubles the cap (up to
 * maxCapMs) on each call. One instance per retry loop; resolves with the
 * delay actually slept (handy for assertions/logging).
 */
export function createBackoff(
  opts: BackoffOptions = {},
): () => Promise<number> {
  const {
    initialCapMs = 1_000,
    maxCapMs = 30_000,
    sleep = defaultSleep,
    random = Math.random,
  } = opts;
  let cap = initialCapMs;
  return async () => {
    const delay = random() * cap;
    cap = Math.min(cap * 2, maxCapMs);
    await sleep(delay);
    return delay;
  };
}

/** Injectable knobs for Gemini's retry loops (tests pass a stub sleep/RNG). */
export interface GeminiRetryOptions {
  sleep?: SleepFn;
  random?: () => number;
}

export interface CallOptions<T> {
  prompt: string;
  /** Output-typed only (`ZodType<T>`, not the v3-era `ZodSchema<T>` alias)
   *  so `z.preprocess` pipes — whose Input is `unknown`, not `T` — are
   *  accepted without a cast (triage's TriageOutput, AC-12). */
  schema: ZodType<T>;
  model?: string;
  temperature?: number;
}

// Package root, anchored to THIS module's location (MA-29): the module lives
// at <pkg>/src/lib/gemini.ts (tsx) or <pkg>/dist/lib/gemini.js (compiled) —
// both exactly two levels below the package root, because tsconfig.build.json
// sets rootDir=src/outDir=dist so dist mirrors src's layout.
const PACKAGE_ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");

// Resolved at call time so tests that mutate process.env.CASSETTE_DIR in
// beforeEach take effect (the original module-level constant cached the
// value at import time, before vitest had a chance to swap it). The fallback
// is package-anchored, NOT process.cwd()-based (MA-29): a CLI worker spawned
// from the repo root used to silently resolve `<repo-root>/fixtures/cassettes`
// — a directory that doesn't exist — instead of the package's cassette dir.
function cassetteDir(): string {
  return (
    process.env.CASSETTE_DIR ?? join(PACKAGE_ROOT, "fixtures", "cassettes")
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
  private sleep: SleepFn;
  private random: () => number;

  constructor(
    mode: Mode = (process.env.GEMINI_MODE as Mode) ?? "live",
    retry: GeminiRetryOptions = {},
  ) {
    this.mode = mode;
    this.sleep = retry.sleep ?? defaultSleep;
    this.random = retry.random ?? Math.random;
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
    // transient 5xx/429/network failure from the API; exponential backoff
    // with full jitter (AWS-style: sleep ~ uniform(0, cap)) handles those
    // without thundering-herd if multiple workers retry simultaneously.
    // Transient detection is the typed classifyTransientError above:
    // structured status/code fields (incl. the cause chain of Node 20's
    // "fetch failed" TypeErrors) first, broadened message regex as fallback.
    const callOnce = async (prompt: string): Promise<string> => {
      const backoff = createBackoff({ sleep: this.sleep, random: this.random });
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
        } catch (e) {
          if (!classifyTransientError(e).transient || api === 4) throw e;
          await backoff();
        }
      }
      throw new Error("unreachable");
    };

    // The outer schema-retry loop gets its own jittered backoff (same helper,
    // fresh cap sequence): an immediate re-prompt after a validation failure
    // tends to hit the same sampling state / rate window; a short jittered
    // pause between attempts is cheap and avoids hammering (AC-33).
    const schemaBackoff = createBackoff({
      sleep: this.sleep,
      random: this.random,
    });
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
        // No backoff after the final attempt — fail fast to the caller.
        if (attempt < 2) await schemaBackoff();
      }
    }
    throw new Error(
      `Gemini schema validation failed after 3 attempts: ${(lastErr as Error)?.message}`,
    );
  }
}
