#!/usr/bin/env bash
set -euo pipefail

DIST=""
RUNTIME_BIN=""
EXPECTED_RUNTIME_SHA=""
LOG_LEVEL="${LOG_LEVEL:-info}"
TORII_PORTS="29080,29081,29082,29083"
STOP_TIMEOUT_SECONDS=5
START_AFTER=0
APPLY=0
DRY_RUN_HAS_WARNINGS=0

usage() {
  cat <<'EOF'
Usage: clear_volatile_consensus_state.sh --dist PATH --apply [options]

Quarantine volatile consensus recovery files for a Taira localnet-style
validator bundle without deleting durable ledger state. This is intended for
the observed public-finality stall where `/v1/sumeragi/status` is one block
ahead of the commit QC at a high view and signed writes expire.

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
  local port trimmed port_number normalized
  local -a ports
  declare -A seen_ports=()

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
    [[ -z "${seen_ports[$port_number]:-}" ]] || die "--torii-ports contains duplicate port ${port_number}"
    seen_ports[$port_number]=1
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
  [[ "$command_line" =~ (^|[[:space:]/])(irohad|iroha3d|peer-runtime)([[:space:]]|$) ]]
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
TORII_PORTS="$(normalize_torii_ports_csv "$TORII_PORTS")"
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
      [[ "$ok" == "1" ]] && break
      sleep 2
    done

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
canonical = payload.get("canonical") or {}
print(json.dumps({
    "commit_qc_height": (payload.get("commit_qc") or {}).get("height"),
    "highest_qc_height": (payload.get("highest_qc") or canonical.get("highest_qc") or {}).get("height"),
    "locked_qc_height": (payload.get("locked_qc") or canonical.get("locked_qc") or {}).get("height"),
    "canonical_height": canonical.get("height"),
    "membership_height": (payload.get("membership") or {}).get("height"),
    "tx_queue": payload.get("tx_queue"),
    "worker_stage": (payload.get("worker_loop") or {}).get("stage"),
}, sort_keys=True))
PY
      fi
    done
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
