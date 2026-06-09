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
  --target-dir <DIR>    Set CARGO_TARGET_DIR for builds and binary lookup
  --fast                Run cargo via scripts/cargo_fast.sh when available
  --fast-zero-debug     With --fast, set CARGO_PROFILE_{DEV,TEST}_DEBUG=0
  --fast-no-incremental With --fast, set CARGO_INCREMENTAL=0
  --no-build            Skip cargo build (assumes binaries already exist)
  --reuse-run-dir       Reuse an existing generated run-N directory instead of regenerating it with kagami
  --force               Remove existing run directories under --out-dir
  --ready-timeout <S>   Seconds to wait for /status (default: 30)
  --height-timeout <S>  Seconds to wait for block height targets (default: 30)
  --stall-threshold <S> Seconds to flag slow block cadence (default: 40)
  -h, --help            Show this help
EOF
}

require_option_value() {
  local flag="$1"
  local value="${2-}"
  if [[ -z "$value" ]] || [[ "$value" == --* ]]; then
    echo "Missing value for ${flag}" >&2
    exit 2
  fi
}

RUNS=1
OUT_DIR="/tmp/iroha-localnet-training"
SEED="training"
PEERS=4
BASE_API_PORT=29080
BASE_P2P_PORT=33337
PROFILE="release"
DO_BUILD=true
REUSE_RUN_DIR=false
READY_TIMEOUT=30
HEIGHT_TIMEOUT=30
FORCE=false
AUTO_PORTS=false
STALL_THRESHOLD=40
PUBLIC_HOST="127.0.0.1"
BIND_HOST="127.0.0.1"
TRAINING_ASSET_DEFINITION_ID="7EAD8EFYUx1aVKZPUU1fyKvr8dF1"
TARGET_DIR=""
USE_CARGO_FAST=false
FAST_ZERO_DEBUG=false
FAST_NO_INCREMENTAL=false

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
    --no-build)
      DO_BUILD=false
      shift
      ;;
    --reuse-run-dir)
      REUSE_RUN_DIR=true
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

resolve_dir() {
  local path="$1"
  local candidate
  if [[ "${path}" = /* ]]; then
    candidate="${path}"
  else
    candidate="${REPO_ROOT}/${path}"
  fi
  mkdir -p "${candidate}"
  (
    cd "${candidate}"
    pwd
  )
}

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

cargo_runner=(cargo)
if [[ "$USE_CARGO_FAST" == true ]]; then
  cargo_fast_script="${REPO_ROOT}/scripts/cargo_fast.sh"
  if [[ ! -x "${cargo_fast_script}" ]]; then
    echo "scripts/cargo_fast.sh is not available or not executable" >&2
    exit 2
  fi
  cargo_runner=("${cargo_fast_script}")
  if [[ "$FAST_ZERO_DEBUG" == true ]]; then
    cargo_runner+=("--zero-debug")
  fi
  if [[ "$FAST_NO_INCREMENTAL" == true ]]; then
    cargo_runner+=("--no-incremental")
  fi
  echo "[training-script-2] using scripts/cargo_fast.sh for cargo commands"
elif [[ "$FAST_ZERO_DEBUG" == true || "$FAST_NO_INCREMENTAL" == true ]]; then
  echo "--fast-zero-debug and --fast-no-incremental require --fast" >&2
  exit 2
fi

if [[ -n "${TARGET_DIR}" ]]; then
  export CARGO_TARGET_DIR="$(resolve_dir "${TARGET_DIR}")"
fi
TARGET_DIR="$(resolve_dir "${CARGO_TARGET_DIR:-target}")"
export CARGO_TARGET_DIR="${TARGET_DIR}"

if [[ "$DO_BUILD" == true ]]; then
  echo "Building kagami/irohad/iroha ($PROFILE)..."
  if [[ "$PROFILE" == "release" ]]; then
    (cd "$REPO_ROOT" && "${cargo_runner[@]}" -- build --release --bin kagami --bin irohad --bin iroha)
  else
    (cd "$REPO_ROOT" && "${cargo_runner[@]}" -- build --bin kagami --bin irohad --bin iroha)
  fi
fi

default_kagami_bin="$TARGET_DIR/$PROFILE/kagami"
if [[ -n "${KAGAMI_BIN:-}" ]]; then
  if [[ ! -x "$KAGAMI_BIN" ]]; then
    echo "Missing binary: $KAGAMI_BIN" >&2
    exit 1
  fi
elif [[ -x "$default_kagami_bin" ]]; then
  KAGAMI_BIN="$default_kagami_bin"
else
  KAGAMI_BIN=""
fi

IROHAD_BIN="${IROHAD_BIN:-"$TARGET_DIR/$PROFILE/irohad"}"
IROHA_BIN="${IROHA_BIN:-"$TARGET_DIR/$PROFILE/iroha"}"
PEER_STATUS_NAMES=()
PEER_STATUS_URLS=()

for bin in "$IROHAD_BIN" "$IROHA_BIN"; do
  if [[ ! -x "$bin" ]]; then
    echo "Missing binary: $bin" >&2
    exit 1
  fi
done

read_toml_section_string() {
  local section="$1"
  local key="$2"
  local path="$3"
  python3 - "$section" "$key" "$path" <<'PY'
import re
import sys

section = sys.argv[1]
key = sys.argv[2]
path = sys.argv[3]
text = open(path, encoding="utf-8").read()
section_pattern = rf'(?ms)^\[{re.escape(section)}\]\s*(.*?)(?=^\[|\Z)'
section_match = re.search(section_pattern, text)
if not section_match:
    raise SystemExit(1)
body = section_match.group(1)
key_pattern = rf'(?m)^\s*{re.escape(key)}\s*=\s*"([^"]*)"'
key_match = re.search(key_pattern, body)
if not key_match:
    raise SystemExit(1)
print(key_match.group(1))
PY
}

load_peer_status_endpoints() {
  local run_dir="$1"
  PEER_STATUS_NAMES=()
  PEER_STATUS_URLS=()

  local cfg
  for cfg in "$run_dir"/peer*.toml; do
    [[ -f "$cfg" ]] || continue
    local peer_name
    local torii_address
    peer_name="$(basename "$cfg" .toml)"
    if ! torii_address="$(read_toml_section_string "torii" "address" "$cfg")"; then
      echo "[run $run] failed to resolve [torii].address from $cfg" >&2
      return 1
    fi
    torii_address="${torii_address#addr:}"
    torii_address="${torii_address%%#*}"
    PEER_STATUS_NAMES+=("$peer_name")
    PEER_STATUS_URLS+=("http://${torii_address}/status")
  done

  if ((${#PEER_STATUS_URLS[@]} == 0)); then
    echo "[run $run] no peer status endpoints discovered under $run_dir" >&2
    return 1
  fi
}

fetch_status_fields() {
  local url="$1"
  local payload
  payload="$(curl -sf "$url" 2>/dev/null || true)"
  STATUS_PAYLOAD="$payload" python3 - <<'PY'
import json
import os
import sys

raw = os.environ.get("STATUS_PAYLOAD", "").strip()
if not raw:
    raise SystemExit(1)
try:
    data = json.loads(raw)
except json.JSONDecodeError:
    raise SystemExit(1)

def as_int(value):
    try:
        return int(value or 0)
    except (TypeError, ValueError):
        return 0

print(
    as_int(data.get("blocks", data.get("height", 0))),
    as_int(data.get("queue_size", 0)),
    as_int(data.get("view_changes", 0)),
    as_int(data.get("commit_time_ms", 0)),
)
PY
}

peer_status_snapshot() {
  local all_reachable=true
  local idx
  for idx in "${!PEER_STATUS_URLS[@]}"; do
    local name="${PEER_STATUS_NAMES[$idx]}"
    local url="${PEER_STATUS_URLS[$idx]}"
    local fields=""
    if fields="$(fetch_status_fields "$url")"; then
      local blocks queue_size view_changes commit_time_ms
      read -r blocks queue_size view_changes commit_time_ms <<<"$fields"
      printf '%s|%s|%s|%s|%s|%s\n' \
        "$name" "$url" "$blocks" "$queue_size" "$view_changes" "$commit_time_ms"
    else
      printf '%s|%s|UNREACHABLE|UNREACHABLE|UNREACHABLE|UNREACHABLE\n' "$name" "$url"
      all_reachable=false
    fi
  done
  [[ "$all_reachable" == true ]]
}

array_contains() {
  local needle="$1"
  shift
  local candidate
  for candidate in "$@"; do
    if [[ "$candidate" == "$needle" ]]; then
      return 0
    fi
  done
  return 1
}

dump_peer_status_snapshot() {
  local label="$1"
  local snapshot
  snapshot="$(peer_status_snapshot || true)"
  echo "[run $run] ${label}" >&2
  while IFS='|' read -r name url blocks queue_size view_changes commit_time_ms; do
    [[ -n "$name" ]] || continue
    echo "[run $run] ${name} status url=${url} blocks=${blocks} queue_size=${queue_size} view_changes=${view_changes} commit_time_ms=${commit_time_ms}" >&2
  done <<<"$snapshot"
}

wait_for_ready() {
  local deadline=$((SECONDS + READY_TIMEOUT))
  local seen_names=()
  while ((SECONDS < deadline)); do
    local snapshot
    snapshot="$(peer_status_snapshot || true)"
    local min_height=""
    local max_height=0
    local all_reachable=true
    local ready=false
    while IFS='|' read -r name _url blocks _queue_size _view_changes _commit_time_ms; do
      [[ -n "$name" ]] || continue
      if [[ "$blocks" == "UNREACHABLE" ]]; then
        all_reachable=false
        continue
      fi
      if ! array_contains "$name" "${seen_names[@]-}"; then
        seen_names+=("$name")
      fi
      if [[ -z "$min_height" || "$blocks" -lt "$min_height" ]]; then
        min_height="$blocks"
      fi
      if [[ "$blocks" -gt "$max_height" ]]; then
        max_height="$blocks"
      fi
    done <<<"$snapshot"
    if [[ "$all_reachable" == true && ${#seen_names[@]} -eq ${#PEER_STATUS_URLS[@]} && -n "$min_height" ]]; then
      local spread=$((max_height - min_height))
      if ((spread <= 1)); then
        ready=true
      fi
    fi
    if [[ "$ready" == true ]]; then
      return 0
    fi
    sleep 1
  done
  return 1
}

fetch_height() {
  local snapshot
  snapshot="$(peer_status_snapshot || true)"
  local max_height=0
  while IFS='|' read -r _name _url blocks _queue_size _view_changes _commit_time_ms; do
    [[ "$blocks" =~ ^[0-9]+$ ]] || continue
    if [[ "$blocks" -gt "$max_height" ]]; then
      max_height="$blocks"
    fi
  done <<<"$snapshot"
  printf '%s\n' "$max_height"
}

fetch_commit_time_ms() {
  local snapshot
  snapshot="$(peer_status_snapshot || true)"
  local max_commit_time_ms=0
  while IFS='|' read -r _name _url _blocks _queue_size _view_changes commit_time_ms; do
    [[ "$commit_time_ms" =~ ^[0-9]+$ ]] || continue
    if [[ "$commit_time_ms" -gt "$max_commit_time_ms" ]]; then
      max_commit_time_ms="$commit_time_ms"
    fi
  done <<<"$snapshot"
  printf '%s\n' "$max_commit_time_ms"
}

wait_for_height() {
  local target="$1"
  local start=$SECONDS
  local deadline=$((start + HEIGHT_TIMEOUT))
  while ((SECONDS < deadline)); do
    local snapshot
    snapshot="$(peer_status_snapshot || true)"
    local min_height=""
    local max_height=0
    local all_reachable=true
    while IFS='|' read -r _name _url blocks _queue_size _view_changes _commit_time_ms; do
      [[ -n "$blocks" ]] || continue
      if [[ "$blocks" == "UNREACHABLE" ]]; then
        all_reachable=false
        continue
      fi
      if [[ -z "$min_height" || "$blocks" -lt "$min_height" ]]; then
        min_height="$blocks"
      fi
      if [[ "$blocks" -gt "$max_height" ]]; then
        max_height="$blocks"
      fi
    done <<<"$snapshot"
    if [[ "$all_reachable" == true && -n "$min_height" && "$min_height" -ge "$target" && $((max_height - min_height)) -le 1 ]]; then
      echo "$((SECONDS - start))"
      return 0
    fi
    sleep 1
  done
  return 1
}

wait_for_reuse_stabilization() {
  local baseline_height="$1"
  local deadline=$((SECONDS + HEIGHT_TIMEOUT))
  local zero_queue_since=0
  local converged_since=0
  while ((SECONDS < deadline)); do
    local snapshot
    snapshot="$(peer_status_snapshot || true)"
    local all_reachable=true
    local all_zero_queue=true
    local min_height=""
    local max_height=0
    while IFS='|' read -r _name _url blocks queue_size _view_changes _commit_time_ms; do
      [[ -n "$blocks" ]] || continue
      if [[ "$blocks" == "UNREACHABLE" ]]; then
        all_reachable=false
        all_zero_queue=false
        continue
      fi
      if [[ -z "$min_height" || "$blocks" -lt "$min_height" ]]; then
        min_height="$blocks"
      fi
      if [[ "$blocks" -gt "$max_height" ]]; then
        max_height="$blocks"
      fi
      if [[ "$queue_size" != "0" ]]; then
        all_zero_queue=false
      fi
    done <<<"$snapshot"
    if [[ "$all_reachable" == true && "$max_height" -gt "$baseline_height" ]]; then
      echo "[run $run] reused network advanced from restart baseline ${baseline_height} to ${max_height}" >&2
      return 0
    fi
    if [[ "$all_reachable" == true && -n "$min_height" && "$min_height" -ge "$baseline_height" && "$min_height" -eq "$max_height" ]]; then
      if ((converged_since == 0)); then
        converged_since=$SECONDS
      elif ((SECONDS - converged_since >= 5)); then
        echo "[run $run] reused network converged at common height ${max_height}" >&2
        return 0
      fi
    else
      converged_since=0
    fi
    if [[ "$all_reachable" == true && "$all_zero_queue" == true ]]; then
      if ((zero_queue_since == 0)); then
        zero_queue_since=$SECONDS
      elif ((SECONDS - zero_queue_since >= 5)); then
        echo "[run $run] reused network held queue_size=0 across all peers for 5s at height ${max_height}" >&2
        return 0
      fi
    else
      zero_queue_since=0
    fi
    sleep 1
  done
  return 1
}

dump_reuse_stall_diagnostics() {
  local run_dir="$1"
  local label="$2"
  dump_peer_status_snapshot "$label"
  local pidfile
  for pidfile in "$run_dir"/peer*.pid; do
    [[ -f "$pidfile" ]] || continue
    local pid
    pid="$(cat "$pidfile" 2>/dev/null || true)"
    [[ -n "$pid" ]] || continue
    if kill -0 "$pid" 2>/dev/null; then
      echo "[run $run] $(basename "$pidfile") pid=${pid} state=alive" >&2
    else
      echo "[run $run] $(basename "$pidfile") pid=${pid} state=stale" >&2
    fi
  done
  local log
  for log in "$run_dir"/peer*.log; do
    [[ -f "$log" ]] || continue
    echo "[run $run] $(basename "$log") recent relevant lines:" >&2
    if ! rg -n "queue_size|view change|view_changes|timeout|stall|panic|error|warn|availability|RBC|consensus" "$log" | tail -n 20 >&2; then
      tail -n 20 "$log" >&2 || true
    fi
  done
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

already_exists_output() {
  local output="$1"
  [[ "$output" == *"Repeated instruction: Repetition of \`Register\`"* ]] || [[ "$output" == *" already exists"* ]]
}

retry_cmd_allow_existing() {
  local allow_existing="$1"
  local label="$2"
  local attempts="$3"
  local delay="$4"
  shift 4
  local attempt=1
  local output=""
  while true; do
    output=""
    if output="$("$@" 2>&1)"; then
      if [[ -n "$output" ]]; then
        printf '%s\n' "$output"
      fi
      return 0
    fi
    if [[ -n "$output" ]]; then
      printf '%s\n' "$output" >&2
    fi
    if [[ "$allow_existing" == true ]] && already_exists_output "$output"; then
      echo "[run $run] ${label} already exists on reused run; continuing" >&2
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

wait_for_account() {
  local cfg="$1"
  local account="$2"
  retry_cmd "account ready (${account})" 10 1 \
    "$IROHA_BIN" --config "$cfg" account get \
    --id "$account"
}

build_multisig_spec_json() {
  local quorum="$1"
  local transaction_ttl_ms="$2"
  shift 2
  python3 - "$quorum" "$transaction_ttl_ms" "$@" <<'PY'
import json
import sys

quorum = int(sys.argv[1])
transaction_ttl_ms = int(sys.argv[2])
signatories = {account: 1 for account in sys.argv[3:]}
print(
    json.dumps(
        {
            "quorum": quorum,
            "signatories": signatories,
            "transaction_ttl_ms": transaction_ttl_ms,
        },
        sort_keys=True,
    )
)
PY
}

resolve_multisig_account_by_spec() {
  local cfg="$1"
  local expected_spec_json="$2"
  local accounts_json=""
  accounts_json="$(retry_list_json "multisig account discovery" 3 1 \
    "$IROHA_BIN" --config "$cfg" ledger account list all --verbose)" || return 1
  ACCOUNTS_PAYLOAD="$accounts_json" EXPECTED_SPEC_JSON="$expected_spec_json" python3 - <<'PY'
import json
import os
import sys

accounts = json.loads(os.environ["ACCOUNTS_PAYLOAD"])
expected = json.loads(os.environ["EXPECTED_SPEC_JSON"])
matches = []
for account in accounts:
    metadata = account.get("metadata") or {}
    if metadata.get("multisig/spec") == expected:
        matches.append(account["id"])

if len(matches) == 1:
    print(matches[0])
    raise SystemExit(0)
if len(matches) == 0:
    raise SystemExit(1)
print(
    f"multiple multisig accounts matched the expected spec: {matches}",
    file=sys.stderr,
)
raise SystemExit(2)
PY
}

wait_for_multisig_account_by_spec() {
  local cfg="$1"
  local expected_spec_json="$2"
  local attempt=1
  local output=""
  while true; do
    output=""
    if output="$(resolve_multisig_account_by_spec "$cfg" "$expected_spec_json")"; then
      printf '%s' "$output"
      return 0
    fi
    if [[ "$attempt" -ge 10 ]]; then
      echo "[run $run] canonical multisig account discovery failed after 10 attempts" >&2
      return 1
    fi
    echo "[run $run] canonical multisig account discovery attempt ${attempt}/10 failed; retrying..." >&2
    sleep 1
    attempt=$((attempt + 1))
  done
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

public_key_to_i105() {
  local public_key="$1"
  ADDRESS_PAYLOAD="$("$IROHA_BIN" --output-format json tools address convert "$public_key" --format json | extract_json_output)" \
    python3 - <<'PY'
import json
import os

payload = os.environ["ADDRESS_PAYLOAD"]
data = json.loads(payload)
print(data["i105"]["value"])
PY
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
  "$KAGAMI_BIN" advanced client-configs \
    --base-config "$base_config" \
    --out-dir "$out_dir" \
    --domain "$domain" \
    --seed-prefix "$seed_prefix" \
    --names "$names_csv"
}

require_kagami_bin() {
  if [[ -n "${KAGAMI_BIN}" ]] && [[ -x "${KAGAMI_BIN}" ]]; then
    return 0
  fi
  echo "Missing binary: ${default_kagami_bin}" >&2
  return 1
}

stop_localnet() {
  local run_dir="$1"
  if [[ -f "$run_dir/stop.sh" ]]; then
    (cd "$run_dir" && ./stop.sh) || true
  fi
  for pidfile in "$run_dir"/peer*.pid; do
    [[ -f "$pidfile" ]] || continue
    pid="$(cat "$pidfile" 2>/dev/null || true)"
    if [[ -z "$pid" ]]; then
      rm -f "$pidfile"
      continue
    fi
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
    rm -f "$pidfile"
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
  reuse_existing_run=false
  echo ""
  echo "[run $run/$RUNS] generating localnet..."
  stop_localnet "$run_dir"
  if [[ -e "$run_dir" ]]; then
    if [[ "$FORCE" == true ]]; then
      rm -rf "$run_dir"
    elif [[ "$REUSE_RUN_DIR" == true ]]; then
      reuse_existing_run=true
      echo "[run $run] reusing generated localnet from $run_dir"
    else
      echo "[run $run] run dir exists: $run_dir (use --force to remove)" >&2
      failures=$((failures + 1))
      continue
    fi
  fi
  mkdir -p "$OUT_DIR"

  if [[ "$reuse_existing_run" != true ]]; then
    if ! ensure_ports_available; then
      failures=$((failures + 1))
      continue
    fi

    if ! require_kagami_bin; then
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
  if ! load_peer_status_endpoints "$run_dir"; then
    stop_localnet "$run_dir"
    cleanup_run_dir=""
    failures=$((failures + 1))
    continue
  fi

  if ! wait_for_ready; then
    echo "[run $run] readiness timeout after ${READY_TIMEOUT}s" >&2
    dump_peer_status_snapshot "peer readiness timeout snapshot"
    stop_localnet "$run_dir"
    cleanup_run_dir=""
    failures=$((failures + 1))
    continue
  fi

  if [[ "$reuse_existing_run" == true ]]; then
    restart_baseline_height="$(fetch_height)"
    if [[ ! "$restart_baseline_height" =~ ^[0-9]+$ ]]; then
      restart_baseline_height=0
    fi
    if ! wait_for_reuse_stabilization "$restart_baseline_height"; then
      echo "[run $run] reused-run stabilization timeout after ${HEIGHT_TIMEOUT}s" >&2
      dump_reuse_stall_diagnostics "$run_dir" "reused-run stabilization timed out"
      stop_localnet "$run_dir"
      cleanup_run_dir=""
      failures=$((failures + 1))
      continue
    fi
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
  sender_account="$(public_key_to_i105 "$sender_pub")"
  asset_def="$TRAINING_ASSET_DEFINITION_ID"
  asset_name="train${run}"

  clients_dir="$run_dir/clients"
  if [[ "$reuse_existing_run" == true ]]; then
    if [[ ! -f "$clients_dir/sig1.toml" ]] || [[ ! -f "$clients_dir/sig2.toml" ]] || [[ ! -f "$clients_dir/sig3.toml" ]] || [[ ! -f "$clients_dir/multisig.toml" ]]; then
      echo "[run $run] existing run dir is missing generated client configs under $clients_dir" >&2
      stop_localnet "$run_dir"
      cleanup_run_dir=""
      failures=$((failures + 1))
      continue
    fi
  else
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
  fi

  height_traffic_start=$SECONDS
  echo "[run $run] asset flow..."
  asset_ok=true
  if ! retry_cmd_allow_existing "$reuse_existing_run" "asset definition register" 3 2 \
      "$IROHA_BIN" --config "$client_cfg" ledger asset definition register \
      --id "$asset_def" \
      --name "$asset_name" \
      --scale 0; then
    asset_ok=false
  fi
  if ! retry_cmd "asset mint" 3 2 \
      "$IROHA_BIN" --config "$client_cfg" ledger asset mint \
      --definition "$asset_def" \
      --account "$sender_account" \
      --quantity 10; then
    asset_ok=false
  fi

  recipient_cfg="$clients_dir/recipient.toml"
  if ! recipient_pub="$(read_public_key "$recipient_cfg")"; then
    echo "[run $run] failed to read recipient public key" >&2
    asset_ok=false
  fi
  recipient_account="$(public_key_to_i105 "$recipient_pub")"
  if ! retry_cmd_allow_existing "$reuse_existing_run" "recipient account register" 3 2 \
      "$IROHA_BIN" --config "$client_cfg" ledger account register --id "$recipient_account"; then
    asset_ok=false
  elif ! wait_for_account "$client_cfg" "$recipient_account"; then
    asset_ok=false
  fi
  if ! retry_cmd "asset transfer" 3 2 \
      "$IROHA_BIN" --config "$client_cfg" ledger asset transfer \
      --definition "$asset_def" \
      --account "$sender_account" \
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
  sig1_account="$(public_key_to_i105 "$sig1_pub")"
  sig2_account="$(public_key_to_i105 "$sig2_pub")"
  sig3_account="$(public_key_to_i105 "$sig3_pub")"

  for acct in "$sig1_account" "$sig2_account" "$sig3_account"; do
    if ! retry_cmd_allow_existing "$reuse_existing_run" "signatory account register" 3 2 \
        "$IROHA_BIN" --config "$client_cfg" ledger account register --id "$acct"; then
      multisig_ok=false
    elif ! wait_for_account "$client_cfg" "$acct"; then
      multisig_ok=false
    fi
  done

  multisig_seed_account="$(public_key_to_i105 "$multisig_pub")"
  multisig_spec_json="$(build_multisig_spec_json 3 120000 "$sig1_account" "$sig2_account" "$sig3_account")"

  if ! retry_cmd_allow_existing "$reuse_existing_run" "multisig register" 3 2 \
      "$IROHA_BIN" --config "$client_cfg" ledger multisig register \
      --account "$multisig_seed_account" \
      --signatories "$sig1_account" "$sig2_account" "$sig3_account" \
      --weights 1 1 1 \
      --quorum 3 \
      --transaction-ttl "2m"; then
    multisig_ok=false
  elif ! multisig_account="$(wait_for_multisig_account_by_spec "$client_cfg" "$multisig_spec_json")"; then
    multisig_ok=false
  else
    echo "[run $run] canonical multisig account: $multisig_account"
  fi

  propose_output=""
  instructions_hash=""
  if propose_output="$(retry_cmd_output "multisig propose" 3 2 bash -c \
    "echo '\"congratulations\"' | \"$IROHA_BIN\" --machine --config \"$sig1_cfg\" -o --output-format json ledger account meta set --id \"$multisig_account\" --key success_marker | \"$IROHA_BIN\" --machine --config \"$sig1_cfg\" --output-format text ledger multisig propose --account \"$multisig_account\"")"; then
    instructions_hash="$(printf '%s\n' "$propose_output" | sed -n 's/^instructions_hash: //p' | head -n 1)"
    if [[ -z "$instructions_hash" ]]; then
      echo "[run $run] failed to parse multisig instructions hash" >&2
      multisig_ok=false
    fi
  else
    multisig_ok=false
  fi

  if [[ -n "${instructions_hash:-}" ]]; then
    if ! retry_cmd "multisig approve (sig2)" 10 1 \
        "$IROHA_BIN" --config "$sig2_cfg" ledger multisig approve \
        --account "$multisig_account" \
        --instructions-hash "$instructions_hash"; then
      multisig_ok=false
    fi
    if ! retry_cmd "multisig approve (sig3)" 10 1 \
        "$IROHA_BIN" --config "$sig3_cfg" ledger multisig approve \
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

  final_height="$(fetch_height)"
  if [[ ! "$final_height" =~ ^[0-9]+$ ]]; then
    final_height=0
  fi
  if [[ -z "$height2_s" && "$final_height" -ge 2 ]]; then
    height2_s="$((SECONDS - height_traffic_start))"
    echo "[run $run] height 2 reached after traffic in ${height2_s}s"
  fi
  if [[ -z "$height10_s" && "$final_height" -ge 10 ]]; then
    height10_s="$((SECONDS - height_traffic_start))"
    echo "[run $run] height 10 reached after traffic in ${height10_s}s"
  elif [[ -z "$height10_s" ]]; then
    if wait_for_height 10 >/dev/null; then
      height10_s="$((SECONDS - height_traffic_start))"
      if [[ -z "$height2_s" ]]; then
        height2_s="$height10_s"
      fi
      echo "[run $run] height 10 reached after traffic in ${height10_s}s"
      final_height="$(fetch_height)"
      if [[ ! "$final_height" =~ ^[0-9]+$ ]]; then
        final_height=0
      fi
    fi
  fi

  if rg -n "DA availability gate still active|DA availability still missing \\(advisory\\)" "$run_dir"/peer*.log >/dev/null 2>&1; then
    warn_availability=$((warn_availability + 1))
    echo "[run $run] warning: DA availability gate still active detected"
  fi
  if rg -n "DAG fingerprint mismatch" "$run_dir"/peer*.log >/dev/null 2>&1; then
    warn_dag=$((warn_dag + 1))
    echo "[run $run] warning: DAG fingerprint mismatch detected"
  fi

  if [[ "$reuse_existing_run" == true && ( "$asset_ok" != true || "$multisig_ok" != true || -z "$height10_s" ) ]]; then
    dump_reuse_stall_diagnostics "$run_dir" "reused run stalled after submission"
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
  final_height_note=", final_height=${final_height}"

  if [[ "$asset_ok" == true && "$multisig_ok" == true && "$cadence_ok" == true && -n "$height2_s" && -n "$height10_s" ]]; then
    successes=$((successes + 1))
    echo "[run $run] ok (height2=${height2_s}s, height10=${height10_s}s${final_height_note}${commit_note})"
  else
    failures=$((failures + 1))
    echo "[run $run] failed (asset_ok=${asset_ok}, multisig_ok=${multisig_ok}, cadence_ok=${cadence_ok}, height2=${height2_s:-timeout}, height10=${height10_s:-timeout}${final_height_note}${commit_note})"
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
