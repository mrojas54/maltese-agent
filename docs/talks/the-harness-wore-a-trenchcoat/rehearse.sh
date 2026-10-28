#!/usr/bin/env bash
# Assert the live stdio beats, then run the backing tests and workshop guard.
set -euo pipefail

REPO="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$REPO" # Select the repo's pinned Rust toolchain even from another cwd.
BEATS_ONLY=false
BUILD=true
for arg in "$@"; do
  case "$arg" in
    --beats) BEATS_ONLY=true ;;
    --no-build) BUILD=false ;;
    *) echo "Usage: $0 [--beats] [--no-build]" >&2; exit 2 ;;
  esac
done
for tool in cargo python3 git; do
  command -v "$tool" >/dev/null || { echo "Missing prerequisite: $tool" >&2; exit 2; }
done
if ! "$BEATS_ONLY"; then
  # fs_test otherwise reports AST tests as passing after silently skipping them.
  for tool in ast-grep jq; do
    command -v "$tool" >/dev/null || { echo "Full rehearsal requires $tool (or use --beats)." >&2; exit 2; }
  done
fi

if "$BUILD"; then cargo build --locked -q -p falcon-mcp; fi
TARGET_DIR="$(cargo metadata --offline --no-deps --format-version=1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
BIN="$TARGET_DIR/debug/falcon-mcp"
[ -x "$BIN" ] || { echo "Build falcon-mcp before using --no-build." >&2; exit 2; }

# Never reuse /tmp/jail or overwrite a caller's existing fixtures.
if [ -n "${JAIL:-}" ]; then
  mkdir -- "$JAIL" || { echo "JAIL must name a new, nonexistent directory." >&2; exit 2; }
else
  JAIL="$(mktemp -d "${TMPDIR:-/tmp}/falcon-rehearsal.XXXXXX")"
fi
trap 'rm -rf -- "$JAIL"' EXIT
git -C "$JAIL" init -q
printf 'AWS_SECRET_ACCESS_KEY=decoy-not-a-real-key\n' > "$JAIL/CONFIDENTIAL_KEYS.txt"
printf 'hello\n' > "$JAIL/note.txt"
python3 "$REPO/docs/talks/the-harness-wore-a-trenchcoat/rehearse.py" "$BIN" "$JAIL"

"$BEATS_ONLY" && exit 0
printf '\nBacking tests (complete falcon-mcp suite)\n'
cargo test --locked -p falcon-mcp --all-targets
printf '\ndemo00 (audience exercises)\n'
(cd women-in-rust/demo00 && cargo run --locked -q -- verify)
printf '\nG-1 guardrail\n'
bash scripts/check-planted-defects.sh
