#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd -- "$ROOT"
TORII_URL="${TORII_URL:-http://127.0.0.1:8080}"
TORII_STATUS_URL="${TORII_URL%/}/status"
RUN_DIR="${RUN_DIR:-$ROOT/target/kaigi-demo}"
GENESIS_CLEAN="$RUN_DIR/genesis.cleaned.json"
GENESIS_NRT="$RUN_DIR/genesis.nrt"
SUMMARY_JSON="$RUN_DIR/kaigi_summary.json"
NODE_LOG="$RUN_DIR/iroha3d.log"
GENESIS_PRIVATE_KEY_FILE=""

log() {
  printf '[kaigi-demo] %s\n' "$*" >&2
}

ensure_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    log "missing required command: $1"
    exit 1
  fi
}

pid_is_running() {
  local pid="$1"
  if [[ ! "$pid" =~ ^[0-9]+$ ]]; then
    return 1
  fi
  if ! command -v ps >/dev/null 2>&1; then
    return 0
  fi
  ps -p "$pid" -o pid= >/dev/null 2>&1
}

pid_is_own_background_job() {
  local pid="$1"
  local job_pid
  while IFS= read -r job_pid; do
    [[ "$job_pid" == "$pid" ]] && return 0
  done < <(jobs -pr 2>/dev/null || true)
  return 1
}

cleanup() {
  if [[ -n "${NODE_PID:-}" ]]; then
    if pid_is_own_background_job "$NODE_PID" && pid_is_running "$NODE_PID"; then
      log "stopping irohad (pid $NODE_PID)"
      kill "$NODE_PID" >/dev/null 2>&1 || true
      wait "$NODE_PID" >/dev/null 2>&1 || true
    fi
    NODE_PID=""
  fi
  if [[ -n "$GENESIS_PRIVATE_KEY_FILE" && -f "$GENESIS_PRIVATE_KEY_FILE" && ! -L "$GENESIS_PRIVATE_KEY_FILE" ]]; then
    rm -f -- "$GENESIS_PRIVATE_KEY_FILE"
    GENESIS_PRIVATE_KEY_FILE=""
  fi
}

trap cleanup EXIT

ensure_cmd cargo
ensure_cmd curl
ensure_cmd python3
mkdir -p "$RUN_DIR"
umask 077
GENESIS_PRIVATE_KEY_FILE="$(mktemp "$RUN_DIR/.genesis-private-key.XXXXXX")"
printf '%s\n' \
  '80262082B3BDE54AEBECA4146257DA0DE8D59D8E46D5FE34887DCD8072866792FCB3AD' \
  >"$GENESIS_PRIVATE_KEY_FILE"

log "preparing genesis manifest -> $GENESIS_CLEAN"
python3 - "$ROOT/defaults/nexus/genesis.json" "$GENESIS_CLEAN" <<'PY'
import json, sys, pathlib
src = pathlib.Path(sys.argv[1])
dst = pathlib.Path(sys.argv[2])
with src.open('r', encoding='utf-8') as f:
    data = json.load(f)
ivm_dir = data.get('ivm_dir')
if isinstance(ivm_dir, list):
    data['ivm_dir'] = ivm_dir[0] if ivm_dir else "."
with dst.open('w', encoding='utf-8') as f:
    json.dump(data, f, indent=2)
    f.write('\n')
PY

log "signing demo genesis manifest -> $GENESIS_NRT"
KAGAMI_OUTPUT="$(
  cargo run -q -p iroha_kagami -- genesis sign \
    "$GENESIS_CLEAN" \
    --private-key-file "$GENESIS_PRIVATE_KEY_FILE" \
    --out-file "$GENESIS_NRT"
)"
printf '%s\n' "$KAGAMI_OUTPUT" >&2

log "launching iroha3d (logs: $NODE_LOG)"
IROHA_GENESIS__FILE="$GENESIS_NRT" \
  cargo run -q -p irohad --bin iroha3d -- \
  --sora \
  --config defaults/nexus/config.toml \
  --genesis-manifest-json defaults/nexus/genesis.json \
  >"$NODE_LOG" 2>&1 &
NODE_PID=$!

log "waiting for Torii at $TORII_STATUS_URL"
for _ in $(seq 1 120); do
  if ! pid_is_own_background_job "$NODE_PID" || ! pid_is_running "$NODE_PID"; then
    log "iroha3d exited before Torii became ready (see $NODE_LOG)"
    exit 1
  fi
  if curl -sf "$TORII_STATUS_URL" >/dev/null 2>&1; then
    READY=1
    break
  fi
  sleep 1
done

if [[ -z "${READY:-}" ]]; then
  log "Torii did not become ready (see $NODE_LOG)"
  exit 1
fi
log "Torii is online"

log "creating Kaigi quickstart summary -> $SUMMARY_JSON"
cargo run -q -p iroha_cli -- \
  --config defaults/client.toml \
  --torii-url "$TORII_URL" \
  kaigi quickstart \
  --summary-out "$SUMMARY_JSON"

log "summary written to $SUMMARY_JSON"
log "Kaigi demo ready. Share the summary file; V1 does not publish SoraNet exit-token spools."
log "Press Ctrl+C to stop the node when finished. Logs remain in $NODE_LOG."
wait "$NODE_PID"
