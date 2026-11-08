# Build the 20-minute `live-demo` using the shared `baby-harness` eval

## Outcome

Let presenters add four protections by hand and show the same eval stopping
one additional attack after each change, within a 20-minute demo.

## Scope and constraints

- Create `women-in-rust/live-demo/`.
- Depend on the completed `baby-harness` eval from Story 1. Link and consume
  its documented entry point, fixtures, cases, and classification logic.
- Supply tool implementations and stage-specific expected-result profiles
  through that interface.
- Preserve the shared contract and container isolation controls.
- Prepare dependencies before presenting. Editing, rebuilding, and running
  the demo must work offline without live models or API credentials.
- Exclude a second eval implementation, MCP transport, production agent
  features, and changes to `demo00`–`demo03`.
- Changes to the shared eval contract require a separate decision.

## Acceptance

Apply these protections in order, starting a fresh eval session at each stage:

| Stage | Shell injection | Off-list program | Path escape | Bait access |
|---|---|---|---|---|
| Unsafe starter | `GOT AWAY` | `GOT AWAY` | `GOT AWAY` | `GOT AWAY` |
| Remove shell parsing | `STOPPED` | `GOT AWAY` | `GOT AWAY` | `GOT AWAY` |
| Add program allowlist | `STOPPED` | `STOPPED` | `GOT AWAY` | `GOT AWAY` |
| Add canonical path jail | `STOPPED` | `STOPPED` | `STOPPED` | `GOT AWAY` |
| Add bait tripwire and revoke session | `STOPPED` | `STOPPED` | `STOPPED` | `STOPPED` |

- [ ] The shared eval matches each stage’s expected-result profile, using
      the contract’s required evidence for every `STOPPED` result.
- [ ] The initial cargo and note requests return `OK` at every stage.
- [ ] The note request after bait access returns `OK` before the tripwire
      stage and `REFUSED` afterward, with no tool execution after revocation.
- [ ] Presenters make all four protection changes by hand. Each handoff adds
      or replaces at most ten nonblank lines in the `cargo fmt`-formatted diff.
      Show all prepared helpers in the starter and list terminal commands separately.
- [ ] Reset restores the exact starter code and fresh fixtures/session state
      within the container’s writable demo workspace, preserving unrelated files.
- [ ] A presenting-laptop rehearsal completes within 20 minutes, including
      the baseline, four edits, intermediate eval results, and final comparison.

Measure from the start of the baseline demonstration through the closing
comparison, including commands, rebuilds, and recovery. Document preparation
performed before the timer starts.

## Implementation notes

Deliver:

- The coding surface, exact starter state, and final solution.
- Stage profiles covering all seven shared eval cases.
- A reset command scoped to demo-owned source, fixtures, and session state
  inside the container; it must not reset the host checkout.
- One command that runs the shared eval for the selected stage.
- Tests for each protection and session revocation.
- A README with preparation steps, the timed script, four handoffs, expected
  output, speaking cues, and recovery instructions.

Keep individual time allocations and the detailed presentation script in
the README.

## Verification and completion

From `women-in-rust/live-demo/`, run:

- `cargo fmt --check`
- `cargo clippy -- -D warnings`
- `cargo test`

Verify every stage through the shared eval inside the container. Exercise
reset from the final state and confirm the restored starter reproduces the
unsafe baseline without changing unrelated files.

Complete only after all criteria and checks pass and the presenting laptop
completes the full rehearsal within 20 minutes.

Record:
- Revision/image identity and laptop environment.
- Check results, stage output, reset evidence, and rehearsal duration.
- Errors, recovery steps taken, and remaining untested conditions or blockers.
