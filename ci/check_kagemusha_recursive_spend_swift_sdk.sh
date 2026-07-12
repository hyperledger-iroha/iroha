#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_SWIFT_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
SWIFTC_BIN="${KAGEMUSHA_RECURSIVE_SPEND_SWIFTC_BIN:-swiftc}"
SWIFT_BIN="${KAGEMUSHA_RECURSIVE_SPEND_SWIFT_BIN:-swift}"
SWIFT_TEST_ARGS=()
if [[ -n "${KAGEMUSHA_RECURSIVE_SPEND_SWIFT_SCRATCH_PATH:-}" ]]; then
  SWIFT_TEST_ARGS+=(
    --scratch-path
    "${KAGEMUSHA_RECURSIVE_SPEND_SWIFT_SCRATCH_PATH}"
  )
fi

cd "${ROOT_DIR}"
"${SWIFTC_BIN}" --version
"${SWIFT_BIN}" --version
"${SWIFTC_BIN}" -parse -parse-as-library \
  IrohaSwift/Sources/IrohaSwift/NativeBridge.swift \
  IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift \
  IrohaSwift/Sources/IrohaSwift/PrivacyConfidentialWitness.swift \
  IrohaSwift/Sources/IrohaSwift/VerifyingKeyBackendTag.swift \
  IrohaSwift/Sources/IrohaSwift/CanonicalNoritoDecoding.swift \
  IrohaSwift/Sources/IrohaSwift/CanonicalNoritoEncoding.swift \
  IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2.swift \
  IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2Codecs.swift \
  IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendV2Native.swift \
  IrohaSwift/Sources/IrohaSwift/KagemushaScaledAmount.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineDeviceAttestation.swift \
  IrohaSwift/Sources/IrohaSwift/ProofAttachment.swift \
  IrohaSwift/Sources/IrohaSwift/TxBuilder.swift \
  IrohaSwift/Sources/IrohaSwift/ToriiClient.swift \
  IrohaSwift/Sources/IrohaSwift/ToriiCanonicalRequest.swift \
  IrohaSwift/Sources/IrohaSwift/ToriiKagemushaAPIModels.swift \
  IrohaSwift/Sources/IrohaSwift/TransactionEncoder.swift \
  IrohaSwift/Tests/IrohaSwiftTests/CanonicalRequestTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/IrohaSDKSigningAlgorithmTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendV2Tests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaScaledAmountTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ConnectAsyncSequenceTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ConnectClientTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ConnectCryptoTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ConnectEnvelopeCodecTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ConnectEnvelopeTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ConnectEventsTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ConnectFixtureLoader.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ConnectFixtureLoaderTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ConnectFlowControlTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ConnectFramesTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ConnectKeyStoreTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ConnectQueueDiagnosticsTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ConnectQueueJournalTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ConnectReplayRecorderTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ConnectRetryPolicyTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ConnectSessionBalanceTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ConnectSessionEventStreamTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ConnectSessionTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ConnectTestUtilities.swift \
  IrohaSwift/Tests/IrohaSwiftTests/NexusAppClientTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift \
  IrohaSwift/Sources/IrohaSwift/KagemushaPeerTransport.swift \
  IrohaSwift/Sources/IrohaSwift/KagemushaQRStream.swift \
  IrohaSwift/Sources/IrohaSwift/KagemushaNFC.swift \
  IrohaSwift/Sources/IrohaSwift/KagemushaNearby.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaPeerTransportTestFixtures.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaPeerTransportTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaQRStreamTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaNFCTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaNearbyTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/PrivacyConfidentialWitnessTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/NativeBridgeLoaderTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ToriiCanonicalRequestTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ToriiKagemushaAPIModelsTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/VerifyingKeyBackendTagTests.swift

(
  cd IrohaSwift
  "${SWIFT_BIN}" test "${SWIFT_TEST_ARGS[@]}" --filter ToriiClientTests/testGetOfflineReadiness
)

(
  cd IrohaSwift
  "${SWIFT_BIN}" test "${SWIFT_TEST_ARGS[@]}" --filter 'ToriiClientTests/testCanonicalQuerySelectorsRejectSurroundingWhitespace|ToriiClientTests/testAccountAssetQueryHelpersRejectSurroundingWhitespace|ToriiClientTests/testGetAssetsEncodesScopeSelectorFilter|ToriiClientTests/testGetAssetsRejectsPaddedScopeBeforeNetwork|ToriiClientTests/testGetUaidPortfolioRejectsPaddedLiteralBeforeNetwork'
)

(
  cd IrohaSwift
  "${SWIFT_BIN}" test "${SWIFT_TEST_ARGS[@]}" --filter ToriiKagemushaAPIModelsTests
)

(
  cd IrohaSwift
  "${SWIFT_BIN}" test "${SWIFT_TEST_ARGS[@]}" \
    --filter 'ToriiClientTests/testGetOfflineReadinessParsesExactContract|ToriiClientTests/testGetOfflineReadinessRejectsProtocolSubstitutionAndContradictoryClaims'
)

(
  cd IrohaSwift
  "${SWIFT_BIN}" test "${SWIFT_TEST_ARGS[@]}" --filter PrivacyConfidentialWitnessTests
)

(
  cd IrohaSwift
  "${SWIFT_BIN}" test "${SWIFT_TEST_ARGS[@]}" --filter KagemushaScaledAmountTests
)

(
  cd IrohaSwift
  "${SWIFT_BIN}" test "${SWIFT_TEST_ARGS[@]}" --filter KagemushaRecursiveSpendTests
)

(
  cd IrohaSwift
  "${SWIFT_BIN}" test "${SWIFT_TEST_ARGS[@]}" --filter 'ToriiClientTests/testGetVerifyingKeyAsync|ToriiClientTests/testGetVerifyingKeyRejectsCrossWiredDetail|ToriiClientTests/testVerifyingKeyDetailConvertsExactNoritoRecordForKagemusha|ToriiClientTests/testVerifyingKeyDetailRejectsNoncanonicalRecordNoritoBase64'
)

(
  cd IrohaSwift
  "${SWIFT_BIN}" test "${SWIFT_TEST_ARGS[@]}" --filter NoritoTests
)

(
  cd IrohaSwift
  "${SWIFT_BIN}" test "${SWIFT_TEST_ARGS[@]}" \
    --filter 'KagemushaPeerTransportTests|KagemushaQRStreamTests|KagemushaNFCTests|KagemushaNearbyTests'
)

(
  cd IrohaSwift
  "${SWIFT_BIN}" test "${SWIFT_TEST_ARGS[@]}" --filter ToriiKagemushaAPIModelsTests
)
