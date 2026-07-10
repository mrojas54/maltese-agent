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
# Gate 2 (AC-13, WS-5): no underscore-private Barnum surface in
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

# --- append new gates above this line (SPEC R-5) ---------------------------

if [[ "$FAILURES" -gt 0 ]]; then
  printf '%d gate(s) failed\n' "$FAILURES" >&2
  exit 1
fi
exit 0
