#!/usr/bin/env bash
set -euo pipefail

BASE=/Users/administrator/dev/iroha/dist/taira-localnet
BIN=/Users/administrator/dev/iroha-build-taira-latest/target/debug/irohad
ts=$(date -u +%Y%m%dT%H%M%SZ)

pids=$(
  ps ax -o pid=,command= \
    | awk '/\/target\/debug\/irohad --sora --config .*taira-localnet\/peer[0-3]\.toml/ && !/awk/ {print $1}'
)
if [ -n "${pids:-}" ]; then
  printf 'killing %s\n' "$(echo "$pids" | tr '\n' ' ')"
  kill $pids || true
  sleep 4
fi

pids=$(
  ps ax -o pid=,command= \
    | awk '/\/target\/debug\/irohad --sora --config .*taira-localnet\/peer[0-3]\.toml/ && !/awk/ {print $1}'
)
if [ -n "${pids:-}" ]; then
  printf 'force killing %s\n' "$(echo "$pids" | tr '\n' ' ')"
  kill -9 $pids || true
  sleep 1
fi

for i in 0 1 2 3; do
  log="$BASE/peer${i}.log"
  printf '\n=== patched replay start peer%s %s ===\n' "$i" "$ts" >> "$log"
  nohup "$BIN" --sora --config "$BASE/peer${i}.toml" >> "$log" 2>&1 &
  printf 'peer%s_pid=%s\n' "$i" "$!"
done

sleep 30

bash /tmp/codex_taira_check.sh
