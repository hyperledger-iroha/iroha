#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_SWIFT_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
SWIFTC_BIN="${KAGEMUSHA_RECURSIVE_SPEND_SWIFTC_BIN:-swiftc}"
SWIFT_BIN="${KAGEMUSHA_RECURSIVE_SPEND_SWIFT_BIN:-swift}"

run_swift_test() {
  if [[ -n "${KAGEMUSHA_RECURSIVE_SPEND_SWIFT_SCRATCH_PATH:-}" ]]; then
    "${SWIFT_BIN}" test \
      --scratch-path "${KAGEMUSHA_RECURSIVE_SPEND_SWIFT_SCRATCH_PATH}" \
      "$@"
  else
    "${SWIFT_BIN}" test "$@"
  fi
}

cd "${ROOT_DIR}"
if grep -Fq 'getKagemushaReadiness' IrohaSwift/Sources/IrohaSwift/ToriiClient.swift; then
  echo "error: Swift offline capability must not expose a selector-taking readiness alias" >&2
  exit 1
fi
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
  IrohaSwift/Sources/IrohaSwift/KagemushaArtifactCoordinator.swift \
  IrohaSwift/Sources/IrohaSwift/KagemushaOperationFinalityCoordinator.swift \
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
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaArtifactCoordinatorTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaOperationFinalityCoordinatorTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaRedemptionChangeV4Tests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaDeviceAuthorityV2Tests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaHardwareAuthorizationV2Tests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaDeviceAttestationSignedTransactionTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaScaledAmountTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/OfflineDeviceAttestationABI21ParityTests.swift \
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
  run_swift_test --filter ToriiClientTests/testGetOfflineCapability
)

(
  cd IrohaSwift
  run_swift_test --filter 'ToriiClientTests/testCanonicalQuerySelectorsRejectSurroundingWhitespace|ToriiClientTests/testAccountAssetQueryHelpersRejectSurroundingWhitespace|ToriiClientTests/testGetAssetsEncodesScopeSelectorFilter|ToriiClientTests/testGetAssetsRejectsPaddedScopeBeforeNetwork|ToriiClientTests/testGetUaidPortfolioRejectsPaddedLiteralBeforeNetwork'
)

(
  cd IrohaSwift
  run_swift_test --filter ToriiKagemushaAPIModelsTests
)

(
  cd IrohaSwift
  run_swift_test \
    --filter 'ToriiClientTests/testGetOfflineCapabilityParsesExactUniversalContractOnExactRoute|ToriiClientTests/testGetOfflineCapabilityRejectsNonUniversalClaims'
)

(
  cd IrohaSwift
  run_swift_test --filter PrivacyConfidentialWitnessTests
)

(
  cd IrohaSwift
  run_swift_test --filter KagemushaScaledAmountTests
)

(
  cd IrohaSwift
  run_swift_test --filter OfflineDeviceAttestationABI21ParityTests
)

(
  cd IrohaSwift
  run_swift_test --filter KagemushaRecursiveSpendTests
)

(
  cd IrohaSwift
  run_swift_test \
    --filter 'KagemushaArtifactCoordinatorTests|KagemushaOperationFinalityCoordinatorTests|KagemushaRedemptionChangeV4Tests|KagemushaDeviceAuthorityV2Tests|KagemushaHardwareAuthorizationV2Tests|KagemushaDeviceAttestationSignedTransactionTests'
)

(
  cd IrohaSwift
  run_swift_test --filter 'ToriiClientTests/testGetVerifyingKeyAsync|ToriiClientTests/testGetVerifyingKeyRejectsCrossWiredDetail|ToriiClientTests/testVerifyingKeyDetailPreservesExactNoritoRecord|ToriiClientTests/testVerifyingKeyDetailRejectsNoncanonicalRecordNoritoBase64'
)

(
  cd IrohaSwift
  run_swift_test --filter NoritoTests
)

(
  cd IrohaSwift
  run_swift_test \
    --filter 'KagemushaPeerTransportTests|KagemushaQRStreamTests|KagemushaNFCTests|KagemushaNearbyTests'
)

(
  cd IrohaSwift
  run_swift_test --filter ToriiKagemushaAPIModelsTests
)
