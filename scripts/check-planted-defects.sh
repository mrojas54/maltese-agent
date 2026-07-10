#!/usr/bin/env bash
# check-planted-defects.sh — planted-defect guard for falcon-agent (G-1).
#
# Contract: EVALUATION.md H-8 / G-1 / AC-9; SPEC.md WS-0.
#
# falcon-agent ships DELIBERATELY broken: a poisoned few-shot example,
# missing input/output guards, three planted clippy lints, and an
# `#[ignore]`'d smoking-gun test. That defect set is the workshop PRODUCT.
# This script asserts each defect is PRESENT and carries its intentional
# marker in the TARGET (post-WS-2) state, with file:line precision.
# "Fixing" a planted defect is the failure mode this guard exists to catch.
#
# IMPORTANT: this script encodes the CORRECTED marker state that WS-2
# (ticket MA-3) establishes. On a tree where WS-2 has not landed it fails
# by design (misplaced lint-#1 marker, "flaky" ignore reason, missing
# prompt.rs module-doc flag, accidental 4th lint). MA-3 wires it into CI
# in the same PR that makes it pass (SPEC R-2).
#
# The three clippy invocations are byte-identical to the ones documented
# in falcon-agent/README.md ("Verify the planted state at any time");
# per AC-9 this script is the single source of truth for their expected
# warning sets. `--message-format=json` is added for parsing only and
# does not change lint behavior.
#
# Output: one line per assertion, `PASS <id> <what>` or `FAIL <id> <what> — <detail>`.
# Exit codes: 0 all assertions pass; 1 at least one assertion failed;
#             2 environment/prerequisite error (cargo/jq missing, compile error).

set -u -o pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT" || exit 2

PROMPT_RS="falcon-agent/src/prompt.rs"
INTERROGATE_RS="falcon-agent/src/interrogate.rs"
LLM_RS="falcon-agent/src/llm.rs"
INTEGRATION_RS="falcon-agent/tests/integration.rs"

FAILURES=0
pass() { printf 'PASS  %-3s %s\n' "$1" "$2"; }
fail() {
    printf 'FAIL  %-3s %s — %s\n' "$1" "$2" "$3"
    FAILURES=$((FAILURES + 1))
}

require() {
    command -v "$1" >/dev/null 2>&1 || {
        echo "ERROR: required tool '$1' not found on PATH" >&2
        exit 2
    }
}
require cargo
require jq

for f in "$PROMPT_RS" "$INTERROGATE_RS" "$LLM_RS" "$INTEGRATION_RS"; do
    [ -f "$f" ] || {
        echo "ERROR: expected file missing: $f" >&2
        exit 2
    }
done

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

# first line number of a fixed string in a file ("" if absent)
line_of() { grep -nF -- "$1" "$2" | head -n 1 | cut -d: -f1; }

# ---------------------------------------------------------------------------
# Static assertions (grep; line numbers discovered, never hardcoded)
# ---------------------------------------------------------------------------

# P1 — poisoned third few-shot example present in prompt.rs (defect-present).
if grep -qF 'the bird flew at midnight' "$PROMPT_RS" &&
    grep -qF '"attribution": "(unknown)"' "$PROMPT_RS"; then
    pass P1 "poisoned few-shot example present in $PROMPT_RS"
else
    fail P1 "poisoned few-shot example present in $PROMPT_RS" \
        "expected literals 'the bird flew at midnight' and '\"attribution\": \"(unknown)\"'; the plant must never be removed (G-1)"
fi

# P2 — prompt.rs module doc flags the third example as the intentional plant
#      and points at the README (TARGET state, added by WS-2).
if grep -E '^//!' "$PROMPT_RS" | grep -i 'intentional' | grep -i 'poison' | grep -qi 'readme'; then
    pass P2 "module doc flags the poison plant (intentional + poison + README)"
else
    fail P2 "module doc flags the poison plant (intentional + poison + README)" \
        "no '//!' line in $PROMPT_RS mentions intentional+poison+README"
fi

# G1/G2/G3 — missing-guard markers present (defect-present; EVALUATION G-1
# "missing input/output guards").
if grep -qF 'BUG (intentional): no input sanitization' "$INTERROGATE_RS"; then
    pass G1 "missing-input-sanitization marker present in $INTERROGATE_RS"
else
    fail G1 "missing-input-sanitization marker present in $INTERROGATE_RS" \
        "marker 'BUG (intentional): no input sanitization' not found"
fi
if grep -qF 'BUG (intentional): no schema-level checks' "$INTERROGATE_RS"; then
    pass G2 "missing-output-schema marker present in $INTERROGATE_RS"
else
    fail G2 "missing-output-schema marker present in $INTERROGATE_RS" \
        "marker 'BUG (intentional): no schema-level checks' not found"
fi
if grep -qF 'BUG (intentional): no schema validation here' "$LLM_RS"; then
    pass G3 "missing-output-schema marker present in $LLM_RS"
else
    fail G3 "missing-output-schema marker present in $LLM_RS" \
        "marker 'BUG (intentional): no schema validation here' not found"
fi

# I1/I2/I3 — smoking-gun test stays #[ignore]'d (defect-present), with the
# TARGET reason: names the intentional workshop smoking-gun, points at the
# README, and does NOT claim flakiness.
TEST_LINE="$(line_of 'async fn bird_themed_inputs_arent_special' "$INTEGRATION_RS")"
IGNORE_ATTR=""
if [ -n "$TEST_LINE" ]; then
    SCAN_FROM=$((TEST_LINE > 6 ? TEST_LINE - 6 : 1))
    IGNORE_ATTR="$(sed -n "${SCAN_FROM},${TEST_LINE}p" "$INTEGRATION_RS" | grep -F '#[ignore' | tail -n 1)"
fi
if [ -z "$TEST_LINE" ]; then
    fail I1 "#[ignore] on bird_themed_inputs_arent_special" \
        "test fn not found in $INTEGRATION_RS — the smoking-gun test must never be deleted (G-1)"
elif [ -n "$IGNORE_ATTR" ]; then
    pass I1 "#[ignore] on bird_themed_inputs_arent_special"
else
    fail I1 "#[ignore] on bird_themed_inputs_arent_special" \
        "no #[ignore] attribute within 6 lines above the test fn (line $TEST_LINE) — un-ignoring the smoking gun violates G-1"
fi
if [ -n "$IGNORE_ATTR" ] && printf '%s' "$IGNORE_ATTR" | grep -i 'intentional' | grep -F 'smoking-gun' | grep -qi 'readme'; then
    pass I2 "ignore reason names the intentional workshop smoking-gun + README"
else
    fail I2 "ignore reason names the intentional workshop smoking-gun + README" \
        "expected intentional+smoking-gun+README in the reason; got: ${IGNORE_ATTR:-<none>}"
fi
if [ -n "$IGNORE_ATTR" ] && ! printf '%s' "$IGNORE_ATTR" | grep -qi 'flaky'; then
    pass I3 "ignore reason does not claim flakiness"
else
    fail I3 "ignore reason does not claim flakiness" \
        "the word 'flaky' must not appear (the test is an intentional plant, not flaky); got: ${IGNORE_ATTR:-<none>}"
fi

# M1 — planted-lint-#1 marker sits on the real dead-import line
#      `use std::io::Write;` and nowhere else (TARGET state).
WRITE_LINE="$(line_of 'use std::io::Write;' "$INTERROGATE_RS")"
MARKER1_LINES="$(grep -nF -- 'BUG (intentional): planted lint #1' "$INTERROGATE_RS" | cut -d: -f1 | tr '\n' ' ' | sed 's/ $//')"
if [ -z "$WRITE_LINE" ]; then
    fail M1 "lint-#1 marker on the 'use std::io::Write;' line" \
        "'use std::io::Write;' not found in $INTERROGATE_RS — the dead import must never be removed (G-1)"
elif [ "$MARKER1_LINES" = "$WRITE_LINE" ]; then
    pass M1 "lint-#1 marker on the 'use std::io::Write;' line ($INTERROGATE_RS:$WRITE_LINE)"
else
    fail M1 "lint-#1 marker on the 'use std::io::Write;' line" \
        "dead import at $INTERROGATE_RS:$WRITE_LINE but 'planted lint #1' marker at line(s): ${MARKER1_LINES:-<none>}"
fi

# M2/M3 — lint-#2 / lint-#3 markers present (their lines anchor L2/L3).
LINT2_LINE="$(line_of 'BUG (intentional): planted lint #2' "$INTERROGATE_RS")"
LINT3_LINE="$(line_of 'BUG (intentional): planted lint #3' "$LLM_RS")"
if [ -n "$LINT2_LINE" ]; then
    pass M2 "lint-#2 (unwrap) marker present ($INTERROGATE_RS:$LINT2_LINE)"
else
    fail M2 "lint-#2 (unwrap) marker present in $INTERROGATE_RS" \
        "marker 'BUG (intentional): planted lint #2' not found"
fi
if [ -n "$LINT3_LINE" ]; then
    pass M3 "lint-#3 (needless clone) marker present ($LLM_RS:$LINT3_LINE)"
else
    fail M3 "lint-#3 (needless clone) marker present in $LLM_RS" \
        "marker 'BUG (intentional): planted lint #3' not found"
fi

# ---------------------------------------------------------------------------
# Dynamic assertions (cargo check + the three README-documented clippy runs)
# ---------------------------------------------------------------------------

# Emit "code<TAB>file<TAB>line_start<TAB>line_end" per primary warning span.
# shellcheck disable=SC2016  # jq program: $m is jq syntax, not a shell variable
JQ_WARNINGS='select(.reason=="compiler-message")
  | .message
  | select(.level=="warning" and .code != null)
  | . as $m
  | ($m.spans[] | select(.is_primary))
  | [$m.code.code, .file_name, (.line_start|tostring), (.line_end|tostring)]
  | @tsv'

# run_diagnostics <out.tsv> <cargo args...>
# `cargo clean -p falcon-agent` first: cached fingerprints suppress or replay
# stale diagnostics across invocations with different lint flags.
run_diagnostics() {
    local out="$1"
    shift
    cargo clean -p falcon-agent --quiet >/dev/null 2>&1 || cargo clean -p falcon-agent >/dev/null
    if ! cargo "$@" >"$WORKDIR/raw.json" 2>"$WORKDIR/cargo-stderr.log"; then
        echo "ERROR: 'cargo $*' failed to compile — the guard cannot evaluate lint state" >&2
        tail -n 40 "$WORKDIR/cargo-stderr.log" >&2
        exit 2
    fi
    jq -r "$JQ_WARNINGS" <"$WORKDIR/raw.json" | sort -u >"$out"
}

# tsv_summary <tsv> — human-readable "code@file:start[-end]" list
tsv_summary() {
    awk -F'\t' '{ printf "%s%s@%s:%s%s", (NR>1?", ":""), $1, $2, $3, ($4!=$3?"-"$4:"") } END { if (NR==0) printf "<no warnings>" }' "$1"
}

# span_hits <tsv> <code> <file> <marker_line> <allow_adjacent(0|1)>
# True if a primary span of <code> in <file> covers the marker line, or
# (adjacency allowed) starts on the line immediately after a standalone
# marker comment placed directly above the offending line.
span_hits() {
    awk -F'\t' -v c="$2" -v f="$3" -v l="$4" -v adj="$5" '
        $1==c && $2==f && ((($3+0)<=(l+0) && ($4+0)>=(l+0)) || (adj==1 && ($3+0)==(l+0)+1)) { found=1 }
        END { exit found ? 0 : 1 }' "$1"
}

echo "-- running cargo check + the 3 README-documented clippy invocations (fresh, uncached) --"
run_diagnostics "$WORKDIR/check.tsv" check -p falcon-agent --message-format=json
run_diagnostics "$WORKDIR/inv1.tsv" clippy -p falcon-agent --message-format=json -- -W clippy::all
run_diagnostics "$WORKDIR/inv2.tsv" clippy -p falcon-agent --message-format=json -- -W clippy::all -W clippy::unwrap_used
run_diagnostics "$WORKDIR/inv3.tsv" clippy -p falcon-agent --message-format=json -- -W clippy::all -W clippy::nursery -W clippy::unwrap_used

EXPECTED_UNUSED_IMPORT="$(printf 'unused_imports\t%s\t%s\t%s' "$INTERROGATE_RS" "$WRITE_LINE" "$WRITE_LINE")"

# C1 — `cargo check -p falcon-agent` emits exactly one warning: the planted
#      unused import, on the `use std::io::Write;` line (defect-present).
if [ -n "$WRITE_LINE" ] && [ "$(cat "$WORKDIR/check.tsv")" = "$EXPECTED_UNUSED_IMPORT" ]; then
    pass C1 "cargo check: exactly the planted unused-import warning ($INTERROGATE_RS:$WRITE_LINE)"
else
    fail C1 "cargo check: exactly the planted unused-import warning" \
        "expected only unused_imports@$INTERROGATE_RS:${WRITE_LINE:-?}; got: $(tsv_summary "$WORKDIR/check.tsv")"
fi

# L1 — invocation 1 (`-W clippy::all` alone): the unused-import plant is the
#      ONLY warning (TARGET state — holds once WS-2 removes the accidental
#      4th lint).
if [ -n "$WRITE_LINE" ] && [ "$(cat "$WORKDIR/inv1.tsv")" = "$EXPECTED_UNUSED_IMPORT" ]; then
    pass L1 "clippy -W clippy::all: unused-import plant is the only warning"
else
    fail L1 "clippy -W clippy::all: unused-import plant is the only warning" \
        "expected only unused_imports@$INTERROGATE_RS:${WRITE_LINE:-?}; got: $(tsv_summary "$WORKDIR/inv1.tsv")"
fi

# L2 — invocation 2 (+ -W clippy::unwrap_used) surfaces the planted unwrap
#      on its marked line (presence; span must cover the marker line).
if [ -n "$LINT2_LINE" ] && span_hits "$WORKDIR/inv2.tsv" 'clippy::unwrap_used' "$INTERROGATE_RS" "$LINT2_LINE" 0; then
    pass L2 "clippy + unwrap_used: planted unwrap surfaces at its marked line ($INTERROGATE_RS:$LINT2_LINE)"
else
    fail L2 "clippy + unwrap_used: planted unwrap surfaces at its marked line" \
        "no clippy::unwrap_used span covering $INTERROGATE_RS:${LINT2_LINE:-?}; got: $(tsv_summary "$WORKDIR/inv2.tsv")"
fi

# L3 — invocation 3 (+ -W clippy::nursery) surfaces the planted needless
#      clone on its marked line (presence-only: nursery may add unrelated
#      noise, SPEC WS-0(b)). The marker is a standalone comment directly
#      above the offending line, so adjacency (span starts on marker+1)
#      also satisfies file:line precision.
if [ -n "$LINT3_LINE" ] && span_hits "$WORKDIR/inv3.tsv" 'clippy::redundant_clone' "$LLM_RS" "$LINT3_LINE" 1; then
    pass L3 "clippy + nursery: planted redundant_clone surfaces at its marked line ($LLM_RS:$LINT3_LINE)"
else
    fail L3 "clippy + nursery: planted redundant_clone surfaces at its marked line" \
        "no clippy::redundant_clone span at/adjacent to $LLM_RS:${LINT3_LINE:-?}; got: $(tsv_summary "$WORKDIR/inv3.tsv")"
fi

# L4 — the undocumented `default_constructed_unit_structs` lint is ABSENT
#      from every invocation: the canonical planted set is exactly the three
#      README-documented lints (TARGET state — WS-2 fixes the accident).
FOURTH="$(cut -f1,2,3 "$WORKDIR/inv1.tsv" "$WORKDIR/inv2.tsv" "$WORKDIR/inv3.tsv" 2>/dev/null | grep -F 'clippy::default_constructed_unit_structs' | sort -u | tr '\t' ':' | tr '\n' ' ' | sed 's/ $//')"
if [ -z "$FOURTH" ]; then
    pass L4 "no undocumented default_constructed_unit_structs lint (planted set = exactly 3)"
else
    fail L4 "no undocumented default_constructed_unit_structs lint (planted set = exactly 3)" \
        "found: $FOURTH — not in the README's documented planted set"
fi

# ---------------------------------------------------------------------------
echo
if [ "$FAILURES" -eq 0 ]; then
    echo "OK: all planted-defect assertions hold — falcon-agent's workshop defects are intact and correctly marked (G-1)."
    exit 0
else
    echo "VIOLATION: $FAILURES planted-defect assertion(s) failed."
    echo "If a defect was 'fixed': revert it — the defects are the workshop product (G-1, falcon-agent/README.md §Planted bugs)."
    echo "If a marker is misplaced/misworded: this is the pre-WS-2 state; ticket MA-3 corrects markers and wires this guard into CI."
    exit 1
fi
