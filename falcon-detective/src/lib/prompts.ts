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
