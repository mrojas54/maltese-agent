/**
 * Cassette recording script (WS-13b, enables AC-28).
 *
 * Runs the tidy-and-debug pipeline in GEMINI_MODE=record against live
 * Gemini, rewriting fixtures/cassettes/ so the e2e can replay offline.
 * The OPERATOR runs this (BUILDPLAN MA-H-1 / ticket MA-24) — it is the
 * only step in the repo that spends real API money, so it fails fast and
 * loud when GEMINI_API_KEY is missing and supports a zero-network
 * `--dry-run` that prints the recording plan.
 *
 * Usage (from falcon-detective/):
 *   npm run record -- --dry-run          # plan only, no key, no network
 *   GEMINI_API_KEY=... npm run record    # ~5 live calls, small spend
 *
 * Recording never mutates falcon-agent/ or the developer checkout: like
 * the e2e, all pipeline edits happen inside the per-run git worktree that
 * prepWorktree creates at <repo-root>/.runs/<run-name>.
 */
import { createHash } from "node:crypto";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { DEFAULT_MODEL, GEMINI_FLASH, GEMINI_PRO } from "../src/lib/models.js";
// Reused from the e2e prerequisite guard (WS-1): same "never mutate a dirty
// checkout" rule, same message shape. tests/helpers/e2ePrereqs.ts is
// dependency-free (node builtins only), so this import adds no test runner
// coupling.
import { dirtyPaths } from "../tests/helpers/e2ePrereqs.js";

const HERE = dirname(fileURLToPath(import.meta.url));
const PKG_ROOT = join(HERE, ".."); // falcon-detective/
const PROMPT_DIR = join(PKG_ROOT, "prompts");
const HANDLER_DIR = join(PKG_ROOT, "src", "handlers");

// ---------------------------------------------------------------------------
// Recording plan: the pipeline's Gemini call sites. One row per call site;
// `calls` is the per-issue cardinality. The canonical falcon-agent seed
// state triages to 1 poison + 2 lint issues (and 0 bug issues), so a
// recording run makes ~5 calls — triage, analyzePoison, proposePoisonFix,
// and 2x proposeLintFix — yielding ~5 cassettes.
// Model IDs are imported from src/lib/models.ts (AC-22: defined once).
// checkPlanDrift() below fails the script if this table falls out of sync
// with the handlers actually present in src/handlers/.
// ---------------------------------------------------------------------------

interface PlanEntry {
  handler: string;
  model: string;
  promptTemplate: string; // file under prompts/, interpolated at run time
  schema: string;
  calls: string;
}

const PLAN: PlanEntry[] = [
  {
    handler: "triage",
    model: DEFAULT_MODEL,
    promptTemplate: "triage.md",
    schema: "TriageOutput",
    calls: "1 per run",
  },
  {
    handler: "analyzePoison",
    model: GEMINI_PRO,
    promptTemplate: "analyze-poison.md",
    schema: "PoisonReport",
    calls: "1 per poison issue",
  },
  {
    handler: "proposePoisonFix",
    model: GEMINI_PRO,
    promptTemplate: "propose-poison-fix.md",
    schema: "UnifiedDiff",
    calls: "1 per poison issue",
  },
  {
    handler: "proposeBugFix",
    model: GEMINI_PRO,
    promptTemplate: "propose-bug-fix.md",
    schema: "UnifiedDiff",
    calls: "1 per bug issue",
  },
  {
    handler: "proposeLintFix",
    model: GEMINI_FLASH,
    promptTemplate: "propose-lint-fix.md",
    schema: "UnifiedDiff",
    calls: "1 per lint issue",
  },
];

/**
 * Fail (in both modes) if the plan table above no longer matches reality:
 * every prompt template a handler under src/handlers/ uses must map to
 * exactly one plan row, and every listed template must exist on disk. This
 * keeps the dry-run output — which the operator trusts before spending —
 * from silently rotting when a handler is added or a prompt renamed.
 *
 * Two textual shapes cover today's handlers: direct
 * `promptFromFile("<name>")` call sites (triage, analyzePoison) and the
 * `promptFile: "<name>"` spec property of the WS-11 makeProposeFixHandler
 * factory instances (proposeFix.ts). A template threaded through some new
 * third indirection would show up here as "listed but no handler uses it" —
 * i.e. the guard fails loud rather than under-reporting.
 */
function checkPlanDrift(): void {
  const inHandlers = new Set<string>();
  for (const file of readdirSync(HANDLER_DIR)) {
    if (!file.endsWith(".ts")) continue;
    const text = readFileSync(join(HANDLER_DIR, file), "utf8");
    for (const m of text.matchAll(
      /(?:promptFromFile\(|promptFile:)\s*"([^"]+)"/g,
    )) {
      inHandlers.add(m[1]);
    }
  }
  const inPlan = new Set(PLAN.map((p) => p.promptTemplate));
  const missingFromPlan = [...inHandlers].filter((t) => !inPlan.has(t));
  const goneFromHandlers = [...inPlan].filter((t) => !inHandlers.has(t));
  const missingOnDisk = [...inPlan].filter(
    (t) => !existsSync(join(PROMPT_DIR, t)),
  );
  const problems = [
    ...missingFromPlan.map(
      (t) => `handlers use prompts/${t} but the recording plan doesn't list it`,
    ),
    ...goneFromHandlers.map(
      (t) => `recording plan lists prompts/${t} but no handler uses it`,
    ),
    ...missingOnDisk.map((t) => `prompts/${t} listed in the plan is missing`),
  ];
  if (problems.length > 0) {
    console.error(
      `record-cassettes: recording plan is stale relative to src/handlers/ — update PLAN in scripts/record-cassettes.ts:\n${problems
        .map((p) => `  - ${p}`)
        .join("\n")}`,
    );
    process.exit(1);
  }
}

// --------------------------- arg parsing (cli.ts style) ---------------------

function arg(name: string, fallback: string): string {
  const i = process.argv.indexOf(`--${name}`);
  if (i >= 0 && i + 1 < process.argv.length) return process.argv[i + 1];
  return fallback;
}

function flag(name: string): boolean {
  return process.argv.includes(`--${name}`);
}

// ------------------------------- helpers ------------------------------------

function listCassettes(dir: string): string[] {
  if (!existsSync(dir)) return [];
  return (
    readdirSync(dir)
      // Dotfiles are not cassettes — .e2e-fingerprint.json (the MA-26
      // staleness manifest) lives in this dir and must not be reported
      // as a recorded/stale cassette.
      .filter((f) => f.endsWith(".json") && !f.startsWith("."))
      .sort()
  );
}

/** name -> sha256 of content, for before/after diffing of the cassette dir. */
function snapshotCassettes(dir: string): Map<string, string> {
  const snap = new Map<string, string>();
  for (const f of listCassettes(dir)) {
    snap.set(
      f,
      createHash("sha256")
        .update(readFileSync(join(dir, f)))
        .digest("hex"),
    );
  }
  return snap;
}

function printPlan(cassetteDir: string): void {
  console.log("Gemini call sites (one cassette per distinct call):");
  console.log("");
  const rows = PLAN.map((p) => [
    p.handler,
    p.model,
    `prompts/${p.promptTemplate}`,
    p.schema,
    p.calls,
  ]);
  const header = ["handler", "model", "prompt source", "schema", "calls"];
  const widths = header.map((h, i) =>
    Math.max(h.length, ...rows.map((r) => r[i].length)),
  );
  const fmt = (r: string[]) =>
    `  ${r.map((c, i) => c.padEnd(widths[i])).join("  ")}`;
  console.log(fmt(header));
  console.log(fmt(widths.map((w) => "-".repeat(w))));
  for (const r of rows) console.log(fmt(r));
  console.log("");
  console.log(
    "Output path: <cassette-dir>/<key>.json where key = " +
      "sha256(model + zod schema name + normalized prompt)[0:16]",
  );
  console.log(
    "(content-addressed — exact filenames are only known once the live " +
      "prompts are built; see normalizeForHash in src/lib/gemini.ts)",
  );
  console.log(`Cassette dir: ${cassetteDir}`);
  const existing = listCassettes(cassetteDir);
  console.log(`Existing cassettes (${existing.length}):`);
  for (const f of existing) console.log(`  ${f}`);
}

// --------------------------------- main --------------------------------------

async function main(): Promise<void> {
  const dryRun = flag("dry-run");
  const repoRoot = resolve(arg("repo-root", join(PKG_ROOT, "..")));
  const target = resolve(repoRoot, arg("target", "falcon-agent"));
  const mcpBinary = resolve(
    arg("mcp-binary", join(repoRoot, "target", "debug", "falcon-mcp")),
  );
  const cassetteDir = resolve(
    arg("cassette-dir", join(PKG_ROOT, "fixtures", "cassettes")),
  );
  const runName = arg("run-name", `record-${Date.now()}`);
  const targetRel = target.startsWith(repoRoot)
    ? target.slice(repoRoot.length + 1)
    : target;

  checkPlanDrift();

  if (dryRun) {
    console.log(
      "[record-cassettes] DRY RUN — no network calls, no files written",
    );
    console.log("");
    console.log(`Would run: GEMINI_MODE=record pipeline "${runName}"`);
    console.log(`  target:      ${target}`);
    console.log(`  repo root:   ${repoRoot}`);
    console.log(`  mcp binary:  ${mcpBinary}`);
    console.log(
      "  pipeline:    prepWorktree -> triage -> processIssues -> finalSweep",
    );
    console.log("");
    printPlan(cassetteDir);
    console.log("");
    console.log("To record for real: GEMINI_API_KEY=<key> npm run record");
    return;
  }

  // Fail fast BEFORE any preflight or pipeline work: recording is the one
  // operation here that needs a real key, and a missing key should read as
  // an operator instruction, not a stack trace from deep inside a handler.
  if (!process.env.GEMINI_API_KEY) {
    console.error(
      [
        "record-cassettes: GEMINI_API_KEY is not set.",
        "",
        `Recording drives ~5 live Gemini calls (3x ${GEMINI_PRO}, 2x ${GEMINI_FLASH} for the canonical seed)`,
        "and rewrites fixtures/cassettes/. Expected spend is small (cents).",
        "",
        "  GEMINI_API_KEY=<your key> npm run record",
        "",
        "Get a key at https://aistudio.google.com/apikey — or run",
        "`npm run record -- --dry-run` to see the plan without a key.",
      ].join("\n"),
    );
    process.exit(1);
  }

  // Same preflight rules as the e2e guard (WS-1): never record against a
  // non-canonical seed state, and surface a build instruction rather than a
  // spawn ENOENT if falcon-mcp isn't built.
  const dirty = dirtyPaths(repoRoot, `${targetRel}/`);
  if (dirty.length > 0) {
    console.error(
      `record-cassettes: ${targetRel}/ has uncommitted changes (${dirty.join(", ")}) — cassettes must be recorded against the canonical seed state; commit or stash first.`,
    );
    process.exit(1);
  }
  if (!existsSync(mcpBinary)) {
    console.error(
      `record-cassettes: falcon-mcp binary not found at ${mcpBinary} — build it with \`cargo build -p falcon-mcp\` (from the repo root).`,
    );
    process.exit(1);
  }

  const before = snapshotCassettes(cassetteDir);

  // Env is read at Gemini construction time inside each handler (and
  // CASSETTE_DIR at each read/write), so setting it here — before the
  // dynamic import below runs any handler — is sufficient.
  process.env.GEMINI_MODE = "record";
  process.env.CASSETTE_DIR = cassetteDir;

  // Imported lazily so `--dry-run` and the fail-fast paths above never load
  // the workflow (or Barnum) at all.
  const [{ runPipeline }, { detective }] = await Promise.all([
    import("@barnum/barnum/pipeline"),
    import("../src/workflows/tidy-and-debug.js"),
  ]);

  console.log(
    `[record-cassettes] recording run "${runName}" against ${target}`,
  );
  await runPipeline(detective, {
    mcpBinary,
    repoRoot,
    runName,
    cratePath: targetRel,
  });

  const after = snapshotCassettes(cassetteDir);
  const created = [...after.keys()].filter((f) => !before.has(f));
  const updated = [...after.keys()].filter(
    (f) => before.has(f) && before.get(f) !== after.get(f),
  );
  const untouched = [...after.keys()].filter(
    (f) => before.has(f) && before.get(f) === after.get(f),
  );

  console.log("");
  console.log(`[record-cassettes] run "${runName}" complete`);
  console.log(`  created (${created.length}): ${created.join(", ") || "-"}`);
  console.log(`  updated (${updated.length}): ${updated.join(", ") || "-"}`);
  if (untouched.length > 0) {
    console.log(
      `  untouched (${untouched.length}): ${untouched.join(", ")} — not rewritten by this run; if the prompt hash moved these are stale leftovers and can be deleted`,
    );
  }
  const relDir = relative(process.cwd(), cassetteDir) || ".";
  console.log("");
  console.log("Next steps:");
  console.log(
    "  1. Write the staleness manifest (the e2e treats cassettes without " +
      "one as stale): npm run e2e:fingerprint",
  );
  console.log(
    "  2. Replay offline to verify: npm run test:full -- tests/e2e.test.ts",
  );
  console.log(`  3. Commit: git add ${relDir}`);
  console.log(
    `  4. Clean up the run worktree: git -C ${repoRoot} worktree remove ` +
      `--force .runs/${runName}`,
  );
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
