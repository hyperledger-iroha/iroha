#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${PRIVACY_SWIFT_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
SWIFTC_BIN="${PRIVACY_SWIFT_SDK_SWIFTC_BIN:-swiftc}"
SWIFT_BIN="${PRIVACY_SWIFT_SDK_SWIFT_BIN:-swift}"
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
  -z "${MOBILE_SDK_PYTHON_BINARY:-}" ]]; then
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
    echo "error: privacy Swift scratch directory must be outside the source tree" >&2
    exit 1
    ;;
esac

MOBILE_SDK_APPLE_ARTIFACT_DIR="${APPLE_ARTIFACT_DIRECTORY}" \
MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT=1 \
MOBILE_SDK_PYTHON_BINARY="${MOBILE_SDK_PYTHON_BINARY}" \
  bash "${APPLE_ARTIFACT_CHECKER}" --apple-only

cd "${ROOT_DIR}"

"${SWIFTC_BIN}" --version
"${SWIFTC_BIN}" -parse -parse-as-library \
  IrohaSwift/Sources/IrohaSwift/NativeBridge.swift \
  IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift \
  IrohaSwift/Sources/IrohaSwift/ProofAttachment.swift \
  IrohaSwift/Sources/IrohaSwift/VerifyingKeyBackendTag.swift \
  IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ProofAttachmentNoritoTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/VerifyingKeyBackendTagTests.swift

"${SWIFT_BIN}" test \
  --package-path IrohaSwift \
  --disable-automatic-resolution \
  --scratch-path "${SWIFT_SCRATCH_DIRECTORY}"
