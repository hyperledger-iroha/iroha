#!/usr/bin/env bash
# Validate SwiftPM manifest behaviour with and without the Norito bridge present.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PACKAGE_DIR="${REPO_ROOT}/IrohaSwift"
BRIDGE_DIR="${REPO_ROOT}/dist/NoritoBridge.xcframework"
REPORT_DIR="${SWIFT_SPM_REPORT_DIR:-${REPO_ROOT}/artifacts/swift_spm_validation}"
SUMMARY_PATH="${SWIFT_SPM_SUMMARY:-${REPORT_DIR}/summary.json}"
WITH_BRIDGE_LOG="${SWIFT_SPM_WITH_BRIDGE_LOG:-${REPORT_DIR}/with_bridge.log}"
MISSING_BRIDGE_LOG="${SWIFT_SPM_MISSING_BRIDGE_LOG:-${REPORT_DIR}/missing_bridge_required.log}"
OPTIONAL_BRIDGE_LOG="${SWIFT_SPM_OPTIONAL_BRIDGE_LOG:-${REPORT_DIR}/missing_bridge_optional.log}"
MODULE_CACHE="${SWIFT_SPM_MODULE_CACHE:-${REPORT_DIR}/modulecache}"

write_summary() {
  local status="$1"
  local with_rc="$2"
  local missing_rc="$3"
  local optional_rc="$4"
  mkdir -p "$(dirname "${SUMMARY_PATH}")"
  cat >"${SUMMARY_PATH}" <<EOF
{"status":"${status}","with_bridge":{"rc":${with_rc},"log":"${WITH_BRIDGE_LOG}"},"missing_bridge_required":{"rc":${missing_rc},"expected":"fail","log":"${MISSING_BRIDGE_LOG}"},"missing_bridge_optional":{"rc":${optional_rc},"expected":"pass","log":"${OPTIONAL_BRIDGE_LOG}"},"bridge_present":$( [[ -d "${BRIDGE_DIR}" ]] && echo true || echo false ),"package_dir":"${PACKAGE_DIR}"}
EOF
}

restore_bridge() {
  if [[ -n "${BRIDGE_STASH:-}" && -d "${BRIDGE_STASH}" && ! -d "${BRIDGE_DIR}" ]]; then
    mv "${BRIDGE_STASH}" "${BRIDGE_DIR}"
  fi
}
trap restore_bridge EXIT

if ! command -v swift >/dev/null 2>&1; then
  echo "[swift-spm] error: swift toolchain not available" >&2
  write_summary "failed" 127 127 127
  exit 127
fi

if [[ ! -d "${PACKAGE_DIR}" ]]; then
  echo "[swift-spm] error: missing package dir ${PACKAGE_DIR}" >&2
  write_summary "failed" 66 66 66
  exit 66
fi

mkdir -p "${REPORT_DIR}"
touch "${WITH_BRIDGE_LOG}" "${MISSING_BRIDGE_LOG}" "${OPTIONAL_BRIDGE_LOG}"
mkdir -p "${MODULE_CACHE}"

export SWIFT_MODULE_CACHE_PATH="${MODULE_CACHE}"
export CLANG_MODULE_CACHE_PATH="${MODULE_CACHE}"

if [[ ! -d "${BRIDGE_DIR}" ]]; then
  echo "[swift-spm] error: expected bridge at ${BRIDGE_DIR}" >&2
  write_summary "failed" 65 65 65
  exit 65
fi

echo "[swift-spm] building with bridge present (required mode)"
set +e
swift build --package-path "${PACKAGE_DIR}" --configuration debug 2>&1 | tee "${WITH_BRIDGE_LOG}"
WITH_RC=${PIPESTATUS[0]}
set -e

BRIDGE_STASH="${BRIDGE_DIR}.spmcheck.$RANDOM.$$"
mv "${BRIDGE_DIR}" "${BRIDGE_STASH}"

echo "[swift-spm] expecting failure when the bridge is missing (required mode)"
set +e
swift build --package-path "${PACKAGE_DIR}" --configuration debug 2>&1 | tee "${MISSING_BRIDGE_LOG}"
MISSING_RC=${PIPESTATUS[0]}
set -e

echo "[swift-spm] building without bridge in optional mode"
set +e
IROHASWIFT_USE_BRIDGE=0 swift build --package-path "${PACKAGE_DIR}" --configuration debug 2>&1 | tee "${OPTIONAL_BRIDGE_LOG}"
OPTIONAL_RC=${PIPESTATUS[0]}
set -e

restore_bridge

OVERALL_STATUS="passed"
if [[ ${WITH_RC} -ne 0 ]]; then
  OVERALL_STATUS="failed"
fi
if [[ ${MISSING_RC} -eq 0 ]]; then
  OVERALL_STATUS="failed"
fi
if [[ ${OPTIONAL_RC} -ne 0 ]]; then
  OVERALL_STATUS="failed"
fi

write_summary "${OVERALL_STATUS}" "${WITH_RC}" "${MISSING_RC}" "${OPTIONAL_RC}"

if [[ "${OVERALL_STATUS}" != "passed" ]]; then
  echo "[swift-spm] failure detected (see ${SUMMARY_PATH})" >&2
  exit 1
fi

echo "[swift-spm] validation succeeded (summary: ${SUMMARY_PATH})"
