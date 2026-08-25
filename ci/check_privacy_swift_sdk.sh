#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${PRIVACY_SWIFT_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)}"
SWIFTC_BIN="${PRIVACY_SWIFT_SDK_SWIFTC_BIN:-swiftc}"
SWIFT_BIN="${PRIVACY_SWIFT_SDK_SWIFT_BIN:-swift}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
FROZEN_CARGO_LOCK_SHA256="ccf4acebfe63ad981193b87afd559c195d8a67642d9536b8082f77bbf24a11f0"
TRACKED_ROOT_CARGO_LOCK_SHA256="ad0d209abaa51d4c77a9e67ccbb0c7660a0f8b7b5dbe3e3fbe4a70e142711bf7"
PYTHON_BIN="${MOBILE_SDK_PYTHON_BINARY:-${PRIVACY_SWIFT_SDK_PYTHON_BIN:-}}"
APPLE_ARTIFACT_CHECKER="${ROOT_DIR}/scripts/check_mobile_sdk_artifacts.sh"

# shellcheck source=ci/privacy_sdk_cargo_lockfile.sh
source "${SCRIPT_DIR}/privacy_sdk_cargo_lockfile.sh"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "error: privacy Swift native tests require an Apple macOS host" >&2
  exit 1
fi
if [[ "${MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT:-}" != "1" ]]; then
  echo "error: MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT=1 is required" >&2
  exit 1
fi
if [[ -z "${MOBILE_SDK_APPLE_ARTIFACT_DIR:-}" || \
  -z "${MOBILE_SDK_SWIFT_SCRATCH_DIR:-}" || \
  -z "${MOBILE_SDK_PYTHON_BINARY:-}" || \
  -z "${PYTHON_BIN}" ]]; then
  echo "error: authenticated external Apple artifact, Swift scratch, and Python paths are required" >&2
  exit 1
fi
if ! APPLE_ARTIFACT_DIRECTORY="$(cd "${MOBILE_SDK_APPLE_ARTIFACT_DIR}" && pwd -P)"; then
  echo "error: privacy Swift Apple artifact directory is unavailable" >&2
  exit 1
fi
if ! SWIFT_SCRATCH_DIRECTORY="$(cd "${MOBILE_SDK_SWIFT_SCRATCH_DIR}" && pwd -P)"; then
  echo "error: privacy Swift scratch directory is unavailable" >&2
  exit 1
fi
if [[ "${MOBILE_SDK_APPLE_ARTIFACT_DIR}" != "${APPLE_ARTIFACT_DIRECTORY}" || \
  "${MOBILE_SDK_SWIFT_SCRATCH_DIR}" != "${SWIFT_SCRATCH_DIRECTORY}" ]]; then
  echo "error: privacy Swift artifact and scratch paths must already be canonical" >&2
  exit 1
fi
case "${APPLE_ARTIFACT_DIRECTORY}/" in
  "${ROOT_DIR}/"*)
    echo "error: privacy Swift Apple artifact directory must be outside the source tree" >&2
    exit 1
    ;;
esac
case "${SWIFT_SCRATCH_DIRECTORY}/" in
  "${ROOT_DIR}/"*)
    echo "error: privacy Swift scratch output must remain outside the source tree" >&2
    exit 1
    ;;
esac
[[ -d "${APPLE_ARTIFACT_DIRECTORY}/NoritoBridge.xcframework" && \
  ! -L "${APPLE_ARTIFACT_DIRECTORY}/NoritoBridge.xcframework" ]] || {
  echo "error: freshly built external NoritoBridge.xcframework is unavailable" >&2
  exit 1
}
[[ -f "${ROOT_DIR}/Cargo.lock" && ! -L "${ROOT_DIR}/Cargo.lock" ]] || {
  echo "error: privacy Swift gate requires the tracked root Cargo.lock" >&2
  exit 1
}
[[ "$("${PYTHON_BIN}" -I -S -c 'import hashlib,pathlib,sys; print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())' "${ROOT_DIR}/Cargo.lock")" == \
  "${TRACKED_ROOT_CARGO_LOCK_SHA256}" ]] || {
  echo "error: privacy Swift tracked root Cargo.lock authority changed" >&2
  exit 1
}
PRIVACY_RELEASE_CARGO_LOCK="${IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH:-}"
[[ -f "${PRIVACY_RELEASE_CARGO_LOCK}" && ! -L "${PRIVACY_RELEASE_CARGO_LOCK}" && \
  "${PRIVACY_RELEASE_CARGO_LOCK}" != "${ROOT_DIR}/Cargo.lock" ]] || {
  echo "error: privacy Swift gate requires a distinct external privacy release Cargo.lock" >&2
  exit 1
}
[[ "$("${PYTHON_BIN}" -I -S -c 'import hashlib,pathlib,sys; print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())' "${PRIVACY_RELEASE_CARGO_LOCK}")" == \
  "${FROZEN_CARGO_LOCK_SHA256}" ]] || {
  echo "error: privacy Swift external Cargo.lock is not the frozen release lock" >&2
  exit 1
}
WORKSPACE_CARGO_LOCK_STATE="$(
  privacy_sdk_capture_optional_file_state \
    "${ROOT_DIR}/Cargo.lock" \
    "privacy Swift tracked root Cargo.lock" \
    "${PYTHON_BIN}"
)"
PRIVACY_RELEASE_CARGO_LOCK_SEAL="$(
  privacy_sdk_file_seal "${PRIVACY_RELEASE_CARGO_LOCK}" "${PYTHON_BIN}"
)" || {
  echo "error: privacy Swift external Cargo.lock cannot be identity-sealed" >&2
  exit 1
}

assert_privacy_swift_lock_state() {
  local status=0
  privacy_sdk_assert_optional_file_state \
    "${ROOT_DIR}/Cargo.lock" \
    "${WORKSPACE_CARGO_LOCK_STATE}" \
    "privacy Swift tracked root Cargo.lock" \
    "${PYTHON_BIN}" || status=1
  privacy_sdk_assert_file_seal \
    "${PRIVACY_RELEASE_CARGO_LOCK}" \
    "${PRIVACY_RELEASE_CARGO_LOCK_SEAL}" \
    "privacy Swift external Cargo.lock" \
    "${PYTHON_BIN}" || status=1
  return "${status}"
}

cleanup_privacy_swift_lock_state() {
  local status=$?
  trap - EXIT HUP INT TERM
  if ! assert_privacy_swift_lock_state; then
    status=1
  fi
  exit "${status}"
}
trap cleanup_privacy_swift_lock_state EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

DEVELOPER_DIR="$(xcode-select -p)"
[[ "${DEVELOPER_DIR}" == */Xcode*.app/Contents/Developer ]] || {
  echo "error: privacy Swift native execution requires full Xcode" >&2
  exit 1
}
xcodebuild -version

cd "${ROOT_DIR}"

MOBILE_SDK_APPLE_ARTIFACT_DIR="${APPLE_ARTIFACT_DIRECTORY}" \
MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT=1 \
MOBILE_SDK_PYTHON_BINARY="${PYTHON_BIN}" \
IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH="${PRIVACY_RELEASE_CARGO_LOCK}" \
  bash "${APPLE_ARTIFACT_CHECKER}" --apple-only

"${SWIFTC_BIN}" --version
"${SWIFTC_BIN}" -parse -parse-as-library \
  IrohaSwift/Sources/IrohaSwift/NativeBridge.swift \
  IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift \
  IrohaSwift/Sources/IrohaSwift/PrivacyExact12CapabilityManifestV1.swift \
  IrohaSwift/Sources/IrohaSwift/PrivacyExact12FixtureBundle.swift \
  IrohaSwift/Sources/IrohaSwift/ProofAttachment.swift \
  IrohaSwift/Sources/IrohaSwift/MusubiInstructionsV1.swift \
  IrohaSwift/Sources/IrohaSwift/ToriiClient.swift \
  IrohaSwift/Sources/IrohaSwift/TransactionEncoder.swift \
  IrohaSwift/Sources/IrohaSwift/TxBuilder.swift \
  IrohaSwift/Sources/IrohaSwift/VerifyingKeyBackendTag.swift \
  IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/PrivacyExact12CapabilityManifestV1Tests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/PrivacyExact12FixtureBundleTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ProofAttachmentNoritoTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/MusubiInstructionsV1Tests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/TxBuilderTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/VerifyingKeyBackendTagTests.swift

"${SWIFT_BIN}" test \
  --package-path IrohaSwift \
  --disable-automatic-resolution \
  --scratch-path "${SWIFT_SCRATCH_DIRECTORY}"

assert_privacy_swift_lock_state
