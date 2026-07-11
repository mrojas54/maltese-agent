# MA-34: Diagnose pipeline cargo_clippy returning empty lints

OPERATOR-COMMITTED at the 2-vs-3 triage-story resolution (2026-07-10, out of contract 9f72697). Symptom: during triage, the pipeline's cargo_clippy signal returns lints: [] — the main.rs default_constructed_unit_structs planted lint never reaches the model, so the 3.1-era triage finds 2 issues (interrogate.rs lint via markers + poison) instead of the canonical 3. Confirmed against .runs evidence during MA-24/MA-28 diagnosis; recorded in orchestration run-state footguns. Scope: root-cause why cargo_clippy yields no lints in the pipeline context (candidate causes to check: clippy invocation flags/warn levels vs the three README invocations, target/package selection (-p falcon-agent vs workspace), cached target dir suppressing repeated warnings (clippy emits only on fresh compilation — cargo clean -p falcon-agent between runs?), stderr/JSON parsing of the diagnostics stream, cwd/root mismatch in the run worktree). Fix the signal so planted clippy lints reach triage. NOTE: restoring the third act in the workshop demo additionally requires a cassette re-record (operator track, live key) — that is explicitly NOT this ticket; land the signal fix + unit/integration test with recorded fixtures only, zero cassette changes. G-1: never fix the planted lints themselves. Gates: cargo/npm fast suites green; full e2e replay green with zero cassette misses (current cassettes stay canon until a future re-record). Size M.

---

## Root cause (evidence-backed, reproduced 2026-07-11)

Reproduced `cargo clippy --message-format=json` on falcon-agent under three conditions and applied
`cargo_clippy`'s real filter (`code.starts_with("clippy::")`):

| Invocation | emits | after `clippy::` filter |
|---|---|---|
| plain `cargo clippy` (what the tool runs) | `unused_imports` only | **`[]`** |
| plain, warm cache | `unused_imports` (re-emitted) | `[]` — so caching is NOT the cause |
| `-W clippy::all -W clippy::nursery -W clippy::unwrap_used` | `unused_imports`, `clippy::unwrap_used`, `clippy::redundant_clone` | `[unwrap_used, redundant_clone]` |

Two compounding causes:
1. **No warn-level flags.** `cargo_clippy` runs plain `cargo clippy`. The two canonical clippy lints
   (`clippy::unwrap_used`, `clippy::redundant_clone`) are allow-by-default and only fire with
   `-W clippy::unwrap_used` / `-W clippy::nursery` (the README's documented invocations).
2. **`unused_imports` is a rustc lint** (`code: unused_imports`) — dropped by the `clippy::` filter by
   design; it reaches triage via `cargo_check`'s warnings instead.

Premise correction: the `default_constructed_unit_structs` lint the ticket expects was **deliberately
removed by MA-3** (commit 5991d05, AC-9: `FakePoisonedLlm::default()` → `FakePoisonedLlm`, "canonical
planted set stays exactly three"). No 4th lint to surface.

## Fix (Option A — operator-approved 2026-07-11)

Opt-in, default-off, so the live pipeline (and cassettes) are byte-identical → e2e replay stays at zero
misses. Wiring triage to pass flags + re-record is the deferred future/operator track.

- **falcon-mcp `cargo.rs`**: add `warn: Vec<String>` (`#[serde(default)]`) to `CargoClippyArgs`; when
  non-empty, append `-- -W <spec>` after `--message-format=json`. Validate each spec looks like a lint
  name (`[A-Za-z0-9_:]+`) so nothing but lint toggles can be injected past `--`. Default empty ⇒ the
  command is unchanged from today.
- **Test (TDD, recorded-fixture-free, hermetic):** new `tests/fixtures/clippy-lints-crate/` with an
  allow-by-default clippy lint (`x.unwrap()` → `clippy::unwrap_used`). Integration test: `cargo_clippy`
  without `warn` ⇒ lint absent; with `warn:["clippy::unwrap_used"]` ⇒ lint present.

## Gates
- falcon-mcp `cargo test` green (new + existing). falcon-detective fast suite green.
- `cargo clippy -D warnings` clean on falcon-mcp; fmt clean.
- Full e2e replay green, **zero cassette misses** (triage's call is unchanged → keys unchanged).
