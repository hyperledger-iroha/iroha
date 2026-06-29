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
  --queue-soft-limit <N>  pause batches above this queue delta per shard (default: 50000)
  --queue-hard-limit <N>  abort batches above this queue delta per shard (default: 120000)
  --queue-wait-timeout <SEC>
                          seconds to wait below soft queue limit (default: 60)
  --out-base <DIR>        output directory base (default: /tmp/iroha-10k)
  --target-dir <DIR>      Set CARGO_TARGET_DIR for builds and binary reuse
  --fast                  Run cargo via scripts/cargo_fast.sh when available
  --fast-zero-debug       With --fast, set CARGO_PROFILE_{DEV,TEST}_DEBUG=0
  --fast-no-incremental   With --fast, set CARGO_INCREMENTAL=0
  --no-skip-build         Do not skip deploy_localnet cargo build when binaries already exist
  --release               use release binaries (default)
  --debug                 use debug binaries
  -h, --help              show this help
USAGE
}

require_option_value() {
  local flag="$1"
  local value="${2-}"
  if [[ -z "$value" ]] || [[ "$value" == --* ]]; then
    echo "Missing value for ${flag}" >&2
    exit 2
  fi
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
QUEUE_SOFT_LIMIT=50000
QUEUE_HARD_LIMIT=120000
QUEUE_WAIT_TIMEOUT=60
OUT_BASE="/tmp/iroha-10k"
PROFILE="release"
TARGET_DIR=""
USE_CARGO_FAST=false
FAST_ZERO_DEBUG=false
FAST_NO_INCREMENTAL=false
AUTO_SKIP_BUILD=true

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
    --queue-soft-limit)
      require_option_value "--queue-soft-limit" "${2-}"
      QUEUE_SOFT_LIMIT="$2"
      shift 2
      ;;
    --queue-hard-limit)
      require_option_value "--queue-hard-limit" "${2-}"
      QUEUE_HARD_LIMIT="$2"
      shift 2
      ;;
    --queue-wait-timeout)
      require_option_value "--queue-wait-timeout" "${2-}"
      QUEUE_WAIT_TIMEOUT="$2"
      shift 2
      ;;
    --out-base)
      require_option_value "--out-base" "${2-}"
      OUT_BASE="$2"
      shift 2
      ;;
    --target-dir)
      require_option_value "--target-dir" "${2-}"
      TARGET_DIR="$2"
      shift 2
      ;;
    --fast)
      USE_CARGO_FAST=true
      shift
      ;;
    --fast-zero-debug)
      FAST_ZERO_DEBUG=true
      shift
      ;;
    --fast-no-incremental)
      FAST_NO_INCREMENTAL=true
      shift
      ;;
    --no-skip-build)
      AUTO_SKIP_BUILD=false
      shift
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

resolve_dir() {
  local path="$1"
  local candidate
  if [[ "${path}" = /* ]]; then
    candidate="${path}"
  else
    candidate="${IROHA_DIR}/${path}"
  fi
  mkdir -p "${candidate}"
  (
    cd "${candidate}"
    pwd
  )
}

bin_name() {
  local raw="$1"
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*)
      printf '%s.exe\n' "${raw}"
      ;;
    *)
      printf '%s\n' "${raw}"
      ;;
  esac
}

profile_binary_exists() {
  local root="$1"
  local profile="$2"
  local raw="$3"
  local bin
  bin="$(bin_name "${raw}")"
  [[ -x "${root}/${profile}/${bin}" ]]
}

DEPLOY_ARGS=()
if [[ "$USE_CARGO_FAST" == true ]]; then
  cargo_fast_script="${IROHA_DIR}/scripts/cargo_fast.sh"
  if [[ ! -x "${cargo_fast_script}" ]]; then
    echo "scripts/cargo_fast.sh is not available or not executable" >&2
    exit 2
  fi
  DEPLOY_ARGS+=(--fast)
  if [[ "$FAST_ZERO_DEBUG" == true ]]; then
    DEPLOY_ARGS+=(--fast-zero-debug)
  fi
  if [[ "$FAST_NO_INCREMENTAL" == true ]]; then
    DEPLOY_ARGS+=(--fast-no-incremental)
  fi
elif [[ "$FAST_ZERO_DEBUG" == true || "$FAST_NO_INCREMENTAL" == true ]]; then
  echo "--fast-zero-debug and --fast-no-incremental require --fast" >&2
  exit 2
fi

if [[ -n "${TARGET_DIR}" ]]; then
  export CARGO_TARGET_DIR="$(resolve_dir "${TARGET_DIR}")"
fi
target_root="$(resolve_dir "${CARGO_TARGET_DIR:-target}")"
export CARGO_TARGET_DIR="${target_root}"
DEPLOY_ARGS+=(--target-dir "${target_root}")
CLI_BIN="${target_root}/${PROFILE}/$(bin_name "iroha")"

DEPLOY_ENV=()
if [[ "${AUTO_SKIP_BUILD}" == true ]] \
  && profile_binary_exists "${target_root}" "${PROFILE}" "kagami" \
  && profile_binary_exists "${target_root}" "${PROFILE}" "irohad" \
  && profile_binary_exists "${target_root}" "${PROFILE}" "iroha"; then
  DEPLOY_ENV+=(SKIP_TOOL_BUILD=true)
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
  local -a deploy_cmd=(
    "${SCRIPT_DIR}/deploy_localnet.sh"
    --iroha-dir "$IROHA_DIR"
    --out-dir "$out_dir"
    --peers "$PEERS"
    --seed "$seed"
    --build-line iroha3
    --perf-profile "$perf_profile"
    --base-api-port "$base_api_port"
    --base-p2p-port "$base_p2p_port"
    --force
    --skip-asset-register
    "${DEPLOY_ARGS[@]}"
  )
  if [[ ${#PROFILE_ARGS[@]} -gt 0 ]]; then
    deploy_cmd+=("${PROFILE_ARGS[@]}")
  fi
  if [[ ${#DEPLOY_ENV[@]} -gt 0 ]]; then
    env "${DEPLOY_ENV[@]}" "${deploy_cmd[@]}"
  else
    "${deploy_cmd[@]}"
  fi

  started=1

  TX_LOAD_ARGS=(
    --iroha-bin "$CLI_BIN"
    --client-config "$out_dir/client.toml"
    --peer-count "$PEERS"
    --base-api-port "$base_api_port"
    --count "$COUNT"
    --parallel "$PARALLEL"
    --batch-size "$BATCH_SIZE"
    --batch-interval "$BATCH_INTERVAL"
    --drain-timeout "$DRAIN_TIMEOUT"
    --queue-soft-limit "$QUEUE_SOFT_LIMIT"
    --queue-hard-limit "$QUEUE_HARD_LIMIT"
    --queue-wait-timeout "$QUEUE_WAIT_TIMEOUT"
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
