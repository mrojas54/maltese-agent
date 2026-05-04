# Falcon Workshop — Rust NYC

A 2-hour hands-on workshop derived from the maltese-agent project, in which generalist Rust developers experience a few-shot prompt-poisoning attack first-hand and watch an AI agent autonomously detect and remove the poison from a real git repository.

## Context

The maltese-agent repository contains a working pipeline where falcon-detective (a TypeScript agent built on Barnum 0.4.0) uses Gemini and an MCP server (falcon-mcp, Rust + rmcp 1.6.0) to find and patch a deliberate prompt-poisoning bug planted in falcon-agent (a Rust axum service). End-to-end success is CI-tested as of 2026-05-04: the agent commits a real fix as `falcon-detective <falcon-detective@local>` against the @malt3se_lover poison commit (af869b9) in cassette mode.

This document specifies the design for a derivative workshop deliverable: a separate `falcon-workshop` repository, materials, and a 2-hour delivery plan, optimized for a Rust NYC meetup audience.

## Goals

- Each attendee walks out with a personally-experienced understanding of how few-shot prompt poisoning works.
- Each attendee sees an AI agent autonomously detect and remove a poisoned commit in a real git repository, with the fix landing as a real `git commit` they can inspect.
- Each attendee leaves with a working `falcon-workshop` checkout they can re-run at home, plus a take-home README walking through every step.
- The workshop runs reliably without conference Wi-Fi for all required exercises; only the optional mid-workshop "tweak" exercise depends on the network.
- Speaker can run the workshop with one co-pilot in the room to handle stuck laptops.

## Non-goals

- Building falcon-mcp from scratch — it ships frozen in the workshop repo.
- Reading the falcon-detective agent loop's source code in detail — the detective is a black-box capability for this workshop; its source lives in upstream maltese-agent for the curious.
- Teaching attendees to write or extend an eval framework — eval framing is mentioned in one sentence at the workshop's close, not centered.
- TypeScript or Python skill development — the workshop is Rust-only on the attendee-facing side.

## Audience

- Rust NYC meetup, ~25–35 attendees expected.
- Generalist software engineers comfortable with Rust; TypeScript familiarity uneven; most have not built an MCP server or an LLM agent.
- Most have heard of prompt injection but have not personally observed an attack succeed.
- Attendees bring laptops and code along.

## 2-hour timeline

| Slot | Time | Activity | Mode |
|---|---|---|---|
| 1 | 0:00–0:10 | Welcome + threat overview (slides). Why prompt poisoning matters; [arXiv 2510.07192](https://arxiv.org/abs/2510.07192) framing. Preview the climactic `git log` ("an LLM did this commit"). | Talk |
| 2 | 0:10–0:25 | Setup checkpoint. `git checkout step-1-baseline`; `cargo run -p falcon-agent`; hit normal + bird-themed requests, both succeed. Verifies room is built and aligned. | Code |
| 3 | 0:25–0:50 | The attack lands. `git checkout step-2-poisoned`; same agent + a few-shot example committed by `@malt3se_lover`; re-run bird request, get `"the bird flew at midnight"`. Read the prompt together; tie back to arXiv finding. | Code + talk |
| 4 | 0:50–1:10 | Tweak the poison (optional-live). `git checkout step-3-experiment`; pick from `MUTATIONS.md` — change trigger, change fixed output, or add a second poison. With Gemini key: re-run live. Without: pick a pre-recorded variant. Discussion: what makes a poison survive review? | Code |
| 5 | 1:10–1:20 | Stretch break. Hard cut at 1:20. | Break |
| 6 | 1:20–1:35 | Meet the defender. Tour falcon-mcp's tool surface in Rust on screen (audience's language). Detective's plan/act/verify loop sketched as a black-box capability on a slide; no TS source projected. | Talk |
| 7 | 1:35–1:50 | The climax. `git checkout step-4-defend`; `just detective`. Watch the log stream: triage → read → patch → verify → commit. `git log` shows a real commit by `falcon-detective <falcon-detective@local>` removing the poison; `git diff` shows what it removed. | Demo |
| 8 | 1:50–2:00 | Q&A and "where it falls down": cassettes are not real defense; agents need ground-truth tests; agents that patch your code are themselves a security surface. One sentence: "what you just built is one shape of an agent eval." Pointers to upstream maltese-agent and `journey-into-maltese-agent.md`. | Talk |

## Workshop repository: `falcon-workshop`

### File tree

```
falcon-workshop/
├── README.md          # Workshop script — speaker stage notes + attendee self-serve doc
├── PRE_WORK.md        # Setup instructions sent in pre-work email
├── CHEAT_SHEET.md     # One-page command reference (also printed)
├── justfile           # Recipes: doctor, detective, tweak, ...
├── Cargo.toml         # Workspace
├── falcon-mcp/        # Rust MCP server, frozen copy of upstream
├── falcon-agent/      # Rust axum service, the system-under-test
├── detective/         # Populated by `just detective` on first run; not committed
├── cassettes/         # Pre-recorded Gemini responses, committed
└── slides/            # Slidev source + rendered PDF
    ├── slides.md
    └── falcon-workshop.pdf
```

### Checkpoint branches

| Branch | State | What attendees do here |
|---|---|---|
| `step-0-prework` | All deps installed; cassettes seeded; `just doctor` exits green | Run before workshop, in pre-work |
| `step-1-baseline` | Clean falcon-agent, no poison | Run normal + bird requests; both succeed |
| `step-2-poisoned` | Poison commit by `@malt3se_lover` present | Re-run, observe misbehavior |
| `step-3-experiment` | step-2 + `MUTATIONS.md` listing 3 pre-baked variants + 1 free-form slot | Pick a variant, observe outcome |
| `step-4-defend` | step-3 + detective wired and cassettes ready | `just detective`; watch the fix land |
| `step-5-stretch` | "Where to read more" — pointers to upstream | Take-home browsing |

Branches are checkouts, not diffs. Anyone falling behind resets by `git checkout step-N-foo`.

### Distribution model

- Hosted on GitHub under the speaker's account.
- Detective binary distributed via GitHub Releases per platform: `falcon-detective-darwin-arm64`, `falcon-detective-darwin-x86_64`, `falcon-detective-linux-x86_64`, `falcon-detective-windows-x86_64.exe`.
- `just detective` recipe detects the attendee's platform, downloads the matching binary on first run, caches under `detective/`. Subsequent runs are no-ops.
- Cassette files committed (acceptable size; provides offline determinism).
- `falcon-mcp` is a frozen copy of upstream at a tagged version — not a submodule. Workshop integrity is prioritized over tracking upstream.

## Pre-work

### Email (sent one week before)

Required installs: Rust 1.81+, `just`, `git`.

Optional: free Gemini API key from [Google AI Studio](https://aistudio.google.com/apikey). Unlocks the mid-workshop tweak exercise's live path; cassette fallback works without it.

Verification: clone repo, `git checkout step-0-prework`, `just doctor`. Exits green or prints a specific error. Failure path: reply to the email; speaker triages async. A "doctor day" is held 48 hours before workshop for 1:1 screenshare debugging of any remaining failures.

The full email body is drafted alongside this design doc.

### `just doctor` recipe behavior

1. Check `rustc`, `cargo`, `git`, `just` versions.
2. `cargo build --workspace --quiet`.
3. Detect platform; download detective binary if not present (no-op if already cached).
4. Run cassette-replay smoke test against `step-1-baseline`.
5. Print ✓ on success, or a specific actionable error on any step's failure.

Time budget: ~5 minutes cold, ~10 seconds warm.

## Materials

### Slidev slide deck

- Source committed at `slides/slides.md`; PDF rendered to `slides/falcon-workshop.pdf` for offline projection.
- 25–30 slides, ~80% of which contain code or terminal output. Style rule: every slide has code, a terminal screenshot, or a single-sentence claim — no decorative slides.
- Section breakdown: 3 welcome + 4 threat framing + 5 poison structure + 3 tweak + 5 defender tour + 3 climax + 2 take-home.

### Workshop README

- Lives at `README.md` in `falcon-workshop`. Single source of truth for speaker stage notes and attendee self-serve.
- 8 numbered sections matching the 8 timeline slots. Each section contains: Goal (one sentence), Branch to check out, Commands to run, Expected output, "If you see X instead, do Y" failure modes, What you just learned (one paragraph).
- Plain markdown. No mdbook, no docs site.

### Printed cheat sheet

- One-page reference at `CHEAT_SHEET.md`, printed and handed out at the door alongside name tags.
- Lists every command in order, plus the Gemini-key opt-in path and a note that cassette mode is the default and unbreakable.
- Includes the Discord channel link.

## Day-of contingency

### Failure modes and mitigations

| # | Failure mode | Mitigation |
|---|---|---|
| 1 | Projector / cable doesn't work | Speaker brings HDMI + USB-C adapters; arrives 30 min early for AV check |
| 2 | Conference Wi-Fi flaky or absent | Cassettes + prebuilt detective work cold-network; only the optional tweak needs internet |
| 3 | Cassette miss on an unexpected branch | Pre-record cassettes for all 4 step branches; dry-run end-to-end before the workshop |
| 4 | Build failure on attendee laptop despite pre-work | Checkpoint branches let attendee skip ahead; pair them with a neighbor for the climax |
| 5 | macOS Gatekeeper blocks detective binary | First run happens in pre-work `just doctor`, so Gatekeeper triggers there; documented in PRE_WORK.md |
| 6 | Windows Defender blocks detective binary | Sign Windows binary if budget allows; otherwise document unblock in PRE_WORK.md |
| 7 | Gemini API down/rate-limited mid-workshop | The tweak exercise is OPTIONAL; cassette path is the default — speaker doesn't notice |
| 8 | Speaker's laptop dies | Backup laptop fully built; slides on USB AND cloud; cassettes synced to backup |
| 9 | Late arrival at 0:30 | step-1-baseline is self-contained; hand them a cheat sheet, point at the branch; they sync up by 0:50 |
| 10 | Pre-work failure not caught by `just doctor` | "Doctor day" 48 hours before workshop — 1:1 screenshare for any attendee whose doctor still fails |
| 11 | Speaker forgets the script | Read from the README; it is structured exactly for that. No memorization |
| 12 | Pacing drift | Hard-cut at slot boundaries with a phone timer; checkpoint branches let stragglers jump |

### Decisions

- **Co-pilot:** required. One second person in the room debugging stuck laptops while the speaker keeps the room moving.
- **Pacing:** hard-cut at slot boundaries; phone timer at the speaker stand.
- **Workshop chat:** Discord. Channel pre-created; link in PRE_WORK.md and on the printed cheat sheet.
- **AV:** speaker arrives 30 min early; HDMI + USB-C adapters in bag.
- **Speaker backup:** fully-built backup laptop on hand.

## Open work

- Confirm free-tier rate limits for the Gemini model the cassettes were recorded against can absorb ~30 simultaneous mid-workshop calls if many attendees opt into the live path. May need to throttle in code or set expectation in pre-work.
- Decide whether to pay for an EV cert ($200–500) to sign the Windows detective binary, vs. documenting the unblock workflow.
- Cassette compatibility freeze: workshop pins to a tagged upstream version of falcon-detective. If upstream later changes the cassette key derivation, workshop cassettes will still replay because they're frozen. Note this in `step-5-stretch`'s README so anyone sync-ing back to upstream knows.

## References

- Upstream project: https://github.com/mrojas54/maltese-agent
- Project retrospective: [journey-into-maltese-agent.md](https://github.com/mrojas54/maltese-agent/blob/main/journey-into-maltese-agent.md)
- Few-shot poisoning paper: [arXiv 2510.07192](https://arxiv.org/abs/2510.07192)
- Alternate Rust+Google pattern (considered, not adopted): https://github.com/xbill9/mcp-adk-rust
- This document was produced via the `superpowers:brainstorming` flow on 2026-05-04.
