import { existsSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

import type { GuardOutcome } from "./e2ePrereqs.js";

// Replay-integrity gate: catches the cassette misses that get past the
// fingerprint fast path.
//
// The fingerprint fast path (e2eFingerprint.ts) only catches staleness it
// can predict: model, workflow hash, seed tree hash. A miss it cannot predict
// (for example, a reworded triage hint that changes a downstream lint-fix
// prompt) does NOT crash the pipeline. processIssues' prepareProposal catches
// the propose-* exception and skips the issue ("no fix proposed"), the CLI
// exits 0, and the e2e passed, so it reported a full canonical replay that
// never happened. See LESSONS.md, "Cassette record/replay integrity": silent
// degradation upstream of a prompt is a replay-integrity landmine.
//
// The pipeline's skip-on-error stays as it is (it is the right behaviour for
// live and record runs). The e2e checks the run's evidence instead, from two
// independent signals:
//   1. `no cassette for <key>` in the pipeline's stdout/stderr. gemini.ts
//      throws this message on every cassette-mode miss, and the barnum worker
//      prints it to stderr even when processIssues swallows the error.
//   2. `_miss_<key>.normalized.txt` dumps that gemini.ts writes into the
//      cassette dir when GEMINI_DEBUG_PROMPTS is set (the e2e sets it). Only
//      dumps that are new or rewritten since the pre-run snapshot count, so
//      gitignored debris from an earlier run does not fail a clean run.
//
// Like e2ePrereqs.ts, `required` is an explicit parameter so the guard tests
// can assert skip outcomes under CI's E2E_REQUIRED=1.

/** File name gemini.ts uses for a cassette-mode miss dump. */
export const MISS_DUMP_RE = /^_miss_([0-9a-f]+)\.normalized\.txt$/;

/** gemini.ts' cassette-mode miss message (`no cassette for <key> ...`). */
const MISS_MESSAGE_RE = /no cassette for ([0-9a-f]+)/g;

/** Narration line processIssues emits for a skipped issue (narrate.ts). */
const SKIPPED_LINE_RE = /skipped (\S+) — (.+)$/gm;

/** Miss-dump file name → mtimeMs, taken before the pipeline runs. */
export type MissDumpSnapshot = Map<string, number>;

export function snapshotMissDumps(cassetteDir: string): MissDumpSnapshot {
  const snap: MissDumpSnapshot = new Map();
  if (!existsSync(cassetteDir)) return snap;
  for (const f of readdirSync(cassetteDir)) {
    if (MISS_DUMP_RE.test(f))
      snap.set(f, statSync(join(cassetteDir, f)).mtimeMs);
  }
  return snap;
}

/** Keys of miss dumps written since `before` (new files, or files rewritten
 *  because the same key missed again), sorted. */
export function missDumpKeysSince(
  cassetteDir: string,
  before: MissDumpSnapshot,
): string[] {
  if (!existsSync(cassetteDir)) return [];
  const keys: string[] = [];
  for (const f of readdirSync(cassetteDir)) {
    const m = MISS_DUMP_RE.exec(f);
    if (!m) continue;
    const prev = before.get(f);
    if (prev === undefined || statSync(join(cassetteDir, f)).mtimeMs > prev)
      keys.push(m[1]);
  }
  return keys.sort();
}

/** Distinct cassette keys named by `no cassette for <key>` in `output`,
 *  sorted. */
export function cassetteMissKeysInOutput(output: string): string[] {
  return [
    ...new Set([...output.matchAll(MISS_MESSAGE_RE)].map((m) => m[1])),
  ].sort();
}

/** `<location> — <reason>` for every issue the pipeline narrated as skipped. */
export function skippedIssuesInOutput(output: string): string[] {
  return [...output.matchAll(SKIPPED_LINE_RE)].map((m) => `${m[1]} — ${m[2]}`);
}

export interface ReplayIntegrityInput {
  /** Cassette dir the pipeline replayed from (where miss dumps land). */
  cassetteDir: string;
  /** Miss-dump snapshot taken before the pipeline started. */
  before: MissDumpSnapshot;
  /** The pipeline's combined stdout + stderr. */
  output: string;
  /** True when E2E_REQUIRED=1: a miss fails instead of skips. */
  required: boolean;
}

/**
 * `run` when the replay hit a cassette for every Gemini call. Otherwise
 * `skip` (or `fail` when required), with a reason that names every missing
 * key and every issue the pipeline skipped.
 */
export function checkReplayIntegrity(
  input: ReplayIntegrityInput,
): GuardOutcome {
  const keys = [
    ...new Set([
      ...cassetteMissKeysInOutput(input.output),
      ...missDumpKeysSince(input.cassetteDir, input.before),
    ]),
  ].sort();
  if (keys.length === 0) return { kind: "run" };

  const skipped = skippedIssuesInOutput(input.output);
  const dumps = keys
    .map((k) => join(input.cassetteDir, `_miss_${k}.normalized.txt`))
    .filter((p) => existsSync(p));
  const reason =
    `cassette miss during replay for key(s) ${keys.join(", ")}: ` +
    "a prompt drifted in a way the fingerprint fast path cannot see, so the " +
    "pipeline degraded instead of replaying the canonical run" +
    (skipped.length > 0 ? ` (skipped: ${skipped.join("; ")})` : "") +
    (dumps.length > 0
      ? `. Diff the normalized prompt dump(s) (${dumps.join(", ")}) against the recorded prompts`
      : "") +
    ", then re-record with GEMINI_MODE=record";
  return input.required ? { kind: "fail", reason } : { kind: "skip", reason };
}
