#!/usr/bin/env bash
# Validate that the CocoaPods spec builds with the bundled Norito bridge.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PODSPEC_PATH="${REPO_ROOT}/IrohaSwift/IrohaSwift.podspec"
BRIDGE_PATH="${REPO_ROOT}/dist/NoritoBridge.xcframework"
ARTIFACTS_PATH="${REPO_ROOT}/dist/NoritoBridge.artifacts.json"
REPORT_DIR="${SWIFT_POD_REPORT_DIR:-${REPO_ROOT}/artifacts/swift_pod_bridge}"
SUMMARY_PATH="${SWIFT_POD_SUMMARY:-${REPORT_DIR}/summary.json}"
LOG_PATH="${SWIFT_POD_LOG:-${REPORT_DIR}/pod_lint.log}"

write_summary() {
  local status="$1"
  local reason="$2"
  mkdir -p "$(dirname "${SUMMARY_PATH}")"
  cat >"${SUMMARY_PATH}" <<EOF
{"status":"${status}","reason":"${reason}","podspec":"${PODSPEC_PATH}","bridge_present":$( [[ -d "${BRIDGE_PATH}" ]] && echo true || echo false ),"artifact_manifest_present":$( [[ -f "${ARTIFACTS_PATH}" ]] && echo true || echo false ),"log_path":"${LOG_PATH}"}
EOF
}

if ! command -v pod >/dev/null 2>&1; then
  echo "[swift-pod-bridge] warning: cocoapods (pod) not installed; skipping lint"
  write_summary "skipped" "cocoapods CLI not available"
  exit 0
fi

if [[ ! -f "${PODSPEC_PATH}" ]]; then
  write_summary "failed" "missing podspec at ${PODSPEC_PATH}"
  echo "[swift-pod-bridge] error: missing podspec at ${PODSPEC_PATH}" >&2
  exit 1
fi

if [[ ! -d "${BRIDGE_PATH}" ]]; then
  write_summary "failed" "missing NoritoBridge.xcframework under ${BRIDGE_PATH}"
  echo "[swift-pod-bridge] error: missing ${BRIDGE_PATH}" >&2
  exit 1
fi

mkdir -p "${REPORT_DIR}"
touch "${LOG_PATH}"

export COCOAPODS_DISABLE_STATS=1
export COCOAPODS_NO_REPO_UPDATE=1

LINT_ARGS=(
  "lib" "lint" "${PODSPEC_PATH}"
  "--fail-fast"
  "--allow-warnings"
  "--skip-tests"
  "--configuration=Debug"
  "--private"
  "--use-libraries"
  "--platforms=ios,osx"
  "--no-clean"
  "--no-repo-update"
  "--verbose"
)

set +e
pod "${LINT_ARGS[@]}" 2>&1 | tee "${LOG_PATH}"
rc=${PIPESTATUS[0]}
set -e

if [[ ${rc} -ne 0 ]]; then
  write_summary "failed" "pod lib lint failed (see ${LOG_PATH})"
  exit ${rc}
fi

write_summary "passed" "pod lib lint succeeded"
echo "[swift-pod-bridge] pod lib lint succeeded (summary: ${SUMMARY_PATH})"
