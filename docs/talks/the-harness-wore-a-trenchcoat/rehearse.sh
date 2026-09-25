#!/usr/bin/env bash
# Dress rehearsal for "The Harness Wore a Trenchcoat".
#
#   docs/talks/the-harness-wore-a-trenchcoat/rehearse.sh          # beats + tests
#   docs/talks/the-harness-wore-a-trenchcoat/rehearse.sh --beats  # live beats only
#
# Each beat starts its own falcon-mcp so replies print in slide order (one
# server handles concurrent calls and answers out of order). The jail is
# /tmp/jail so error messages stay short enough to read from the back row.
set -euo pipefail

REPO="$(cd "$(dirname "$0")/../../.." && pwd)"
JAIL="${JAIL:-/tmp/jail}"
BIN="$REPO/target/debug/falcon-mcp"

INIT='{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"rehearsal","version":"0"}}}'
READY='{"jsonrpc":"2.0","method":"notifications/initialized"}'

beat() { # beat "<title>" "<server flags>" '<tools/call frame>'
  local title="$1" flags="$2" frame="$3"
  printf '\n\033[1m%s\033[0m\n  > %s\n  < ' "$title" "$frame"
  # shellcheck disable=SC2086
  { printf '%s\n%s\n%s\n' "$INIT" "$READY" "$frame"; sleep 2; } \
    | "$BIN" --stdio --root "$JAIL" $flags 2>/dev/null \
    | grep -v '"id":0'
}

# One server, one session, several calls in order, with the server's stderr
# shown so the TRIPWIRE alarm line is on screen.
session() { # session "<title>" '<frame>' '<frame>' ...
  local title="$1"; shift
  printf '\n\033[1m%s\033[0m\n' "$title"
  {
    printf '%s\n%s\n' "$INIT" "$READY"
    for f in "$@"; do printf '%s\n' "$f"; sleep 1; done
    sleep 1
  } | "$BIN" --stdio --root "$JAIL" 2>&1 | grep -v '"id":0' | sed 's/^/  < /'
}

cargo build -q -p falcon-mcp --manifest-path "$REPO/Cargo.toml"
mkdir -p "$JAIL"
[ -d "$JAIL/.git" ] || git -C "$JAIL" init -q
# The bait. Untracked on purpose: a committed decoy would show in git_diff.
printf 'AWS_SECRET_ACCESS_KEY=decoy-not-a-real-key\n' > "$JAIL/CONFIDENTIAL_KEYS.txt"
printf 'hello\n' > "$JAIL/note.txt"

beat "Beat 2: allowed binary" "--enable-exec" \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"exec_run","arguments":{"cmd":"cargo","args":["--version"]}}}'
beat "Beat 3: shell injection" "--enable-exec" \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"exec_run","arguments":{"cmd":"sh","args":["-c","cat /etc/passwd"]}}}'
beat "Beat 4: path traversal" "" \
  '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"fs_read","arguments":{"path":"../../.ssh/id_rsa"}}}'
session "Beat 5: tripwire (list the bait, take it, get revoked)" \
  '{"jsonrpc":"2.0","id":51,"method":"tools/call","params":{"name":"fs_list","arguments":{"path":"."}}}' \
  '{"jsonrpc":"2.0","id":52,"method":"tools/call","params":{"name":"fs_read","arguments":{"path":"CONFIDENTIAL_KEYS.txt"}}}' \
  '{"jsonrpc":"2.0","id":53,"method":"tools/call","params":{"name":"fs_read","arguments":{"path":"note.txt"}}}'
beat "Bonus: read-only refuses a write" "--read-only" \
  '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"fs_write","arguments":{"path":"loot.txt","content":"pwned"}}}'
beat "Bonus: exec is off unless asked for" "" \
  '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"exec_run","arguments":{"cmd":"cargo","args":["--version"]}}}'

[ "${1:-}" = "--beats" ] && exit 0

printf '\n\033[1mBacking tests\033[0m\n'
for t in --lib "--test exec_test" "--test exec_impostor_test" "--test fs_test" \
         "--test timeout_test" "--test error_codes_test" "--test tripwire_test" \
         "--test http_smoke"; do
  # shellcheck disable=SC2086
  printf '  %-28s ' "$t"
  cargo test -q -p falcon-mcp --manifest-path "$REPO/Cargo.toml" $t 2>&1 | grep '^test result' | tail -1
done

printf '\n\033[1mdemo00 (audience exercises)\033[0m\n'
(cd "$REPO/women-in-rust/demo00" && cargo run -q -- verify)

printf '\n\033[1mG-1 guardrail\033[0m\n  '
bash "$REPO/scripts/check-planted-defects.sh" | tail -1
