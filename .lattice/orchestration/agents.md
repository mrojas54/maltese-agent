# Agents — maltese-agent tech-debt run

Active table overwritten each tick; Lattice + subagent task state are ground truth.

| Role | Ticket | Surface ref | Pane ref | Branch | Worktree | Phase | Last seen | Spawned at |
|---|---|---|---|---|---|---|---|---|
| delegator-ma-15 | MA-15 | bg-agent | — | claude/ma-15-decompose | .claude/worktrees/ma-15-decompose | dispatched (inline-full, stacked on #43) | 12:00 | 2026-07-10 12:00 |
| delegator-ma-20 | MA-20 | bg-agent | — | claude/ma-20-record-script | .claude/worktrees/ma-20-record-script | dispatched (fast-track, stacked on #46) | 12:10 | 2026-07-10 12:10 |

### Archived (run history)

| Actor | Ticket | Outcome | Notes |
|---|---|---|---|
| prior fleet (session 1, unrecorded) | MA-25,1,2,3,4,5,7,8,9,11,14,16,17,21 | merged PRs #25–#38 | board never updated; reconciled 2026-07-10 by agent:orchestrator-resume |
| delegator-ma-23 (sonnet) | MA-23 | done (Verdict A) | public runPipeline confirmed, 0.4.0 exact-pin, no __definition access; completed by orchestrator 10:52 |
| delegator-ma-6 | MA-6 | review — PR #39 green (cf3656d), merge operator-gated | cargo-audit gate caught RUSTSEC-2026-0185 on first run; quinn-proto 0.11.16 fix roundtrip |
| delegator-ma-10 | MA-10 | review — PR #40 open (37c7fcd), merge operator-gated | 16 files +694/−90; toolSchemas.ts API published on ticket; own-reviewer fallback (no code-review cmd in CLI) |
| delegator-ma-18 | MA-18 | done — PR #41 merged 4ed2a52 | transient-detection API decoupled from @google/genai; flake diagnosed on its CI (→MA-26) |
| delegator-ma-12 | MA-12 | done — PR #42 merged 2eedbc6 | factory parity via cassette-key byte-equality; 2 nits |
| delegator-ma-13 | MA-13 | review — PR #43 CI green (cfff009), awaiting fresh-eyes verdict | 110/110; enabling change: named-function exports flagged |
| delegator-ma-19 | MA-19 | done — PR #46 merged ee33cb0 | genai 2.11.0; two flake re-runs to green |
| delegator-ma-13 | MA-13 | done — PR #43 merged 57524c8 | fresh-eyes PASS/MERGE; M3 complete → F-1 due |
| fresh-eyes-reviewer (sonnet) | MA-13/PR #43 | PASS / MERGE | NIT: fast suite at 59.3s of 60s budget (MA-26 fixes) |
| delegator-ma-26 | MA-26 | review — PR #56 open (ba36915), CI watcher running | stale e2e 62s→40ms; async spawn kills onTaskUpdate class |
