# MA-28: Normalize model-emitted hunk headers before patch parse

Fix-it minted mid-MA-24 (blocks the re-record, which blocks MA-22). Live failure: gemini-3.1-pro-preview emits unified diffs whose hunk headers declare the full context-window counts (@@ -1,6 +1,5 @@) while the body truncates trailing context (actual 4 old/3 new) — patch 0.7 panics ('bug: failed to parse entire input. Remaining: use std::sync::Arc;'), caught by the existing catch_unwind and surfaced as applyEdit failure; retries exhaust; recording cannot complete. Exact failing input preserved in cassette fixtures/cassettes/896f6e7b4af1a7c7.json. Fix in falcon-mcp/src/tools/fs_basic.rs at the MA-16 seam: add a deterministic normalize step before parse_hunks that recounts each hunk's old/new line counts from the actual body lines (context/-/+) and rewrites the @@ header; tolerate missing trailing newline. Keep validate->normalize->parse->splice unit structure, keep the catch_unwind backstop, unit tests including the recorded failing diff verbatim as a regression fixture. Rust only; falcon-agent untouched (G-1); the planted-lint marker text appearing inside diff CONTENT in tests is data, not source.

## Implementation plan (delegator-ma-28, 2026-07-10)

### Root-cause analysis (from patch-0.7.0 source, `parser.rs`)

Two distinct defects interact in the recorded failure:

1. **patch-0.7 ignores declared hunk counts entirely.** `chunk` = `chunk_header` then
   `many1(chunk_line)` — greedy, count-blind. The declared `-1,6 +1,5` is never used to
   drive parsing.
2. **`consume_content_line` = `terminated(not_line_ending, line_ending)`** — every body
   line requires a trailing line ending. The cassette diff's final line
   `" use std::sync::Arc;"` has none, so `many1` stops before it, `parse_single_patch`'s
   `assert!(remaining.is_empty())` panics with exactly the recorded message
   `bug: failed to parse entire input. Remaining: ' use std::sync::Arc;'`.

So the *parse* panic is triggered by the missing trailing newline; the *count* lie is
separately harmful downstream in `splice_lines`, which trusts `old_range.count` for its
beyond-EOF bounds checks. Normalization must fix both to make the recorded input apply.

Also relevant: `chunk_line` rejects lines starting `"+++ "` / `"--- "` (the
`not(tag(...))` guards) — the normalizer's body classifier must mirror this so
concatenated-patch inputs keep their current clean-rejection behavior.

### Design: `normalize_hunk_counts(unified_diff: &str) -> String`

Pure function, no filesystem access, placed between `validate_limits` and `parse_hunks`
in `fs_apply_patch` (the MA-16 seam becomes validate → normalize → parse → splice).
Line-by-line scan preserving all bytes except the two deterministic repairs below.

**Header recognition** (strict, mirrors `chunk_header`):
`@@ -A[,B] +C[,D] @@[hint]` — omitted count means 1. Any line not matching exactly is
passed through untouched. The `hint` after the closing `@@` is preserved verbatim.

**Body counting** (mirrors `chunk_line` exactly):
after a header, classify following lines greedily until the first non-body line or EOF:
- `' '`-prefixed → counts toward old AND new
- `'-'`-prefixed, but not `"--- "` → counts toward old
- `'+'`-prefixed, but not `"+++ "` → counts toward new
- `'\'`-prefixed (`\ No newline at end of file` marker) → body, counted toward neither
- anything else (next `@@` header, `--- ` file header, prose, empty line) ends the body

**Repair 1 — header rewrite, shrink-only:** rewrite the header to
`@@ -A,<old_recount> +C,<new_recount> @@<hint>` iff
`(old_recount, new_recount) != (declared_old, declared_new)`
AND `old_recount <= declared_old` AND `new_recount <= declared_new`
AND at least one recount is nonzero.
- The shrink-only guard is load-bearing: the existing integration regression
  `fs_apply_patch_overflow_on_huge_count_returns_conflict` (falcon-mcp/tests/fs_test.rs)
  sends `@@ -1,4294967295 +1,1 @@` with body 1 old / 2 new. new_recount(2) >
  declared(1) blocks the rewrite, so the huge count survives to `splice_lines` and still
  conflicts. Without the guard the attack diff would silently apply.
- The nonzero guard keeps a body-less header from being rewritten to `-A,0 +C,0` (an
  empty hunk that would then parse as a no-op instead of being rejected).
- Correct-count diffs recount to exactly the declared values → no rewrite → byte-identical
  passthrough (including omitted-count style, e.g. `@@ -1 +1 @@` stays unexpanded).

**Repair 2 — final-newline termination:** if the last body line of a hunk lacks a
trailing `\n`, emit it with one. This is the actual parse-panic fix for the recorded
input. It applies only to countable body lines (' ', '-', '+'), never to the
`\ No newline at end of file` marker (which patch-0.7 already tolerates without a
trailing newline) and never to trailing non-body content.

**What is deliberately NOT touched:** trailing prose after the body, concatenated
second patches (`--- `/`+++ ` stop the body), lines after the last hunk body line —
preserving the existing catch_unwind recovery tests' behavior. `parse_hunks` keeps its
catch_unwind backstop exactly as is (defense in depth). MAX_DIFF_BYTES is validated on
the raw input before normalization; normalization adds at most a handful of bytes per
header plus one newline.

### Wiring

In `fs_apply_patch`: `let normalized = normalize_hunk_counts(&args.unified_diff);`
immediately after the `validate_limits` conflict check; `parse_hunks(&normalized)`;
`normalized` (a local String) outlives the borrowed `Patch`. Doc comment updated to the
four-unit composition.

### Tests (unit, in fs_basic.rs `mod tests`, existing style)

1. Recorded cassette diff VERBATIM (falcon-detective/fixtures/cassettes/896f6e7b4af1a7c7.json)
   as a `const` — assert normalize rewrites the header to `@@ -1,4 +1,3 @@` and appends
   the missing final newline (planted-lint marker text inside the fixture string is data,
   not source — G-1).
2. Full validate → normalize → parse → splice path on the recorded diff against an
   original shaped like the real falcon-agent/src/interrogate.rs head; assert the
   `use std::io::Write;` line is removed, everything else intact, add_count == 0.
3. Correct-count diffs (single-hunk, multi-hunk, no-newline-marker variant) pass through
   byte-identical.
4. Multi-hunk recount: hunk 1 truncated + hunk 2 adjacent and correct → only hunk 1
   rewritten; full path applies.
5. `\ No newline at end of file` marker not counted (truncated hunk with marker →
   rewrite to actual counts, marker preserved; splice keeps no-trailing-newline output).
6. Omitted-count headers: `@@ -1 +1 @@` means 1/1 — identity when body matches;
   rewrite when the other side's body is truncated.
7. Missing final newline with already-correct counts → newline appended, path applies.
8. Shrink-only guard: huge-count attack diff byte-identical through normalize; splice
   still conflicts (unit mirror of the integration test, now through the normalize path).
9. Trailing-prose and concatenated-patches inputs byte-identical through normalize;
   `parse_hunks(normalized)` still fails with the panicked-parser conflict reason.
10. Empty-body header (header followed by prose/EOF) left untouched.

### Gates (from the worktree root)

- `cargo fmt --check`
- `cargo clippy -p falcon-mcp --all-targets -- -D warnings` (CI's exact invocation;
  falcon-agent's planted warnings are expected and out of scope)
- `cargo test -p falcon-mcp` all green
- `scripts/check-planted-defects.sh` exit 0 (G-1 — falcon-agent source untouched)
- `(cd falcon-detective && npm test)` still green (sanity; no TS changes)

### Scope

Only `falcon-mcp/src/tools/fs_basic.rs`. No falcon-agent changes (G-1). No new
dependencies (hand-rolled header parse; no regex).

Commit: `fix(falcon-mcp): normalize model-emitted hunk counts before patch parse (MA-28)`
