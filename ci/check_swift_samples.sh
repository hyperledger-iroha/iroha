#!/usr/bin/env bash
# CI wrapper that runs the Swift sample smoke tests for IOS5.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHECK_SCRIPT="${REPO_ROOT}/scripts/check_swift_samples.sh"
REPORT_DIR="${SWIFT_SAMPLES_REPORT_DIR:-${REPO_ROOT}/artifacts/swift_samples}"

if [[ ! -x "${CHECK_SCRIPT}" ]]; then
  echo "error: missing helper ${CHECK_SCRIPT}" >&2
  exit 1
fi

mkdir -p "${REPORT_DIR}"

if [[ -z "${SWIFT_SAMPLES_DERIVED_DATA+x}" ]]; then
  export SWIFT_SAMPLES_DERIVED_DATA="${REPORT_DIR}"
fi
if [[ -z "${SWIFT_SAMPLES_SUMMARY+x}" ]]; then
  export SWIFT_SAMPLES_SUMMARY="${REPORT_DIR}/summary.json"
fi
if [[ -z "${SWIFT_SAMPLES_JUNIT+x}" ]]; then
  export SWIFT_SAMPLES_JUNIT="${REPORT_DIR}/swift_samples.junit.xml"
fi
SWIFT_SAMPLES_METRICS_OUT="${SWIFT_SAMPLES_METRICS_OUT:-}"
SWIFT_SAMPLES_METRICS_CLUSTER="${SWIFT_SAMPLES_METRICS_CLUSTER:-buildkite}"

"${CHECK_SCRIPT}"

summary_path="${SWIFT_SAMPLES_SUMMARY}"
junit_path="${SWIFT_SAMPLES_JUNIT}"

if [[ -n "${summary_path}" && -f "${summary_path}" ]]; then
  echo "[swift-samples] summary written to ${summary_path}"
  if [[ -n "${SWIFT_SAMPLES_METRICS_OUT}" ]]; then
    python3 "${REPO_ROOT}/scripts/swift_samples_metrics.py" \
      --summary "${summary_path}" \
      --output "${SWIFT_SAMPLES_METRICS_OUT}" \
      --cluster "${SWIFT_SAMPLES_METRICS_CLUSTER}"
    echo "[swift-samples] metrics written to ${SWIFT_SAMPLES_METRICS_OUT}"
  fi
fi
if [[ -n "${junit_path}" && -f "${junit_path}" ]]; then
  echo "[swift-samples] junit written to ${junit_path}"
fi
