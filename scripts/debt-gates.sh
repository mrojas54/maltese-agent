#!/usr/bin/env bash
# debt-gates.sh — tech-debt regression gates (EVALUATION harness H-9).
#
# Created by WS-10 (MA-04); per SPEC R-5 each workstream whose AC cites H-9
# appends its gate here in its own PR (WS-2: AC-7/AC-8 grep gates; WS-3: AC-25;
# WS-4: AC-22; WS-5: AC-13). CI runs this script on every push/PR.
#
# Contract: one "PASS: <gate>" or "FAIL: <gate> — <detail>" line per gate;
# exit non-zero if any gate fails. Gates must be read-only checks.

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FAILURES=0

pass() { printf 'PASS: %s\n' "$1"; }
fail() { printf 'FAIL: %s — %s\n' "$1" "$2"; FAILURES=$((FAILURES + 1)); }

# ---------------------------------------------------------------------------
# Gate 1 (AC-23, WS-10): Dockerfile base image rust minor matches the
# rust-toolchain.toml pin.
# ---------------------------------------------------------------------------
gate_toolchain_dockerfile_match() {
  local gate="toolchain-dockerfile-match (AC-23)"
  local toml="$REPO_ROOT/rust-toolchain.toml"
  local dockerfile="$REPO_ROOT/falcon-mcp/Dockerfile"

  if [[ ! -f "$toml" ]]; then
    fail "$gate" "rust-toolchain.toml not found at repo root"
    return
  fi
  if [[ ! -f "$dockerfile" ]]; then
    fail "$gate" "falcon-mcp/Dockerfile not found"
    return
  fi

  local channel
  channel="$(sed -n 's/^channel = "\([0-9][0-9.]*\)"$/\1/p' "$toml" | head -n 1)"
  if [[ -z "$channel" ]]; then
    fail "$gate" "could not parse a version channel from rust-toolchain.toml"
    return
  fi

  local image_version
  image_version="$(sed -n 's/^FROM rust:\([0-9][0-9.]*\)-.*$/\1/p' "$dockerfile" | head -n 1)"
  if [[ -z "$image_version" ]]; then
    fail "$gate" "could not parse a rust:<version> base image from falcon-mcp/Dockerfile"
    return
  fi

  # Compare major.minor only: the toml may pin "1.92" or "1.92.x", the image
  # tag may be "1.92" or "1.92.x" — the pinned minor must agree (AC-23).
  local channel_minor image_minor
  channel_minor="$(printf '%s' "$channel" | cut -d. -f1-2)"
  image_minor="$(printf '%s' "$image_version" | cut -d. -f1-2)"

  if [[ "$channel_minor" == "$image_minor" ]]; then
    pass "$gate"
  else
    fail "$gate" "rust-toolchain.toml pins $channel but Dockerfile base image is rust:$image_version (minor $channel_minor != $image_minor)"
  fi
}

gate_toolchain_dockerfile_match

# ---------------------------------------------------------------------------
# Gate 2 (AC-25, WS-3): MAX_STDERR_DISPLAY (stderr truncation for error
# messages) is defined exactly once, in falcon-mcp/src/limits.rs. Before WS-3
# it was defined twice (fs_basic.rs and fs_ast.rs).
# ---------------------------------------------------------------------------
gate_single_stderr_display_definition() {
  local gate="single-MAX_STDERR_DISPLAY-definition (AC-25)"
  local src="$REPO_ROOT/falcon-mcp/src"

  local def_count
  def_count="$(grep -r --include='*.rs' -E '(pub )?const MAX_STDERR_DISPLAY' "$src" | wc -l | tr -d '[:space:]')"
  if [[ "$def_count" -ne 1 ]]; then
    fail "$gate" "expected exactly 1 definition of MAX_STDERR_DISPLAY under falcon-mcp/src, found $def_count"
    return
  fi
  if ! grep -q -E '^pub const MAX_STDERR_DISPLAY' "$src/limits.rs" 2>/dev/null; then
    fail "$gate" "the single MAX_STDERR_DISPLAY definition is not in falcon-mcp/src/limits.rs"
    return
  fi
  pass "$gate"
}

gate_single_stderr_display_definition

# ---------------------------------------------------------------------------
# Gate 3 (AC-25, WS-3): fs_search and fs_search_ast share the one streaming
# subprocess helper (tools/subprocess.rs) rather than each owning a private
# spawn/stream/truncate/kill pipeline.
# ---------------------------------------------------------------------------
gate_shared_search_subprocess_helper() {
  local gate="shared-search-subprocess-helper (AC-25)"
  local tools="$REPO_ROOT/falcon-mcp/src/tools"

  local f
  for f in fs_basic.rs fs_ast.rs; do
    if ! grep -q 'run_streaming' "$tools/$f" 2>/dev/null; then
      fail "$gate" "$f does not use subprocess::run_streaming (search tools must share the helper)"
      return
    fi
    if grep -q -E '\.spawn\(' "$tools/$f" 2>/dev/null; then
      fail "$gate" "$f still spawns a child directly instead of via the shared helper"
      return
    fi
  done
  pass "$gate"
}

gate_shared_search_subprocess_helper

# ---------------------------------------------------------------------------
# Gate 4 (AC-22, WS-4): no `gemini-` model-ID string literal under
# falcon-detective/src outside the shared constants module src/lib/models.ts.
# ---------------------------------------------------------------------------
gate_no_gemini_literals() {
  local gate="no-gemini-literals (AC-22)"
  local src_dir="$REPO_ROOT/falcon-detective/src"

  if [[ ! -d "$src_dir" ]]; then
    fail "$gate" "falcon-detective/src not found"
    return
  fi

  local matches
  matches="$(grep -rn 'gemini-' "$src_dir" | grep -v '/src/lib/models\.ts:' || true)"
  if [[ -z "$matches" ]]; then
    pass "$gate"
  else
    fail "$gate" "gemini- literal outside src/lib/models.ts: $(printf '%s' "$matches" | cut -d: -f1 | sort -u | sed "s|$REPO_ROOT/||" | tr '\n' ' ')"
  fi
}

gate_no_gemini_literals

# ---------------------------------------------------------------------------
# Gate 3 (AC-13, WS-5): no underscore-private Barnum surface in
# falcon-detective/src. Handler invocation goes exclusively through the
# public runPipeline seam (falcon-detective/src/lib/invokeHandler.ts);
# `__definition` (and any other dunder property reach) is the engine
# worker's internal contract, not supported API. Scope is production src
# only — test files' in-process reaches are MA-13's charter.
# ---------------------------------------------------------------------------
gate_barnum_private_surface() {
  local gate="barnum-private-surface (AC-13)"
  local src="$REPO_ROOT/falcon-detective/src"

  if [[ ! -d "$src" ]]; then
    fail "$gate" "falcon-detective/src not found"
    return
  fi

  local hits
  hits="$(grep -rnE '__definition|\.__[A-Za-z]' "$src" 2>/dev/null)"
  if [[ -n "$hits" ]]; then
    fail "$gate" "underscore-private Barnum surface referenced in src: $(printf '%s' "$hits" | head -n 3 | tr '\n' ' | ')"
  else
    pass "$gate"
  fi
}

gate_barnum_private_surface

# ---------------------------------------------------------------------------
# Gate (AC-7, WS-2): the #[ignore] reason on the smoking-gun test
# bird_themed_inputs_arent_special names the INTENTIONAL workshop plant and
# points at the README; it must never claim flakiness. The attribute is
# located by a window scan (6 lines above the test fn, mirroring H-8's
# check-planted-defects.sh) so an unrelated #[ignore] elsewhere in the file
# can neither satisfy nor shadow this gate.
# ---------------------------------------------------------------------------
gate_smoking_gun_ignore_reason() {
  local gate="smoking-gun-ignore-reason (AC-7)"
  local test_file="$REPO_ROOT/falcon-agent/tests/integration.rs"

  if [[ ! -f "$test_file" ]]; then
    fail "$gate" "falcon-agent/tests/integration.rs not found"
    return
  fi

  local fn_line
  fn_line="$(grep -nF 'async fn bird_themed_inputs_arent_special' "$test_file" | head -n 1 | cut -d: -f1)"
  if [[ -z "$fn_line" ]]; then
    fail "$gate" "test fn bird_themed_inputs_arent_special not found (the smoking gun must never be deleted, G-1)"
    return
  fi

  local scan_from attr
  scan_from=$((fn_line > 6 ? fn_line - 6 : 1))
  attr="$(sed -n "${scan_from},${fn_line}p" "$test_file" | grep -F '#[ignore' | tail -n 1)"
  if [[ -z "$attr" ]]; then
    fail "$gate" "no #[ignore] attribute within 6 lines above the test fn (un-ignoring the smoking gun violates G-1)"
    return
  fi
  if printf '%s' "$attr" | grep -qi 'flaky'; then
    fail "$gate" "ignore reason claims flakiness (the test is an intentional plant, not flaky): $attr"
    return
  fi
  if ! printf '%s' "$attr" | grep -i 'intentional' | grep -F 'smoking-gun' | grep -qi 'readme'; then
    fail "$gate" "ignore reason must name the intentional workshop smoking-gun and point at the README; got: $attr"
    return
  fi
  pass "$gate"
}

gate_smoking_gun_ignore_reason

# ---------------------------------------------------------------------------
# Gate (AC-8, WS-2): falcon-agent/src/prompt.rs module doc flags the poisoned
# third few-shot example as the intentional plant — one '//!' line carrying
# intentional + poison + README (same per-line grep chain as H-8's P2).
# ---------------------------------------------------------------------------
gate_prompt_poison_flag() {
  local gate="prompt-poison-flag (AC-8)"
  local prompt_file="$REPO_ROOT/falcon-agent/src/prompt.rs"

  if [[ ! -f "$prompt_file" ]]; then
    fail "$gate" "falcon-agent/src/prompt.rs not found"
    return
  fi

  if grep -E '^//!' "$prompt_file" | grep -i 'intentional' | grep -i 'poison' | grep -qi 'readme'; then
    pass "$gate"
  else
    fail "$gate" "no '//!' module-doc line in prompt.rs mentions intentional+poison+README"
  fi
}

gate_prompt_poison_flag

# --- append new gates above this line (SPEC R-5) ---------------------------

if [[ "$FAILURES" -gt 0 ]]; then
  printf '%d gate(s) failed\n' "$FAILURES" >&2
  exit 1
fi
exit 0
