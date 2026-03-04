#!/usr/bin/env bash
set -euo pipefail

abs_path() {
  local input="$1"
  if [[ "$input" = /* ]]; then
    printf '%s\n' "$input"
  else
    local dir
    dir="$(cd "$(dirname "$input")" && pwd)"
    printf '%s/%s\n' "$dir" "$(basename "$input")"
  fi
}

usage() {
  cat <<'USAGE'
sorafs_direct_mode_smoke.sh [--plan PATH] [--manifest-id HEX] --provider name=ALIAS,provider-id=HEX,base-url=URL,stream-token=BASE64 [...]

Runs a direct-mode fetch against Torii gateways using `sorafs_cli fetch`. The helper wires the
direct-mode orchestrator policy JSON, captures the summary, and ensures scoreboard persistence for
rollout evidence. Parameters can be supplied via CLI flags or a key=value config file; flags take
precedence.

Required:
  --plan PATH                     Chunk plan JSON consumed by sorafs_cli fetch.
  --manifest-id HEX               32-byte manifest hash in lowercase hex.
  --provider SPEC                 Provider advert (may be repeated). Format:
                                  name=ALIAS,provider-id=HEX,base-url=URL,stream-token=BASE64

Optional:
  --config PATH                   Config file with key=value entries (plan, manifest_id, provider,
                                  policy, output, summary, workspace, cli).
  --policy PATH                   Orchestrator policy JSON (defaults to docs/examples/...).
  --output PATH                   Output payload path (default: <workspace>/artifacts/sorafs_direct_mode/payload.bin).
  --summary PATH                  JSON summary path (default: alongside output).
  --adoption-report PATH          Adoption report path (default: alongside summary).
  --min-providers N               Require at least N eligible providers when running the adoption gate (default: provider count).
  --require-telemetry             Fail the adoption gate when telemetry metadata is missing.
  --skip-adoption-check           Skip the adoption gate (not recommended; use only for local diagnostics).
  --workspace PATH                Repository root (default: current directory).
  --cli PATH                      Explicit sorafs_cli binary to use.
  --help                          Show this help message.

Config file format:
  plan=relative/or/absolute/path.json
  manifest_id=hexstring
  provider=name=...,provider-id=...,base-url=...,stream-token=...
  policy=docs/examples/sorafs_direct_mode_policy.json
  output=artifacts/sorafs_direct_mode/payload.bin
  summary=artifacts/sorafs_direct_mode/fetch_summary.json
  workspace=/path/to/repo
  cli=/path/to/sorafs_cli

Lines starting with `#` are ignored.
USAGE
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

workspace=""
config_path=""
plan_path=""
manifest_id=""
policy_path=""
output_path=""
summary_path=""
adoption_report_path=""
cli_path=""
min_providers_override=""
require_telemetry=0
skip_adoption_check=0

declare -a providers_cli=()
declare -a providers_config=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --config)
      [[ $# -lt 2 ]] && { echo "error: --config requires a path" >&2; exit 1; }
      config_path="$(abs_path "$2")"
      shift 2
      ;;
    --workspace)
      [[ $# -lt 2 ]] && { echo "error: --workspace requires a path" >&2; exit 1; }
      workspace="$2"
      shift 2
      ;;
    --plan)
      [[ $# -lt 2 ]] && { echo "error: --plan requires a path" >&2; exit 1; }
      plan_path="$2"
      shift 2
      ;;
    --manifest-id)
      [[ $# -lt 2 ]] && { echo "error: --manifest-id requires a value" >&2; exit 1; }
      manifest_id="$2"
      shift 2
      ;;
    --policy)
      [[ $# -lt 2 ]] && { echo "error: --policy requires a path" >&2; exit 1; }
      policy_path="$2"
      shift 2
      ;;
    --output)
      [[ $# -lt 2 ]] && { echo "error: --output requires a path" >&2; exit 1; }
      output_path="$2"
      shift 2
      ;;
    --summary)
      [[ $# -lt 2 ]] && { echo "error: --summary requires a path" >&2; exit 1; }
      summary_path="$2"
      shift 2
      ;;
    --adoption-report)
      [[ $# -lt 2 ]] && { echo "error: --adoption-report requires a path" >&2; exit 1; }
      adoption_report_path="$2"
      shift 2
      ;;
    --min-providers)
      [[ $# -lt 2 ]] && { echo "error: --min-providers requires a value" >&2; exit 1; }
      min_providers_override="$2"
      shift 2
      ;;
    --require-telemetry)
      require_telemetry=1
      shift
      ;;
    --skip-adoption-check)
      skip_adoption_check=1
      shift
      ;;
    --cli)
      [[ $# -lt 2 ]] && { echo "error: --cli requires a path" >&2; exit 1; }
      cli_path="$2"
      shift 2
      ;;
    --provider)
      [[ $# -lt 2 ]] && { echo "error: --provider requires a spec" >&2; exit 1; }
      providers_cli+=("$2")
      shift 2
      ;;
    --provider=*)
      providers_cli+=("${1#*=}")
      shift
      ;;
    --plan=*|--manifest-id=*|--policy=*|--output=*|--summary=*|--workspace=*|--cli=*|--adoption-report=*|--min-providers=*)
      value="${1#*=}"
      case "$1" in
        --plan=*) plan_path="$value" ;;
        --manifest-id=*) manifest_id="$value" ;;
        --policy=*) policy_path="$value" ;;
        --output=*) output_path="$value" ;;
        --summary=*) summary_path="$value" ;;
        --adoption-report=*) adoption_report_path="$value" ;;
        --min-providers=*) min_providers_override="$value" ;;
        --workspace=*) workspace="$value" ;;
        --cli=*) cli_path="$value" ;;
      esac
      shift
      ;;
    *)
      echo "error: unknown option '$1'" >&2
      usage
      exit 1
      ;;
  esac
done

if [[ -n "$config_path" ]]; then
  if [[ ! -f "$config_path" ]]; then
    echo "error: config file not found at ${config_path}" >&2
    exit 1
  fi
  while IFS= read -r raw_line || [[ -n "$raw_line" ]]; do
    line="${raw_line%%#*}"
    line="${line%$'\r'}"
    if [[ -z "$line" ]]; then
      continue
    fi
    if [[ "$line" != *"="* ]]; then
      continue
    fi
    key="${line%%=*}"
    value="${line#*=}"
    case "$key" in
      plan) [[ -z "$plan_path" ]] && plan_path="$value" ;;
      manifest_id) [[ -z "$manifest_id" ]] && manifest_id="$value" ;;
      policy) [[ -z "$policy_path" ]] && policy_path="$value" ;;
      provider) providers_config+=("$value") ;;
      output) [[ -z "$output_path" ]] && output_path="$value" ;;
      summary) [[ -z "$summary_path" ]] && summary_path="$value" ;;
      workspace) [[ -z "$workspace" ]] && workspace="$value" ;;
      cli) [[ -z "$cli_path" ]] && cli_path="$value" ;;
      *)
        echo "warning: unknown config key '${key}' in ${config_path}" >&2
        ;;
    esac
  done < "$config_path"
fi

workspace="${workspace:-.}"
workspace="$(abs_path "$workspace")"

if [[ -z "$plan_path" ]]; then
  echo "error: --plan is required (provide via flag or config)" >&2
  exit 1
fi
plan_path="$(abs_path "$plan_path")"
if [[ ! -f "$plan_path" ]]; then
  echo "error: plan JSON not found at ${plan_path}" >&2
  exit 1
fi

if [[ -z "$manifest_id" ]]; then
  echo "error: --manifest-id is required" >&2
  exit 1
fi
manifest_id="$(echo "$manifest_id" | tr 'A-Z' 'a-z')"
if [[ ! "$manifest_id" =~ ^[0-9a-f]{64}$ ]]; then
  echo "error: manifest id must be 64 hex characters" >&2
  exit 1
fi

policy_path="${policy_path:-${workspace}/docs/examples/sorafs_direct_mode_policy.json}"
policy_path="$(abs_path "$policy_path")"
if [[ ! -f "$policy_path" ]]; then
  echo "error: orchestrator policy JSON not found at ${policy_path}" >&2
  exit 1
fi

providers=("${providers_config[@]}" "${providers_cli[@]}")
if [[ "${#providers[@]}" -eq 0 ]]; then
  echo "error: at least one --provider specification is required" >&2
  exit 1
fi
if [[ -n "$min_providers_override" ]]; then
  if [[ ! "$min_providers_override" =~ ^[0-9]+$ || "$min_providers_override" -lt 1 ]]; then
    echo "error: --min-providers must be a positive integer" >&2
    exit 1
  fi
fi

if [[ -z "$output_path" ]]; then
  output_path="${workspace}/artifacts/sorafs_direct_mode/payload.bin"
fi
if [[ -z "$summary_path" ]]; then
  summary_path="$(dirname "$(abs_path "$output_path")")/fetch_summary.json"
else
  summary_path="$(abs_path "$summary_path")"
fi
output_path="$(abs_path "$output_path")"
mkdir -p "$(dirname "$output_path")"
mkdir -p "$(dirname "$summary_path")"
if [[ -n "$adoption_report_path" ]] ; then
  adoption_report_path="$(abs_path "$adoption_report_path")"
fi

persist_path=""
if command -v python3 >/dev/null 2>&1; then
  persist_path="$(python3 - "$policy_path" <<'PY'
import json, sys
path = sys.argv[1]
try:
    with open(path, 'r', encoding='utf-8') as fh:
        data = json.load(fh)
    persist = data.get("scoreboard", {}).get("persist_path")
    if isinstance(persist, str) and persist.strip():
        print(persist.strip())
except Exception:
    pass
PY
)"
fi

if [[ -n "$persist_path" ]]; then
  if [[ "$persist_path" != /* ]]; then
    persist_path="${workspace}/${persist_path}"
  fi
  mkdir -p "$(dirname "$persist_path")"
fi

if [[ -n "$cli_path" ]]; then
  cli_bin="$(abs_path "$cli_path")"
else
  if command -v sorafs_cli >/dev/null 2>&1; then
    cli_bin="$(command -v sorafs_cli)"
  else
    cli_bin="${workspace}/target/release/sorafs_cli"
    if [[ ! -x "$cli_bin" ]]; then
      echo "Building sorafs_cli (release)..."
      (cd "$workspace" && cargo build -p sorafs_orchestrator --bin sorafs_cli --release)
    fi
  fi
fi

if [[ ! -x "$cli_bin" ]]; then
  echo "error: sorafs_cli binary not executable at ${cli_bin}" >&2
  exit 1
fi

echo "==> Running direct-mode smoke fetch"
echo " workspace: ${workspace}"
echo " plan:      ${plan_path}"
echo " manifest:  ${manifest_id}"
echo " policy:    ${policy_path}"
for provider in "${providers[@]}"; do
  echo " provider:  ${provider}"
done
echo " output:    ${output_path}"
echo " summary:   ${summary_path}"
if [[ -n "$persist_path" ]]; then
  echo " scoreboard:${persist_path}"
fi

cmd=("$cli_bin" fetch "--plan=${plan_path}" "--manifest-id=${manifest_id}" "--orchestrator-config=${policy_path}" "--json-out=${summary_path}" "--output=${output_path}")
for provider in "${providers[@]}"; do
  cmd+=("--provider=${provider}")
done

(
  cd "${workspace}"
  "${cmd[@]}"
)

echo "==> Smoke fetch complete"
echo "Summary saved to ${summary_path}"
if [[ -n "$persist_path" ]]; then
  if [[ ! -f "$persist_path" ]]; then
    echo "warning: orchestrator persisted scoreboard not found at ${persist_path}" >&2
  else
    echo "Scoreboard snapshot stored at ${persist_path}"
  fi
fi
echo "Payload written to ${output_path}"

if [[ ${skip_adoption_check} -eq 0 ]]; then
  if [[ -z "$persist_path" ]]; then
    echo "error: adoption check enabled but the orchestrator policy omitted scoreboard.persist_path" >&2
    exit 1
  fi
  if [[ ! -s "$persist_path" ]]; then
    echo "error: expected scoreboard at ${persist_path} before running adoption gate" >&2
    exit 1
  fi
  if [[ ! -s "$summary_path" ]]; then
    echo "error: summary JSON at ${summary_path} is missing or empty" >&2
    exit 1
  fi
  min_providers_effective="$min_providers_override"
  if [[ -z "$min_providers_effective" ]]; then
    min_providers_effective="${#providers[@]}"
  fi
  if [[ -z "$min_providers_effective" || "$min_providers_effective" -lt 1 ]]; then
    min_providers_effective=1
  fi
  if [[ -z "$adoption_report_path" ]]; then
    adoption_report_path="$(dirname "$summary_path")/adoption_report.json"
  fi
  mkdir -p "$(dirname "$adoption_report_path")"

  echo "Running cargo xtask sorafs-adoption-check (min providers: ${min_providers_effective})"
  # shellcheck disable=SC2206
  ADOPTION_FLAGS=()
  if [[ -n "${XTASK_SORAFS_ADOPTION_FLAGS:-}" ]]; then
    if ! command -v python3 >/dev/null 2>&1; then
      echo "error: python3 is required to parse XTASK_SORAFS_ADOPTION_FLAGS" >&2
      exit 1
    fi
    while IFS= read -r flag; do
      if [[ -n "$flag" ]]; then
        ADOPTION_FLAGS+=("$flag")
      fi
    done < <(
      XTASK_SORAFS_ADOPTION_FLAGS_INPUT="${XTASK_SORAFS_ADOPTION_FLAGS}" python3 <<'PY'
import os
import shlex

flags = os.environ.get("XTASK_SORAFS_ADOPTION_FLAGS_INPUT", "")
try:
    parts = shlex.split(flags)
except ValueError as exc:
    raise SystemExit(f"error: failed to parse XTASK_SORAFS_ADOPTION_FLAGS: {exc}") from exc
for part in parts:
    print(part)
PY
    )
  fi
  if [[ ${require_telemetry} -eq 1 ]]; then
    needs_flag=1
    if (( ${#ADOPTION_FLAGS[@]} )); then
      for flag in "${ADOPTION_FLAGS[@]}"; do
        if [[ "$flag" == "--require-telemetry" ]]; then
          needs_flag=0
          break
        fi
      done
    fi
    if [[ ${needs_flag} -eq 1 ]]; then
      ADOPTION_FLAGS+=("--require-telemetry")
    fi
  fi
  needs_direct_only=1
  if (( ${#ADOPTION_FLAGS[@]} )); then
    for flag in "${ADOPTION_FLAGS[@]}"; do
      if [[ "$flag" == "--require-direct-only" ]]; then
        needs_direct_only=0
        break
      fi
    done
  fi
  if [[ ${needs_direct_only} -eq 1 ]]; then
    ADOPTION_FLAGS+=("--require-direct-only")
  fi
  adoption_cmd=(
    cargo xtask sorafs-adoption-check
    --scoreboard "${persist_path}"
    --summary "${summary_path}"
    --min-providers "${min_providers_effective}"
    --require-metadata
    --report "${adoption_report_path}"
  )
  if (( ${#ADOPTION_FLAGS[@]} )); then
    adoption_cmd+=("${ADOPTION_FLAGS[@]}")
  fi
  (
    cd "${workspace}"
    "${adoption_cmd[@]}"
  )
  if [[ ! -s "$adoption_report_path" ]]; then
    echo "error: adoption report not written to ${adoption_report_path}" >&2
    exit 1
  fi
  echo "Adoption report stored at ${adoption_report_path}"
else
  echo "Skipping adoption check (per --skip-adoption-check)"
fi
