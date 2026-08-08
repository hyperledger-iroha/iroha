#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${PRIVACY_SWIFT_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
SWIFTC_BIN="${PRIVACY_SWIFT_SDK_SWIFTC_BIN:-swiftc}"
SWIFT_BIN="${PRIVACY_SWIFT_SDK_SWIFT_BIN:-swift}"
FROZEN_CARGO_LOCK_SHA256="cd9e829e454171f17540abeb7fd1aa14129252082bd8b076a0199b0ffa4e3f79"
PYTHON_BIN="${PRIVACY_SWIFT_SDK_PYTHON_BIN:-python3}"

[[ "${MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT:-}" == "1" ]] || {
  echo "error: privacy Swift tests require an external authenticated XCFramework" >&2
  exit 1
}
[[ -n "${MOBILE_SDK_APPLE_ARTIFACT_DIR:-}" && \
  "${MOBILE_SDK_APPLE_ARTIFACT_DIR}" == /* ]] || {
  echo "error: MOBILE_SDK_APPLE_ARTIFACT_DIR must be an absolute external directory" >&2
  exit 1
}
[[ -n "${MOBILE_SDK_SWIFT_SCRATCH_DIR:-}" && \
  "${MOBILE_SDK_SWIFT_SCRATCH_DIR}" == /* ]] || {
  echo "error: MOBILE_SDK_SWIFT_SCRATCH_DIR must be an absolute external directory" >&2
  exit 1
}
case "${MOBILE_SDK_APPLE_ARTIFACT_DIR}/" in
  "${ROOT_DIR}/"*)
    echo "error: privacy Swift XCFramework must remain outside the source tree" >&2
    exit 1
    ;;
esac
case "${MOBILE_SDK_SWIFT_SCRATCH_DIR}/" in
  "${ROOT_DIR}/"*)
    echo "error: privacy Swift scratch output must remain outside the source tree" >&2
    exit 1
    ;;
esac
[[ -d "${MOBILE_SDK_APPLE_ARTIFACT_DIR}/NoritoBridge.xcframework" && \
  ! -L "${MOBILE_SDK_APPLE_ARTIFACT_DIR}/NoritoBridge.xcframework" ]] || {
  echo "error: freshly built external NoritoBridge.xcframework is unavailable" >&2
  exit 1
}
[[ -f "${ROOT_DIR}/Cargo.lock" && ! -L "${ROOT_DIR}/Cargo.lock" ]] || {
  echo "error: privacy Swift gate requires the frozen workspace Cargo.lock" >&2
  exit 1
}
[[ "$("${PYTHON_BIN}" -I -S -c 'import hashlib,pathlib,sys; print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())' "${ROOT_DIR}/Cargo.lock")" == \
  "${FROZEN_CARGO_LOCK_SHA256}" ]] || {
  echo "error: privacy Swift Cargo.lock is not the frozen release lock" >&2
  exit 1
}

DEVELOPER_DIR="$(xcode-select -p)"
[[ "${DEVELOPER_DIR}" == */Xcode*.app/Contents/Developer ]] || {
  echo "error: privacy Swift native execution requires full Xcode" >&2
  exit 1
}
xcodebuild -version

cd "${ROOT_DIR}"

bash scripts/check_mobile_sdk_artifacts.sh --apple-only

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
  IrohaSwift/Tests/IrohaSwiftTests/VerifyingKeyBackendTagTests.swift

"${SWIFT_BIN}" test \
  --package-path IrohaSwift \
  --disable-automatic-resolution \
  --scratch-path "${MOBILE_SDK_SWIFT_SCRATCH_DIR}"
