#!/usr/bin/env bash
set -euo pipefail
shopt -s nocasematch

usage() {
  cat <<'USAGE'
Usage: scripts/release/load_env.sh [--file PATH] <profile>

Prints `export KEY=VALUE` lines for the requested release profile. The script
searches for an environment file in the following order (unless --file is provided):
  1. ./configs/release_env/<profile>.env
  2. $XDG_CONFIG_HOME/iroha-release/<profile>.env
  3. $HOME/.config/iroha-release/<profile>.env

Example:
  eval "$(scripts/release/load_env.sh android-maven-staging)"

Options:
  --file PATH    Load secrets from the specified file instead of the default search paths.
  -h, --help     Show this message.
USAGE
}

profile=""
override_file=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --file)
      override_file="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    -*)
      echo "[load_env] unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
    *)
      profile="$1"
      shift
      ;;
  esac
done

if [[ -z "$profile" ]]; then
  echo "[load_env] profile is required" >&2
  usage >&2
  exit 1
fi

find_env_file() {
  if [[ -n "$override_file" ]]; then
    echo "$override_file"
    return
  fi
  local candidates=()
  candidates+=("$(git rev-parse --show-toplevel)/configs/release_env/${profile}.env")
  local xdg="${XDG_CONFIG_HOME:-$HOME/.config}"
  candidates+=("${xdg}/iroha-release/${profile}.env")
  for candidate in "${candidates[@]}"; do
    if [[ -f "$candidate" ]]; then
      echo "$candidate"
      return
    fi
  done
  echo ""
}

env_file="$(find_env_file)"
if [[ -z "$env_file" ]]; then
  echo "[load_env] no environment file found for profile '${profile}'." >&2
  echo "[load_env] create configs/release_env/${profile}.env or place one under ~/.config/iroha-release/." >&2
  exit 1
fi

declare -a exports=()
while IFS= read -r raw_line || [[ -n "$raw_line" ]]; do
  line="${raw_line#"${raw_line%%[![:space:]]*}"}"
  if [[ -z "$line" || "${line:0:1}" == "#" ]]; then
    continue
  fi
  if [[ "$line" == export[[:space:]]* ]]; then
    line="${line:6}"
    line="${line#"${line%%[![:space:]]*}"}"
  fi
  if [[ "$line" != *"="* ]]; then
    echo "[load_env] skipping malformed line: $line" >&2
    continue
  fi
  key="${line%%=*}"
  value="${line#*=}"
  key="${key//[[:space:]]/}"
  value="${value%$'\r'}"
  value="${value#"${value%%[![:space:]]*}"}"
  value="${value%"${value##*[![:space:]]}"}"
  if [[ ${#value} -ge 2 ]]; then
    first_char="${value:0:1}"
    last_index=$(( ${#value} - 1 ))
    last_char="${value:$last_index:1}"
    if [[ "$first_char" == "\"" && "$last_char" == "\"" ]]; then
      value="${value:1:$((${#value} - 2))}"
    fi
  fi
  exports+=("export ${key}=$(printf '%q' "$value")")
done < "$env_file"

printf "%s\n" "${exports[@]}"
>&2 echo "[load_env] loaded ${#exports[@]} entries from ${env_file}"
