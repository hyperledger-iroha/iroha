#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${KAGEMUSHA_RECURSIVE_SPEND_SWIFT_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
SWIFTC_BIN="${KAGEMUSHA_RECURSIVE_SPEND_SWIFTC_BIN:-swiftc}"
SWIFT_BIN="${KAGEMUSHA_RECURSIVE_SPEND_SWIFT_BIN:-swift}"

cd "${ROOT_DIR}"
"${SWIFTC_BIN}" --version
"${SWIFT_BIN}" --version
"${SWIFTC_BIN}" -parse -parse-as-library \
  IrohaSwift/Sources/IrohaSwift/AccountAddress.swift \
  IrohaSwift/Sources/IrohaSwift/AssetDefinitionAddress.swift \
  IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift \
  IrohaSwift/Sources/IrohaSwift/Crypto.swift \
  IrohaSwift/Sources/IrohaSwift/NativeBridge.swift \
  IrohaSwift/Sources/IrohaSwift/ConnectAsyncSequence.swift \
  IrohaSwift/Sources/IrohaSwift/ConnectClient.swift \
  IrohaSwift/Sources/IrohaSwift/ConnectCodec.swift \
  IrohaSwift/Sources/IrohaSwift/ConnectCrypto.swift \
  IrohaSwift/Sources/IrohaSwift/ConnectEnvelope.swift \
  IrohaSwift/Sources/IrohaSwift/ConnectEnvelopeCodec.swift \
  IrohaSwift/Sources/IrohaSwift/ConnectError.swift \
  IrohaSwift/Sources/IrohaSwift/ConnectEvents.swift \
  IrohaSwift/Sources/IrohaSwift/ConnectFlowControl.swift \
  IrohaSwift/Sources/IrohaSwift/ConnectFrames.swift \
  IrohaSwift/Sources/IrohaSwift/ConnectKeyStore.swift \
  IrohaSwift/Sources/IrohaSwift/ConnectQueueDiagnostics.swift \
  IrohaSwift/Sources/IrohaSwift/ConnectQueueJournal.swift \
  IrohaSwift/Sources/IrohaSwift/ConnectReplayRecorder.swift \
  IrohaSwift/Sources/IrohaSwift/ConnectRetryPolicy.swift \
  IrohaSwift/Sources/IrohaSwift/ConnectSession.swift \
  IrohaSwift/Sources/IrohaSwift/NexusAppClient.swift \
  IrohaSwift/Sources/IrohaSwift/PrivacyNativeBridge.swift \
  IrohaSwift/Sources/IrohaSwift/VerifyingKeyBackendTag.swift \
  IrohaSwift/Sources/IrohaSwift/Halo2OfflineNoteProver.swift \
  IrohaSwift/Sources/IrohaSwift/Halo2OfflineNoteV2Prover.swift \
  IrohaSwift/Sources/IrohaSwift/KagemushaCompactPaymentTokenProver.swift \
  IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveAggregationProofBundleProver.swift \
  IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendProver.swift \
  IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveSpendRequestCodecs.swift \
  IrohaSwift/Sources/IrohaSwift/KagemushaRecursiveCompactPaymentTokenProver.swift \
  IrohaSwift/Sources/IrohaSwift/KagemushaInstructionTransactionEncoder.swift \
  IrohaSwift/Sources/IrohaSwift/TxBuilder.swift \
  IrohaSwift/Sources/IrohaSwift/ToriiClient.swift \
  IrohaSwift/Sources/IrohaSwift/ToriiCanonicalRequest.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineBearerCashWallet.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineCashLifecycle.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineCashModels.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineIssuerPublicKey.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineKagemushaAbi7CapabilityContract.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineNoritoDecoding.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineNoritoEncoding.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineNote.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineNoteCompatibility.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineNoteInstances.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineNoteRedeemPlanner.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineNoteSecureStore.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineNoteTextTransferContract.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineNoteTransactionEncoder.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineNoteTransferHandoff.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineNoteTransferProtocols.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineNoteV2.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineNoteV2Instances.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineNoteV2TransactionEncoder.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineNoteWallet.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineProofVerifiers.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineQrStream.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineQrStreamScan.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineReceiptChallenge.swift \
  IrohaSwift/Sources/IrohaSwift/OfflineTransferDiagnostics.swift \
  IrohaSwift/Sources/IrohaSwiftMobileTransports/OfflineNfcMobileTransports.swift \
  IrohaSwift/Tests/IrohaSwiftTests/CanonicalRequestTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/IrohaSDKSigningAlgorithmTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaCompactPaymentTokenProverTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveAggregationProofBundleProverTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendProverTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveSpendRequestCodecsTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaRecursiveCompactPaymentTokenProverTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/KagemushaInstructionTransactionEncoderTests.swift \
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
  IrohaSwift/Tests/IrohaSwiftTests/UC4DecodePaymentTokenTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/PrivacyNativeBridgeTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/TxBuilderTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/ToriiCanonicalRequestTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/VerifyingKeyBackendTagTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/OfflineCashLifecycleTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/OfflineIssuerPublicKeyTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/OfflineKagemushaAbi7CapabilityContractTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/OfflineNoritoEncodingTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/OfflineNoteNfcApduPayloadBudgetTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/OfflineNoteRedeemPlannerTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/OfflineNoteTextTransferContractTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/OfflineNoteTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/OfflineProofVerifierTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/OfflineQrStreamTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/OfflineReceiptChallengeTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/OfflineRevocationBundleTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/OfflineTransferDiagnosticsTests.swift \
  IrohaSwift/Tests/IrohaSwiftTests/OfflineNoteV2Tests.swift \
  IrohaSwift/Tests/IrohaSwiftTransportUITests/OfflineTransferWidgetTests.swift

(
  cd IrohaSwift
  "${SWIFT_BIN}" test --filter ToriiClientTests/testGetOfflineReadiness
)

(
  cd IrohaSwift
  "${SWIFT_BIN}" test --filter 'ToriiClientTests/testCanonicalQuerySelectorsRejectSurroundingWhitespace|ToriiClientTests/testAccountAssetQueryHelpersRejectSurroundingWhitespace|ToriiClientTests/testGetAssetsEncodesScopeSelectorFilter|ToriiClientTests/testGetAssetsRejectsPaddedScopeBeforeNetwork|ToriiClientTests/testGetUaidPortfolioRejectsPaddedLiteralBeforeNetwork'
)

(
  cd IrohaSwift
  "${SWIFT_BIN}" test --filter OfflineNoteRedeemPlannerTests
)
