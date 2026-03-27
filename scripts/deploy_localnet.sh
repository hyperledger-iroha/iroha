#!/usr/bin/env bash
set -euo pipefail

# Purpose: Build Iroha binaries and spin up a local multi-peer network using `kagami localnet`.
# Prerequisites: Rust toolchain (cargo), curl, and this repository checked out locally.
# Environment overrides:
#   IROHA_DIR        Path to the workspace root (default: repository root).
#   CARGO_TARGET_DIR Cargo target directory override (default: <IROHA_DIR>/target).
#   KAGAMI_BIN       Path to the `kagami` binary (default: <target-dir>/<profile>/kagami).
#   IROHAD_BIN       Path to the `irohad` binary (default: <IROHA_DIR>/target/<profile>/irohad).
#   IROHA_CLI_BIN    Path to the `iroha` CLI binary (default: <IROHA_DIR>/target/<profile>/iroha).
#   IROHA_LOCALNET_NOFILE_MIN Minimum RLIMIT_NOFILE for localnet peers (default: 4096).
#   IROHA_LOCALNET_GUEST_STACK_BYTES Override [concurrency].guest_stack_bytes in generated peer configs.
#   IROHA_LOCALNET_GAS_TO_STACK_MULTIPLIER Override [concurrency].gas_to_stack_multiplier in generated peer configs.
#   IROHA_LOCALNET_MEMORY_BUDGET_PROFILE Override [ivm].memory_budget_profile in generated peer configs.
#   IROHA_LOCALNET_MAX_STACK_BYTES Add/update a compute resource profile with this max_stack_bytes value.

usage() {
  cat <<'EOF'
Usage: deploy_localnet.sh [OPTIONS]

Builds `kagami`, `irohad`, and `iroha`, generates a fresh localnet, starts peers,
waits for readiness, and optionally registers an asset definition.

Options:
  --iroha-dir <DIR>          Workspace root (default: repo root)
  --out-dir <DIR>            Localnet output directory (default: /tmp/iroha-localnet)
  --peers <N>                Number of peers (default: 4)
  --seed <SEED>              Deterministic key seed (default: Iroha)
  --build-line <LINE>        Build line for generated configs: iroha2 or iroha3 (default: iroha3)
  --block-time-ms <MS>       Override block time (ms) in generated configs
  --commit-time-ms <MS>      Override commit time (ms) in generated configs
  --consensus-mode <MODE>    Override consensus mode (permissioned or npos)
  --perf-profile <NAME>      Apply Kagami localnet perf profile (10k-permissioned or 10k-npos)
  --queue-capacity <N>       Override transaction queue capacity in peer configs
  --queue-capacity-per-user <N> Override per-user transaction queue capacity (defaults to --queue-capacity when set)
  --queue-ttl-ms <MS>        Override queue transaction_time_to_live_ms (ms)
  --logger-level <LEVEL>     Override logger.level in peer configs (e.g., warn)
  --logger-filter <SPEC>     Override logger.filter in peer configs
  --base-api-port <PORT>     Base Torii API port (default: 29080)
  --base-p2p-port <PORT>     Base P2P port (default: 33337)
  --bind-host <HOST>         Bind host (default: 127.0.0.1)
  --public-host <HOST>       Public host (default: 127.0.0.1)
  --release                  Build and run release binaries
  --no-sample-asset          Do not include kagami's sample asset
  --asset-id <ID>            Canonical Base58 asset definition id to register (default: 7EAD8EFYUx1aVKZPUU1fyKvr8dF1)
  --asset-name <NAME>        Asset definition name to register (default: USD)
  --skip-asset-register      Skip asset definition registration
  --telemetry-profile <NAME> Set telemetry_profile in generated peer configs (e.g., extended)
  --timeout <SECS>           Seconds to wait for readiness (default: 30)
  --force                    Remove existing out-dir before regenerating
  -h, --help                 Show this help
EOF
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IROHA_DIR="${IROHA_DIR:-"$(cd "${SCRIPT_DIR}/.." && pwd)"}"

OUT_DIR="/tmp/iroha-localnet"
PEERS=4
SEED="Iroha"
BUILD_LINE="iroha3"
BASE_API_PORT=29080
BASE_P2P_PORT=33337
BIND_HOST="127.0.0.1"
PUBLIC_HOST="127.0.0.1"
PROFILE="debug"
SAMPLE_ASSET=true
ASSET_ID="7EAD8EFYUx1aVKZPUU1fyKvr8dF1"
ASSET_NAME="USD"
SKIP_ASSET_REGISTER=false
TELEMETRY_PROFILE=""
TIMEOUT_SECS=30
FORCE=false
BLOCK_TIME_MS=""
COMMIT_TIME_MS=""
CONSENSUS_MODE=""
PERF_PROFILE=""
QUEUE_CAPACITY=""
QUEUE_CAPACITY_PER_USER=""
QUEUE_TTL_MS=""
LOGGER_LEVEL=""
LOGGER_FILTER=""
CURL_TIMEOUT_SECS=2
PORT_SCAN_MAX_TRIES=200
SKIP_TOOL_BUILD="${SKIP_TOOL_BUILD:-false}"
PYTHON_BIN="${PYTHON_BIN:-python3}"
LOCALNET_GUEST_STACK_BYTES="${IROHA_LOCALNET_GUEST_STACK_BYTES:-}"
LOCALNET_GAS_TO_STACK_MULTIPLIER="${IROHA_LOCALNET_GAS_TO_STACK_MULTIPLIER:-}"
LOCALNET_MEMORY_BUDGET_PROFILE="${IROHA_LOCALNET_MEMORY_BUDGET_PROFILE:-}"
LOCALNET_MAX_STACK_BYTES="${IROHA_LOCALNET_MAX_STACK_BYTES:-${LOCALNET_GUEST_STACK_BYTES:-}}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --iroha-dir)
      IROHA_DIR="$2"
      shift 2
      ;;
    --out-dir)
      OUT_DIR="$2"
      shift 2
      ;;
    --peers)
      PEERS="$2"
      shift 2
      ;;
    --seed)
      SEED="$2"
      shift 2
      ;;
    --build-line)
      BUILD_LINE="$2"
      shift 2
      ;;
    --block-time-ms)
      BLOCK_TIME_MS="$2"
      shift 2
      ;;
    --commit-time-ms)
      COMMIT_TIME_MS="$2"
      shift 2
      ;;
    --consensus-mode)
      CONSENSUS_MODE="$2"
      shift 2
      ;;
    --perf-profile)
      PERF_PROFILE="$2"
      shift 2
      ;;
    --queue-capacity)
      QUEUE_CAPACITY="$2"
      shift 2
      ;;
    --queue-capacity-per-user)
      QUEUE_CAPACITY_PER_USER="$2"
      shift 2
      ;;
    --queue-ttl-ms)
      QUEUE_TTL_MS="$2"
      shift 2
      ;;
    --logger-level)
      LOGGER_LEVEL="$2"
      shift 2
      ;;
    --logger-filter)
      LOGGER_FILTER="$2"
      shift 2
      ;;
    --base-api-port)
      BASE_API_PORT="$2"
      shift 2
      ;;
    --base-p2p-port)
      BASE_P2P_PORT="$2"
      shift 2
      ;;
    --bind-host)
      BIND_HOST="$2"
      shift 2
      ;;
    --public-host)
      PUBLIC_HOST="$2"
      shift 2
      ;;
    --release)
      PROFILE="release"
      shift
      ;;
    --no-sample-asset)
      SAMPLE_ASSET=false
      shift
      ;;
    --asset-id)
      ASSET_ID="$2"
      shift 2
      ;;
    --asset-name)
      ASSET_NAME="$2"
      shift 2
      ;;
    --skip-asset-register)
      SKIP_ASSET_REGISTER=true
      shift
      ;;
    --telemetry-profile)
      TELEMETRY_PROFILE="$2"
      shift 2
      ;;
    --timeout)
      TIMEOUT_SECS="$2"
      shift 2
      ;;
    --force)
      FORCE=true
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

if [[ -n "$QUEUE_CAPACITY" && -z "$QUEUE_CAPACITY_PER_USER" ]]; then
  QUEUE_CAPACITY_PER_USER="$QUEUE_CAPACITY"
fi

BUILD_LINE_LOWER="$(printf '%s' "$BUILD_LINE" | tr '[:upper:]' '[:lower:]')"
case "$BUILD_LINE_LOWER" in
  iroha2|i2|2)
    BUILD_LINE="iroha2"
    ;;
  iroha3|i3|3)
    BUILD_LINE="iroha3"
    ;;
  *)
    echo "Invalid --build-line value: $BUILD_LINE (expected iroha2 or iroha3)" >&2
    exit 2
    ;;
esac

if [[ -z "$BLOCK_TIME_MS" && -z "$COMMIT_TIME_MS" && "$PROFILE" == "debug" ]]; then
  BLOCK_TIME_MS=1000
  COMMIT_TIME_MS=1000
  echo "Debug build: defaulting block/commit time to 1000ms for localnet stability."
fi

for cmd in cargo curl; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "Missing prerequisite: $cmd" >&2
    exit 1
  fi
done
if ! command -v "$PYTHON_BIN" >/dev/null 2>&1; then
  if command -v python >/dev/null 2>&1; then
    PYTHON_BIN="python"
  else
    echo "Missing prerequisite: python3 (or python)" >&2
    exit 1
  fi
fi

if [[ ! -d "$IROHA_DIR" ]]; then
  echo "IROHA_DIR does not exist: $IROHA_DIR" >&2
  exit 1
fi

if [[ -d "$OUT_DIR" ]]; then
  if [[ -f "$OUT_DIR/stop.sh" ]]; then
    echo "Stopping existing Iroha peers in $OUT_DIR..."
    (cd "$OUT_DIR" && ./stop.sh 2>/dev/null) || true
    for pidfile in "$OUT_DIR"/peer*.pid; do
      [[ -f "$pidfile" ]] || continue
      pid="$(cat "$pidfile" 2>/dev/null || true)"
      [[ -n "$pid" ]] || continue
      for _ in {1..20}; do
        if kill -0 "$pid" 2>/dev/null; then
          sleep 0.25
        else
          break
        fi
      done
      if kill -0 "$pid" 2>/dev/null; then
        kill -9 "$pid" 2>/dev/null || true
      fi
    done
  fi
  if [[ "$FORCE" == true ]]; then
    echo "Removing existing out-dir $OUT_DIR..."
    rm -rf "$OUT_DIR"
  else
    echo "Out-dir $OUT_DIR already exists. Re-run with --force to regenerate." >&2
    exit 1
  fi
fi

port_range_free() {
  local host="$1"
  local base="$2"
  local peers="$3"
  "$PYTHON_BIN" - "$host" "$base" "$peers" <<'PY'
import socket
import sys

host = sys.argv[1]
base = int(sys.argv[2])
peers = int(sys.argv[3])
for port in range(base, base + peers):
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        sock.bind((host, port))
    except OSError:
        sys.exit(1)
    finally:
        sock.close()
sys.exit(0)
PY
}

find_free_port_range() {
  local host="$1"
  local base="$2"
  local peers="$3"
  local tries="$4"
  for ((i = 0; i < tries; i++)); do
    local candidate=$((base + i))
    if port_range_free "$host" "$candidate" "$peers"; then
      echo "$candidate"
      return 0
    fi
  done
  return 1
}

if ! port_range_free "$BIND_HOST" "$BASE_API_PORT" "$PEERS"; then
  new_base=$(find_free_port_range "$BIND_HOST" "$BASE_API_PORT" "$PEERS" "$PORT_SCAN_MAX_TRIES") \
    || { echo "No free API port range found near ${BASE_API_PORT} (peers=${PEERS})." >&2; exit 2; }
  echo "API port range ${BASE_API_PORT}..$((BASE_API_PORT + PEERS - 1)) in use; using ${new_base}..$((new_base + PEERS - 1))."
  BASE_API_PORT="$new_base"
fi

if ! port_range_free "$BIND_HOST" "$BASE_P2P_PORT" "$PEERS"; then
  new_base=$(find_free_port_range "$BIND_HOST" "$BASE_P2P_PORT" "$PEERS" "$PORT_SCAN_MAX_TRIES") \
    || { echo "No free P2P port range found near ${BASE_P2P_PORT} (peers=${PEERS})." >&2; exit 2; }
  echo "P2P port range ${BASE_P2P_PORT}..$((BASE_P2P_PORT + PEERS - 1)) in use; using ${new_base}..$((new_base + PEERS - 1))."
  BASE_P2P_PORT="$new_base"
fi

echo "Building Iroha tools ($PROFILE)..."
cd "$IROHA_DIR"
if [[ "$SKIP_TOOL_BUILD" == "true" ]]; then
  echo "Skipping Iroha tool build; using existing binaries."
elif [[ "$PROFILE" == "release" ]]; then
  cargo build --release --bin kagami --bin irohad --bin iroha
else
  cargo build --bin kagami --bin irohad --bin iroha
fi

TARGET_DIR="${CARGO_TARGET_DIR:-"$IROHA_DIR/target"}"
if [[ "$TARGET_DIR" != /* ]]; then
  TARGET_DIR="$IROHA_DIR/$TARGET_DIR"
fi

KAGAMI_BIN="${KAGAMI_BIN:-"$TARGET_DIR/$PROFILE/kagami"}"
IROHAD_BIN="${IROHAD_BIN:-"$TARGET_DIR/$PROFILE/irohad"}"
CLI_BIN="${IROHA_CLI_BIN:-"$TARGET_DIR/$PROFILE/iroha"}"

echo "Generating localnet in $OUT_DIR..."
KAGAMI_ARGS=(
  localnet
  --build-line "$BUILD_LINE"
  --out-dir "$OUT_DIR"
  --peers "$PEERS"
  --seed "$SEED"
  --base-api-port "$BASE_API_PORT"
  --base-p2p-port "$BASE_P2P_PORT"
  --bind-host "$BIND_HOST"
  --public-host "$PUBLIC_HOST"
)
if [[ "$SAMPLE_ASSET" == true ]]; then
  KAGAMI_ARGS+=(--sample-asset)
fi
if [[ -n "$BLOCK_TIME_MS" ]]; then
  KAGAMI_ARGS+=(--block-time-ms "$BLOCK_TIME_MS")
fi
if [[ -n "$COMMIT_TIME_MS" ]]; then
  KAGAMI_ARGS+=(--commit-time-ms "$COMMIT_TIME_MS")
fi
if [[ -n "$CONSENSUS_MODE" ]]; then
  KAGAMI_ARGS+=(--consensus-mode "$CONSENSUS_MODE")
fi
if [[ -n "$PERF_PROFILE" ]]; then
  KAGAMI_ARGS+=(--perf-profile "$PERF_PROFILE")
fi
"$KAGAMI_BIN" "${KAGAMI_ARGS[@]}"

if [[ -n "$TELEMETRY_PROFILE" ]]; then
  echo "Setting telemetry_profile=${TELEMETRY_PROFILE} in peer configs..."
  for cfg in "$OUT_DIR"/peer*.toml; do
    [[ -f "$cfg" ]] || continue
    awk -v profile="$TELEMETRY_PROFILE" '
      BEGIN { inserted = 0; replaced = 0 }
      /^[[:space:]]*telemetry_profile[[:space:]]*=/ {
        print "telemetry_profile = \"" profile "\""
        replaced = 1
        next
      }
      /^\[/ {
        if (!inserted && !replaced) {
          print "telemetry_profile = \"" profile "\""
          inserted = 1
        }
      }
      { print }
      END {
        if (!inserted && !replaced) {
          print "telemetry_profile = \"" profile "\""
        }
      }
    ' "$cfg" > "${cfg}.tmp" && mv "${cfg}.tmp" "$cfg"
  done
fi

if [[ -n "$QUEUE_CAPACITY" || -n "$QUEUE_CAPACITY_PER_USER" || -n "$QUEUE_TTL_MS" ]]; then
  echo "Applying queue overrides in peer configs..."
  for cfg in "$OUT_DIR"/peer*.toml; do
    [[ -f "$cfg" ]] || continue
    awk -v cap="$QUEUE_CAPACITY" -v cap_user="$QUEUE_CAPACITY_PER_USER" -v ttl="$QUEUE_TTL_MS" '
      function flush_queue() {
        if (!in_queue) return
        if (cap != "" && !seen_cap) print "capacity = " cap
        if (cap_user != "" && !seen_cap_user) print "capacity_per_user = " cap_user
        if (ttl != "" && !seen_ttl) print "transaction_time_to_live_ms = " ttl
      }
      function flush_torii() {
        if (!in_torii) return
        if (cap != "" && !seen_torii_threshold) print "api_high_load_tx_threshold = " cap
      }
      BEGIN {
        in_queue = 0
        in_torii = 0
        seen_cap = 0
        seen_cap_user = 0
        seen_ttl = 0
        seen_torii_threshold = 0
      }
      /^[[:space:]]*\[queue\][[:space:]]*$/ {
        flush_torii()
        in_torii = 0
        seen_torii_threshold = 0
        flush_queue()
        in_queue = 1
        seen_cap = 0
        seen_cap_user = 0
        seen_ttl = 0
        print
        next
      }
      /^[[:space:]]*\[torii\][[:space:]]*$/ {
        flush_queue()
        in_queue = 0
        flush_torii()
        in_torii = 1
        seen_torii_threshold = 0
        print
        next
      }
      /^[[:space:]]*\[/ {
        flush_queue()
        in_queue = 0
        flush_torii()
        in_torii = 0
        print
        next
      }
      {
        if (in_queue) {
          if (cap != "" && $1 == "capacity") {
            print "capacity = " cap
            seen_cap = 1
            next
          }
          if (cap_user != "" && $1 == "capacity_per_user") {
            print "capacity_per_user = " cap_user
            seen_cap_user = 1
            next
          }
          if (ttl != "" && $1 == "transaction_time_to_live_ms") {
            print "transaction_time_to_live_ms = " ttl
            seen_ttl = 1
            next
          }
        } else if (in_torii) {
          if (cap != "" && $1 == "api_high_load_tx_threshold") {
            print "api_high_load_tx_threshold = " cap
            seen_torii_threshold = 1
            next
          }
        }
        print
      }
      END {
        flush_queue()
        flush_torii()
      }
    ' "$cfg" > "${cfg}.tmp" && mv "${cfg}.tmp" "$cfg"
  done
fi

if [[ -n "$LOGGER_LEVEL" || -n "$LOGGER_FILTER" ]]; then
  echo "Applying logger overrides in peer configs..."
  for cfg in "$OUT_DIR"/peer*.toml; do
    [[ -f "$cfg" ]] || continue
    awk -v level="$LOGGER_LEVEL" -v filter="$LOGGER_FILTER" '
      function flush_logger() {
        if (!in_logger) return
        if (level != "" && !seen_level) print "level = \"" level "\""
        if (filter != "" && !seen_filter) print "filter = \"" filter "\""
      }
      BEGIN {
        in_logger = 0
        seen_level = 0
        seen_filter = 0
      }
      /^[[:space:]]*\[logger\][[:space:]]*$/ {
        flush_logger()
        in_logger = 1
        seen_level = 0
        seen_filter = 0
        print
        next
      }
      /^[[:space:]]*\[/ {
        flush_logger()
        in_logger = 0
        print
        next
      }
      {
        if (in_logger) {
          if (level != "" && $1 == "level") {
            print "level = \"" level "\""
            seen_level = 1
            next
          }
          if (filter != "" && $1 == "filter") {
            print "filter = \"" filter "\""
            seen_filter = 1
            next
          }
        }
        print
      }
      END {
        flush_logger()
      }
    ' "$cfg" > "${cfg}.tmp" && mv "${cfg}.tmp" "$cfg"
  done
fi

if [[ -n "$LOCALNET_GUEST_STACK_BYTES" || -n "$LOCALNET_GAS_TO_STACK_MULTIPLIER" || -n "$LOCALNET_MEMORY_BUDGET_PROFILE" || -n "$LOCALNET_MAX_STACK_BYTES" ]]; then
  if [[ -z "$LOCALNET_GUEST_STACK_BYTES" || -z "$LOCALNET_GAS_TO_STACK_MULTIPLIER" || -z "$LOCALNET_MEMORY_BUDGET_PROFILE" || -z "$LOCALNET_MAX_STACK_BYTES" ]]; then
    echo "IROHA localnet stack overrides require guest stack bytes, gas-to-stack multiplier, memory budget profile, and max stack bytes together." >&2
    exit 2
  fi

  echo "Applying IVM stack overrides in peer configs..."
  for cfg in "$OUT_DIR"/peer*.toml; do
    [[ -f "$cfg" ]] || continue
    cat >> "$cfg" <<EOF

[ivm]
memory_budget_profile = "$LOCALNET_MEMORY_BUDGET_PROFILE"

[concurrency]
guest_stack_bytes = $LOCALNET_GUEST_STACK_BYTES
gas_to_stack_multiplier = $LOCALNET_GAS_TO_STACK_MULTIPLIER

[compute]
default_resource_profile = "$LOCALNET_MEMORY_BUDGET_PROFILE"

[compute.resource_profiles."$LOCALNET_MEMORY_BUDGET_PROFILE"]
max_cycles = 10000000
max_memory_bytes = 268435456
max_stack_bytes = $LOCALNET_MAX_STACK_BYTES
max_io_bytes = 25165824
max_egress_bytes = 12582912
allow_gpu_hints = true
allow_wasi = true
EOF
  done
fi

echo "Starting Iroha peers..."
cd "$OUT_DIR"
chmod +x start.sh stop.sh
LOCALNET_NOFILE_MIN="${IROHA_LOCALNET_NOFILE_MIN:-4096}"
CURRENT_NOFILE="$(ulimit -n 2>/dev/null || echo 0)"
if [[ "$CURRENT_NOFILE" -lt "$LOCALNET_NOFILE_MIN" ]]; then
  if ulimit -n "$LOCALNET_NOFILE_MIN" 2>/dev/null; then
    echo "Raised RLIMIT_NOFILE to ${LOCALNET_NOFILE_MIN} for localnet."
  else
    echo "Warning: failed to raise RLIMIT_NOFILE (current=${CURRENT_NOFILE})." >&2
  fi
fi
if [[ -z "${RUST_LOG:-}" && "$PROFILE" == "debug" ]]; then
  export RUST_LOG="warn"
fi
IROHAD_BIN="$IROHAD_BIN" ./start.sh

format_host_for_url() {
  local host="$1"
  if [[ "$host" == \[*\] ]]; then
    printf '%s' "$host"
  elif [[ "$host" == *:* ]]; then
    printf '[%s]' "$host"
  else
    printf '%s' "$host"
  fi
}

PUBLIC_HOST_URL="$(format_host_for_url "$PUBLIC_HOST")"

echo "Waiting for network to be ready..."
READY=false
for ((i = 1; i <= TIMEOUT_SECS; i++)); do
  if curl -sf --connect-timeout "$CURL_TIMEOUT_SECS" --max-time "$CURL_TIMEOUT_SECS" \
    "http://$PUBLIC_HOST_URL:$BASE_API_PORT/health" >/dev/null 2>&1; then
    READY=true
    break
  fi
  sleep 1
done

if [[ "$READY" != true ]]; then
  echo "Network did not become ready after ${TIMEOUT_SECS}s." >&2
  echo "Check logs in $OUT_DIR or re-run with a longer --timeout." >&2
  exit 1
fi

echo "Network is ready:"
curl -sf --connect-timeout "$CURL_TIMEOUT_SECS" --max-time "$CURL_TIMEOUT_SECS" \
  "http://$PUBLIC_HOST_URL:$BASE_API_PORT/health" || true

# /status may be protected by operator auth depending on Torii config; it is not required for
# localnet readiness checks.
curl -s --connect-timeout "$CURL_TIMEOUT_SECS" --max-time "$CURL_TIMEOUT_SECS" \
  "http://$PUBLIC_HOST_URL:$BASE_API_PORT/status" || true

echo "Waiting for consensus mode tag..."
MODE_TAG=""
MODE_READY=false
# Mode tag is informational and may be behind operator auth depending on Torii config.
# Don't block localnet startup for the full readiness timeout.
MODE_TAG_TIMEOUT_SECS=10
for ((i = 1; i <= MODE_TAG_TIMEOUT_SECS; i++)); do
  MODE_TAG="$(
    { curl -s --connect-timeout "$CURL_TIMEOUT_SECS" --max-time "$CURL_TIMEOUT_SECS" \
        "http://$PUBLIC_HOST_URL:$BASE_API_PORT/v1/sumeragi/status" 2>/dev/null || true; } \
      | sed -n 's/.*"mode_tag"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
      | head -n1
  )"
  if [[ -n "$MODE_TAG" ]]; then
    MODE_READY=true
    break
  fi
  sleep 1
done
if [[ "$MODE_READY" == true ]]; then
  echo "Consensus mode tag: $MODE_TAG"
else
  echo "Warning: consensus mode tag not reported yet; /status may show a default mode briefly." >&2
fi

CFG="$OUT_DIR/client.toml"
if [[ "$SKIP_ASSET_REGISTER" != true ]]; then
  echo ""
  echo "Registering asset definition $ASSET_ID..."
  "$CLI_BIN" --config "$CFG" asset definition register --id "$ASSET_ID" --name "$ASSET_NAME" --scale 0
fi

echo ""
echo "Iroha localnet is running in $OUT_DIR."
echo "To stop: cd $OUT_DIR && ./stop.sh"
