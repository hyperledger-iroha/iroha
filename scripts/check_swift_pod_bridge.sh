#!/usr/bin/env bash
# Validate that the CocoaPods spec builds with the bundled Norito bridge.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PODSPEC_PATH="${REPO_ROOT}/IrohaSwift/IrohaSwift.podspec"
ARTIFACT_CHECKER="${REPO_ROOT}/scripts/check_mobile_sdk_artifacts.sh"
APPLE_ARTIFACT_DIR="${MOBILE_SDK_APPLE_ARTIFACT_DIR:-${REPO_ROOT}/dist}"
if [[ "${APPLE_ARTIFACT_DIR}" != /* ]]; then
  echo "[swift-pod-bridge] error: MOBILE_SDK_APPLE_ARTIFACT_DIR must be absolute" >&2
  exit 1
fi
BRIDGE_PATH="${APPLE_ARTIFACT_DIR}/NoritoBridge.xcframework"
ARTIFACTS_PATH="${APPLE_ARTIFACT_DIR}/NoritoBridge.artifacts.json"
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
  echo "[swift-pod-bridge] error: cocoapods (pod) is required; refusing to skip lint" >&2
  write_summary "failed" "cocoapods CLI not available"
  exit 1
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

if [[ ! -f "${ARTIFACTS_PATH}" ]]; then
  write_summary "failed" "missing NoritoBridge artifact manifest under ${ARTIFACTS_PATH}"
  echo "[swift-pod-bridge] error: missing ${ARTIFACTS_PATH}" >&2
  exit 1
fi

if [[ ! -x "${ARTIFACT_CHECKER}" ]]; then
  write_summary "failed" "missing Apple artifact checker at ${ARTIFACT_CHECKER}"
  echo "[swift-pod-bridge] error: missing ${ARTIFACT_CHECKER}" >&2
  exit 1
fi

if ! MOBILE_SDK_APPLE_ARTIFACT_DIR="${APPLE_ARTIFACT_DIR}" \
  bash "${ARTIFACT_CHECKER}" --root "${REPO_ROOT}" --apple-only; then
  write_summary "failed" "NoritoBridge artifact authentication failed"
  echo "[swift-pod-bridge] error: refusing to lint against an unauthenticated NoritoBridge artifact" >&2
  exit 1
fi

mkdir -p "${REPORT_DIR}"
touch "${LOG_PATH}"

export COCOAPODS_DISABLE_STATS=1
export COCOAPODS_NO_REPO_UPDATE=1

LINT_ARGS=(
  "lib" "lint" "${PODSPEC_PATH}"
  "--fail-fast"
  "--configuration=Release"
  "--private"
  "--use-libraries"
  "--platforms=ios"
  "--no-clean"
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
