#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: run_10k_localnet.sh [OPTIONS]

Spin up a 10k TPS perf-profile localnet and drive load via tx_load.py.

Options:
  --mode <MODE>           permissioned, npos, or both (default: both)
  --peers <N>             number of peers (default: 7)
  --count <N>             total ping transactions to submit (default: 100000)
  --parallel <N>          parallel ping workers (default: 256)
  --per-peer              treat --count/--parallel as per-peer values
  --batch-size <N>        tx_load batch size (default: 10000)
  --batch-interval <SEC>  tx_load batch interval seconds (default: 1)
  --drain-timeout <SEC>   seconds to wait for queue drain (default: 120)
  --out-base <DIR>        output directory base (default: /tmp/iroha-10k)
  --release               use release binaries (default)
  --debug                 use debug binaries
  -h, --help              show this help
USAGE
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IROHA_DIR="${IROHA_DIR:-"$(cd "${SCRIPT_DIR}/.." && pwd)"}"
PYTHON_BIN="${PYTHON_BIN:-python3}"

MODE="both"
PEERS=7
COUNT=100000
PARALLEL=256
PER_PEER=false
BATCH_SIZE=10000
BATCH_INTERVAL=1
DRAIN_TIMEOUT=120
OUT_BASE="/tmp/iroha-10k"
PROFILE="release"

BASE_API_PORT_PERM=48080
BASE_P2P_PORT_PERM=48337
BASE_API_PORT_NPOS=58080
BASE_P2P_PORT_NPOS=58337
SEED_PERM="perf-profile-permissioned"
SEED_NPOS="perf-profile-npos"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --mode)
      MODE="$2"
      shift 2
      ;;
    --peers)
      PEERS="$2"
      shift 2
      ;;
    --count)
      COUNT="$2"
      shift 2
      ;;
    --parallel)
      PARALLEL="$2"
      shift 2
      ;;
    --per-peer)
      PER_PEER=true
      shift
      ;;
    --batch-size)
      BATCH_SIZE="$2"
      shift 2
      ;;
    --batch-interval)
      BATCH_INTERVAL="$2"
      shift 2
      ;;
    --drain-timeout)
      DRAIN_TIMEOUT="$2"
      shift 2
      ;;
    --out-base)
      OUT_BASE="$2"
      shift 2
      ;;
    --release)
      PROFILE="release"
      shift
      ;;
    --debug)
      PROFILE="debug"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

case "$MODE" in
  permissioned|npos|both)
    ;;
  *)
    echo "Invalid --mode: $MODE (expected permissioned, npos, or both)" >&2
    exit 2
    ;;
esac

if ! command -v "$PYTHON_BIN" >/dev/null 2>&1; then
  echo "Missing prerequisite: $PYTHON_BIN" >&2
  exit 1
fi
if [[ ! -x "${SCRIPT_DIR}/deploy_localnet.sh" ]]; then
  echo "Missing deploy_localnet.sh in ${SCRIPT_DIR}" >&2
  exit 1
fi
if [[ ! -f "${SCRIPT_DIR}/tx_load.py" ]]; then
  echo "Missing tx_load.py in ${SCRIPT_DIR}" >&2
  exit 1
fi

PROFILE_ARGS=()
if [[ "$PROFILE" == "release" ]]; then
  PROFILE_ARGS+=(--release)
fi

run_mode() {
  local label="$1"
  local perf_profile="$2"
  local out_dir="$3"
  local base_api_port="$4"
  local base_p2p_port="$5"
  local seed="$6"

  local started=0
  trap 'if [[ "$started" -eq 1 && -f "${out_dir}/stop.sh" ]]; then (cd "$out_dir" && ./stop.sh) || true; fi' RETURN

  echo ""
  echo "=== ${label} 10k TPS localnet ==="
  "${SCRIPT_DIR}/deploy_localnet.sh" \
    --iroha-dir "$IROHA_DIR" \
    --out-dir "$out_dir" \
    --peers "$PEERS" \
    --seed "$seed" \
    --build-line iroha3 \
    --perf-profile "$perf_profile" \
    --base-api-port "$base_api_port" \
    --base-p2p-port "$base_p2p_port" \
    --force \
    --skip-asset-register \
    "${PROFILE_ARGS[@]}"

  started=1

  TX_LOAD_ARGS=(
    --client-config "$out_dir/client.toml"
    --peer-count "$PEERS"
    --count "$COUNT"
    --parallel "$PARALLEL"
    --batch-size "$BATCH_SIZE"
    --batch-interval "$BATCH_INTERVAL"
    --drain-timeout "$DRAIN_TIMEOUT"
    --no-wait
    --no-index
  )
  if [[ "$PER_PEER" == true ]]; then
    TX_LOAD_ARGS+=(--per-peer)
  fi

  "$PYTHON_BIN" "${SCRIPT_DIR}/tx_load.py" "${TX_LOAD_ARGS[@]}"
}

if [[ "$MODE" == "permissioned" || "$MODE" == "both" ]]; then
  run_mode "Permissioned" "10k-permissioned" "${OUT_BASE}-permissioned" \
    "$BASE_API_PORT_PERM" "$BASE_P2P_PORT_PERM" "$SEED_PERM"
fi

if [[ "$MODE" == "npos" || "$MODE" == "both" ]]; then
  run_mode "NPoS" "10k-npos" "${OUT_BASE}-npos" \
    "$BASE_API_PORT_NPOS" "$BASE_P2P_PORT_NPOS" "$SEED_NPOS"
fi
