#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: training_script_2.sh [OPTIONS]

Run the localnet training flow (asset + multisig) with bounded readiness checks.

Options:
  --runs <N>            Number of runs to execute (default: 1)
  --out-dir <DIR>       Localnet output directory (default: /tmp/iroha-localnet-training)
  --seed <SEED>         Kagami seed for localnet generation (default: training)
  --peers <N>           Number of peers (default: 4)
  --base-api-port <P>   Base Torii API port (default: 29080)
  --base-p2p-port <P>   Base P2P port (default: 33337)
  --auto-ports          Auto-advance base ports if the range is already in use
  --profile <NAME>      Cargo profile: release or debug (default: release)
  --no-build            Skip cargo build (assumes binaries already exist)
  --force               Remove existing run directories under --out-dir
  --ready-timeout <S>   Seconds to wait for /status (default: 30)
  --height-timeout <S>  Seconds to wait for block height targets (default: 30)
  --stall-threshold <S> Seconds to flag slow block cadence (default: 40)
  -h, --help            Show this help
EOF
}

RUNS=1
OUT_DIR="/tmp/iroha-localnet-training"
SEED="training"
PEERS=4
BASE_API_PORT=29080
BASE_P2P_PORT=33337
PROFILE="release"
DO_BUILD=true
READY_TIMEOUT=30
HEIGHT_TIMEOUT=30
FORCE=false
AUTO_PORTS=false
STALL_THRESHOLD=40
PUBLIC_HOST="127.0.0.1"
BIND_HOST="127.0.0.1"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --runs)
      RUNS="$2"
      shift 2
      ;;
    --out-dir)
      OUT_DIR="$2"
      shift 2
      ;;
    --seed)
      SEED="$2"
      shift 2
      ;;
    --peers)
      PEERS="$2"
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
    --auto-ports)
      AUTO_PORTS=true
      shift
      ;;
    --profile)
      PROFILE="$2"
      shift 2
      ;;
    --no-build)
      DO_BUILD=false
      shift
      ;;
    --force)
      FORCE=true
      shift
      ;;
    --ready-timeout)
      READY_TIMEOUT="$2"
      shift 2
      ;;
    --height-timeout)
      HEIGHT_TIMEOUT="$2"
      shift 2
      ;;
    --stall-threshold)
      STALL_THRESHOLD="$2"
      shift 2
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

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

if [[ "$PROFILE" != "release" && "$PROFILE" != "debug" ]]; then
  echo "Invalid --profile: $PROFILE (expected release or debug)" >&2
  exit 2
fi

for cmd in cargo curl python3 rg; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "Missing prerequisite: $cmd" >&2
    exit 1
  fi
done

if [[ "$DO_BUILD" == true ]]; then
  echo "Building kagami/irohad/iroha ($PROFILE)..."
  if [[ "$PROFILE" == "release" ]]; then
    cargo build --release --bin kagami --bin irohad --bin iroha
  else
    cargo build --bin kagami --bin irohad --bin iroha
  fi
fi

TARGET_DIR="${CARGO_TARGET_DIR:-"$REPO_ROOT/target"}"
if [[ "$TARGET_DIR" != /* ]]; then
  TARGET_DIR="$REPO_ROOT/$TARGET_DIR"
fi

KAGAMI_BIN="${KAGAMI_BIN:-"$TARGET_DIR/$PROFILE/kagami"}"
IROHAD_BIN="${IROHAD_BIN:-"$TARGET_DIR/$PROFILE/irohad"}"
IROHA_BIN="${IROHA_BIN:-"$TARGET_DIR/$PROFILE/iroha"}"

for bin in "$KAGAMI_BIN" "$IROHAD_BIN" "$IROHA_BIN"; do
  if [[ ! -x "$bin" ]]; then
    echo "Missing binary: $bin" >&2
    exit 1
  fi
done

status_url() {
  printf 'http://%s:%s/status' "$BIND_HOST" "$BASE_API_PORT"
}

wait_for_ready() {
  local deadline=$((SECONDS + READY_TIMEOUT))
  while ((SECONDS < deadline)); do
    if curl -sf "$(status_url)" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  return 1
}

fetch_height() {
  local payload
  payload="$(curl -sf "$(status_url)" 2>/dev/null || true)"
  STATUS_PAYLOAD="$payload" python3 - <<'PY'
import json
import os

raw = os.environ.get("STATUS_PAYLOAD", "").strip()
if not raw:
    print(0)
    raise SystemExit(0)
try:
    data = json.loads(raw)
except json.JSONDecodeError:
    print(0)
    raise SystemExit(0)
height = data.get("blocks", data.get("height", 0))
print(int(height or 0))
PY
}

fetch_commit_time_ms() {
  local payload
  payload="$(curl -sf "$(status_url)" 2>/dev/null || true)"
  STATUS_PAYLOAD="$payload" python3 - <<'PY'
import json
import os

raw = os.environ.get("STATUS_PAYLOAD", "").strip()
if not raw:
    print(0)
    raise SystemExit(0)
try:
    data = json.loads(raw)
except json.JSONDecodeError:
    print(0)
    raise SystemExit(0)
value = data.get("commit_time_ms", 0)
try:
    print(int(value or 0))
except (TypeError, ValueError):
    print(0)
PY
}

wait_for_height() {
  local target="$1"
  local start=$SECONDS
  local deadline=$((start + HEIGHT_TIMEOUT))
  local height=0
  while ((SECONDS < deadline)); do
    height="$(fetch_height)"
    if [[ ! "$height" =~ ^[0-9]+$ ]]; then
      height=0
    fi
    if [[ "$height" -ge "$target" ]]; then
      echo "$((SECONDS - start))"
      return 0
    fi
    sleep 1
  done
  return 1
}

retry_cmd() {
  local label="$1"
  local attempts="$2"
  local delay="$3"
  shift 3
  local attempt=1
  while true; do
    if "$@"; then
      return 0
    fi
    if [[ "$attempt" -ge "$attempts" ]]; then
      echo "[run $run] ${label} failed after ${attempts} attempts"
      return 1
    fi
    echo "[run $run] ${label} attempt ${attempt}/${attempts} failed; retrying..."
    sleep "$delay"
    attempt=$((attempt + 1))
  done
}

retry_cmd_output() {
  local label="$1"
  local attempts="$2"
  local delay="$3"
  shift 3
  local attempt=1
  local output=""
  while true; do
    output=""
    if output="$("$@")"; then
      printf '%s' "$output"
      return 0
    fi
    if [[ "$attempt" -ge "$attempts" ]]; then
      echo "[run $run] ${label} failed after ${attempts} attempts" >&2
      return 1
    fi
    echo "[run $run] ${label} attempt ${attempt}/${attempts} failed; retrying..." >&2
    sleep "$delay"
    attempt=$((attempt + 1))
  done
}

wait_for_multisig_proposal() {
  local cfg="$1"
  local account="$2"
  local hash="$3"
  local proposal_key="multisig/proposals/${hash}"
  retry_cmd "multisig proposal ready" 10 1 \
    "$IROHA_BIN" --config "$cfg" account meta get \
    --id "$account" \
    --key "$proposal_key"
}

wait_for_account() {
  local cfg="$1"
  local account="$2"
  retry_cmd "account ready (${account})" 10 1 \
    "$IROHA_BIN" --config "$cfg" account get \
    --id "$account"
}

retry_list_json() {
  local label="$1"
  local attempts="$2"
  local delay="$3"
  shift 3
  local attempt=1
  local output=""
  while true; do
    output=""
    if output="$("$@")"; then
      if output="$(printf '%s' "$output" | extract_json_output)"; then
        if [[ -n "$output" && "$output" != "{}" && "$output" != "[]" ]]; then
          printf '%s' "$output"
          return 0
        fi
      fi
    fi
    if [[ "$attempt" -ge "$attempts" ]]; then
      echo "[run $run] ${label} failed after ${attempts} attempts" >&2
      return 1
    fi
    echo "[run $run] ${label} attempt ${attempt}/${attempts} failed; retrying..." >&2
    sleep "$delay"
    attempt=$((attempt + 1))
  done
}

extract_json_output() {
  python3 -c 'import json,sys
raw = sys.stdin.read()
if not raw:
    raise SystemExit(1)
decoder = json.JSONDecoder()
for idx, ch in enumerate(raw):
    if ch not in "{[":
        continue
    try:
        value, _ = decoder.raw_decode(raw[idx:])
    except json.JSONDecodeError:
        continue
    sys.stdout.write(json.dumps(value))
    raise SystemExit(0)
raise SystemExit(1)
'
}

read_toml_string() {
  local key="$1"
  local path="$2"
  python3 - "$key" "$path" <<'PY'
import re
import sys

key = sys.argv[1]
path = sys.argv[2]
text = open(path, encoding="utf-8").read()
pattern = rf'(?m)^\s*{re.escape(key)}\s*=\s*"([^"]*)"'
match = re.search(pattern, text)
if not match:
    raise SystemExit(1)
print(match.group(1))
PY
}

read_public_key() {
  read_toml_string "public_key" "$1"
}

read_domain() {
  read_toml_string "domain" "$1"
}

generate_client_configs() {
  local base_config="$1"
  local out_dir="$2"
  local domain="$3"
  local seed_prefix="$4"
  local names_csv="$5"
  "$KAGAMI_BIN" client-configs \
    --base-config "$base_config" \
    --out-dir "$out_dir" \
    --domain "$domain" \
    --seed-prefix "$seed_prefix" \
    --names "$names_csv"
}

stop_localnet() {
  local run_dir="$1"
  if [[ -f "$run_dir/stop.sh" ]]; then
    (cd "$run_dir" && ./stop.sh) || true
  fi
  for pidfile in "$run_dir"/peer*.pid; do
    [[ -f "$pidfile" ]] || continue
    pid="$(cat "$pidfile" 2>/dev/null || true)"
    [[ -n "$pid" ]] || continue
    for _ in {1..40}; do
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
}

port_is_free() {
  local host="$1"
  local port="$2"
  python3 - "$host" "$port" <<'PY'
import socket
import sys

host = sys.argv[1]
port = int(sys.argv[2])
sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
try:
    sock.bind((host, port))
except OSError:
    sys.exit(1)
finally:
    sock.close()
sys.exit(0)
PY
}

collect_used_ports() {
  local api_base="$1"
  local p2p_base="$2"
  for offset in $(seq 0 $((PEERS - 1))); do
    local api_port=$((api_base + offset))
    local p2p_port=$((p2p_base + offset))
    if ! port_is_free "$BIND_HOST" "$api_port"; then
      printf 'api:%s\n' "$api_port"
    fi
    if ! port_is_free "$BIND_HOST" "$p2p_port"; then
      printf 'p2p:%s\n' "$p2p_port"
    fi
  done
}

ensure_ports_available() {
  local api_base="$BASE_API_PORT"
  local p2p_base="$BASE_P2P_PORT"
  local used=()
  while IFS= read -r line; do
    [[ -n "$line" ]] || continue
    used+=("$line")
  done < <(collect_used_ports "$api_base" "$p2p_base")
  if ((${#used[@]} == 0)); then
    return 0
  fi
  if [[ "$AUTO_PORTS" != true ]]; then
    echo "[run $run] ports already in use: ${used[*]}" >&2
    echo "[run $run] stop existing localnet or pass --base-api-port/--base-p2p-port (or --auto-ports)" >&2
    return 1
  fi
  local attempts=50
  for _ in $(seq 1 "$attempts"); do
    api_base=$((api_base + PEERS))
    p2p_base=$((p2p_base + PEERS))
    if ((api_base + PEERS - 1 > 65535 || p2p_base + PEERS - 1 > 65535)); then
      break
    fi
    used=()
    while IFS= read -r line; do
      [[ -n "$line" ]] || continue
      used+=("$line")
    done < <(collect_used_ports "$api_base" "$p2p_base")
    if ((${#used[@]} == 0)); then
      BASE_API_PORT="$api_base"
      BASE_P2P_PORT="$p2p_base"
      echo "[run $run] ports busy; using base-api-port=$BASE_API_PORT base-p2p-port=$BASE_P2P_PORT"
      return 0
    fi
  done
  echo "[run $run] unable to find a free port range after ${attempts} attempts" >&2
  return 1
}

verify_peers_started() {
  local run_dir="$1"
  local dead_peers=()
  for pidfile in "$run_dir"/peer*.pid; do
    [[ -f "$pidfile" ]] || continue
    pid="$(cat "$pidfile" 2>/dev/null || true)"
    [[ -n "$pid" ]] || continue
    if ! kill -0 "$pid" 2>/dev/null; then
      dead_peers+=("$(basename "$pidfile" .pid)")
    fi
  done
  if ((${#dead_peers[@]} > 0)); then
    echo "[run $run] peers exited early: ${dead_peers[*]}" >&2
    if rg -n "Address already in use|Failed to bind TCP listener" "$run_dir"/peer*.log >/dev/null 2>&1; then
      rg -n "Address already in use|Failed to bind TCP listener" "$run_dir"/peer*.log >&2 || true
    fi
    return 1
  fi
  if rg -n "Address already in use|Failed to bind TCP listener" "$run_dir"/peer*.log >/dev/null 2>&1; then
    echo "[run $run] bind failure detected in peer logs" >&2
    rg -n "Address already in use|Failed to bind TCP listener" "$run_dir"/peer*.log >&2 || true
    return 1
  fi
  return 0
}

cleanup_run_dir=""
trap 'if [[ -n "${cleanup_run_dir:-}" ]]; then stop_localnet "$cleanup_run_dir"; fi' EXIT

successes=0
failures=0
warn_availability=0
warn_dag=0
slow_runs=0

for run in $(seq 1 "$RUNS"); do
  run_dir="${OUT_DIR}/run-${run}"
  echo ""
  echo "[run $run/$RUNS] generating localnet..."
  stop_localnet "$run_dir"
  if [[ -e "$run_dir" ]]; then
    if [[ "$FORCE" == true ]]; then
      rm -rf "$run_dir"
    else
      echo "[run $run] run dir exists: $run_dir (use --force to remove)" >&2
      failures=$((failures + 1))
      continue
    fi
  fi
  mkdir -p "$OUT_DIR"

  if ! ensure_ports_available; then
    failures=$((failures + 1))
    continue
  fi

  if ! "$KAGAMI_BIN" localnet \
      --build-line "iroha3" \
      --out-dir "$run_dir" \
      --peers "$PEERS" \
      --seed "$SEED" \
      --base-api-port "$BASE_API_PORT" \
      --base-p2p-port "$BASE_P2P_PORT" \
      --bind-host "$BIND_HOST" \
      --public-host "$PUBLIC_HOST"; then
    echo "[run $run] kagami localnet failed" >&2
    failures=$((failures + 1))
    continue
  fi

  echo "[run $run] starting peers..."
  if ! (cd "$run_dir" && IROHAD_BIN="$IROHAD_BIN" ./start.sh); then
    echo "[run $run] start.sh failed" >&2
    stop_localnet "$run_dir"
    failures=$((failures + 1))
    continue
  fi
  cleanup_run_dir="$run_dir"
  sleep 1
  if ! verify_peers_started "$run_dir"; then
    stop_localnet "$run_dir"
    cleanup_run_dir=""
    failures=$((failures + 1))
    continue
  fi

  if ! wait_for_ready; then
    echo "[run $run] readiness timeout after ${READY_TIMEOUT}s" >&2
    stop_localnet "$run_dir"
    cleanup_run_dir=""
    failures=$((failures + 1))
    continue
  fi

  echo "[run $run] waiting for block heights..."
  height2_s=""
  height10_s=""
  commit_time_ms=""
  if ! height2_s="$(wait_for_height 2)"; then
    echo "[run $run] height 2 timeout after ${HEIGHT_TIMEOUT}s" >&2
  fi
  if ! height10_s="$(wait_for_height 10)"; then
    echo "[run $run] height 10 timeout after ${HEIGHT_TIMEOUT}s" >&2
  fi
  commit_time_ms="$(fetch_commit_time_ms)"
  if [[ ! "$commit_time_ms" =~ ^[0-9]+$ ]]; then
    commit_time_ms=0
  fi

  client_cfg="$run_dir/client.toml"
  domain="$(read_domain "$client_cfg")"
  sender_pub="$(read_public_key "$client_cfg")"
  sender_account="${sender_pub}@${domain}"
  asset_def="train${run}#${domain}"

  clients_dir="$run_dir/clients"
  rm -rf "$clients_dir"
  mkdir -p "$clients_dir"
  client_seed_prefix="${SEED}-clients-${run}"
  client_names="recipient,sig1,sig2,sig3,multisig"
  if ! generate_client_configs "$client_cfg" "$clients_dir" "$domain" "$client_seed_prefix" "$client_names"; then
    echo "[run $run] kagami client-configs failed" >&2
    stop_localnet "$run_dir"
    cleanup_run_dir=""
    failures=$((failures + 1))
    continue
  fi

  echo "[run $run] asset flow..."
  asset_ok=true
  if ! retry_cmd "asset definition register" 3 2 \
      "$IROHA_BIN" --config "$client_cfg" asset definition register --id "$asset_def"; then
    asset_ok=false
  fi
  if ! retry_cmd "asset mint" 3 2 \
      "$IROHA_BIN" --config "$client_cfg" asset mint --id "${asset_def}#${sender_account}" --quantity 10; then
    asset_ok=false
  fi

  recipient_cfg="$clients_dir/recipient.toml"
  if ! recipient_pub="$(read_public_key "$recipient_cfg")"; then
    echo "[run $run] failed to read recipient public key" >&2
    asset_ok=false
  fi
  recipient_account="${recipient_pub}@${domain}"
  if ! retry_cmd "recipient account register" 3 2 \
      "$IROHA_BIN" --config "$client_cfg" account register --id "$recipient_account"; then
    asset_ok=false
  elif ! wait_for_account "$client_cfg" "$recipient_account"; then
    asset_ok=false
  fi
  if ! retry_cmd "asset transfer" 3 2 \
      "$IROHA_BIN" --config "$client_cfg" asset transfer \
      --id "${asset_def}#${sender_account}" \
      --to "$recipient_account" \
      --quantity 1; then
    asset_ok=false
  fi

  echo "[run $run] multisig flow..."
  multisig_ok=true
  sig1_cfg="$clients_dir/sig1.toml"
  sig2_cfg="$clients_dir/sig2.toml"
  sig3_cfg="$clients_dir/sig3.toml"
  multisig_cfg="$clients_dir/multisig.toml"
  if ! sig1_pub="$(read_public_key "$sig1_cfg")"; then
    echo "[run $run] failed to read sig1 public key" >&2
    multisig_ok=false
  fi
  if ! sig2_pub="$(read_public_key "$sig2_cfg")"; then
    echo "[run $run] failed to read sig2 public key" >&2
    multisig_ok=false
  fi
  if ! sig3_pub="$(read_public_key "$sig3_cfg")"; then
    echo "[run $run] failed to read sig3 public key" >&2
    multisig_ok=false
  fi
  if ! multisig_pub="$(read_public_key "$multisig_cfg")"; then
    echo "[run $run] failed to read multisig public key" >&2
    multisig_ok=false
  fi
  sig1_account="${sig1_pub}@${domain}"
  sig2_account="${sig2_pub}@${domain}"
  sig3_account="${sig3_pub}@${domain}"

  for acct in "$sig1_account" "$sig2_account" "$sig3_account"; do
    if ! retry_cmd "signatory account register" 3 2 \
        "$IROHA_BIN" --config "$client_cfg" account register --id "$acct"; then
      multisig_ok=false
    elif ! wait_for_account "$client_cfg" "$acct"; then
      multisig_ok=false
    fi
  done

  multisig_account="${multisig_pub}@${domain}"

  if ! retry_cmd "multisig register" 3 2 \
      "$IROHA_BIN" --config "$client_cfg" multisig register \
      --account "$multisig_account" \
      --signatories "$sig1_account" "$sig2_account" "$sig3_account" \
      --weights 1 1 1 \
      --quorum 3 \
      --transaction-ttl "2m"; then
    multisig_ok=false
  elif ! wait_for_account "$client_cfg" "$multisig_account"; then
    multisig_ok=false
  fi

  propose_output=""
  instructions_hash=""
  if propose_output="$(retry_cmd_output "multisig propose" 3 2 bash -c \
    "echo '\"congratulations\"' | \"$IROHA_BIN\" --config \"$sig1_cfg\" -o account meta set --id \"$multisig_account\" --key success_marker | \"$IROHA_BIN\" --config \"$sig1_cfg\" multisig propose --account \"$multisig_account\"")"; then
    instructions_hash="$(printf '%s\n' "$propose_output" | rg -m1 -o '^[0-9a-f]{64}$' || true)"
    if [[ -z "$instructions_hash" ]]; then
      echo "[run $run] failed to parse multisig instructions hash" >&2
      multisig_ok=false
    fi
  else
    multisig_ok=false
  fi

  proposal_ready=true
  if [[ -n "${instructions_hash:-}" ]]; then
    if ! wait_for_multisig_proposal "$client_cfg" "$multisig_account" "$instructions_hash"; then
      proposal_ready=false
      multisig_ok=false
    fi
  else
    proposal_ready=false
  fi

  if [[ "$proposal_ready" == true ]]; then
    if ! retry_cmd "multisig approve (sig2)" 3 2 \
        "$IROHA_BIN" --config "$sig2_cfg" multisig approve \
        --account "$multisig_account" \
        --instructions-hash "$instructions_hash"; then
      multisig_ok=false
    fi
    if ! retry_cmd "multisig approve (sig3)" 3 2 \
        "$IROHA_BIN" --config "$sig3_cfg" multisig approve \
        --account "$multisig_account" \
        --instructions-hash "$instructions_hash"; then
      multisig_ok=false
    fi
  else
    multisig_ok=false
  fi

  if ! retry_cmd "multisig meta get" 10 2 \
      "$IROHA_BIN" --config "$client_cfg" account meta get \
      --id "$multisig_account" \
      --key success_marker; then
    multisig_ok=false
  fi

  if rg -n "DA availability still missing \\(advisory\\)" "$run_dir"/peer*.log >/dev/null 2>&1; then
    warn_availability=$((warn_availability + 1))
    echo "[run $run] warning: DA availability still missing (advisory) detected"
  fi
  if rg -n "DAG fingerprint mismatch" "$run_dir"/peer*.log >/dev/null 2>&1; then
    warn_dag=$((warn_dag + 1))
    echo "[run $run] warning: DAG fingerprint mismatch detected"
  fi

  stop_localnet "$run_dir"
  cleanup_run_dir=""

  cadence_ok=true
  if [[ -n "$height10_s" && "$height10_s" -gt "$STALL_THRESHOLD" ]]; then
    cadence_ok=false
    slow_runs=$((slow_runs + 1))
    echo "[run $run] warning: height10=${height10_s}s exceeds ${STALL_THRESHOLD}s threshold"
  fi

  commit_note=""
  if [[ "$commit_time_ms" -gt 0 ]]; then
    commit_note=", last_commit_ms=${commit_time_ms}ms"
  fi

  if [[ "$asset_ok" == true && "$multisig_ok" == true && "$cadence_ok" == true && -n "$height2_s" && -n "$height10_s" ]]; then
    successes=$((successes + 1))
    echo "[run $run] ok (height2=${height2_s}s, height10=${height10_s}s${commit_note})"
  else
    failures=$((failures + 1))
    echo "[run $run] failed (asset_ok=${asset_ok}, multisig_ok=${multisig_ok}, cadence_ok=${cadence_ok}, height2=${height2_s:-timeout}, height10=${height10_s:-timeout}${commit_note})"
  fi
done

echo ""
echo "training_script_2 summary:"
echo "  runs: $RUNS"
echo "  successes: $successes"
echo "  failures: $failures"
echo "  slow runs (> ${STALL_THRESHOLD}s to height 10): $slow_runs"
echo "  warning: missing availability QC: $warn_availability"
echo "  warning: DAG fingerprint mismatch: $warn_dag"
