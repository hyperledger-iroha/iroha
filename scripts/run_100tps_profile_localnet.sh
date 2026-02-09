#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: run_100tps_profile_localnet.sh [OPTIONS]

Spin up a 7-peer localnet (permissioned and/or NPoS), drive ~100 TPS of ping
transactions, and capture a peer0 CPU profile via /debug/pprof/profile.

This wrapper applies conservative queue limits to avoid unbounded RAM growth
during perf/profiling runs.

Options:
  --mode <MODE>              permissioned, npos, or both (default: both)
  --peers <N>                number of peers (default: 7)
  --tps <N>                  target TPS across the whole network (default: 100)
  --duration <SEC>           load duration seconds (default: 120)
  --parallel <N>             total ping parallelism across peers (default: 140)
  --queue-capacity <N>       queue.capacity/capacity_per_user override (default: 20000)
  --queue-ttl-ms <MS>        queue.transaction_time_to_live_ms override (default: 600000)
  --queue-soft-limit <N>     tx_load soft queue delta limit (default: 5000)
  --queue-hard-limit <N>     tx_load hard queue delta limit (default: 15000)
  --localnet-timeout <SEC>   seconds to wait for localnet readiness (default: 180)
  --cli-timeout <SEC>        tx_load CLI timeout seconds (default: 15)
  --pprof-seconds <SEC>      CPU profile duration seconds (default: 30)
  --load-drain-timeout <SEC> tx_load drain timeout seconds (default: 180)
  --rust-log <SPEC>          RUST_LOG override used when launching peers (default: warn)
  --artifact-base <DIR>      artifact base directory (default: ./artifacts/localnet-100tps-profile)
  --out-base <DIR>           localnet base directory (default: /tmp/iroha-localnet-100tps)
  --base-api-port-perm <N>   permissioned base API port (default: 29080)
  --base-p2p-port-perm <N>   permissioned base P2P port (default: 33337)
  --base-api-port-npos <N>   NPoS base API port (default: 39080)
  --base-p2p-port-npos <N>   NPoS base P2P port (default: 34337)
  --release                  use release binaries (default)
  --debug                    use debug binaries
  -h, --help                 show this help
USAGE
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IROHA_DIR="${IROHA_DIR:-"$(cd "${SCRIPT_DIR}/.." && pwd)"}"
PYTHON_BIN="${PYTHON_BIN:-python3}"

MODE="both"
PEERS=7
TPS=100
DURATION=120
PARALLEL=140
QUEUE_CAPACITY=20000
QUEUE_TTL_MS=600000
QUEUE_SOFT_LIMIT=5000
QUEUE_HARD_LIMIT=15000
LOCALNET_TIMEOUT=180
CLI_TIMEOUT=15
PPROF_SECONDS=30
LOAD_DRAIN_TIMEOUT=180
RUST_LOG_SPEC="${RUST_LOG_SPEC:-warn}"
ARTIFACT_BASE=""
OUT_BASE="/tmp/iroha-localnet-100tps"
PROFILE="release"

BASE_API_PORT_PERM="${BASE_API_PORT_PERM:-29080}"
BASE_P2P_PORT_PERM="${BASE_P2P_PORT_PERM:-33337}"
BASE_API_PORT_NPOS="${BASE_API_PORT_NPOS:-39080}"
BASE_P2P_PORT_NPOS="${BASE_P2P_PORT_NPOS:-34337}"
SEED_PERM="profile-100tps-permissioned"
SEED_NPOS="profile-100tps-npos"

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
    --tps)
      TPS="$2"
      shift 2
      ;;
    --duration)
      DURATION="$2"
      shift 2
      ;;
    --parallel)
      PARALLEL="$2"
      shift 2
      ;;
    --queue-capacity)
      QUEUE_CAPACITY="$2"
      shift 2
      ;;
    --queue-ttl-ms)
      QUEUE_TTL_MS="$2"
      shift 2
      ;;
    --queue-soft-limit)
      QUEUE_SOFT_LIMIT="$2"
      shift 2
      ;;
    --queue-hard-limit)
      QUEUE_HARD_LIMIT="$2"
      shift 2
      ;;
    --localnet-timeout)
      LOCALNET_TIMEOUT="$2"
      shift 2
      ;;
    --cli-timeout)
      CLI_TIMEOUT="$2"
      shift 2
      ;;
    --pprof-seconds)
      PPROF_SECONDS="$2"
      shift 2
      ;;
    --load-drain-timeout)
      LOAD_DRAIN_TIMEOUT="$2"
      shift 2
      ;;
    --rust-log)
      RUST_LOG_SPEC="$2"
      shift 2
      ;;
    --artifact-base)
      ARTIFACT_BASE="$2"
      shift 2
      ;;
    --out-base)
      OUT_BASE="$2"
      shift 2
      ;;
    --base-api-port-perm)
      BASE_API_PORT_PERM="$2"
      shift 2
      ;;
    --base-p2p-port-perm)
      BASE_P2P_PORT_PERM="$2"
      shift 2
      ;;
    --base-api-port-npos)
      BASE_API_PORT_NPOS="$2"
      shift 2
      ;;
    --base-p2p-port-npos)
      BASE_P2P_PORT_NPOS="$2"
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

if (( LOAD_DRAIN_TIMEOUT < 0 )); then
  echo "--load-drain-timeout must be >= 0" >&2
  exit 2
fi

if [[ -z "$ARTIFACT_BASE" ]]; then
  ARTIFACT_BASE="$(pwd)/artifacts/localnet-100tps-profile"
fi
mkdir -p "$ARTIFACT_BASE"

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

PPROF_TARGET_DIR="${IROHA_DIR}/target/pprof"
PPROF_IROHAD_BIN="${PPROF_TARGET_DIR}/release/irohad"
if [[ "$PROFILE" != "release" ]]; then
  PPROF_IROHAD_BIN="${PPROF_TARGET_DIR}/debug/irohad"
fi

echo "Building irohad with profiling endpoint ($PROFILE)..."
mkdir -p "$PPROF_TARGET_DIR"
if [[ "$PROFILE" == "release" ]]; then
  (cd "$IROHA_DIR" && CARGO_TARGET_DIR="$PPROF_TARGET_DIR" cargo build --release -p irohad --features profiling-endpoint)
else
  (cd "$IROHA_DIR" && CARGO_TARGET_DIR="$PPROF_TARGET_DIR" cargo build -p irohad --features profiling-endpoint)
fi

wait_for_pid_with_timeout() {
  local pid="$1"
  local timeout="$2"
  local label="$3"
  local started_at now
  started_at="$(date +%s)"
  while kill -0 "$pid" 2>/dev/null; do
    now="$(date +%s)"
    if (( now - started_at >= timeout )); then
      echo "Warning: ${label} exceeded ${timeout}s; terminating pid ${pid}" >&2
      kill -TERM "$pid" 2>/dev/null || true
      sleep 5
      if kill -0 "$pid" 2>/dev/null; then
        kill -KILL "$pid" 2>/dev/null || true
      fi
      wait "$pid" || true
      return 124
    fi
    sleep 1
  done
  wait "$pid" || true
  return 0
}

run_mode() {
  local label="$1"
  local consensus_mode="$2"
  local out_dir="$3"
  local base_api_port="$4"
  local base_p2p_port="$5"
  local seed="$6"

  local run_id
  run_id="$(date +%Y%m%dT%H%M%S)"
  local artifact_dir="${ARTIFACT_BASE}/${run_id}-${label}"
  mkdir -p "$artifact_dir"

  local started=0
  trap 'if [[ "$started" -eq 1 && -f "${out_dir}/stop.sh" ]]; then (cd "$out_dir" && ./stop.sh) || true; fi' RETURN

  echo ""
  echo "=== ${label} localnet (${PEERS} peers, ~${TPS} TPS, ${DURATION}s) ==="
  IROHAD_BIN="$PPROF_IROHAD_BIN" \
    RUST_LOG="$RUST_LOG_SPEC" \
    "${SCRIPT_DIR}/deploy_localnet.sh" \
      --iroha-dir "$IROHA_DIR" \
      --out-dir "$out_dir" \
      --peers "$PEERS" \
      --seed "$seed" \
      --build-line iroha3 \
      --consensus-mode "$consensus_mode" \
      --block-time-ms 1000 \
      --commit-time-ms 1000 \
      --queue-capacity "$QUEUE_CAPACITY" \
      --queue-ttl-ms "$QUEUE_TTL_MS" \
      --base-api-port "$base_api_port" \
      --base-p2p-port "$base_p2p_port" \
      --timeout "$LOCALNET_TIMEOUT" \
      --force \
      --skip-asset-register \
      "${PROFILE_ARGS[@]}"

  started=1

  local torii_url
  torii_url="$(awk -F'\"' '/^[[:space:]]*torii_url[[:space:]]*=/{print $2; exit}' "${out_dir}/client.toml")"
  if [[ -z "$torii_url" ]]; then
    echo "Failed to parse torii_url from ${out_dir}/client.toml" >&2
    exit 1
  fi
  local pprof_url="${torii_url%/}/debug/pprof/profile?seconds=${PPROF_SECONDS}"

  local count=$((TPS * DURATION))
  local batch_size=$TPS
  local load_timeout=$((DURATION + LOAD_DRAIN_TIMEOUT + 120))

  echo "Artifacts: ${artifact_dir}"
  echo "Torii: ${torii_url}"
  echo "Load: count=${count} batch=${batch_size}/s parallel=${PARALLEL}"
  echo "RUST_LOG: ${RUST_LOG_SPEC}"
  echo ""

  # Drive load in the background so we can capture a steady-state pprof profile.
  local tx_log="${artifact_dir}/tx_load.log"
  PYTHONUNBUFFERED=1 "$PYTHON_BIN" "${SCRIPT_DIR}/tx_load.py" \
    --client-config "${out_dir}/client.toml" \
    --peer-count "$PEERS" \
    --base-api-port "$(printf '%s' "$torii_url" | sed -E 's#.*:([0-9]+)/?$#\1#')" \
    --count "$count" \
    --parallel "$PARALLEL" \
    --batch-size "$batch_size" \
    --batch-interval 1 \
    --drain-timeout "$LOAD_DRAIN_TIMEOUT" \
    --cli-timeout "$CLI_TIMEOUT" \
    --queue-soft-limit "$QUEUE_SOFT_LIMIT" \
    --queue-hard-limit "$QUEUE_HARD_LIMIT" \
    --queue-wait-timeout 60 \
    --no-wait \
    --no-index \
    --continue-on-failure \
    >"$tx_log" 2>&1 &
  local load_pid=$!

  sleep 10

  local pprof_out="${artifact_dir}/pprof_peer0.pb.gz"
  echo "Capturing CPU profile (${PPROF_SECONDS}s) ..."
  if curl -fsS -m "$((PPROF_SECONDS + 60))" "$pprof_url" >"$pprof_out"; then
    echo "Saved profile: ${pprof_out}"
  else
    echo "Warning: failed to capture pprof profile from ${pprof_url}" >&2
    rm -f "$pprof_out"
  fi

  if ! wait_for_pid_with_timeout "$load_pid" "$load_timeout" "tx_load (${label})"; then
    echo "Warning: tx_load did not finish cleanly for ${label}; see ${tx_log}" >&2
  fi

  # Copy the first peer log for quick inspection (others can be huge).
  if [[ -f "${out_dir}/peer0.log" ]]; then
    cp "${out_dir}/peer0.log" "${artifact_dir}/peer0.log"
  fi

  (cd "$out_dir" && ./stop.sh) || true
}

if [[ "$MODE" == "permissioned" || "$MODE" == "both" ]]; then
  run_mode "permissioned" "permissioned" "${OUT_BASE}-permissioned" \
    "$BASE_API_PORT_PERM" "$BASE_P2P_PORT_PERM" "$SEED_PERM"
fi

if [[ "$MODE" == "npos" || "$MODE" == "both" ]]; then
  run_mode "npos" "npos" "${OUT_BASE}-npos" \
    "$BASE_API_PORT_NPOS" "$BASE_P2P_PORT_NPOS" "$SEED_NPOS"
fi
