#!/usr/bin/env bash
# Run one complete private Kaigi call on a disposable four-validator localnet.
#
# The host creates the call, a second account joins, the host records usage, the
# participant leaves and rejoins, and the host ends the call. Every proof is generated
# locally by `iroha app kaigi --secret-file`; the script prints the call after each step.
#
# Prerequisites: cargo, curl and python3. The script builds kagami, iroha3d and iroha.
#
# Environment:
#   RUN_DIR        Working directory, which must not exist yet.
#                  Default: target/kaigi-demo/<UTC timestamp>.
#   BASE_API_PORT  First Torii port; the other three peers use the next ones. Default: 18180.
#   BASE_P2P_PORT  First peer-to-peer port; likewise. Default: 18337.
#   KEEP_NETWORK   Set to 1 to leave the network running after the call ends.
set -euo pipefail

usage() {
  sed -n '2,/^set -euo pipefail/p' "${BASH_SOURCE[0]}" | sed -e '$d' -e 's/^# \{0,1\}//'
}
if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi
if [[ $# -ne 0 ]]; then
  usage >&2
  exit 2
fi

ROOT="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd -- "$ROOT"
RUN_DIR="${RUN_DIR:-$ROOT/target/kaigi-demo/$(date -u +%Y%m%dT%H%M%SZ)}"
BASE_API_PORT="${BASE_API_PORT:-18180}"
BASE_P2P_PORT="${BASE_P2P_PORT:-18337}"
KEEP_NETWORK="${KEEP_NETWORK:-0}"
NETWORK_DIR="$RUN_DIR/network"
TORII_URL="http://127.0.0.1:$BASE_API_PORT"
# Every generated developer network lets its client host calls in this domain.
DOMAIN="universal.universal"
CALL_NAME="demo_call"
NETWORK_STARTED=0

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
  if [[ "$NETWORK_STARTED" == 1 && "$KEEP_NETWORK" != 1 ]]; then
    log "stopping the network"
    "$NETWORK_DIR/stop.sh" >/dev/null 2>&1 || log "could not stop the network: run $NETWORK_DIR/stop.sh"
  fi
}
trap cleanup EXIT

ensure_cmd cargo
ensure_cmd curl
ensure_cmd python3
if [[ -e "$RUN_DIR" ]]; then
  log "RUN_DIR already exists: $RUN_DIR"
  exit 1
fi
mkdir -p "$RUN_DIR"
chmod 700 "$RUN_DIR"

log "building kagami, iroha3d and iroha"
cargo build --quiet --locked -p iroha_kagami --bin kagami -p irohad --bin iroha3d -p iroha_cli --bin iroha
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}/debug"
KAGAMI="$TARGET_DIR/kagami"
IROHA="$TARGET_DIR/iroha"

log "generating a four-validator network in $NETWORK_DIR"
"$KAGAMI" localnet generate \
  --out-dir "$NETWORK_DIR" \
  --base-api-port "$BASE_API_PORT" \
  --base-p2p-port "$BASE_P2P_PORT" \
  >"$RUN_DIR/localnet.txt"
HOST_CONFIG="$NETWORK_DIR/client.toml"

log "starting the network (logs: $NETWORK_DIR/peer*.log)"
NETWORK_STARTED=1
IROHAD_BIN="$TARGET_DIR/iroha3d" "$NETWORK_DIR/start.sh" >/dev/null

log "waiting for Torii at $TORII_URL"
for attempt in $(seq 1 180); do
  if curl -sf "$TORII_URL/health" >/dev/null 2>&1; then
    break
  fi
  if [[ "$attempt" == 180 ]]; then
    log "Torii did not become ready; see $NETWORK_DIR/peer0.log"
    exit 1
  fi
  sleep 1
done

# Each account pays its own fees; the CLI requires an explicit payer on every transaction.
host() {
  "$IROHA" --config "$HOST_CONFIG" --fee-payer authority "$@"
}
participant() {
  "$IROHA" --config "$PARTICIPANT_CONFIG" --fee-payer authority "$@"
}
show_call() {
  log "$1"
  host --output-format text app kaigi show --domain "$DOMAIN" --call-name "$CALL_NAME" >&2
}

# Genesis funds the client account of client.toml, which hosts the call and pays fees in XOR.
HOST_ACCOUNT="$(sed -n 's/^client_account: //p' "$RUN_DIR/localnet.txt")"
if [[ -z "$HOST_ACCOUNT" ]]; then
  log "could not find client_account in $RUN_DIR/localnet.txt"
  exit 1
fi

log "creating and funding a participant account"
"$KAGAMI" keys --out-dir "$RUN_DIR/participant-keys" >/dev/null
PARTICIPANT_CONFIG="$RUN_DIR/participant.toml"
python3 - "$HOST_CONFIG" "$PARTICIPANT_CONFIG" "$RUN_DIR/participant-keys" <<'PY'
"""Derive the participant's client configuration from the host's, changing only its keys."""
import os
import re
import sys
from pathlib import Path

host_config, participant_config, keys = sys.argv[1:]
keys = Path(keys)
text = Path(host_config).read_text()
base = Path(host_config).parent
text = re.sub(
    r'^network_id_file = "(.*)"$',
    lambda match: f'network_id_file = "{base / match.group(1)}"',
    text,
    flags=re.M,
)
text = re.sub(
    r'^private_key = ".*"$',
    f'private_key = "{(keys / "private.key").read_text().strip()}"',
    text,
    flags=re.M,
)
text = re.sub(
    r'^public_key = ".*"$',
    f'public_key = "{(keys / "public.key").read_text().strip()}"',
    text,
    flags=re.M,
)
descriptor = os.open(participant_config, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
with os.fdopen(descriptor, "w") as output:
    output.write(text)
PY
PARTICIPANT_ACCOUNT="$(
  host tools address convert "$(cat "$RUN_DIR/participant-keys/public.key")" --format i105
)"
host account register --id "$PARTICIPANT_ACCOUNT" >/dev/null
host ledger asset transfer \
  --definition-alias "xor#universal" --account "$HOST_ACCOUNT" \
  --to "$PARTICIPANT_ACCOUNT" --quantity 100 >/dev/null

log "host creates a private call"
host app kaigi create \
  --domain "$DOMAIN" --call-name "$CALL_NAME" --host "$HOST_ACCOUNT" \
  --privacy-mode zk-roster-v1 --secret-file "$RUN_DIR/host.secret" >/dev/null
show_call "the call is live, with an empty roster:"

log "participant joins, proving against the current roster"
participant app kaigi join \
  --domain "$DOMAIN" --call-name "$CALL_NAME" --participant "$PARTICIPANT_ACCOUNT" \
  --host "$HOST_ACCOUNT" --secret-file "$RUN_DIR/participant.secret" >/dev/null
log "the participant's own standing:"
participant --output-format text app kaigi standing --domain "$DOMAIN" --call-name "$CALL_NAME" >&2

log "host records a usage segment"
host app kaigi record-usage \
  --domain "$DOMAIN" --call-name "$CALL_NAME" --duration-ms 60000 --billed-gas 345 \
  --secret-file "$RUN_DIR/host.secret" >/dev/null
show_call "usage is recorded:"

log "participant leaves, consuming its participation sequence"
participant app kaigi leave \
  --domain "$DOMAIN" --call-name "$CALL_NAME" --participant "$PARTICIPANT_ACCOUNT" \
  --secret-file "$RUN_DIR/participant.secret" >/dev/null
log "participant rejoins at the next sequence, which the ledger now reveals with the host"
participant app kaigi join \
  --domain "$DOMAIN" --call-name "$CALL_NAME" --participant "$PARTICIPANT_ACCOUNT" \
  --secret-file "$RUN_DIR/participant.secret" >/dev/null
participant --output-format text app kaigi standing --domain "$DOMAIN" --call-name "$CALL_NAME" >&2

log "host ends the call"
host app kaigi end \
  --domain "$DOMAIN" --call-name "$CALL_NAME" --secret-file "$RUN_DIR/host.secret" >/dev/null
show_call "the call has ended:"

log "done. Working files are in $RUN_DIR"
if [[ "$KEEP_NETWORK" == 1 ]]; then
  log "the network is still running at $TORII_URL; stop it with $NETWORK_DIR/stop.sh"
fi
