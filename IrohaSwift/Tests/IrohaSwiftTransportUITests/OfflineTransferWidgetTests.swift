import Foundation
import XCTest
import IrohaSwift
import IrohaSwiftMobileTransports
import IrohaSwiftTransferUI

#if canImport(Network)
@preconcurrency import Network
#endif

final class OfflineTransferWidgetTests: XCTestCase {
    func testNfcConfigurationBuildsCustomSelectAidApdu() throws {
        let aid = Data([0xF0, 0x50, 0x4B, 0x45, 0x50, 0x4B, 0x52, 0x4E, 0x46, 0x43, 0x01])
        let configuration = IrohaOfflineNfcConfiguration(applicationIdentifier: aid)

        XCTAssertTrue(configuration.hasValidApplicationIdentifier)
        XCTAssertEqual(try configuration.validatedApplicationIdentifier(), aid)
        XCTAssertEqual(configuration.applicationIdentifierHex, "F0504B45504B524E464301")
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(configuration.selectAidAPDUData(), aid: aid),
            .select
        )
    }

    func testNfcConfigurationRejectsInvalidAidBeforeRuntimeUse() async {
        let configuration = IrohaOfflineNfcConfiguration(
            applicationIdentifier: Data([0xF0, 0x50, 0x4B, 0x45])
        )

        XCTAssertFalse(configuration.hasValidApplicationIdentifier)
        XCTAssertThrowsError(try configuration.validatedApplicationIdentifier()) { error in
            XCTAssertEqual(
                error as? OfflineNoteNfcAidError,
                .invalidLength(
                    actual: 4,
                    minimum: OfflineNoteNfcApduProtocol.minAidBytes,
                    maximum: OfflineNoteNfcApduProtocol.maxAidBytes
                )
            )
        }
        let availability = await IrohaOfflineNfcCardSessionController.cardSessionAvailability(
            configuration: configuration
        )
        XCTAssertEqual(availability, .unavailable(.invalidPayload))
        do {
            try await IrohaOfflineNfcCardSessionController(configuration: configuration).startReceiveRequest(
                "iroha:offline:receive:v1:payload",
                onIncomingPaymentToken: { _ in "iroha:offline:ack:v1:payload" }
            )
            XCTFail("Expected invalid AID configuration to fail before CardSession starts.")
        } catch {
            XCTAssertEqual(error as? IrohaOfflineNfcExchangeError, .invalidPayload)
        }
    }

    func testDeviceTransferPayloadRuntimeGateIsOpenForProductionBuilds() {
        XCTAssertTrue(
            IrohaOfflineDeviceTransferPayloadRuntime.acceptsCurrentDeviceTransferPayloads(environment: [:])
        )
        XCTAssertTrue(
            IrohaOfflineDeviceTransferPayloadRuntime.acceptsCurrentDeviceTransferPayloads(
                environment: ["UITEST_OFFLINE_KEY_CERTIFICATE_ISSUER_PUBLIC_KEY": String(repeating: "A", count: 44)]
            )
        )
        XCTAssertTrue(
            IrohaOfflineDeviceTransferPayloadRuntime.acceptsCurrentDeviceTransferPayloads(
                environment: ["XCTestConfigurationFilePath": "/tmp/RetailWalletIOSTests.xctestconfiguration"]
            )
        )
    }

    func testDeviceTransferPayloadRuntimePolicyAcceptsOnlyCurrentStrictTextPayloads() throws {
        let receiveRequest = try Self.validReceiveRequestTextPayload()

        XCTAssertEqual(
            IrohaOfflineDeviceTransferPayloadRuntime.currentTextPayloadKinds,
            [.receiveRequest, .paymentToken, .receiptAck]
        )
        XCTAssertTrue(
            IrohaOfflineDeviceTransferPayloadRuntime.acceptsCurrentTextPayload(
                receiveRequest,
                expectedKind: .receiveRequest
            )
        )
        XCTAssertEqual(
            IrohaOfflineDeviceTransferPayloadRuntime.currentTextPayloadKind(for: receiveRequest),
            .receiveRequest
        )
        XCTAssertEqual(
            try IrohaOfflineDeviceTransferPayloadRuntime.normalizedCurrentTextPayload(
                " \t\(receiveRequest)\n",
                expectedKind: .receiveRequest
            ),
            receiveRequest
        )
        XCTAssertFalse(
            IrohaOfflineDeviceTransferPayloadRuntime.acceptsCurrentTextPayload(
                receiveRequest,
                expectedKind: .paymentToken
            )
        )

        let adversarialPayloads = [
            "",
            "wallet-offline-challenge-legacy:payload",
            "wallet-offline-payment-legacy:payload",
            "wallet-offline-payment-json-legacy:payload",
            "https://example.test/\(receiveRequest)",
            "{\"payload\":\"\(receiveRequest)\"}",
            "\(receiveRequest)\u{0000}",
            "\(receiveRequest)\u{2028}",
            "\(receiveRequest)é",
            " \(receiveRequest)",
            "\(receiveRequest)\n",
        ]
        for payload in adversarialPayloads {
            XCTAssertNil(
                IrohaOfflineDeviceTransferPayloadRuntime.currentTextPayloadKind(for: payload),
                payload
            )
            XCTAssertFalse(
                IrohaOfflineDeviceTransferPayloadRuntime.acceptsCurrentTextPayload(
                    payload,
                    expectedKind: .receiveRequest
                ),
                payload
            )
        }
    }

    func testOfflineDisplayAmountPolicyCanonicalizesPositiveAmountsForUi() {
        let validCases: [(String, String, String)] = [
            ("integer", "00125", "125"),
            ("trailingZeros", "00125.5000", "125.5"),
            ("smallestDisplayable", "0.00000001", "0.00000001"),
            ("truncatedFraction", "1.234567899", "1.23456789"),
            ("maxScaleZero", "9.99", "9"),
        ]

        for (label, amount, expectedDisplay) in validCases {
            let maxScale = label == "maxScaleZero" ? 0 : IrohaOfflineDisplayAmountPolicy.defaultMaxScale
            XCTAssertNotNil(
                IrohaOfflineDisplayAmountPolicy.canonicalPositiveAmount(amount),
                label
            )
            XCTAssertEqual(
                IrohaOfflineDisplayAmountPolicy.displayPositiveAmount(amount, maxScale: maxScale),
                expectedDisplay,
                label
            )
        }
    }

    func testOfflineDisplayAmountPolicyRejectsNonPositiveAmbiguousAndSubDisplayAmounts() {
        let invalidAmounts = [
            "",
            " ",
            "0",
            "0.0",
            "-1",
            "1e3",
            "1,000",
            "SBD 125",
            "0x10",
            "1_000",
            "1/2",
            "125;drop",
            "12\u{00A0}5",
            "\u{06F1}\u{06F2}\u{06F5}",
            "0.000000001",
            "0.000000009",
            String(repeating: "9", count: 160),
        ]

        for amount in invalidAmounts {
            XCTAssertNil(
                IrohaOfflineDisplayAmountPolicy.displayPositiveAmount(amount),
                amount
            )
        }

        XCTAssertNil(IrohaOfflineDisplayAmountPolicy.displayPositiveAmount("0.1", maxScale: 0))
        XCTAssertNil(IrohaOfflineDisplayAmountPolicy.displayPositiveAmount("1", maxScale: -1))
        XCTAssertNil(IrohaOfflineDisplayAmountPolicy.truncateCanonicalAmountForDisplay("not-a-number"))
    }

    func testNfcCardSessionRuntimePolicyAcceptsOnlyExplicitInfoPlistOptInValues() {
        for value in [true, "1", " true ", "YES", "yes", "TrUe"] as [Any] {
            XCTAssertTrue(
                IrohaOfflineNfcCardSessionRuntimePolicy.isRuntimeEnabled(
                    infoDictionaryValue: value,
                    environment: [:],
                    hasCompiledCardSessionSupport: true,
                    allowsDebugEnvironmentOptIn: false
                ),
                "Expected \(value) to enable CardSession runtime."
            )
        }

        for value in [false, "", " ", "0", "false", "NO", "enabled", "$(OFFLINE_NFC_CARDSESSION_RUNTIME_ENABLED)", 1, ["yes"]] as [Any] {
            XCTAssertFalse(
                IrohaOfflineNfcCardSessionRuntimePolicy.isRuntimeEnabled(
                    infoDictionaryValue: value,
                    environment: ["OFFLINE_NFC_CARDSESSION_RUNTIME_OPT_IN": "1"],
                    hasCompiledCardSessionSupport: true,
                    allowsDebugEnvironmentOptIn: false
                ),
                "Expected \(value) to be rejected without debug environment opt-in."
            )
        }
    }

    func testNfcCardSessionRuntimePolicyValidatesInfoDictionaryRuntimeConfiguration() {
        let approvedAid = "F0504B45504B524E464301"
        for runtimeValue in [true, "1", " true ", "YES", "TrUe"] as [Any] {
            XCTAssertTrue(
                IrohaOfflineNfcCardSessionRuntimePolicy.hasRequiredInfoDictionaryRuntimeConfiguration(
                    infoDictionary: [
                        IrohaOfflineNfcCardSessionRuntimePolicy.cardSessionEnabledInfoKey: runtimeValue,
                        IrohaOfflineNfcCardSessionRuntimePolicy.readerSelectIdentifiersInfoKey: [
                            "F049524F484132",
                            " \(approvedAid.lowercased()) "
                        ],
                    ],
                    expectedApplicationIdentifierHex: approvedAid
                ),
                "Expected \(runtimeValue) plus the normalized approved AID to pass."
            )
        }

        let invalidInfoDictionaries: [[String: Any]] = [
            [
                IrohaOfflineNfcCardSessionRuntimePolicy.cardSessionEnabledInfoKey: "$(OFFLINE_NFC_CARDSESSION_RUNTIME_ENABLED)",
                IrohaOfflineNfcCardSessionRuntimePolicy.readerSelectIdentifiersInfoKey: [approvedAid],
            ],
            [
                IrohaOfflineNfcCardSessionRuntimePolicy.cardSessionEnabledInfoKey: "NO",
                IrohaOfflineNfcCardSessionRuntimePolicy.readerSelectIdentifiersInfoKey: [approvedAid],
            ],
            [
                IrohaOfflineNfcCardSessionRuntimePolicy.cardSessionEnabledInfoKey: 1,
                IrohaOfflineNfcCardSessionRuntimePolicy.readerSelectIdentifiersInfoKey: [approvedAid],
            ],
            [
                IrohaOfflineNfcCardSessionRuntimePolicy.cardSessionEnabledInfoKey: true,
                IrohaOfflineNfcCardSessionRuntimePolicy.readerSelectIdentifiersInfoKey: ["F0BAD"],
            ],
            [
                IrohaOfflineNfcCardSessionRuntimePolicy.cardSessionEnabledInfoKey: true,
                IrohaOfflineNfcCardSessionRuntimePolicy.readerSelectIdentifiersInfoKey: approvedAid,
            ],
            [
                IrohaOfflineNfcCardSessionRuntimePolicy.cardSessionEnabledInfoKey: true,
                IrohaOfflineNfcCardSessionRuntimePolicy.readerSelectIdentifiersInfoKey: [1],
            ],
            [
                IrohaOfflineNfcCardSessionRuntimePolicy.cardSessionEnabledInfoKey: true,
                IrohaOfflineNfcCardSessionRuntimePolicy.readerSelectIdentifiersInfoKey: ["ZZ"],
            ],
            [:],
        ]

        for infoDictionary in invalidInfoDictionaries {
            XCTAssertFalse(
                IrohaOfflineNfcCardSessionRuntimePolicy.hasRequiredInfoDictionaryRuntimeConfiguration(
                    infoDictionary: infoDictionary,
                    expectedApplicationIdentifierHex: approvedAid
                ),
                "Expected invalid Info.plist runtime config to be rejected: \(infoDictionary)."
            )
        }

        XCTAssertFalse(
            IrohaOfflineNfcCardSessionRuntimePolicy.hasRequiredInfoDictionaryRuntimeConfiguration(
                infoDictionary: [
                    IrohaOfflineNfcCardSessionRuntimePolicy.cardSessionEnabledInfoKey: true,
                    IrohaOfflineNfcCardSessionRuntimePolicy.readerSelectIdentifiersInfoKey: [approvedAid],
                ],
                expectedApplicationIdentifierHex: "F0BAD"
            )
        )
    }

    func testNfcCardSessionRuntimePolicyRequiresCompiledSupport() {
        for value in [true, "yes", nil] as [Any?] {
            XCTAssertFalse(
                IrohaOfflineNfcCardSessionRuntimePolicy.isRuntimeEnabled(
                    infoDictionaryValue: value,
                    environment: ["OFFLINE_NFC_CARDSESSION_RUNTIME_OPT_IN": "1"],
                    hasCompiledCardSessionSupport: false,
                    allowsDebugEnvironmentOptIn: true
                ),
                "CardSession runtime must stay disabled when the app target was not compiled with support."
            )
        }
    }

    func testNfcCardSessionRuntimePolicyAllowsOnlyExactDebugEnvironmentOptIn() {
        XCTAssertTrue(
            IrohaOfflineNfcCardSessionRuntimePolicy.isRuntimeEnabled(
                infoDictionaryValue: nil,
                environment: ["OFFLINE_NFC_CARDSESSION_RUNTIME_OPT_IN": "1"],
                hasCompiledCardSessionSupport: true,
                allowsDebugEnvironmentOptIn: true
            )
        )

        for environmentValue in ["", "true", "yes", " 1 ", "0"] {
            XCTAssertFalse(
                IrohaOfflineNfcCardSessionRuntimePolicy.isRuntimeEnabled(
                    infoDictionaryValue: nil,
                    environment: ["OFFLINE_NFC_CARDSESSION_RUNTIME_OPT_IN": environmentValue],
                    hasCompiledCardSessionSupport: true,
                    allowsDebugEnvironmentOptIn: true
                ),
                "Expected exact debug opt-in only; rejected \(environmentValue)."
            )
        }

        XCTAssertFalse(
            IrohaOfflineNfcCardSessionRuntimePolicy.isRuntimeEnabled(
                infoDictionaryValue: nil,
                environment: ["OFFLINE_NFC_CARDSESSION_RUNTIME_OPT_IN": "1"],
                hasCompiledCardSessionSupport: true,
                allowsDebugEnvironmentOptIn: false
            )
        )
    }

    func testPreparedPaymentRetryClassificationKeepsOnlyTransientFailuresOpen() {
        XCTAssertTrue(IrohaOfflineNfcExchangeError.ackPending.shouldRetryPreparedPaymentTransfer)
        XCTAssertTrue(IrohaOfflineNfcExchangeError.nfcTimeout.shouldRetryPreparedPaymentTransfer)
        XCTAssertTrue(IrohaOfflineNfcExchangeError.peerRejected(statusWord: nil).shouldRetryPreparedPaymentTransfer)
        XCTAssertTrue(IrohaOfflineNfcExchangeError.peerRejected(statusWord: 0x6985).shouldRetryPreparedPaymentTransfer)

        XCTAssertFalse(IrohaOfflineNfcExchangeError.peerRejected(statusWord: 0x6A80).shouldRetryPreparedPaymentTransfer)
        XCTAssertFalse(IrohaOfflineNfcExchangeError.invalidPayload.shouldRetryPreparedPaymentTransfer)
        XCTAssertFalse(IrohaOfflineNfcExchangeError.checksumMismatch.shouldRetryPreparedPaymentTransfer)
    }

    func testDeviceTransferDeliveryTrackerIsMonotonicAndThreadSafe() {
        let tracker = IrohaOfflineDeviceTransferDeliveryTracker()

        XCTAssertFalse(tracker.mayHaveReachedReceiver)
        tracker.markPotentiallyDelivered()
        tracker.markPotentiallyDelivered()

        XCTAssertTrue(tracker.mayHaveReachedReceiver)
    }

    func testNfcDeliveryProgressPolicyMarksOnlyCommitAndAckBoundaryAsPotentiallyDelivered() {
        let deliveredProgress = [
            "write_payload_commit_begin",
            "write_payload_commit_begin offset=0",
            "write_payload_committed",
            "receipt_ack_wait_begin timeout_seconds=45",
            "receipt_ack_received bytes=128",
            "receipt_ack_already_available bytes=128",
        ]

        for progress in deliveredProgress {
            XCTAssertTrue(
                IrohaOfflineNfcDeliveryProgressPolicy.mayHaveReachedReceiver(progress: progress),
                progress
            )
        }

        let preCommitOrNoisyProgress = [
            "",
            "session_active",
            "restart_polling",
            "detected_tags count=1",
            "connect_succeeded",
            "select_aid_ok",
            "read_payload kind=receiveRequest bytes=128",
            "create_payment_token",
            "write_payload kind=paymentToken bytes=512",
            "write_payload_commit",
            " write_payload_commit_begin",
            "xwrite_payload_commit_begin",
            "receipt_ack",
            "receipt_ack_wait",
            "receipt_ack_waiting",
            "write_payload_commit_begin_extra",
            "receipt_ack_received_tampered",
        ]

        for progress in preCommitOrNoisyProgress {
            XCTAssertFalse(
                IrohaOfflineNfcDeliveryProgressPolicy.mayHaveReachedReceiver(progress: progress),
                progress
            )
        }
    }

    func testNearbyMessageExchangeStateRejectsDuplicateAndReorderedApplicationMessages() {
        let senderState = IrohaOfflineNearbyMessageExchangeState()

        XCTAssertNil(senderState.takeQueuedSenderPaymentPayloadForReceiptAck(didFinish: false))
        XCTAssertTrue(senderState.reserveSenderReceiveRequest(didFinish: false))
        XCTAssertFalse(senderState.reserveSenderReceiveRequest(didFinish: false))
        XCTAssertFalse(senderState.reserveSenderReceiveRequest(didFinish: true))
        XCTAssertFalse(senderState.hasQueuedSenderPaymentPayload)

        senderState.queueSenderPaymentPayload("iroha:offline:payment:v1:normalized")
        XCTAssertTrue(senderState.hasQueuedSenderPaymentPayload)
        XCTAssertFalse(senderState.reserveSenderReceiveRequest(didFinish: false))
        XCTAssertEqual(
            senderState.takeQueuedSenderPaymentPayloadForReceiptAck(didFinish: false),
            "iroha:offline:payment:v1:normalized"
        )
        XCTAssertFalse(senderState.hasQueuedSenderPaymentPayload)
        XCTAssertNil(senderState.takeQueuedSenderPaymentPayloadForReceiptAck(didFinish: false))
        XCTAssertNil(senderState.takeQueuedSenderPaymentPayloadForReceiptAck(didFinish: true))

        senderState.reset()
        XCTAssertFalse(senderState.hasQueuedSenderPaymentPayload)
        XCTAssertTrue(senderState.reserveSenderReceiveRequest(didFinish: false))

        let receiverState = IrohaOfflineNearbyMessageExchangeState()
        XCTAssertTrue(receiverState.reserveReceiverPayment(didFinish: false))
        XCTAssertFalse(receiverState.reserveReceiverPayment(didFinish: false))
        receiverState.reset()
        XCTAssertFalse(receiverState.reserveReceiverPayment(didFinish: true))
        XCTAssertTrue(receiverState.reserveReceiverPayment(didFinish: false))
    }

    func testNearbyMessageExchangeStateAllowsOnlyOneConcurrentReservationPerPhase() {
        let senderState = IrohaOfflineNearbyMessageExchangeState()
        let receiverState = IrohaOfflineNearbyMessageExchangeState()
        let senderLock = NSLock()
        let receiverLock = NSLock()
        let group = DispatchGroup()
        let queue = DispatchQueue(label: "iroha.nearby-message-state-test", attributes: .concurrent)
        var senderAccepts = 0
        var receiverAccepts = 0

        for _ in 0..<128 {
            group.enter()
            queue.async {
                if senderState.reserveSenderReceiveRequest(didFinish: false) {
                    senderLock.lock()
                    senderAccepts += 1
                    senderLock.unlock()
                }
                group.leave()
            }

            group.enter()
            queue.async {
                if receiverState.reserveReceiverPayment(didFinish: false) {
                    receiverLock.lock()
                    receiverAccepts += 1
                    receiverLock.unlock()
                }
                group.leave()
            }
        }

        XCTAssertEqual(group.wait(timeout: .now() + 5), .success)
        XCTAssertEqual(senderAccepts, 1)
        XCTAssertEqual(receiverAccepts, 1)
    }

    func testNfcErrorTechnicalCodesDoNotExposePayloads() {
        XCTAssertEqual(
            IrohaOfflineNfcExchangeError.peerRejected(statusWord: 0x6985).technicalCode,
            "IrohaOfflineNfcExchangeError.peerRejected.6985"
        )
        XCTAssertEqual(
            IrohaOfflineNfcExchangeError.peerRejected(statusWord: nil).technicalCode,
            "IrohaOfflineNfcExchangeError.peerRejected.nil"
        )
    }

    func testNfcErrorsExposeOnlyStableDiagnosticsAndTechnicalCodes() {
        let sensitiveNeedles = [
            "iroha:offline:",
            "account@",
            "payment-token",
            "receipt-ack",
            "receive-request",
        ]
        let cases: [(IrohaOfflineNfcExchangeError, IrohaOfflineReceiveDiagnosticReason, String)] = [
            (.unavailable, .unavailable, "IrohaOfflineNfcExchangeError.unavailable"),
            (.cardEmulationUnavailable, .unsupportedDevice, "IrohaOfflineNfcExchangeError.cardEmulationUnavailable"),
            (
                .cardSessionMissingEntitlementOrProfile,
                .missingEntitlementOrProfile,
                "IrohaOfflineNfcExchangeError.cardSessionMissingEntitlementOrProfile"
            ),
            (.cardSessionIneligible, .ineligibleDevice, "IrohaOfflineNfcExchangeError.cardSessionIneligible"),
            (.presentmentDenied, .presentmentDenied, "IrohaOfflineNfcExchangeError.presentmentDenied"),
            (.cardSessionTimeout, .timeout, "IrohaOfflineNfcExchangeError.cardSessionTimeout"),
            (.nfcTimeout, .timeout, "IrohaOfflineNfcExchangeError.nfcTimeout"),
            (.invalidPeer, .invalidPayload, "IrohaOfflineNfcExchangeError.invalidPeer"),
            (.peerRejected(statusWord: 0x6A80), .peerRejected, "IrohaOfflineNfcExchangeError.peerRejected.6A80"),
            (.invalidPayload, .invalidPayload, "IrohaOfflineNfcExchangeError.invalidPayload"),
            (.checksumMismatch, .checksumMismatch, "IrohaOfflineNfcExchangeError.checksumMismatch"),
            (.ackPending, .invalidPayload, "IrohaOfflineNfcExchangeError.ackPending"),
            (.cancelled, .userCancelled, "IrohaOfflineNfcExchangeError.cancelled"),
        ]

        for (error, diagnosticReason, technicalCode) in cases {
            XCTAssertEqual(error.receiveDiagnosticReason, diagnosticReason)
            XCTAssertEqual(error.technicalCode, technicalCode)
            for needle in sensitiveNeedles {
                XCTAssertFalse(error.technicalCode.contains(needle), "\(technicalCode) contains \(needle)")
            }
        }
    }

    func testNfcDiagnosticsRedactGenericNSErrorDomainsAndDescriptions() {
        XCTAssertEqual(
            IrohaOfflineNfcDiagnosticsPolicy.errorCode(IrohaOfflineNfcExchangeError.checksumMismatch),
            IrohaOfflineNfcExchangeError.checksumMismatch.technicalCode
        )

        let sensitiveError = NSError(
            domain: "NfcSensitiveDomain.account@example.test",
            code: 42,
            userInfo: [
                NSLocalizedDescriptionKey: "payment-token iroha:offline:payment:v1:secret failed"
            ]
        )
        let code = IrohaOfflineNfcDiagnosticsPolicy.errorCode(sensitiveError)
        XCTAssertTrue(code.hasPrefix("NSError."))
        XCTAssertTrue(code.hasSuffix(".42"))
        XCTAssertFalse(code.contains("NfcSensitiveDomain"))
        XCTAssertFalse(code.contains("account@example.test"))
        XCTAssertFalse(code.contains("payment-token"))
        XCTAssertFalse(code.contains("iroha:offline:"))
    }

    func testNearbyErrorsExposeOnlyStableTechnicalCodes() {
        let sensitiveNeedles = [
            "iroha:offline:",
            "account@",
            "payment-token",
            "receipt-ack",
            "receive-request",
        ]
        let cases: [(IrohaOfflineNearbyExchangeError, String)] = [
            (.unavailable, "IrohaOfflineNearbyExchangeError.unavailable"),
            (.busy, "IrohaOfflineNearbyExchangeError.busy"),
            (.timedOut, "IrohaOfflineNearbyExchangeError.timedOut"),
            (.connectionFailed, "IrohaOfflineNearbyExchangeError.connectionFailed"),
            (.invalidMessage, "IrohaOfflineNearbyExchangeError.invalidMessage"),
            (.peerRejected, "IrohaOfflineNearbyExchangeError.peerRejected"),
            (.cancelled, "IrohaOfflineNearbyExchangeError.cancelled"),
            (.pairingMismatch, "IrohaOfflineNearbyExchangeError.pairingMismatch"),
            (.localNetworkPermissionDenied, "IrohaOfflineNearbyExchangeError.localNetworkPermissionDenied"),
        ]

        for (error, technicalCode) in cases {
            XCTAssertEqual(error.technicalCode, technicalCode)
            for needle in sensitiveNeedles {
                XCTAssertFalse(error.technicalCode.contains(needle), "\(technicalCode) contains \(needle)")
            }
        }
    }

    func testNearbyDiagnosticsRedactPeerLabelsAndNSErrorDescriptions() {
        let peerName = "bokolo-sensitive-peer-name"
        let peerLabel = IrohaOfflineNearbyDiagnosticsPolicy.peerLabel(
            displayName: peerName,
            selectionHash: 12345
        )

        XCTAssertTrue(peerLabel.hasPrefix("peer_hash="))
        XCTAssertFalse(peerLabel.contains(peerName))
        XCTAssertFalse(peerLabel.contains("12345"))

        let rawError = NSError(
            domain: "NearbySensitiveDomain",
            code: 77,
            userInfo: [
                NSLocalizedDescriptionKey: "raw peer \(peerName) failed with token iroha:offline:payment:v1:secret"
            ]
        )
        let rawErrorCode = IrohaOfflineNearbyDiagnosticsPolicy.errorCode(rawError)
        XCTAssertTrue(rawErrorCode.hasPrefix("NSError."))
        XCTAssertTrue(rawErrorCode.hasSuffix(".77"))
        XCTAssertFalse(rawErrorCode.contains("NearbySensitiveDomain"))
        XCTAssertFalse(rawErrorCode.contains(peerName))
        XCTAssertFalse(rawErrorCode.contains("iroha:offline:"))

        XCTAssertEqual(
            IrohaOfflineNearbyDiagnosticsPolicy.errorCode(IrohaOfflineNearbyExchangeError.invalidMessage),
            IrohaOfflineNearbyExchangeError.invalidMessage.technicalCode
        )
        XCTAssertEqual(
            IrohaOfflineNearbyDiagnosticsPolicy.errorCode(CancellationError()),
            IrohaOfflineNearbyExchangeError.cancelled.technicalCode
        )
    }

    func testNearbyPairingDecisionContractCoversAllOutcomes() {
        let outcomes: [IrohaOfflineNearbyPairingDecision] = [.accepted, .mismatch, .cancelled]

        XCTAssertEqual(outcomes, [.accepted, .mismatch, .cancelled])
        XCTAssertNotEqual(IrohaOfflineNearbyPairingDecision.accepted, .mismatch)
        XCTAssertNotEqual(IrohaOfflineNearbyPairingDecision.accepted, .cancelled)
    }

    func testNfcReceiveCompletionPolicyKeepsAcceptedPaymentSuccessful() {
        XCTAssertTrue(
            IrohaOfflineNfcReceiveCompletionPolicy.shouldPublishReceiveSuccessOnAckReady(
                hasAcceptedPayment: false
            )
        )
        XCTAssertFalse(
            IrohaOfflineNfcReceiveCompletionPolicy.shouldPublishReceiveSuccessOnAckReady(
                hasAcceptedPayment: true
            )
        )

        XCTAssertTrue(
            IrohaOfflineNfcReceiveCompletionPolicy.shouldPublishReceiveSuccessOnAckRead(
                hasAcceptedPayment: false
            )
        )
        XCTAssertFalse(
            IrohaOfflineNfcReceiveCompletionPolicy.shouldPublishReceiveSuccessOnAckRead(
                hasAcceptedPayment: true
            )
        )

        XCTAssertTrue(
            IrohaOfflineNfcReceiveCompletionPolicy.shouldFailSessionInvalidation(
                hasAcceptedPayment: false
            )
        )
        XCTAssertFalse(
            IrohaOfflineNfcReceiveCompletionPolicy.shouldFailSessionInvalidation(
                hasAcceptedPayment: true
            )
        )
    }

    func testNearbyErrorsNormalizeCancellationAndLocalNetworkPermissionDenials() {
        XCTAssertEqual(
            IrohaOfflineNearbyExchangeError.normalized(CancellationError()) as? IrohaOfflineNearbyExchangeError,
            .cancelled
        )

        let netServiceDenied = NSError(domain: NetService.errorDomain, code: -72008)
        XCTAssertEqual(
            IrohaOfflineNearbyExchangeError.normalized(netServiceDenied) as? IrohaOfflineNearbyExchangeError,
            .localNetworkPermissionDenied
        )

        let descriptionDenied = NSError(
            domain: "com.apple.network",
            code: 1,
            userInfo: [
                NSLocalizedDescriptionKey: "Local Network privacy permission denied for this app"
            ]
        )
        XCTAssertEqual(
            IrohaOfflineNearbyExchangeError.normalized(descriptionDenied) as? IrohaOfflineNearbyExchangeError,
            .localNetworkPermissionDenied
        )

        let nestedDenied = NSError(
            domain: "outer",
            code: 2,
            userInfo: [NSUnderlyingErrorKey: descriptionDenied]
        )
        XCTAssertEqual(
            IrohaOfflineNearbyExchangeError.normalized(nestedDenied) as? IrohaOfflineNearbyExchangeError,
            .localNetworkPermissionDenied
        )
    }

    func testNearbyErrorsNormalizeNetworkPolicyDenied() {
#if canImport(Network)
        let policyDenied = NWError.dns(DNSServiceErrorType(kDNSServiceErr_PolicyDenied))
        XCTAssertTrue(IrohaOfflineNearbyExchangeError.isLocalNetworkPermissionError(policyDenied))
        XCTAssertEqual(
            IrohaOfflineNearbyExchangeError.normalized(policyDenied) as? IrohaOfflineNearbyExchangeError,
            .localNetworkPermissionDenied
        )

        let nestedDenied = NSError(
            domain: "outer",
            code: 7,
            userInfo: [NSUnderlyingErrorKey: policyDenied]
        )
        XCTAssertTrue(IrohaOfflineNearbyExchangeError.isLocalNetworkPermissionError(nestedDenied))
        XCTAssertEqual(
            IrohaOfflineNearbyExchangeError.normalized(nestedDenied) as? IrohaOfflineNearbyExchangeError,
            .localNetworkPermissionDenied
        )

        let nonPolicyDnsError = NWError.dns(DNSServiceErrorType(0))
        XCTAssertFalse(IrohaOfflineNearbyExchangeError.isLocalNetworkPermissionError(nonPolicyDnsError))
        XCTAssertNil(
            IrohaOfflineNearbyExchangeError.normalized(nonPolicyDnsError) as? IrohaOfflineNearbyExchangeError
        )
#endif
    }

    func testNearbyTextEnvelopeCodecRoundTripsReceiveRequestAndRejected() throws {
        let challenge = try OfflineNoteNearbyPairingChallenge(assetName: "nearby_pairing_mask")
        let receiveRequest = try Self.validReceiveRequestTextPayload()

        let receiveBytes = try IrohaOfflineNearbyTextEnvelopeCodec.encode(
            payload: "  \(receiveRequest)\n",
            kind: .receiveRequest,
            pairingChallenge: challenge
        )
        let decodedReceive = try IrohaOfflineNearbyTextEnvelopeCodec.decode(receiveBytes)

        XCTAssertEqual(decodedReceive.messageKind, .receiveRequest)
        XCTAssertEqual(decodedReceive.textKind, .receiveRequest)
        XCTAssertEqual(decodedReceive.payload, receiveRequest)
        XCTAssertEqual(decodedReceive.pairingChallenge, challenge)

        let rejectedBytes = try IrohaOfflineNearbyTextEnvelopeCodec.encodeRejected()
        let decodedRejected = try IrohaOfflineNearbyTextEnvelopeCodec.decode(rejectedBytes)
        XCTAssertEqual(
            decodedRejected,
            IrohaOfflineNearbyTextEnvelopeCodec.DecodedEnvelope(
                messageKind: .rejected,
                textKind: nil,
                payload: OfflineNoteTransferHandoff.nearbyRejectedPayload,
                pairingChallenge: nil
            )
        )
    }

    func testNearbyTextEnvelopeCodecRejectsAdversarialShapes() throws {
        let challenge = try OfflineNoteNearbyPairingChallenge(assetName: "nearby_pairing_mask")
        let receiveRequest = try Self.validReceiveRequestTextPayload()
        let invalidPayment = try OfflineNoteTransferTextPayloadCodec.encode(
            Data("not-a-token".utf8),
            kind: .paymentToken
        )
        let invalidAck = try OfflineNoteTransferTextPayloadCodec.encode(
            Data("not-an-ack".utf8),
            kind: .receiptAck
        )

        XCTAssertThrowsError(
            try IrohaOfflineNearbyTextEnvelopeCodec.encode(payload: receiveRequest, kind: .receiveRequest)
        ) { error in
            XCTAssertEqual(error as? IrohaOfflineNearbyExchangeError, .invalidMessage)
        }
        XCTAssertThrowsError(
            try IrohaOfflineNearbyTextEnvelopeCodec.encode(
                payload: invalidPayment,
                kind: .paymentToken,
                pairingChallenge: challenge
            )
        ) { error in
            XCTAssertEqual(error as? IrohaOfflineNearbyExchangeError, .invalidMessage)
        }
        XCTAssertThrowsError(
            try IrohaOfflineNearbyTextEnvelopeCodec.encode(
                payload: invalidAck,
                kind: .receiptAck,
                pairingChallenge: challenge
            )
        ) { error in
            XCTAssertEqual(error as? IrohaOfflineNearbyExchangeError, .invalidMessage)
        }

        let tamperedRejected = try OfflineNoteNearbyEnvelope(
            kind: .rejected,
            payload: Data("cancelled".utf8),
            contentType: OfflineNoteTransferHandoff.nearbyRejectedContentType
        ).encoded()
        XCTAssertThrowsError(try IrohaOfflineNearbyTextEnvelopeCodec.decode(tamperedRejected)) { error in
            XCTAssertEqual(error as? IrohaOfflineNearbyExchangeError, .invalidMessage)
        }

        let decodedInvalidPayment = Data(
            """
            {"kind":"payment","payload":"\(Self.base64Url(Data(invalidPayment.utf8)))","contentType":"text/vnd.iroha.offline.payment-token"}
            """.utf8
        )
        XCTAssertThrowsError(try IrohaOfflineNearbyTextEnvelopeCodec.decode(decodedInvalidPayment)) { error in
            XCTAssertEqual(error as? IrohaOfflineNearbyExchangeError, .invalidMessage)
        }
    }

    func testDeviceToDeviceTextPayloadFacadeNormalizesAndValidatesStrictly() throws {
        let receiveRequest = try Self.validReceiveRequestTextPayload()

        XCTAssertEqual(
            IrohaOfflineDeviceToDeviceTextPayload.trimmingBoundaryWhitespace(" \t\r\n\(receiveRequest)\n"),
            receiveRequest
        )
        XCTAssertTrue(IrohaOfflineDeviceToDeviceTextPayload.hasOnlyTextTransportCharacters(receiveRequest))
        XCTAssertTrue(
            IrohaOfflineDeviceToDeviceTextPayload.hasBase64URLTextBody(
                receiveRequest,
                prefixes: [OfflineNoteTransferTextPayloadCodec.receiveRequestPrefix]
            )
        )
        XCTAssertEqual(
            try IrohaOfflineDeviceToDeviceTextPayload.normalize(
                " \(receiveRequest)\n",
                expectedKind: .receiveRequest
            ),
            receiveRequest
        )
        XCTAssertTrue(
            IrohaOfflineDeviceToDeviceTextPayload.isValid(receiveRequest, expectedKind: .receiveRequest)
        )
        XCTAssertFalse(
            IrohaOfflineDeviceToDeviceTextPayload.isValid(" \(receiveRequest)", expectedKind: .receiveRequest)
        )
        XCTAssertEqual(IrohaOfflineDeviceToDeviceTextPayload.kind(for: .paymentToken), .paymentToken)
        XCTAssertEqual(IrohaOfflineDeviceToDeviceTextPayload.kind(for: .receiptAck), .receiptAck)

        for malformed in ["\(receiveRequest)\u{0000}", "\(receiveRequest)\u{2028}", "\(receiveRequest)é"] {
            XCTAssertFalse(IrohaOfflineDeviceToDeviceTextPayload.hasOnlyTextTransportCharacters(malformed))
        }
        XCTAssertThrowsError(
            try IrohaOfflineDeviceToDeviceTextPayload.normalize(
                receiveRequest,
                expectedKind: .paymentToken
            )
        )
    }

#if canImport(Network)
    @MainActor
    func testNearbyLocalNetworkAuthorizationCachesSessionGrant() {
        IrohaOfflineNearbyLocalNetworkAuthorization.resetSessionGrantForTesting()
        XCTAssertFalse(IrohaOfflineNearbyLocalNetworkAuthorization.isGrantedForSession)

        IrohaOfflineNearbyLocalNetworkAuthorization.markGrantedForSession()
        XCTAssertTrue(IrohaOfflineNearbyLocalNetworkAuthorization.isGrantedForSession)

        IrohaOfflineNearbyLocalNetworkAuthorization.resetSessionGrantForTesting()
        XCTAssertFalse(IrohaOfflineNearbyLocalNetworkAuthorization.isGrantedForSession)
    }

    func testNearbyLocalNetworkPreflightCancelBeforeRunDoesNotSuspend() async {
        let preflight = IrohaOfflineNearbyLocalNetworkPreflight(
            serviceType: "_iroha-offline-test._tcp"
        )
        preflight.cancel()

        do {
            try await preflight.run()
            XCTFail("Expected cancelled preflight to throw before starting NWBrowser.")
        } catch {
            XCTAssertEqual(
                IrohaOfflineNearbyExchangeError.normalized(error) as? IrohaOfflineNearbyExchangeError,
                .cancelled
            )
        }
    }
#endif

    func testNearbyLocalNetworkPermissionDetectionRejectsAmbiguousErrors() {
        let ambiguousErrors = [
            NSError(
                domain: "com.apple.network",
                code: 1,
                userInfo: [NSLocalizedDescriptionKey: "Network permission denied"]
            ),
            NSError(
                domain: "com.apple.network",
                code: 2,
                userInfo: [NSLocalizedDescriptionKey: "Local network route unavailable"]
            ),
            NSError(domain: NetService.errorDomain, code: -72007),
        ]

        for error in ambiguousErrors {
            XCTAssertFalse(
                IrohaOfflineNearbyExchangeError.isLocalNetworkPermissionError(error),
                "\(error)"
            )
            XCTAssertNil(IrohaOfflineNearbyExchangeError.normalized(error) as? IrohaOfflineNearbyExchangeError)
        }
    }

    func testFountainPayloadFramesKeepSmallPayloadSingleFrame() {
        let payload = "offline-small-payload"
        XCTAssertEqual(IrohaOfflineFountainPayloadFrames.frames(for: payload), [payload])
    }

    func testTransportAvailabilityHidesUnsupportedNfc() {
        let capabilities = OfflineNoteTransferCapabilities(
            qrStreaming: true,
            nfc: .unavailable("missing HCE entitlement"),
            nearby: true
        )

        XCTAssertEqual(IrohaOfflineTransferTransportKind.available(in: capabilities), [.qr, .nearby])
    }

    func testTransportAvailabilityShowsNfcOnlyWhenSupported() {
        let capabilities = OfflineNoteTransferCapabilities(
            qrStreaming: true,
            nfc: .supported,
            nearby: false
        )

        XCTAssertEqual(IrohaOfflineTransferTransportKind.available(in: capabilities), [.qr, .nfc])
    }

    func testOfflineCashFlowViewCompilesWithUnsupportedNfcCapabilities() {
        let capabilities = OfflineNoteTransferCapabilities(
            qrStreaming: true,
            nfc: .unavailable("missing HCE entitlement"),
            nearby: true
        )
        let state = IrohaOfflineCashFlowState(
            phase: .ready,
            totalBalance: "1000",
            spendableBalance: "900",
            pendingBalance: "100",
            pendingReceiptCount: 1
        )

        let view = IrohaOfflineCashFlowView(
            state: state,
            capabilities: capabilities,
            selectedTransport: .qr
        ) { _ in }

        XCTAssertFalse(String(describing: type(of: view)).isEmpty)
        XCTAssertEqual(IrohaOfflineTransferTransportKind.available(in: capabilities), [.qr, .nearby])
    }

    private static func validReceiveRequestTextPayload() throws -> String {
        try OfflineNoteTransferTextPayloadCodec.encodeReceiveRequest(
            OfflineReceiveRequestPayload(
                invoiceId: "invoice-1",
                accountId: "recipient@pob.cbsi",
                assetDefinitionId: "sbd",
                amount: "1.00",
                recipientKeyCertificate: OfflineCompactKeyCertificate(
                    platform: "ios",
                    keyId: "key-1",
                    deviceId: "device-1",
                    accountId: "recipient@pob.cbsi",
                    publicKey: Data(repeating: 0x11, count: 32).base64EncodedString(),
                    issuerSignatureBase64: Data(repeating: 0x22, count: 64).base64EncodedString()
                ),
                generatedAtMs: 1_706_000_000_000,
                displayTtlMs: 300_000,
                chainId: "cbsi-minamoto",
                assetId: "sbd#cbsi",
                outputCommitment: String(repeating: "a", count: 64)
            )
        )
    }

    private static func base64Url(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }
}
