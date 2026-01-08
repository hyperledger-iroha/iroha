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
  --base-api-port <PORT>     Base Torii API port (default: 29080)
  --base-p2p-port <PORT>     Base P2P port (default: 33337)
  --bind-host <HOST>         Bind host (default: 127.0.0.1)
  --public-host <HOST>       Public host (default: 127.0.0.1)
  --release                  Build and run release binaries
  --no-sample-asset          Do not include kagami's sample asset
  --asset-id <ID>            Asset definition to register (default: pkr#wonderland)
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
ASSET_ID="pkr#wonderland"
SKIP_ASSET_REGISTER=false
TELEMETRY_PROFILE=""
TIMEOUT_SECS=30
FORCE=false
BLOCK_TIME_MS=""
COMMIT_TIME_MS=""
CURL_TIMEOUT_SECS=2

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

echo "Building Iroha tools ($PROFILE)..."
cd "$IROHA_DIR"
if [[ "$PROFILE" == "release" ]]; then
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

echo "Starting Iroha peers..."
cd "$OUT_DIR"
chmod +x start.sh stop.sh
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
    "http://$PUBLIC_HOST_URL:$BASE_API_PORT/status" >/dev/null 2>&1; then
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
  "http://$PUBLIC_HOST_URL:$BASE_API_PORT/status" || true

CFG="$OUT_DIR/client.toml"
if [[ "$SKIP_ASSET_REGISTER" != true ]]; then
  echo ""
  echo "Registering asset definition $ASSET_ID..."
  "$CLI_BIN" --config "$CFG" asset definition register --id "$ASSET_ID"
fi

echo ""
echo "Iroha localnet is running in $OUT_DIR."
echo "To stop: cd $OUT_DIR && ./stop.sh"
