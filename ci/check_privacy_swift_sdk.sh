#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${PRIVACY_SWIFT_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)}"
SWIFTC_BIN="${PRIVACY_SWIFT_SDK_SWIFTC_BIN:-swiftc}"
SWIFT_BIN="${PRIVACY_SWIFT_SDK_SWIFT_BIN:-swift}"
FROZEN_CARGO_LOCK_SHA256="cd9e829e454171f17540abeb7fd1aa14129252082bd8b076a0199b0ffa4e3f79"
TRACKED_ROOT_CARGO_LOCK_SHA256="d5b8bf5efbdc3ce2a8b1c0d2d75e1c5d1a343a072f836cfb76205bc6ea4cf15f"
PYTHON_BIN="${MOBILE_SDK_PYTHON_BINARY:-${PRIVACY_SWIFT_SDK_PYTHON_BIN:-}}"
APPLE_ARTIFACT_CHECKER="${ROOT_DIR}/scripts/check_mobile_sdk_artifacts.sh"

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
