#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${PRIVACY_SWIFT_SDK_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
SWIFTC_BIN="${PRIVACY_SWIFT_SDK_SWIFTC_BIN:-swiftc}"
SWIFT_BIN="${PRIVACY_SWIFT_SDK_SWIFT_BIN:-swift}"

cd "${ROOT_DIR}"

"${SWIFTC_BIN}" --version
"${SWIFTC_BIN}" -parse -parse-as-library \
  IrohaSwift/Sources/IrohaSwift/NativeBridge.swift \
  IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift \
  IrohaSwift/Sources/IrohaSwift/ProofAttachment.swift \
  IrohaSwift/Sources/IrohaSwift/VerifyingKeyBackendTag.swift \
  IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ProofAttachmentNoritoTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/VerifyingKeyBackendTagTests.swift

"${SWIFT_BIN}" test --package-path IrohaSwift
