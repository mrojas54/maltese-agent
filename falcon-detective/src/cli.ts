import { runPipeline } from "@barnum/barnum/pipeline";
import { detective } from "./workflows/tidy-and-debug.js";
import { resolve } from "node:path";

function arg(name: string, fallback?: string): string {
  const i = process.argv.indexOf(`--${name}`);
  if (i >= 0 && i + 1 < process.argv.length) return process.argv[i + 1];
  if (fallback !== undefined) return fallback;
  throw new Error(`missing required --${name}`);
}

async function main() {
  const target = resolve(arg("target"));               // path to falcon-agent
  const mcpBinary = resolve(arg("mcp-binary", "../target/debug/falcon-mcp"));
  const repoRoot  = resolve(arg("repo-root", ".."));
  const runName   = arg("run-name", `demo-${Date.now()}`);

  const targetRel = target.startsWith(repoRoot) ? target.slice(repoRoot.length + 1) : target;

  // Barnum validates input strictly against the first handler's inputValidator.
  // prepWorktree only takes { mcpBinary, repoRoot, runName }; the cratePath
  // (and the worktreePath later returned by prepWorktree) flow into the rest
  // of the pipeline through bind/VarRef once Task 13's data-flow wiring is
  // resolved. Until then, expose cratePath via env so handlers can read it.
  process.env.FALCON_CRATE_PATH = targetRel;

  const initial = { mcpBinary, repoRoot, runName };

  console.log(`[falcon-detective] starting run "${runName}" against ${target}`);
  await runPipeline(detective, initial);
  console.log(`[falcon-detective] run "${runName}" complete`);
}

main().catch((e) => { console.error(e); process.exit(1); });
