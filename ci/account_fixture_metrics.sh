#!/usr/bin/env bash
# Emit Prometheus metrics for ADDR-2 fixture checks using account_fixture_helper.py.
# Usage:
#   ci/account_fixture_metrics.sh \
#     --out artifacts/account_fixture/address_fixture.prom \
#     --target canonical=fixtures/account/address_vectors.json
# Targets accept the form label=path or label=path::source where `source`
# overrides the remote fixture URL passed to account_fixture_helper.py.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEFAULT_OUT="${REPO_ROOT}/artifacts/account_fixture/address_fixture.prom"
OUTPUT_PATH="${DEFAULT_OUT}"
SOURCE_OVERRIDE=""
declare -a TARGET_SPECS=()

usage() {
  cat <<'EOF'
Usage: ci/account_fixture_metrics.sh [options]

Options:
  --out <path>        Destination for the Prometheus textfile output
                      (default: artifacts/account_fixture/address_fixture.prom)
  --target <spec>     Fixture to check, formatted as label=path or
                      label=path::source. Repeat for multiple surfaces.
  --source <url>      Override the remote fixture URL for all targets.
  --help              Show this message.

Examples:
  ci/account_fixture_metrics.sh \
    --target canonical=fixtures/account/address_vectors.json

  ci/account_fixture_metrics.sh \
    --out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
    --target android=java/iroha_android/address_vectors.json::https://example.com/address_vectors.json
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --out)
      OUTPUT_PATH="$2"
      shift 2
      ;;
    --target)
      TARGET_SPECS+=("$2")
      shift 2
      ;;
    --source)
      SOURCE_OVERRIDE="$2"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "[account-fixture-metrics] unknown argument: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if [[ ${#TARGET_SPECS[@]} -eq 0 ]]; then
  TARGET_SPECS=("canonical=fixtures/account/address_vectors.json")
fi

tmp_output="$(mktemp "${TMPDIR:-/tmp}/account-fixture-metrics.XXXXXX")"
truncate -s 0 "${tmp_output}"

for spec in "${TARGET_SPECS[@]}"; do
  if [[ "${spec}" != *=* ]]; then
    echo "[account-fixture-metrics] invalid target spec (missing '='): ${spec}" >&2
    exit 1
  fi
  label="${spec%%=*}"
  rest="${spec#*=}"
  if [[ -z "${label}" || -z "${rest}" ]]; then
    echo "[account-fixture-metrics] invalid target spec: ${spec}" >&2
    exit 1
  fi
  target_path="${rest}"
  source_arg=()
  if [[ "${rest}" == *"::"* ]]; then
    target_path="${rest%%::*}"
    source_override_local="${rest#*::}"
    if [[ -n "${source_override_local}" ]]; then
      source_arg=(--source "${source_override_local}")
    fi
  elif [[ -n "${SOURCE_OVERRIDE}" ]]; then
    source_arg=(--source "${SOURCE_OVERRIDE}")
  fi
  if [[ "${target_path}" != /* ]]; then
    target_path="${REPO_ROOT}/${target_path}"
  fi
  tmp_target="$(mktemp "${TMPDIR:-/tmp}/account-fixture-metrics-target.XXXXXX")"
  cmd=(python3 "${REPO_ROOT}/scripts/account_fixture_helper.py" check)
  if [[ ${#source_arg[@]} -gt 0 ]]; then
    cmd+=("${source_arg[@]}")
  fi
  cmd+=(
    --target "${target_path}"
    --quiet
    --metrics-out "${tmp_target}"
    --metrics-label "${label}"
  )
  "${cmd[@]}"
  cat "${tmp_target}" >> "${tmp_output}"
  rm -f "${tmp_target}"
done

mkdir -p "$(dirname "${OUTPUT_PATH}")"
mv "${tmp_output}" "${OUTPUT_PATH}"
echo "[account-fixture-metrics] wrote metrics to ${OUTPUT_PATH}"
