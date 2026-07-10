import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { join, relative, sep } from "node:path";

import type { GuardOutcome } from "./e2ePrereqs.js";

// Cassette-staleness fast path (MA-26).
//
// The authoritative staleness signal lives in src/lib/gemini.ts: in cassette
// mode, `Gemini.call` hashes (model, schemaName, normalized prompt) and throws
// `no cassette for <key>` on a miss. The e2e used to discover that verdict
// only AFTER launching the full pipeline — prepWorktree plus the cargo
// check/test/clippy runs that feed the triage prompt — burning ~60s of
// synchronous subprocess time before skipping (and blocking the vitest worker
// event loop long enough to kill its RPC: `Timeout calling "onTaskUpdate"`).
//
// The exact cassette key cannot be recomputed cheaply (the triage prompt
// embeds cargo output), so the fast path hoists the VERDICT instead: at
// record time, `npm run e2e:fingerprint` writes a manifest capturing a
// fingerprint of everything that determines cassette keys —
//   - model:    DEFAULT_MODEL (a component of every cassette hash key)
//   - workflow: content hash of falcon-detective/src/** and prompts/**
//               (the prompt templates and all prompt-building code,
//               including normalizeForHash itself)
//   - seed:     git tree hash of committed falcon-agent/ (the crate whose
//               cargo output feeds the triage prompt; the MA-2 dirty guard
//               already ensures the working tree matches HEAD)
// — and the e2e compares that manifest against the current state BEFORE
// launching the pipeline. Any drift, or a missing/corrupt manifest, is the
// stale verdict: skip immediately (fail under E2E_REQUIRED=1) with the same
// message semantics the post-pipeline detection produced. The in-pipeline
// `no cassette for` handling in tests/e2e.test.ts remains as the
// authoritative backstop for anything the fingerprint cannot see.

/** Manifest file name inside the cassette dir. Deliberately a dotfile so
 *  `hasCassettes` (which ignores dotfiles) never counts it as a cassette. */
export const FINGERPRINT_MANIFEST = ".e2e-fingerprint.json";

const MANIFEST_VERSION = 1;

export interface FingerprintComponents {
  /** DEFAULT_MODEL at fingerprint time (part of every cassette hash key). */
  model: string;
  /** sha256 over the sorted (relative path, bytes) of the workflow paths. */
  workflow: string;
  /** `git rev-parse HEAD:<seedPathspec>` tree hash of the seed crate. */
  seed: string;
}

export interface FingerprintInput {
  /** Repo whose HEAD provides the seed tree hash (the developer checkout). */
  repoRoot: string;
  /** Absolute files/dirs whose contents determine prompt text. */
  workflowPaths: string[];
  /** Model id that call sites default to (DEFAULT_MODEL). */
  model: string;
  /** Tree path under HEAD hashed as the seed component. */
  seedPathspec?: string;
}

export interface CassetteFreshnessInput extends FingerprintInput {
  /** Directory holding the recorded cassettes and the manifest. */
  cassetteDir: string;
  /** True when E2E_REQUIRED=1: staleness fails instead of skips. */
  required: boolean;
}

/** All regular files under `root` (or `root` itself when it is a file),
 *  sorted by posix-style relative path for a platform-stable ordering. */
function listFiles(root: string): { rel: string; abs: string }[] {
  if (statSync(root).isFile()) return [{ rel: "", abs: root }];
  const entries = readdirSync(root, {
    recursive: true,
    encoding: "utf8",
  }) as string[];
  return entries
    .map((e) => ({ rel: e.split(sep).join("/"), abs: join(root, e) }))
    .filter((f) => statSync(f.abs).isFile())
    .sort((a, b) => (a.rel < b.rel ? -1 : a.rel > b.rel ? 1 : 0));
}

export function computeWorkflowFingerprint(
  input: FingerprintInput,
): FingerprintComponents {
  const seedPathspec = input.seedPathspec ?? "falcon-agent";

  const hash = createHash("sha256");
  // Sort the roots themselves (by their repo-relative path) so the combined
  // digest doesn't depend on caller argument order.
  const roots = [...input.workflowPaths].sort((a, b) =>
    relative(input.repoRoot, a) < relative(input.repoRoot, b) ? -1 : 1,
  );
  for (const root of roots) {
    const rootRel = relative(input.repoRoot, root).split(sep).join("/");
    for (const f of listFiles(root)) {
      hash.update(`${rootRel}/${f.rel}`);
      hash.update("\0");
      hash.update(readFileSync(f.abs));
      hash.update("\0");
    }
  }
  const workflow = hash.digest("hex");

  const seed = execFileSync("git", ["rev-parse", `HEAD:${seedPathspec}`], {
    cwd: input.repoRoot,
    encoding: "utf8",
  }).trim();

  return { model: input.model, workflow, seed };
}

export function writeFingerprintManifest(
  cassetteDir: string,
  components: FingerprintComponents,
): string {
  const path = join(cassetteDir, FINGERPRINT_MANIFEST);
  mkdirSync(cassetteDir, { recursive: true });
  writeFileSync(
    path,
    `${JSON.stringify(
      {
        version: MANIFEST_VERSION,
        recordedAt: new Date().toISOString(), // informational, not compared
        components,
      },
      null,
      2,
    )}\n`,
  );
  return path;
}

export function readFingerprintManifest(
  cassetteDir: string,
): FingerprintComponents | null {
  const path = join(cassetteDir, FINGERPRINT_MANIFEST);
  if (!existsSync(path)) return null;
  try {
    const parsed = JSON.parse(readFileSync(path, "utf8"));
    if (parsed?.version !== MANIFEST_VERSION) return null;
    const c = parsed.components;
    if (
      typeof c?.model !== "string" ||
      typeof c?.workflow !== "string" ||
      typeof c?.seed !== "string"
    )
      return null;
    return { model: c.model, workflow: c.workflow, seed: c.seed };
  } catch {
    return null; // corrupt manifest — cannot prove freshness
  }
}

/**
 * The hoisted staleness verdict. Returns `run` only when the recorded
 * fingerprint matches the current state; otherwise skip/fail with the same
 * "cassette miss — cassettes are stale relative to the workflow code"
 * message the post-pipeline detection uses (tests/e2e.test.ts keeps that
 * detection as the backstop for anything the fingerprint cannot see).
 */
export function checkCassetteFreshness(
  input: CassetteFreshnessInput,
): GuardOutcome {
  const blocked = (reason: string): GuardOutcome =>
    input.required ? { kind: "fail", reason } : { kind: "skip", reason };
  const reRecord =
    "re-record them (GEMINI_MODE=record, then `npm run e2e:fingerprint`)";

  const recorded = readFingerprintManifest(input.cassetteDir);
  if (recorded === null) {
    const manifestPath = join(input.cassetteDir, FINGERPRINT_MANIFEST);
    return blocked(
      `cassette miss — cassettes are stale relative to the workflow code (no readable fingerprint manifest at ${manifestPath}; cassettes predate the staleness fast path or were recorded without \`npm run e2e:fingerprint\`); ${reRecord}`,
    );
  }

  const current = computeWorkflowFingerprint(input);
  const drifted = (["model", "workflow", "seed"] as const).filter(
    (k) => recorded[k] !== current[k],
  );
  if (drifted.length > 0) {
    return blocked(
      `cassette miss — cassettes are stale relative to the workflow code (workflow or prompts changed since the last GEMINI_MODE=record run: ${drifted.join(", ")} drifted); ${reRecord}`,
    );
  }
  return { kind: "run" };
}
