#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TORII_URL="${TORII_URL:-http://127.0.0.1:8080}"
RUN_DIR="${RUN_DIR:-$ROOT/target/kaigi-demo}"
GENESIS_CLEAN="$RUN_DIR/genesis.cleaned.json"
GENESIS_NRT="$RUN_DIR/genesis.nrt"
SUMMARY_JSON="$RUN_DIR/kaigi_summary.json"
NODE_LOG="$RUN_DIR/irohad.log"
SPOOL_DIR="$ROOT/storage/streaming/soranet_routes"

log() {
  printf '[kaigi-demo] %s\n' "$*" >&2
}

ensure_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    log "missing required command: $1"
    exit 1
  fi
}

cleanup() {
  if [[ -n "${NODE_PID:-}" ]]; then
    log "stopping irohad (pid $NODE_PID)"
    kill "$NODE_PID" >/dev/null 2>&1 || true
    wait "$NODE_PID" >/dev/null 2>&1 || true
  fi
}

trap cleanup EXIT

ensure_cmd cargo
ensure_cmd curl
ensure_cmd python3
mkdir -p "$RUN_DIR"

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
policy = {
    "mode": "TransparentOnly",
    "vk_set_hash": None,
    "poseidon_params_id": None,
    "pedersen_params_id": None,
    "pending_transition": None,
}
for tx in data.get("transactions", []):
    for ins in tx.get("instructions", []):
        reg = ins.get("Register")
        if not reg:
            continue
        asset_def = reg.get("AssetDefinition")
        if asset_def is None:
            continue
        asset_def.setdefault("confidential_policy", policy.copy())
with dst.open('w', encoding='utf-8') as f:
    json.dump(data, f, indent=2)
    f.write('\n')
PY

log "signing demo genesis manifest -> $GENESIS_NRT"
KAGAMI_OUTPUT="$(
  cargo run -q -p iroha_kagami -- genesis sign \
    "$GENESIS_CLEAN" \
    --out-file "$GENESIS_NRT"
)"
printf '%s\n' "$KAGAMI_OUTPUT" >&2

log "launching irohad (logs: $NODE_LOG)"
IROHA_GENESIS__FILE="$GENESIS_NRT" \
  cargo run -q -p irohad -- \
  --sora \
  --config defaults/nexus/config.toml \
  --genesis-manifest-json defaults/nexus/genesis.json \
  >"$NODE_LOG" 2>&1 &
NODE_PID=$!

log "waiting for Torii at $TORII_URL/status"
for _ in $(seq 1 120); do
  if curl -sf "$TORII_URL/status" >/dev/null 2>&1; then
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
  kaigi quickstart \
  --auto-join-host \
  --summary-out "$SUMMARY_JSON"

log "summary written to $SUMMARY_JSON"
log "Kaigi demo ready. Share the summary file and, if needed, spool contents under:"
log "  $SPOOL_DIR/exit-<relay-id>/kaigi-stream/"
log "Press Ctrl+C to stop the node when finished. Logs remain in $NODE_LOG."
wait "$NODE_PID"
