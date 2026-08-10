#!/usr/bin/env bash
set -euo pipefail

DIST=""
RUNTIME_BIN=""
EXPECTED_RUNTIME_SHA=""
LOG_LEVEL="${LOG_LEVEL:-info}"
TORII_PORTS="29080,29081,29082,29083"
P2P_PORTS="33337,33338,33339,33340"
STOP_TIMEOUT_SECONDS=5
START_AFTER=0
APPLY=0
DRY_RUN_HAS_WARNINGS=0
REPAIR_STORAGE_OWNERSHIP=0
STORAGE_OWNER_USER="${USER:-$(id -un)}"
STORAGE_OWNER_GROUP=""

usage() {
  cat <<'EOF'
Usage: clear_volatile_consensus_state.sh --dist PATH --apply [options]

Quarantine volatile consensus recovery files for a Taira localnet-style
validator bundle without deleting durable ledger state. This is intended for
the observed public-finality stall where authoritative v2 reducer height is
ahead of its durable CommitQC at a high view and signed writes expire.

The script:
  - stops peer processes launched from DIST/peer*.toml
  - removes stale peer*.pid files
  - moves storage/peer*/queue_plan_journal* into queue_journal_quarantine/
  - moves storage/peer*/rbc_sessions into rbc_sessions_quarantine/
  - archives peer*.log into logs/
  - optionally restarts DIST/start.sh with IROHAD_BIN and LOG_LEVEL

Options:
  --dist PATH                    Taira localnet distribution directory.
  --runtime-bin PATH             Runtime binary to use when --start is supplied.
  --expected-runtime-sha SHA256   Verify --runtime-bin before any mutation.
  --start                        Start DIST/start.sh after quarantine.
  --torii-ports CSV              Ports to wait for after --start (default: 29080,29081,29082,29083; empty disables wait).
  --p2p-ports CSV                P2P listener ports to wait for after --start (default: 33337,33338,33339,33340; empty disables wait).
  --repair-storage-ownership     Repair DIST/storage ownership and user-write bits before quarantining state.
  --storage-owner USER[:GROUP]   Runtime owner for DIST/storage checks/repairs (default: current user and primary group).
  --log-level LEVEL              LOG_LEVEL passed to start.sh (default: info).
  --stop-timeout-seconds N       Graceful stop wait before SIGKILL (default: 5).
  --apply                        Perform mutations. Without this flag, only prints the planned actions.
  -h, --help                     Show this help.
EOF
}

die() {
  echo "$*" >&2
  exit 1
}

is_positive_integer() {
  [[ "$1" =~ ^[1-9][0-9]*$ ]]
}

normalize_torii_ports_csv() {
  local value="$1"
  local port trimmed port_number normalized seen_ports
  local -a ports
  seen_ports=" "

  if [[ -z "$value" ]]; then
    printf '\n'
    return 0
  fi

  [[ "$value" != ,* && "$value" != *, ]] || die "--torii-ports contains an empty port entry"
  IFS=',' read -r -a ports <<<"$value"
  for port in "${ports[@]}"; do
    trimmed="${port//[[:space:]]/}"
    [[ -n "$trimmed" ]] || die "--torii-ports contains an empty port entry"
    [[ "$trimmed" =~ ^[0-9]+$ ]] || die "--torii-ports must be a comma-separated list of numeric ports"
    port_number=$((10#$trimmed))
    (( port_number >= 1 && port_number <= 65535 )) || die "--torii-ports contains out-of-range port ${trimmed}; expected 1..65535"
    case "$seen_ports" in
      *" ${port_number} "*)
        die "--torii-ports contains duplicate port ${port_number}"
        ;;
    esac
    seen_ports="${seen_ports}${port_number} "
    if [[ -z "${normalized:-}" ]]; then
      normalized="$port_number"
    else
      normalized="${normalized},${port_number}"
    fi
  done
  printf '%s\n' "$normalized"
}

sha256_file() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  elif command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    die "neither shasum nor sha256sum is available"
  fi
}

is_peer_config_path_fallback() {
  local path="$1"
  local base
  [[ "$path" == "$DIST"/peer*.toml ]] || return 1
  base="$(basename "$path")"
  [[ "$base" =~ ^peer[0-9]+\.toml$ ]]
}

command_references_dist_peer_config_fallback() {
  local command_line="$1"
  local expect_config=0
  local token config_path
  # Fallback parser for minimal shells where python3 is unavailable. The
  # generated Taira localnet paths do not contain spaces.
  for token in $command_line; do
    token="${token#\"}"
    token="${token%\"}"
    token="${token#\'}"
    token="${token%\'}"
    if [[ $expect_config -eq 1 ]]; then
      is_peer_config_path_fallback "$token"
      return
    fi
    case "$token" in
      --config=*)
        config_path="${token#--config=}"
        is_peer_config_path_fallback "$config_path" && return 0
        ;;
      --config)
        expect_config=1
        ;;
    esac
  done
  return 1
}

command_references_dist_peer_config() {
  local command_line="$1"
  if ! command -v python3 >/dev/null 2>&1; then
    command_references_dist_peer_config_fallback "$command_line"
    return
  fi
  COMMAND_LINE="$command_line" DIST_ROOT="$DIST" python3 - <<'PY'
import os
import pathlib
import shlex
import sys

command_line = os.environ["COMMAND_LINE"]
dist = pathlib.Path(os.environ["DIST_ROOT"]).resolve()
try:
    args = shlex.split(command_line)
except ValueError:
    args = command_line.split()


def is_peer_config(value):
    try:
        path = pathlib.Path(value).resolve()
    except OSError:
        return False
    name = path.name
    return (
        path.parent == dist
        and name.startswith("peer")
        and name.endswith(".toml")
        and name[len("peer") : -len(".toml")].isdigit()
    )


for index, arg in enumerate(args):
    if arg == "--config" and index + 1 < len(args) and is_peer_config(args[index + 1]):
        sys.exit(0)
    if arg.startswith("--config=") and is_peer_config(arg.split("=", 1)[1]):
        sys.exit(0)
sys.exit(1)
PY
}

command_looks_like_peer_runtime() {
  local command_line="$1"
  [[ "$command_line" =~ (^|[[:space:]/])(iroha3d|peer-runtime)([.][^[:space:]/]+)?([[:space:]]|$) ]]
}

pid_exists() {
  local pid="$1"
  kill -0 "$pid" 2>/dev/null || ps -p "$pid" >/dev/null 2>&1
}

pid_command_line() {
  local pid="$1"
  ps -p "$pid" -o command= 2>/dev/null || true
}

record_pidfile_peer_if_owned() {
  local pid="$1"
  local pidfile="$2"
  local command_line
  if ! pid_exists "$pid"; then
    return
  fi
  command_line="$(pid_command_line "$pid")"
  if [[ -z "$command_line" ]]; then
    unverified_live_pid_refs+=("${pidfile}:${pid}")
    return
  fi
  if command_looks_like_peer_runtime "$command_line" &&
    command_references_dist_peer_config "$command_line"; then
    stop_pids+=("$pid")
  else
    echo "notice: ignoring stale or reused pidfile ${pidfile}; pid ${pid} command does not reference $DIST/peer*.toml" >&2
  fi
}

run_or_print() {
  local line="+"
  local quoted_arg
  for arg in "$@"; do
    printf -v quoted_arg '%q' "$arg"
    line+=" ${quoted_arg}"
  done
  printf '%s\n' "$line"
  if [[ $APPLY -eq 1 ]]; then
    "$@"
  fi
}

tcp_port_listening() {
  local port="$1"
  if command -v lsof >/dev/null 2>&1; then
    lsof -nP -iTCP:"$port" -sTCP:LISTEN 2>/dev/null | awk 'NR > 1 { found = 1 } END { exit found ? 0 : 1 }'
    return
  fi
  if command -v nc >/dev/null 2>&1; then
    nc -z 127.0.0.1 "$port" >/dev/null 2>&1
    return
  fi
  die "neither lsof nor nc is available to verify P2P listener port $port"
}

storage_owner_spec() {
  if [[ -n "$STORAGE_OWNER_GROUP" ]]; then
    printf '%s:%s\n' "$STORAGE_OWNER_USER" "$STORAGE_OWNER_GROUP"
  else
    printf '%s\n' "$STORAGE_OWNER_USER"
  fi
}

storage_access_issue_sample() {
  local storage_root="$DIST/storage"
  [[ -d "$storage_root" ]] || return 1
  find "$storage_root" \( ! -user "$STORAGE_OWNER_USER" -o ! -perm -u+w \) -print -quit
}

storage_access_issue_count() {
  local storage_root="$DIST/storage"
  [[ -d "$storage_root" ]] || {
    printf '0\n'
    return 0
  }
  find "$storage_root" \( ! -user "$STORAGE_OWNER_USER" -o ! -perm -u+w \) -print | wc -l | awk '{print $1}'
}

warn_or_die_storage_access_issue() {
  local sample count owner_spec message
  sample="$(storage_access_issue_sample || true)"
  [[ -n "$sample" ]] || return 0
  count="$(storage_access_issue_count)"
  owner_spec="$(storage_owner_spec)"
  message="DIST/storage contains ${count} entr$( [[ "$count" == "1" ]] && printf 'y' || printf 'ies' ) not owned by ${STORAGE_OWNER_USER} or not user-writable; first issue: ${sample}. Repair before starting peers, or rerun with --repair-storage-ownership using an account allowed to chown/chmod. Manual repair: sudo chown -R ${owner_spec} ${DIST}/storage && chmod -R u+rwX ${DIST}/storage"
  if [[ $APPLY -eq 1 ]]; then
    die "$message"
  fi
  echo "warning: $message" >&2
  DRY_RUN_HAS_WARNINGS=1
}

repair_storage_access() {
  local owner_spec sample
  [[ -d "$DIST/storage" ]] || return 0
  owner_spec="$(storage_owner_spec)"
  sample="$(storage_access_issue_sample || true)"
  if [[ -z "$sample" ]]; then
    return 0
  fi
  echo "==> repairing storage ownership for $DIST/storage as $owner_spec"
  run_or_print chown -R "$owner_spec" "$DIST/storage"
  run_or_print chmod -R u+rwX "$DIST/storage"
  if [[ $APPLY -eq 1 ]]; then
    sample="$(storage_access_issue_sample || true)"
    [[ -z "$sample" ]] || die "storage ownership repair did not make DIST/storage runtime-writable; first remaining issue: $sample"
  fi
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dist)
      [[ $# -ge 2 ]] || die "missing value for --dist"
      DIST="$2"
      shift 2
      ;;
    --runtime-bin)
      [[ $# -ge 2 ]] || die "missing value for --runtime-bin"
      RUNTIME_BIN="$2"
      shift 2
      ;;
    --expected-runtime-sha)
      [[ $# -ge 2 ]] || die "missing value for --expected-runtime-sha"
      EXPECTED_RUNTIME_SHA="$2"
      shift 2
      ;;
    --start)
      START_AFTER=1
      shift
      ;;
    --torii-ports)
      [[ $# -ge 2 ]] || die "missing value for --torii-ports"
      TORII_PORTS="$2"
      shift 2
      ;;
    --p2p-ports)
      [[ $# -ge 2 ]] || die "missing value for --p2p-ports"
      P2P_PORTS="$2"
      shift 2
      ;;
    --repair-storage-ownership)
      REPAIR_STORAGE_OWNERSHIP=1
      shift
      ;;
    --storage-owner)
      [[ $# -ge 2 ]] || die "missing value for --storage-owner"
      STORAGE_OWNER_USER="${2%%:*}"
      STORAGE_OWNER_GROUP=""
      if [[ "$2" == *:* ]]; then
        STORAGE_OWNER_GROUP="${2#*:}"
      fi
      [[ -n "$STORAGE_OWNER_USER" ]] || die "--storage-owner user must not be empty"
      shift 2
      ;;
    --log-level)
      [[ $# -ge 2 ]] || die "missing value for --log-level"
      LOG_LEVEL="$2"
      shift 2
      ;;
    --stop-timeout-seconds)
      [[ $# -ge 2 ]] || die "missing value for --stop-timeout-seconds"
      STOP_TIMEOUT_SECONDS="$2"
      shift 2
      ;;
    --apply)
      APPLY=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "unknown argument: $1"
      ;;
  esac
done

[[ -n "$DIST" ]] || die "--dist is required"
[[ -d "$DIST" ]] || die "dist directory not found: $DIST"
is_positive_integer "$STOP_TIMEOUT_SECONDS" || die "--stop-timeout-seconds must be a positive integer"
id -u "$STORAGE_OWNER_USER" >/dev/null 2>&1 || die "--storage-owner user does not exist: $STORAGE_OWNER_USER"
if [[ -z "$STORAGE_OWNER_GROUP" ]]; then
  STORAGE_OWNER_GROUP="$(id -gn "$STORAGE_OWNER_USER")"
else
  getent group "$STORAGE_OWNER_GROUP" >/dev/null 2>&1 || dscl . -read "/Groups/${STORAGE_OWNER_GROUP}" >/dev/null 2>&1 || {
    die "--storage-owner group does not exist: $STORAGE_OWNER_GROUP"
  }
fi
TORII_PORTS="$(normalize_torii_ports_csv "$TORII_PORTS")"
P2P_PORTS="$(normalize_torii_ports_csv "$P2P_PORTS")"
if [[ $START_AFTER -eq 1 ]]; then
  [[ -x "$DIST/start.sh" ]] || die "start script is not executable: $DIST/start.sh"
fi
if [[ -n "$RUNTIME_BIN" ]]; then
  [[ -x "$RUNTIME_BIN" ]] || die "runtime binary is not executable: $RUNTIME_BIN"
fi
if [[ -n "$EXPECTED_RUNTIME_SHA" ]]; then
  [[ -n "$RUNTIME_BIN" ]] || die "--expected-runtime-sha requires --runtime-bin"
  [[ "$EXPECTED_RUNTIME_SHA" =~ ^[0-9a-fA-F]{64}$ ]] || die "--expected-runtime-sha must be a 64 character SHA-256 hex digest"
  actual_sha="$(sha256_file "$RUNTIME_BIN")"
  actual_sha_lower="$(printf '%s' "$actual_sha" | tr '[:upper:]' '[:lower:]')"
  expected_sha_lower="$(printf '%s' "$EXPECTED_RUNTIME_SHA" | tr '[:upper:]' '[:lower:]')"
  [[ "$actual_sha_lower" == "$expected_sha_lower" ]] || {
    die "runtime SHA mismatch: expected ${EXPECTED_RUNTIME_SHA}, got ${actual_sha}"
  }
fi
if [[ $START_AFTER -eq 1 && -z "$RUNTIME_BIN" ]]; then
  echo "warning: --start supplied without --runtime-bin; DIST/start.sh will choose its default runtime" >&2
fi

DIST="$(cd "$DIST" && pwd)"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"

if [[ $APPLY -ne 1 ]]; then
  echo "dry-run: pass --apply to stop peers and quarantine volatile consensus state" >&2
fi

echo "==> dist: $DIST"

if [[ $REPAIR_STORAGE_OWNERSHIP -ne 1 ]]; then
  warn_or_die_storage_access_issue
fi

stop_pids=()
unverified_live_pid_refs=()
for pidfile in "$DIST"/peer*.pid; do
  [[ -f "$pidfile" ]] || continue
  pid="$(cat "$pidfile" 2>/dev/null || true)"
  if [[ "$pid" =~ ^[0-9]+$ ]]; then
    record_pidfile_peer_if_owned "$pid" "$pidfile"
  fi
done

while read -r pid command_line; do
  [[ -n "$pid" ]] || continue
  [[ -n "$command_line" ]] || continue
  if command_looks_like_peer_runtime "$command_line" &&
    command_references_dist_peer_config "$command_line"; then
    stop_pids+=("$pid")
  fi
done < <(ps -axo pid=,command=)

if [[ ${#unverified_live_pid_refs[@]} -gt 0 ]]; then
  message="found ${#unverified_live_pid_refs[@]} live pidfile process(es) whose command line could not be inspected; refusing to quarantine volatile consensus state until stale pidfiles are removed or peer ownership is verifiable"
  if [[ $APPLY -eq 1 ]]; then
    die "$message"
  fi
  echo "warning: $message" >&2
  DRY_RUN_HAS_WARNINGS=1
fi

if [[ ${#stop_pids[@]} -gt 0 ]]; then
  unique_pids=()
  while IFS= read -r pid; do
    [[ -n "$pid" ]] || continue
    unique_pids+=("$pid")
  done < <(printf '%s\n' "${stop_pids[@]}" | awk '!seen[$0]++')
  stop_pids=("${unique_pids[@]}")
  echo "==> stopping ${#stop_pids[@]} peer process(es)"
  inaccessible_pids=()
  for pid in "${stop_pids[@]}"; do
    if kill -0 "$pid" 2>/dev/null; then
      run_or_print kill "$pid"
    elif ps -p "$pid" >/dev/null 2>&1; then
      inaccessible_pids+=("$pid")
    fi
  done
  if [[ ${#inaccessible_pids[@]} -gt 0 ]]; then
    message="matched ${#inaccessible_pids[@]} peer process(es) but cannot signal them as the current user; rerun as the peer process owner or with sudo before quarantining volatile consensus state"
    if [[ $APPLY -eq 1 ]]; then
      die "$message"
    fi
    echo "warning: $message" >&2
    DRY_RUN_HAS_WARNINGS=1
  fi
  if [[ $APPLY -eq 1 ]]; then
    sleep "$STOP_TIMEOUT_SECONDS"
  fi
  for pid in "${stop_pids[@]}"; do
    if kill -0 "$pid" 2>/dev/null; then
      run_or_print kill -9 "$pid"
    fi
  done
else
  echo "==> no running peer processes matched $DIST/peer*.toml"
fi

for pidfile in "$DIST"/peer*.pid; do
  [[ -f "$pidfile" ]] || continue
  run_or_print rm -f "$pidfile"
done

if [[ $REPAIR_STORAGE_OWNERSHIP -eq 1 ]]; then
  repair_storage_access
else
  warn_or_die_storage_access_issue
fi

echo "==> quarantining volatile consensus state"
for storage in "$DIST"/storage/peer*; do
  [[ -d "$storage" ]] || continue
  run_or_print mkdir -p "$storage/queue_journal_quarantine" "$storage/rbc_sessions_quarantine"
  for journal in "$storage"/queue_plan_journal*; do
    [[ -e "$journal" ]] || continue
    base="$(basename "$journal")"
    target="$storage/queue_journal_quarantine/${base}.${STAMP}"
    run_or_print mv "$journal" "$target"
  done
  if [[ -d "$storage/rbc_sessions" ]]; then
    target="$storage/rbc_sessions_quarantine/rbc_sessions.${STAMP}"
    run_or_print mv "$storage/rbc_sessions" "$target"
  fi
done

echo "==> archiving peer logs"
run_or_print mkdir -p "$DIST/logs"
for logfile in "$DIST"/peer*.log; do
  [[ -f "$logfile" ]] || continue
  base="$(basename "$logfile")"
  run_or_print mv "$logfile" "$DIST/logs/${base}.pre-volatile-clear-${STAMP}"
done

if [[ $START_AFTER -eq 1 ]]; then
  echo "==> starting peers"
  if [[ $APPLY -eq 1 ]]; then
    (
      cd "$DIST"
      if [[ -n "$RUNTIME_BIN" ]]; then
        IROHAD_BIN="$RUNTIME_BIN" LOG_LEVEL="$LOG_LEVEL" ./start.sh
      else
        LOG_LEVEL="$LOG_LEVEL" ./start.sh
      fi
    )
  else
    if [[ -n "$RUNTIME_BIN" ]]; then
      echo "+ (cd $(printf '%q' "$DIST") && IROHAD_BIN=$(printf '%q' "$RUNTIME_BIN") LOG_LEVEL=$(printf '%q' "$LOG_LEVEL") ./start.sh)"
    else
      echo "+ (cd $(printf '%q' "$DIST") && LOG_LEVEL=$(printf '%q' "$LOG_LEVEL") ./start.sh)"
    fi
  fi

  if [[ $APPLY -eq 1 && -n "$TORII_PORTS" ]]; then
    IFS=',' read -r -a ports <<<"$TORII_PORTS"
    echo "==> waiting for Torii ports: ${ports[*]}"
    torii_ready=0
    for _ in $(seq 1 180); do
      ok=1
      for port in "${ports[@]}"; do
        [[ -n "$port" ]] || continue
        if ! curl -fsS --max-time 3 -H "Accept: application/json" \
          "http://127.0.0.1:${port}/v1/sumeragi/status" >/tmp/taira-clear-sumeragi-"${port}".json; then
          ok=0
          break
        fi
      done
      if [[ "$ok" == "1" ]]; then
        torii_ready=1
        break
      fi
      sleep 2
    done
    [[ "$torii_ready" == "1" ]] || die "Torii readiness failed: one or more configured Torii ports did not return /v1/sumeragi/status"

    echo "==> local summaries"
    for port in "${ports[@]}"; do
      [[ -n "$port" ]] || continue
      echo "port=$port"
      if curl -fsS --max-time 3 -H "Accept: application/json" \
        "http://127.0.0.1:${port}/v1/sumeragi/status" >/tmp/taira-clear-sumeragi-"${port}".json; then
        PORT="$port" python3 - <<'PY'
import json
import os
from pathlib import Path

port = os.environ["PORT"]
payload = json.loads(Path(f"/tmp/taira-clear-sumeragi-{port}.json").read_text())
context = payload.get("height_context") or {}
commit = payload.get("last_commit_qc") or {}
certificate = commit.get("certificate") or {}
round_ = certificate.get("round") or {}
operator = payload.get("operator") or {}
tx_queue = operator.get("tx_queue") or {}


def tag(record, key):
    return record.get(key) if isinstance(record, dict) else None


print(json.dumps({
    "protocol_version": payload.get("protocol_version"),
    "height": payload.get("height"),
    "view": payload.get("view"),
    "phase": tag(payload.get("phase"), "phase"),
    "body_state": tag(payload.get("body_state"), "state"),
    "pending_persistence_id": payload.get("pending_persistence_id"),
    "mode": tag(context.get("mode"), "mode"),
    "epoch": context.get("epoch"),
    "validator_count": context.get("validator_count"),
    "last_committed_height": payload.get("last_committed_height"),
    "commit_qc_height": round_.get("height"),
    "commit_qc_signers": commit.get("signer_count"),
    "commit_qc_signed_power": commit.get("signed_power"),
    "view_change_install_total": operator.get("view_change_install_total"),
    "busy_deferral_total": operator.get("busy_deferral_total"),
    "tx_queue_depth": tx_queue.get("queued_transactions"),
    "tx_queue_capacity": tx_queue.get("capacity"),
    "lane_block_sessions": len(payload.get("lane_block_sessions", []))
        if isinstance(payload.get("lane_block_sessions"), list)
        else None,
}, sort_keys=True))
PY
      fi
    done
  fi

  if [[ $APPLY -eq 1 && -n "$P2P_PORTS" ]]; then
    IFS=',' read -r -a p2p_ports <<<"$P2P_PORTS"
    echo "==> waiting for P2P listener ports: ${p2p_ports[*]}"
    p2p_ready=0
    for _ in $(seq 1 90); do
      ok=1
      for port in "${p2p_ports[@]}"; do
        [[ -n "$port" ]] || continue
        if ! tcp_port_listening "$port"; then
          ok=0
          break
        fi
      done
      if [[ "$ok" == "1" ]]; then
        p2p_ready=1
        break
      fi
      sleep 2
    done
    [[ "$p2p_ready" == "1" ]] || die "P2P listener readiness failed: one or more configured P2P ports are not listening"
  fi
fi

if [[ $APPLY -eq 1 ]]; then
  echo "volatile consensus quarantine completed."
elif [[ $DRY_RUN_HAS_WARNINGS -ne 0 ]]; then
  echo "volatile consensus quarantine dry-run completed with warnings; fix them before running with --apply" >&2
  exit 2
else
  echo "volatile consensus quarantine dry-run completed."
fi
