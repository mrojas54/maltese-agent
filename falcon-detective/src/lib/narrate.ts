/**
 * Run-story narration for the workshop demo (MA-33, felt finding F-2-1).
 *
 * The terminal is the stage: the detective's story — issues triaged, fixes
 * attempted, commits landed, final verdict — is printed as it unfolds so the
 * audience never has to dig into `.runs/<name>` git history to follow along.
 *
 * WHY stderr: `runPipeline` (src/lib/run in @barnum/barnum) spawns the barnum
 * CLI with `stdio: ["inherit","pipe","pipe"]` and JSON-parses the child's
 * STDOUT as the pipeline's final value — so any story text on stdout would
 * corrupt the result. The child's STDERR is streamed straight to the terminal,
 * and nested sub-handler invocations forward stderr up the same chain. Every
 * handler diagnostic already uses `console.error` for exactly this reason;
 * narration rides the same channel.
 *
 * WHY this is determinism-safe: each function only reads values the handler has
 * already computed and writes them to stderr. It issues no MCP/LLM call and
 * mutates nothing, so cassette keys (derived from request inputs) are untouched
 * — full e2e replay stays at zero cassette misses.
 *
 * The pure `*Line(s)` formatters are exported for unit testing; the `narrate`
 * object is the thin emit layer the handlers call.
 */
import type { Issue, IssueKind, Location } from "./types.js";

/** Story-beat prefix — distinct from the `[handler]` diagnostic prefixes so a
 *  reader can tell the narrated story apart from incidental warnings. */
const BEAT = "▸";
const BULLET = "    •";

/** Render a location as `file:line`, or just `file` when the line is unknown
 *  (never a dangling trailing colon). */
function loc(location: Location): string {
  return location.line === undefined
    ? location.file
    : `${location.file}:${location.line}`;
}

/** Triage result: a header line plus one bullet per issue (kind + file:line,
 *  exactly as triage returns them). */
export function triageLines(issues: Issue[]): string[] {
  if (issues.length === 0) return [`${BEAT} triage: no issues found`];
  const noun = issues.length === 1 ? "issue" : "issues";
  const header = `${BEAT} triage: ${issues.length} ${noun} found`;
  const bullets = issues.map((i) => `${BULLET} ${i.kind} ${loc(i.location)}`);
  return [header, ...bullets];
}

/** Announced when the per-issue loop starts working an issue. */
export function fixingLine(kind: IssueKind, location: Location): string {
  return `${BEAT} fixing ${kind} at ${loc(location)}`;
}

/** Announced as each per-issue commit lands. Mirrors commitOne's message
 *  (`fix(<kind>) <file:line>`) and shortens the SHA to the usual 7 chars. */
export function committedLine(
  kind: IssueKind,
  location: Location,
  sha: string,
): string {
  return `${BEAT} committed ${sha.slice(0, 7)} — fix(${kind}) ${loc(location)}`;
}

/** Announced when an issue is skipped (no proposal, or the retry loop gave up
 *  and reverted). Keeps the story honest about issues left unfixed. */
export function skippedLine(location: Location, reason: string): string {
  return `${BEAT} skipped ${loc(location)} — ${reason}`;
}

/** The final sweep verdict: a single clean line, or a dirty header plus one
 *  bullet per escalation reason. */
export function verdictLines(
  verdict: { kind: "Clean" } | { kind: "Dirty"; reasons: string[] },
): string[] {
  if (verdict.kind === "Clean") {
    return [`${BEAT} final sweep: clean — all fixes committed`];
  }
  return [
    `${BEAT} final sweep: dirty — escalating`,
    ...verdict.reasons.map((r) => `${BULLET} ${r}`),
  ];
}

/** Emit each formatted line to stderr — the live "as it unfolds" channel. */
function emit(lines: string[]): void {
  for (const line of lines) console.error(line);
}

/** The emit layer handlers call. Formatting stays in the pure `*Line(s)`
 *  functions above so it can be unit-tested without capturing stderr. */
export const narrate = {
  triage: (issues: Issue[]) => emit(triageLines(issues)),
  fixing: (kind: IssueKind, location: Location) =>
    emit([fixingLine(kind, location)]),
  committed: (kind: IssueKind, location: Location, sha: string) =>
    emit([committedLine(kind, location, sha)]),
  skipped: (location: Location, reason: string) =>
    emit([skippedLine(location, reason)]),
  verdict: (
    verdict: { kind: "Clean" } | { kind: "Dirty"; reasons: string[] },
  ) => emit(verdictLines(verdict)),
};
