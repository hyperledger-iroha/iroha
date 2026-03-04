#!/usr/bin/env bash
# CI wrapper that lints the CocoaPods spec with the bundled Norito bridge.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHECK_SCRIPT="${REPO_ROOT}/scripts/check_swift_pod_bridge.sh"
REPORT_DIR="${SWIFT_POD_REPORT_DIR:-${REPO_ROOT}/artifacts/swift_pod_bridge}"

if [[ ! -x "${CHECK_SCRIPT}" ]]; then
  echo "error: missing helper ${CHECK_SCRIPT}" >&2
  exit 1
fi

mkdir -p "${REPORT_DIR}"

export SWIFT_POD_REPORT_DIR="${REPORT_DIR}"
export SWIFT_POD_SUMMARY="${SWIFT_POD_SUMMARY:-${REPORT_DIR}/summary.json}"
export SWIFT_POD_LOG="${SWIFT_POD_LOG:-${REPORT_DIR}/pod_lint.log}"

"${CHECK_SCRIPT}"

if [[ -f "${SWIFT_POD_SUMMARY}" ]]; then
  echo "[swift-pod-bridge] summary written to ${SWIFT_POD_SUMMARY}"
fi
