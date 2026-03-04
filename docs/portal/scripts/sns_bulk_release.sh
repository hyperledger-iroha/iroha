#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_ROOT}/../.." && pwd)"
PYTHON="${PYTHON:-python3}"

log() {
  printf '[sns-release] %s\n' "$*" >&2
}

err() {
  log "error: $*"
  exit 1
}

usage() {
  cat <<'EOF'
Usage: sns_bulk_release.sh [options]

Automates SNS registrar bulk releases by:
  1. Building a manifest + NDJSON stream from a CSV (or reusing an existing manifest)
  2. Submitting requests via Torii and/or the iroha CLI with structured receipt logging
  3. Writing a release summary alongside the manifest, NDJSON, submission log, and source CSV

Options:
  --csv PATH                 Source CSV with registrations (omit if --manifest is supplied)
  --manifest PATH            Existing manifest to reuse (skips CSV build)
  --release-dir PATH         Directory root for releases (default: artifacts/sns/releases)
  --release-name NAME        Release identifier appended to the root (default: UTC timestamp)
  --metrics PATH             Custom path for Prometheus metrics output (default: <release-dir>/metrics.prom)
  --torii-url URL            Torii base URL for submissions
  --token-env VAR            Env var exposing the Torii bearer token (default: SNS_TORII_TOKEN)
  --token-file PATH          File containing the Torii bearer token (overrides token-env)
  --suffix-map PATH          JSON mapping suffix ids to suffix labels for polling
  --poll-status              Enable Torii status polling after each submission
  --poll-attempts N          Poll attempts (default: 5)
  --poll-interval SECS       Poll interval seconds (default: 2)
  --cli-path PATH            Path to the iroha CLI binary (enables CLI submissions)
  --cli-config PATH          iroha CLI config path
  --cli-extra-arg ARG        Extra argument forwarded to the CLI (repeatable)
  --require-governance       Fail CSV rows that omit a governance column
  --builder-arg ARG          Additional argument forwarded to sns_bulk_onboard.py build step (repeatable)
  --submission-log PATH      Custom path for submission receipts (default: <release-dir>/submissions.log)
  --summary PATH             Custom path for release summary JSON (default: <release-dir>/summary.json)
  -h, --help                 Show this help message

Environment defaults:
  SNS_TORII_URL, SNS_TORII_TOKEN, SNS_RELEASE_DIR
EOF
}

CSV_PATH=""
MANIFEST_PATH=""
RELEASE_ROOT="${SNS_RELEASE_DIR:-artifacts/sns/releases}"
RELEASE_NAME="${SNS_RELEASE_NAME:-}"
METRICS_PATH=""
TORII_URL="${SNS_TORII_URL:-}"
TOKEN_ENV="SNS_TORII_TOKEN"
TOKEN_FILE=""
SUFFIX_MAP=""
POLL_STATUS=false
POLL_ATTEMPTS=5
POLL_INTERVAL=2
CLI_PATH=""
CLI_CONFIG=""
SUBMISSION_LOG=""
SUMMARY_PATH=""
REQUIRE_GOV=false
BUILDER_ARGS=()
CLI_EXTRA_ARGS=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --csv) CSV_PATH="$2"; shift 2 ;;
    --manifest) MANIFEST_PATH="$2"; shift 2 ;;
    --release-dir) RELEASE_ROOT="$2"; shift 2 ;;
    --release-name) RELEASE_NAME="$2"; shift 2 ;;
    --metrics) METRICS_PATH="$2"; shift 2 ;;
    --torii-url) TORII_URL="$2"; shift 2 ;;
    --token-env) TOKEN_ENV="$2"; shift 2 ;;
    --token-file) TOKEN_FILE="$2"; shift 2 ;;
    --suffix-map) SUFFIX_MAP="$2"; shift 2 ;;
    --poll-status) POLL_STATUS=true; shift ;;
    --poll-attempts) POLL_ATTEMPTS="$2"; shift 2 ;;
    --poll-interval) POLL_INTERVAL="$2"; shift 2 ;;
    --cli-path) CLI_PATH="$2"; shift 2 ;;
    --cli-config) CLI_CONFIG="$2"; shift 2 ;;
    --cli-extra-arg) CLI_EXTRA_ARGS+=("$2"); shift 2 ;;
    --submission-log) SUBMISSION_LOG="$2"; shift 2 ;;
    --summary) SUMMARY_PATH="$2"; shift 2 ;;
    --require-governance) REQUIRE_GOV=true; shift ;;
    --builder-arg) BUILDER_ARGS+=("$2"); shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *)
      err "unknown argument: $1"
      ;;
  esac
done

if [[ -z "${RELEASE_NAME}" ]]; then
  RELEASE_NAME="$(date -u +%Y%m%dT%H%M%SZ)"
fi
release_dir="${RELEASE_ROOT%/}/${RELEASE_NAME}"
mkdir -p "${release_dir}"

manifest_out="${release_dir}/registrations.manifest.json"
ndjson_out="${release_dir}/registrations.ndjson"
submission_log="${SUBMISSION_LOG:-${release_dir}/submissions.log}"
summary_out="${SUMMARY_PATH:-${release_dir}/summary.json}"
metrics_out="${METRICS_PATH:-${release_dir}/metrics.prom}"
csv_copy=""

if [[ -n "${MANIFEST_PATH}" ]]; then
  cp "${MANIFEST_PATH}" "${manifest_out}"
else
  if [[ -z "${CSV_PATH}" ]]; then
    err "either --csv or --manifest must be provided"
  fi
  python_args=("${CSV_PATH}" "--output" "${manifest_out}" "--ndjson" "${ndjson_out}")
  if [[ "${REQUIRE_GOV}" == "true" ]]; then
    python_args+=("--require-governance")
  fi
  if [[ "${#BUILDER_ARGS[@]}" -gt 0 ]]; then
    python_args+=("${BUILDER_ARGS[@]}")
  fi
  log "Building manifest via sns_bulk_onboard.py"
  (cd "${REPO_ROOT}" && "${PYTHON}" scripts/sns_bulk_onboard.py "${python_args[@]}")
  csv_copy="${release_dir}/registrations.csv"
  cp "${CSV_PATH}" "${csv_copy}"
fi

if [[ ! -f "${ndjson_out}" ]]; then
  # Manifest was reused, regenerate NDJSON for completeness.
  (cd "${REPO_ROOT}" && "${PYTHON}" scripts/sns_bulk_onboard.py --manifest "${manifest_out}" --ndjson "${ndjson_out}")
fi

submit_args=( "--manifest" "${manifest_out}" "--submission-log" "${submission_log}" )
if [[ "${POLL_STATUS}" == "true" ]]; then
  submit_args+=("--poll-status" "--poll-attempts" "${POLL_ATTEMPTS}" "--poll-interval" "${POLL_INTERVAL}")
fi
if [[ -n "${SUFFIX_MAP}" ]]; then
  submit_args+=("--suffix-map" "${SUFFIX_MAP}")
fi

torii_used=false
cli_used=false
token_value=""

if [[ -n "${TORII_URL}" ]]; then
  if [[ -n "${TOKEN_FILE}" ]]; then
    token_value="$(<"${TOKEN_FILE}")"
  elif [[ -n "${TOKEN_ENV}" && -n "${!TOKEN_ENV:-}" ]]; then
    token_value="${!TOKEN_ENV}"
  fi
  if [[ -z "${token_value}" ]]; then
    err "Torii submissions requested but no token provided (use --token-file or --token-env)"
  fi
  submit_args+=(
    "--submit-torii-url" "${TORII_URL}"
    "--submit-token" "${token_value}"
    "--submit-timeout" "45"
  )
  torii_used=true
fi

if [[ -n "${CLI_PATH}" ]]; then
  submit_args+=("--submit-cli-path" "${CLI_PATH}")
  if [[ -n "${CLI_CONFIG}" ]]; then
    submit_args+=("--submit-cli-config" "${CLI_CONFIG}")
  fi
  for extra in "${CLI_EXTRA_ARGS[@]}"; do
    submit_args+=("--submit-cli-extra-arg" "${extra}")
  done
  cli_used=true
fi

if [[ "${torii_used}" == "true" || "${cli_used}" == "true" ]]; then
  log "Submitting manifest (${manifest_out})"
  (cd "${REPO_ROOT}" && "${PYTHON}" scripts/sns_bulk_onboard.py "${submit_args[@]}")
else
  log "No submission targets specified; skipping submissions"
fi

if [[ ! -f "${submission_log}" ]]; then
  : > "${submission_log}"
fi

summary_json=$(cat <<EOF
{
  "generated_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "source_csv": "${csv_copy}",
  "manifest": "${manifest_out}",
  "ndjson": "${ndjson_out}",
  "submission_log": "${submission_log}",
  "release_name": "${RELEASE_NAME}",
  "release_root": "${RELEASE_ROOT}",
  "metrics": "${metrics_out}",
  "torii_url": "${torii_used:+${TORII_URL}}",
  "cli_path": "${cli_used:+${CLI_PATH}}"
}
EOF
)
printf '%s\n' "${summary_json}" > "${summary_out}"

log "Generating Prometheus metrics (${metrics_out})"
(cd "${REPO_ROOT}" && "${PYTHON}" scripts/sns_bulk_metrics.py \
  --manifest "${manifest_out}" \
  --submission-log "${submission_log}" \
  --release "${RELEASE_NAME}" \
  --output "${metrics_out}")

log "SNS release artifacts written to ${release_dir}"
