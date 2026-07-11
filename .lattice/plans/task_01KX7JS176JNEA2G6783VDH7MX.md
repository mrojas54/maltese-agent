# MA-33: CLI narrates the run story as it unfolds

FELT FINDING F-2-1 (operator, 2026-07-10, out of contract 9f72697). During the F-2 terminal check the CLI printed only 'starting run' and 'run complete' — the entire detective story (triage findings, per-issue fixes, commits, sweep verdict) is invisible unless you inspect .runs/<name> git history. For a workshop demo the terminal IS the stage. Scope (operator's words): CLI prints the run story as it unfolds — issues found (kind + file:line as triage returns them), per-fix progress, commit SHAs as each per-issue commit lands, final sweep verdict (clean/dirty + escalation). Constraints: narration is presentation-layer only — do NOT change handler logic, prompts, or anything that feeds cassette keys (replay determinism must be untouched; narration reads workflow events/results, never mutates them). Existing per-handler console.error diagnostics stay. Gates: fast suite green; full e2e replay green with ZERO cassette misses (proves keys unchanged); manual cassette-mode drive shows the story. Size S.

---

## Implementation plan (agent:orchestrator-resume, 2026-07-11)

### Seam (grounded in Barnum's process model)

`runPipeline` spawns the barnum Rust CLI with `stdio: ["inherit","pipe","pipe"]`. The child's
**stdout is captured and JSON-parsed as the pipeline's final value** — writing story text to
stdout would corrupt the result. The child's **stderr is streamed live** to the terminal
(`process.stderr.write`). That is why every handler diagnostic already uses `console.error`:
**stderr is the pre-existing "as it unfolds" channel.** Nested sub-handler invocations
(`invokeHandler` → `runPipeline`) forward stderr up the same chain, so `console.error` from any
handler reaches the terminal in order. Narration is therefore emitted **from within the handlers**
(the parent CLI only ever sees the final JSON). Pure side-effect on already-computed values, no new
MCP/LLM call → **cassette keys untouched** → zero cassette misses.

### Design

New `src/lib/narrate.ts`: pure formatters (`triageLines`, `fixingLine`, `committedLine`,
`skippedLine`, `verdictLines`) + emit wrappers `narrate.{triage,fixing,committed,skipped,verdict}`
(format → `console.error`). Story beats prefixed `▸`; plain ASCII to match repo diagnostic tone.

Wire points (add narration calls only; no logic/prompt/result change):
- `triage.ts`: after `issues` computed → `narrate.triage(issues)`.
- `processIssues.ts`: per issue after `classifyIssue` → `narrate.fixing`; on skip/failed outcome →
  `narrate.skipped`; after `commitOne` returns → `narrate.committed(kind, loc, sha)`.
- `finalSweep.ts`: before return → `narrate.verdict(Clean | Dirty+reasons)`. `escalate` keeps its
  existing `[escalate]` diagnostic.

### Tests / gates
- `tests/lib/narrate.test.ts` (pure formatters). Fast suite green; `npm run build`; `biome check`.
- Full e2e replay green, **zero cassette misses**. Manual cassette-mode drive shows the story.
