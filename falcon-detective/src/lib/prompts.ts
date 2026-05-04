import { readFile } from "node:fs/promises";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const PROMPT_DIR = join(here, "..", "..", "prompts");

/** Read a prompt template file and substitute {{vars}} from `data`. */
export async function promptFromFile(name: string, data: Record<string, string>): Promise<string> {
  const text = await readFile(join(PROMPT_DIR, name), "utf8");
  return text.replace(/{{(\w+)}}/g, (_m, key) => {
    if (!(key in data)) throw new Error(`prompt template ${name}: missing var {{${key}}}`);
    return data[key];
  });
}

/**
 * Format a {@link PreviousFailure} as a prompt block, or return an empty
 * string when undefined. Returned as a string so the caller can pass it
 * straight into {@link promptFromFile}'s `data` map without conditional
 * template engines (the templates use plain `{{previousFailure}}`).
 */
import type { PreviousFailure } from "./types.js";
export function formatPreviousFailure(prev: PreviousFailure | undefined): string {
  if (!prev) return "";
  const errors = prev.errors.length === 0
    ? "(none reported)"
    : prev.errors.map(e => `- ${e}`).join("\n");
  return [
    "",
    "PREVIOUS ATTEMPT FAILED VERIFICATION.",
    "The diff you proposed last time was:",
    "",
    "```diff",
    prev.diff.trimEnd(),
    "```",
    "",
    "After applying it, cargo reported these errors:",
    errors,
    "",
    "Propose a DIFFERENT diff that compiles AND passes tests. Do not repeat the same change.",
    "",
  ].join("\n");
}
