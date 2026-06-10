#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_SWIFT_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
SWIFTC_BIN="${KAGEMUSHA_RECURSIVE_SPEND_SWIFTC_BIN:-swiftc}"

cd "${ROOT_DIR}"
"${SWIFTC_BIN}" --version
"${SWIFTC_BIN}" -parse -parse-as-library \
  IrohaSwift/Sources/IrohaSwift/NativeBridge.swift \
  IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift \
  IrohaSwift/Sources/IrohaSwift/Halo2OfflineNoteProver.swift \
  IrohaSwift/Sources/IrohaSwift/KagemushaCompactPaymentTokenProver.swift \
  IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveAggregationProofBundleProver.swift \
  IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift \
  IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveCompactPaymentTokenProver.swift \
  IrohaSwift/Sources/IrohaSwift/KagemushaInstructionTransactionEncoder.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineNoteV2.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineNoritoDecoding.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaCompactPaymentTokenProverTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveAggregationProofBundleProverTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendProverTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveCompactPaymentTokenProverTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaInstructionTransactionEncoderTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/UC4DecodePaymentTokenTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/OfflineNoteV2Tests.swift
