#!/usr/bin/env bash
# Validate SwiftPM manifest behaviour with and without the Norito bridge present.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PACKAGE_DIR="${REPO_ROOT}/IrohaSwift"
BRIDGE_DIR="${REPO_ROOT}/dist/NoritoBridge.xcframework"
REPORT_DIR="${SWIFT_SPM_REPORT_DIR:-${REPO_ROOT}/artifacts/swift_spm_validation}"
SUMMARY_PATH="${SWIFT_SPM_SUMMARY:-${REPORT_DIR}/summary.json}"
WITH_BRIDGE_LOG="${SWIFT_SPM_WITH_BRIDGE_LOG:-${REPORT_DIR}/with_bridge.log}"
MISSING_BRIDGE_LOG="${SWIFT_SPM_MISSING_BRIDGE_LOG:-${REPORT_DIR}/missing_bridge_fallback.log}"
MODULE_CACHE="${SWIFT_SPM_MODULE_CACHE:-${REPORT_DIR}/modulecache}"
WITH_BRIDGE_SCRATCH="${SWIFT_SPM_WITH_BRIDGE_SCRATCH:-${REPORT_DIR}/scratch_with_bridge}"
MISSING_BRIDGE_SCRATCH="${SWIFT_SPM_MISSING_BRIDGE_SCRATCH:-${REPORT_DIR}/scratch_missing_bridge}"

write_summary() {
  local status="$1"
  local with_rc="$2"
  local missing_rc="$3"
  local fallback_warning="$4"
  mkdir -p "$(dirname "${SUMMARY_PATH}")"
  cat >"${SUMMARY_PATH}" <<EOF
{"status":"${status}","with_bridge":{"rc":${with_rc},"log":"${WITH_BRIDGE_LOG}"},"missing_bridge_fallback":{"rc":${missing_rc},"expected":"pass","warning_present":${fallback_warning},"log":"${MISSING_BRIDGE_LOG}"},"bridge_present":$( [[ -d "${BRIDGE_DIR}" ]] && echo true || echo false ),"package_dir":"${PACKAGE_DIR}"}
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
  write_summary "failed" 127 127 false
  exit 127
fi

if [[ ! -d "${PACKAGE_DIR}" ]]; then
  echo "[swift-spm] error: missing package dir ${PACKAGE_DIR}" >&2
  write_summary "failed" 66 66 false
  exit 66
fi

mkdir -p "${REPORT_DIR}"
touch "${WITH_BRIDGE_LOG}" "${MISSING_BRIDGE_LOG}"
mkdir -p "${MODULE_CACHE}"
rm -rf "${WITH_BRIDGE_SCRATCH}" "${MISSING_BRIDGE_SCRATCH}"

export SWIFT_MODULE_CACHE_PATH="${MODULE_CACHE}"
export CLANG_MODULE_CACHE_PATH="${MODULE_CACHE}"

if [[ ! -d "${BRIDGE_DIR}" ]]; then
  echo "[swift-spm] error: expected bridge at ${BRIDGE_DIR}" >&2
  write_summary "failed" 65 65 false
  exit 65
fi

echo "[swift-spm] building with bridge present"
set +e
swift build --package-path "${PACKAGE_DIR}" --configuration debug --manifest-cache none --scratch-path "${WITH_BRIDGE_SCRATCH}" 2>&1 | tee "${WITH_BRIDGE_LOG}"
WITH_RC=${PIPESTATUS[0]}
set -e

BRIDGE_STASH="${BRIDGE_DIR}.spmcheck.$RANDOM.$$"
mv "${BRIDGE_DIR}" "${BRIDGE_STASH}"

echo "[swift-spm] building with bridge missing (expect Swift-only fallback warning)"
set +e
swift build --package-path "${PACKAGE_DIR}" --configuration debug --manifest-cache none --scratch-path "${MISSING_BRIDGE_SCRATCH}" 2>&1 | tee "${MISSING_BRIDGE_LOG}"
MISSING_RC=${PIPESTATUS[0]}
set -e

FALLBACK_WARNING=false
if grep -q "continuing with Swift-only fallback" "${MISSING_BRIDGE_LOG}"; then
  FALLBACK_WARNING=true
fi

restore_bridge

OVERALL_STATUS="passed"
if [[ ${WITH_RC} -ne 0 ]]; then
  OVERALL_STATUS="failed"
fi
if [[ ${MISSING_RC} -ne 0 ]]; then
  OVERALL_STATUS="failed"
fi
if [[ "${FALLBACK_WARNING}" != "true" ]]; then
  OVERALL_STATUS="failed"
fi

write_summary "${OVERALL_STATUS}" "${WITH_RC}" "${MISSING_RC}" "${FALLBACK_WARNING}"

if [[ "${OVERALL_STATUS}" != "passed" ]]; then
  echo "[swift-spm] failure detected (see ${SUMMARY_PATH})" >&2
  exit 1
fi

echo "[swift-spm] validation succeeded (summary: ${SUMMARY_PATH})"
