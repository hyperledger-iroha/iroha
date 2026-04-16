#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DEFAULT_PROFILE="single-peer"

usage() {
  cat <<'EOF'
Usage: scripts/mochi_local_sandbox.sh <command>

Commands:
  up               Start a local Mochi sandbox in the background.
  down             Stop the current local Mochi sandbox.
  status           Print the current sandbox status.
  reset            Stop the sandbox and remove its runtime state.
  env              Print shell exports for the current sandbox.
  mcp-add-command  Print the Codex MCP add command for the current sandbox.

Environment:
  MOCHI_WORKSPACE_ROOT         Workspace root for .env.local and .mochi/generated/* (default: current directory)
  MOCHI_PROFILE                Preset profile slug (single-peer|four-peer-bft)
  MOCHI_PROFILE_SLUG           Explicit sandbox slug override when MOCHI_PROFILE is custom
  MOCHI_START_TIMEOUT_SECONDS  Seconds to wait for session.json readiness (default: 1200)

All other MOCHI_* variables supported by `mochi sandbox serve` are forwarded to the child process.
EOF
}

resolve_workspace_root() {
  local root="${MOCHI_WORKSPACE_ROOT:-$PWD}"
  python3 - "$root" <<'PY'
import os
import sys

print(os.path.abspath(sys.argv[1]))
PY
}

resolve_profile_slug() {
  if [[ -n "${MOCHI_PROFILE_SLUG:-}" ]]; then
    printf '%s\n' "${MOCHI_PROFILE_SLUG}"
    return 0
  fi

  local profile="${MOCHI_PROFILE:-$DEFAULT_PROFILE}"
  case "$profile" in
    single-peer|four-peer-bft)
      printf '%s\n' "$profile"
      ;;
    *)
      printf 'Unsupported MOCHI_PROFILE `%s`. Use single-peer, four-peer-bft, or set MOCHI_PROFILE_SLUG explicitly.\n' "$profile" >&2
      return 1
      ;;
  esac
}

resolve_sandbox_root() {
  local workspace_root="$1"
  local profile_slug="$2"
  printf '%s/.mochi/sandbox/%s\n' "$workspace_root" "$profile_slug"
}

shell_quote() {
  local value="${1//\'/\'\"\'\"\'}"
  if [[ -z "$1" ]]; then
    printf "''"
  elif [[ "$1" =~ ^[A-Za-z0-9._/@:-]+$ ]]; then
    printf '%s' "$1"
  else
    printf "'%s'" "$value"
  fi
}

json_field() {
  local session_file="$1"
  local field="$2"
  python3 - "$session_file" "$field" <<'PY'
import json
import sys

path, field = sys.argv[1], sys.argv[2]
with open(path, "r", encoding="utf-8") as handle:
    payload = json.load(handle)

value = payload
for part in field.split("."):
    if isinstance(value, dict):
        value = value.get(part)
    else:
        value = None
        break

if value is None:
    raise SystemExit(1)

if isinstance(value, bool):
    print("true" if value else "false")
elif isinstance(value, list):
    for item in value:
        print(item)
else:
    print(value)
PY
}

session_ready() {
  local session_file="$1"
  [[ -f "$session_file" ]] || return 1
  python3 - "$session_file" <<'PY'
import json
import sys

with open(sys.argv[1], "r", encoding="utf-8") as handle:
    payload = json.load(handle)

raise SystemExit(0 if payload.get("ready") and payload.get("mcp_ready") else 1)
PY
}

pid_from_file() {
  local pid_file="$1"
  [[ -f "$pid_file" ]] || return 1
  tr -d '[:space:]' <"$pid_file"
}

pid_running() {
  local pid="$1"
  [[ -n "$pid" ]] || return 1
  kill -0 "$pid" 2>/dev/null
}

cleanup_stale_pid() {
  local pid_file="$1"
  if ! [[ -f "$pid_file" ]]; then
    return 0
  fi
  local pid
  pid="$(pid_from_file "$pid_file" || true)"
  if [[ -z "$pid" ]] || ! pid_running "$pid"; then
    rm -f "$pid_file"
  fi
}

print_status() {
  local workspace_root profile_slug sandbox_root pid_file log_file session_file
  workspace_root="$(resolve_workspace_root)"
  profile_slug="$(resolve_profile_slug)"
  sandbox_root="$(resolve_sandbox_root "$workspace_root" "$profile_slug")"
  pid_file="${sandbox_root}/serve.pid"
  log_file="${sandbox_root}/serve.log"
  session_file="${sandbox_root}/session.json"

  cleanup_stale_pid "$pid_file"

  local pid=""
  local status="stopped"
  if pid="$(pid_from_file "$pid_file" 2>/dev/null)" && pid_running "$pid"; then
    if session_ready "$session_file"; then
      status="ready"
    else
      status="starting"
    fi
  elif [[ -f "$session_file" ]]; then
    status="stale-session"
  fi

  printf 'status: %s\n' "$status"
  printf 'workspace: %s\n' "$workspace_root"
  printf 'sandbox: %s\n' "$sandbox_root"
  printf 'log: %s\n' "$log_file"
  if [[ -n "$pid" ]]; then
    printf 'pid: %s\n' "$pid"
  fi
  if [[ -f "$session_file" ]]; then
    printf 'session: %s\n' "$session_file"
    printf 'torii: %s\n' "$(json_field "$session_file" torii_url || printf 'unknown')"
    printf 'mcp: %s\n' "$(json_field "$session_file" mcp_url || printf 'unknown')"
    printf 'ready: %s\n' "$(json_field "$session_file" ready || printf 'false')"
    printf 'mcp_ready: %s\n' "$(json_field "$session_file" mcp_ready || printf 'false')"
  fi
}

tail_log_excerpt() {
  local log_file="$1"
  if [[ -f "$log_file" ]]; then
    printf '\nLast log lines:\n' >&2
    tail -n 40 "$log_file" >&2 || true
  fi
}

cmd_up() {
  local workspace_root profile_slug sandbox_root pid_file log_file session_file
  workspace_root="$(resolve_workspace_root)"
  profile_slug="$(resolve_profile_slug)"
  sandbox_root="$(resolve_sandbox_root "$workspace_root" "$profile_slug")"
  pid_file="${sandbox_root}/serve.pid"
  log_file="${sandbox_root}/serve.log"
  session_file="${sandbox_root}/session.json"

  mkdir -p "$sandbox_root"
  cleanup_stale_pid "$pid_file"

  local pid=""
  if pid="$(pid_from_file "$pid_file" 2>/dev/null)" && pid_running "$pid"; then
    printf 'MOCHI sandbox already running.\n'
    print_status
    return 0
  fi

  rm -f "$session_file"
  : >"$log_file"

  local -a cmd=(cargo run -p mochi-ui -- sandbox serve --build-binaries --workspace-root "$workspace_root")
  if [[ -n "${MOCHI_PROFILE:-}" ]]; then
    cmd+=(--profile "${MOCHI_PROFILE}")
  fi

  (
    cd "$REPO_ROOT"
    exec "${cmd[@]}"
  ) >"$log_file" 2>&1 &
  pid=$!
  printf '%s\n' "$pid" >"$pid_file"

  local timeout="${MOCHI_START_TIMEOUT_SECONDS:-1200}"
  local started_at=$SECONDS
  while true; do
    if session_ready "$session_file"; then
      printf 'MOCHI sandbox ready.\n'
      print_status
      return 0
    fi
    if ! pid_running "$pid"; then
      rm -f "$pid_file"
      printf 'MOCHI sandbox failed to start.\n' >&2
      tail_log_excerpt "$log_file"
      return 1
    fi
    if (( SECONDS - started_at >= timeout )); then
      printf 'Timed out waiting for %s after %ss.\n' "$session_file" "$timeout" >&2
      tail_log_excerpt "$log_file"
      return 1
    fi
    sleep 1
  done
}

cmd_down() {
  local workspace_root profile_slug sandbox_root pid_file session_file
  workspace_root="$(resolve_workspace_root)"
  profile_slug="$(resolve_profile_slug)"
  sandbox_root="$(resolve_sandbox_root "$workspace_root" "$profile_slug")"
  pid_file="${sandbox_root}/serve.pid"
  session_file="${sandbox_root}/session.json"

  cleanup_stale_pid "$pid_file"

  local pid
  pid="$(pid_from_file "$pid_file" 2>/dev/null || true)"
  if [[ -z "$pid" ]] || ! pid_running "$pid"; then
    rm -f "$pid_file" "$session_file"
    printf 'MOCHI sandbox is not running.\n'
    return 0
  fi

  kill -TERM "$pid" 2>/dev/null || true
  local deadline=$((SECONDS + 20))
  while pid_running "$pid"; do
    if (( SECONDS >= deadline )); then
      printf 'MOCHI sandbox did not exit after SIGTERM; inspect pid %s manually.\n' "$pid" >&2
      return 1
    fi
    sleep 1
  done

  rm -f "$pid_file" "$session_file"
  printf 'MOCHI sandbox stopped.\n'
}

cmd_reset() {
  local workspace_root profile_slug sandbox_root
  workspace_root="$(resolve_workspace_root)"
  profile_slug="$(resolve_profile_slug)"
  sandbox_root="$(resolve_sandbox_root "$workspace_root" "$profile_slug")"

  cmd_down >/dev/null
  rm -rf "$sandbox_root"
  printf 'Removed sandbox state at %s\n' "$sandbox_root"
}

require_session() {
  local session_file="$1"
  if ! [[ -f "$session_file" ]]; then
    printf 'No MOCHI session.json found at %s. Run `scripts/mochi_local_sandbox.sh up` first.\n' "$session_file" >&2
    return 1
  fi
}

cmd_env() {
  local workspace_root profile_slug sandbox_root session_file account_id private_key
  workspace_root="$(resolve_workspace_root)"
  profile_slug="$(resolve_profile_slug)"
  sandbox_root="$(resolve_sandbox_root "$workspace_root" "$profile_slug")"
  session_file="${sandbox_root}/session.json"

  require_session "$session_file"

  printf '# local dev only; rename variables to match your app\n'
  printf 'export IROHA_API_BASE=%s\n' "$(shell_quote "$(json_field "$session_file" api_base)")"
  printf 'export IROHA_TORII_URL=%s\n' "$(shell_quote "$(json_field "$session_file" torii_url)")"
  printf 'export IROHA_CHAIN_ID=%s\n' "$(shell_quote "$(json_field "$session_file" chain_id)")"
  printf 'export IROHA_MCP_URL=%s\n' "$(shell_quote "$(json_field "$session_file" mcp_url)")"

  if account_id="$(json_field "$session_file" account_id 2>/dev/null)"; then
    printf 'export IROHA_ACCOUNT_ID=%s\n' "$(shell_quote "$account_id")"
  fi
  if private_key="$(json_field "$session_file" private_key 2>/dev/null)"; then
    printf 'export IROHA_PRIVATE_KEY=%s\n' "$(shell_quote "$private_key")"
  fi
}

cmd_mcp_add_command() {
  local workspace_root profile_slug sandbox_root session_file mcp_url
  workspace_root="$(resolve_workspace_root)"
  profile_slug="$(resolve_profile_slug)"
  sandbox_root="$(resolve_sandbox_root "$workspace_root" "$profile_slug")"
  session_file="${sandbox_root}/session.json"

  require_session "$session_file"
  mcp_url="$(json_field "$session_file" mcp_url)"
  printf 'codex mcp add mochi-local --url %s\n' "$(shell_quote "$mcp_url")"
}

main() {
  local command="${1:-}"
  case "$command" in
    up)
      cmd_up
      ;;
    down)
      cmd_down
      ;;
    status)
      print_status
      ;;
    reset)
      cmd_reset
      ;;
    env)
      cmd_env
      ;;
    mcp-add-command)
      cmd_mcp_add_command
      ;;
    -h|--help|help|"")
      usage
      ;;
    *)
      printf 'Unknown command `%s`.\n\n' "$command" >&2
      usage >&2
      return 1
      ;;
  esac
}

main "$@"
