#!/usr/bin/env bash
# CI wrapper that validates the SwiftPM manifest with and without the Norito bridge.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHECK_SCRIPT="${REPO_ROOT}/scripts/check_swift_spm_validation.sh"
REPORT_DIR="${SWIFT_SPM_REPORT_DIR:-${REPO_ROOT}/artifacts/swift_spm_validation}"

if [[ ! -x "${CHECK_SCRIPT}" ]]; then
  echo "error: missing helper ${CHECK_SCRIPT}" >&2
  exit 1
fi

mkdir -p "${REPORT_DIR}"

export SWIFT_SPM_REPORT_DIR="${REPORT_DIR}"
export SWIFT_SPM_SUMMARY="${SWIFT_SPM_SUMMARY:-${REPORT_DIR}/summary.json}"
export SWIFT_SPM_WITH_BRIDGE_LOG="${SWIFT_SPM_WITH_BRIDGE_LOG:-${REPORT_DIR}/with_bridge.log}"
export SWIFT_SPM_MISSING_BRIDGE_LOG="${SWIFT_SPM_MISSING_BRIDGE_LOG:-${REPORT_DIR}/missing_bridge_required.log}"
export SWIFT_SPM_OPTIONAL_BRIDGE_LOG="${SWIFT_SPM_OPTIONAL_BRIDGE_LOG:-${REPORT_DIR}/missing_bridge_optional.log}"

"${CHECK_SCRIPT}"

if [[ -f "${SWIFT_SPM_SUMMARY}" ]]; then
  echo "[swift-spm] summary written to ${SWIFT_SPM_SUMMARY}"
fi
