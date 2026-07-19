#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_ROOT}/../../.." && pwd)"
PYTHON="${PYTHON:-python3}"

log() {
  printf '[alias-setup] %s\n' "$*" >&2
}

err() {
  log "error: $*"
  exit 1
}

usage() {
  cat <<'EOF'
Usage: sns_bulk_release.sh --intent PATH [options]

Plans one typed alias setup vector against live state. Planning is the default.
Use --apply to have the ordinary client verify the plan, locally sign one normal
transaction, and submit that transaction through the existing transaction path.

Options:
  --intent PATH               Secret-free alias setup intent JSON (required)
  --release-dir PATH          Artifact root (default: artifacts/sns/releases)
  --release-name NAME         Artifact directory name (default: UTC timestamp)
  --plan-file PATH            Verified plan output (default: <release>/alias-plan.json)
  --metrics PATH              Plan metrics output (default: <release>/metrics.prom)
  --summary PATH              Secret-free summary output (default: <release>/summary.json)
  --iroha-cli PATH            Iroha CLI executable (default: iroha)
  --config PATH               Ordinary client configuration used for signing
  --apply                     Submit the complete verified plan atomically
  -h, --help                  Show this help message

Raw tokens, private keys, direct Torii mutation URLs, split manifests, suffix
maps, and submission-log inputs are intentionally unsupported.
EOF
}

INTENT_PATH=""
RELEASE_ROOT="${SNS_RELEASE_DIR:-artifacts/sns/releases}"
RELEASE_NAME="${SNS_RELEASE_NAME:-}"
PLAN_PATH=""
METRICS_PATH=""
SUMMARY_PATH=""
IROHA_CLI="iroha"
CLIENT_CONFIG=""
APPLY=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --intent)
      [[ $# -ge 2 ]] || err "--intent requires a path"
      INTENT_PATH="$2"
      shift 2
      ;;
    --release-dir)
      [[ $# -ge 2 ]] || err "--release-dir requires a path"
      RELEASE_ROOT="$2"
      shift 2
      ;;
    --release-name)
      [[ $# -ge 2 ]] || err "--release-name requires a name"
      RELEASE_NAME="$2"
      shift 2
      ;;
    --plan-file)
      [[ $# -ge 2 ]] || err "--plan-file requires a path"
      PLAN_PATH="$2"
      shift 2
      ;;
    --metrics)
      [[ $# -ge 2 ]] || err "--metrics requires a path"
      METRICS_PATH="$2"
      shift 2
      ;;
    --summary)
      [[ $# -ge 2 ]] || err "--summary requires a path"
      SUMMARY_PATH="$2"
      shift 2
      ;;
    --iroha-cli)
      [[ $# -ge 2 ]] || err "--iroha-cli requires a path"
      IROHA_CLI="$2"
      shift 2
      ;;
    --config)
      [[ $# -ge 2 ]] || err "--config requires a path"
      CLIENT_CONFIG="$2"
      shift 2
      ;;
    --apply)
      APPLY=true
      shift
      ;;
    --token|--token=*|--submit-token|--submit-token=*|--private-key|--private-key=*|--private_key|--private_key=*)
      err "raw token and private-key command-line values are forbidden"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      err "unsupported command-line argument"
      ;;
  esac
done

[[ -n "${INTENT_PATH}" ]] || err "--intent is required"
[[ -f "${INTENT_PATH}" ]] || err "alias setup intent is not a readable file"

if [[ -z "${RELEASE_NAME}" ]]; then
  RELEASE_NAME="$(date -u +%Y%m%dT%H%M%SZ)"
fi

release_dir="${RELEASE_ROOT%/}/${RELEASE_NAME}"
mkdir -p "${release_dir}"

plan_out="${PLAN_PATH:-${release_dir}/alias-plan.json}"
metrics_out="${METRICS_PATH:-${release_dir}/metrics.prom}"
summary_out="${SUMMARY_PATH:-${release_dir}/summary.json}"
mkdir -p "$(dirname "${plan_out}")" "$(dirname "${metrics_out}")" "$(dirname "${summary_out}")"

planner_args=(
  "${INTENT_PATH}"
  "--plan-file" "${plan_out}"
  "--iroha-cli" "${IROHA_CLI}"
)
if [[ -n "${CLIENT_CONFIG}" ]]; then
  planner_args+=("--config" "${CLIENT_CONFIG}")
fi
if [[ "${APPLY}" != "true" ]]; then
  planner_args+=("--plan-only")
fi

if [[ "${APPLY}" == "true" ]]; then
  log "Planning and atomically submitting the typed alias setup"
else
  log "Planning the typed alias setup without mutation"
fi
(cd "${REPO_ROOT}" && "${PYTHON}" scripts/sns_bulk_onboard.py "${planner_args[@]}")

log "Generating plan-derived metrics"
(cd "${REPO_ROOT}" && "${PYTHON}" scripts/sns_bulk_metrics.py \
  --plan "${plan_out}" \
  --release "${RELEASE_NAME}" \
  --output "${metrics_out}")

mode="planned"
if [[ "${APPLY}" == "true" ]]; then
  mode="submitted"
fi
"${PYTHON}" - "${summary_out}" "${INTENT_PATH}" "${plan_out}" "${metrics_out}" "${RELEASE_NAME}" "${mode}" <<'PY'
import json
import sys
from pathlib import Path

summary_path, intent_path, plan_path, metrics_path, release_name, mode = sys.argv[1:]
summary = {
    "schema_version": 1,
    "release_name": release_name,
    "mode": mode,
    "intent": intent_path,
    "plan": plan_path,
    "metrics": metrics_path,
}
Path(summary_path).write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8")
PY

log "Secret-free alias setup artifacts written to ${release_dir}"
