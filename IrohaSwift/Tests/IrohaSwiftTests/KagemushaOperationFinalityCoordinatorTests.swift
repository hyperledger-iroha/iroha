import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaOperationFinalityCoordinatorTests: XCTestCase {
    func testConfigurationRejectsUnboundedAndBusyLoopValues() throws {
        XCTAssertThrowsError(
            try KagemushaOperationFinalityConfiguration(
                maximumPollAttempts: 0,
                pollingIntervalNanoseconds: 1_000_000_000
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaOperationFinalityError,
                .invalidConfiguration("maximumPollAttempts")
            )
        }
        XCTAssertThrowsError(
            try KagemushaOperationFinalityConfiguration(
                maximumPollAttempts:
                    KagemushaOperationFinalityConfiguration
                        .maximumSupportedPollAttempts + 1,
                pollingIntervalNanoseconds: 1_000_000_000
            )
        )
        XCTAssertThrowsError(
            try KagemushaOperationFinalityConfiguration(
                maximumPollAttempts: 1,
                pollingIntervalNanoseconds: 0
            )
        )
        XCTAssertThrowsError(
            try KagemushaOperationFinalityConfiguration(
                maximumPollAttempts: 1,
                pollingIntervalNanoseconds: 999_999_999
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaOperationFinalityError,
                .invalidConfiguration("pollingIntervalNanoseconds")
            )
        }
        XCTAssertThrowsError(
            try KagemushaOperationFinalityConfiguration(
                maximumPollAttempts: 1,
                pollingIntervalNanoseconds:
                    KagemushaOperationFinalityConfiguration
                        .maximumSupportedPollingIntervalNanoseconds + 1
            )
        )
        XCTAssertEqual(
            KagemushaOperationFinalityConfiguration.production.maximumPollAttempts,
            120
        )
        XCTAssertEqual(
            KagemushaOperationFinalityConfiguration.production
                .pollingIntervalNanoseconds,
            1_000_000_000
        )
        XCTAssertEqual(
            KagemushaOperationFinalityConfiguration.production
                .overallTimeoutNanoseconds,
            180_000_000_000
        )
        XCTAssertNoThrow(
            try KagemushaOperationFinalityConfiguration(
                maximumPollAttempts: 5,
                pollingIntervalNanoseconds: 60_000_000_000
            )
        )
        XCTAssertThrowsError(
            try KagemushaOperationFinalityConfiguration(
                maximumPollAttempts: 6,
                pollingIntervalNanoseconds: 60_000_000_000
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaOperationFinalityError,
                .invalidConfiguration("scheduledPollingNanoseconds")
            )
        }
        XCTAssertThrowsError(
            try KagemushaOperationFinalityConfiguration(
                maximumPollAttempts: 2,
                pollingIntervalNanoseconds: 1_000_000_000,
                overallTimeoutNanoseconds: 1_999_999_999
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaOperationFinalityError,
                .invalidConfiguration("overallTimeoutNanoseconds")
            )
        }
        XCTAssertThrowsError(
            try KagemushaOperationFinalityConfiguration(
                maximumPollAttempts: 1,
                pollingIntervalNanoseconds: 1_000_000_000,
                overallTimeoutNanoseconds:
                    KagemushaOperationFinalityConfiguration
                        .maximumSupportedOverallTimeoutNanoseconds + 1
            )
        )
    }

    func testAttemptFailureSanitizerRejectsHostileCodeAndBoundsUnicode() {
        let failure = KagemushaOperationAttemptFailure(
            code: " BAD\u{0000}CODE ",
            message: " \u{0000}" + String(repeating: "é", count: 2_000) + "\u{0007} "
        )

        XCTAssertEqual(failure.code, "offline_operation_rejected")
        XCTAssertLessThanOrEqual(
            failure.message.utf8.count,
            KagemushaOperationAttemptFailure.maximumMessageUTF8Bytes
        )
        XCTAssertFalse(
            failure.message.unicodeScalars.contains {
                CharacterSet.controlCharacters.contains($0)
            }
        )
        XCTAssertNotEqual(failure.message.last, "�")
    }

    func testDefinitiveFailureCodableIsStrictAndDropsBinaryDiagnostics() throws {
        let disposition = KagemushaSubmissionFailureClassifier
            .classify(
            ToriiClientError.httpStatus(
                code: 400,
                message: "body_hex=4e525430deadbeef...",
                rejectCode: "offline_redeem_invalid"
            ),
            target: .offlineRedeem
        )
        guard case let .definitivePreAdmission(failure) = disposition else {
            return XCTFail("canonical 400 must be definitive")
        }
        XCTAssertNil(failure.message)
        XCTAssertEqual(failure.target, .offlineRedeem)
        XCTAssertEqual(failure.rejectCode, "offline_redeem_invalid")

        let encoded = try JSONEncoder().encode(failure)
        XCTAssertEqual(try JSONDecoder().decode(
            KagemushaDefinitiveSubmissionFailure.self,
            from: encoded
        ), failure)
        XCTAssertThrowsError(try JSONDecoder().decode(
            KagemushaDefinitiveSubmissionFailure.self,
            from: Data("""
            {"target":"offline_redeem","status_code":400,"reject_code":"offline_redeem_invalid","message":null,"extra":true}
            """.utf8)
        ))
        for (status, rejectCode) in [
            (404, "offline_redeem_invalid"),
            (425, "offline_redeem_invalid"),
            (460, "offline_redeem_invalid"),
            (499, "offline_redeem_invalid"),
            (400, "operation_id_conflict"),
            (409, "offline_redeem_invalid"),
            (413, "offline_redeem_invalid"),
            (415, "request_payload_too_large"),
            (422, "request_content_type_missing"),
        ] {
            XCTAssertThrowsError(try JSONDecoder().decode(
                KagemushaDefinitiveSubmissionFailure.self,
                from: Data("""
                {"target":"offline_redeem","status_code":\(status),"reject_code":"\(rejectCode)"}
                """.utf8)
            ))
            XCTAssertThrowsError(
                try KagemushaDefinitiveSubmissionFailure(
                    target: .offlineRedeem,
                    statusCode: status,
                    rejectCode: rejectCode,
                    message: nil
                )
            )
        }
    }

    func testPipelineTransactionClassifierUsesOnlyProvenNewAdmissionPairs() throws {
        let definitivePairs: [(Int, String)] = [
            (400, "invalid_transaction_payload"),
            (400, "transaction_rejected"),
            (400, "PRTRY:TX_SIGNATURE_INVALID"),
            (400, "PRTRY:TX_SIGNATURE_MISSING"),
            (400, "ED07"),
            (403, "PRTRY:QUEUE_GOVERNANCE_REJECTED"),
            (403, "PRTRY:CONFIDENTIAL_POLICY_REJECTED"),
        ]
        for (status, rejectCode) in definitivePairs {
            let disposition = KagemushaSubmissionFailureClassifier.classify(
                ToriiClientError.httpStatus(
                    code: status,
                    message: "rejected before insertion",
                    rejectCode: rejectCode
                ),
                target: .signedTransaction
            )
            guard case let .definitivePreAdmission(failure) = disposition else {
                XCTFail("expected definitive transaction pair: \(status) \(rejectCode)")
                continue
            }
            XCTAssertEqual(failure.target, .signedTransaction)
            XCTAssertEqual(failure.statusCode, status)
            XCTAssertEqual(failure.rejectCode, rejectCode)
            XCTAssertEqual(
                try JSONDecoder().decode(
                    KagemushaDefinitiveSubmissionFailure.self,
                    from: JSONEncoder().encode(failure)
                ),
                failure
            )
        }

        let ambiguousPairs: [(Int, String?)] = [
            (400, nil),
            (400, "unknown_bad_request"),
            (400, "offline_redeem_invalid"),
            (400, "PRTRY:NTS_UNHEALTHY"),
            (400, "PRTRY:ROUTE_UNRESOLVED"),
            (409, "PRTRY:ALREADY_COMMITTED"),
            (409, "PRTRY:ALREADY_ENQUEUED"),
            (409, "operation_id_conflict"),
            (413, nil),
            (415, nil),
            (429, "PRTRY:QUEUE_RATE"),
            (503, "transaction_admission_worker_failed"),
        ]
        for (status, rejectCode) in ambiguousPairs {
            XCTAssertEqual(
                KagemushaSubmissionFailureClassifier.classify(
                    ToriiClientError.httpStatus(
                        code: status,
                        message: nil,
                        rejectCode: rejectCode
                    ),
                    target: .signedTransaction
                ),
                .ambiguous,
                "must reconcile transaction pair: \(status) \(rejectCode ?? "nil")"
            )
        }
    }

    func testOperationClassifierDoesNotBleedKindOrTransactionOnlyCodes() {
        for target in [
            KagemushaSubmissionTarget.offlineTopUp,
            .offlineRedeem,
        ] {
            guard case .definitivePreAdmission =
                KagemushaSubmissionFailureClassifier.classify(
                    ToriiClientError.httpStatus(
                        code: 400,
                        message: nil,
                        rejectCode: "offline_wrong_network"
                    ),
                    target: target
                ) else {
                return XCTFail("exact-network rejection must be definitive for \(target)")
            }
            guard case .definitivePreAdmission =
                KagemushaSubmissionFailureClassifier.classify(
                    ToriiClientError.httpStatus(
                        code: 409,
                        message: nil,
                        rejectCode: "offline_operation_retry_exhausted"
                    ),
                    target: target
                ) else {
                return XCTFail("retry exhaustion must be definitive for \(target)")
            }
        }
        let redeemOnly = ToriiClientError.httpStatus(
            code: 400,
            message: nil,
            rejectCode: "offline_redeem_invalid"
        )
        XCTAssertEqual(
            KagemushaSubmissionFailureClassifier.classify(
                redeemOnly,
                target: .offlineTopUp
            ),
            .ambiguous
        )
        guard case .definitivePreAdmission =
            KagemushaSubmissionFailureClassifier.classify(
                redeemOnly,
                target: .offlineRedeem
            ) else {
            return XCTFail("redeem-only code must be definitive for redeem")
        }
        XCTAssertEqual(
            KagemushaSubmissionFailureClassifier.classify(
                ToriiClientError.httpStatus(
                    code: 400,
                    message: nil,
                    rejectCode: "invalid_transaction_payload"
                ),
                target: .offlineRedeem
            ),
            .ambiguous
        )
        for target in [
            KagemushaSubmissionTarget.offlineTopUp,
            .offlineRedeem,
            .signedTransaction,
        ] {
            XCTAssertEqual(
                KagemushaSubmissionFailureClassifier.classify(
                    ToriiClientError.httpStatus(
                        code: 409,
                        message: nil,
                        rejectCode: "PRTRY:ALREADY_ENQUEUED"
                    ),
                    target: target
                ),
                .ambiguous
            )
            XCTAssertEqual(
                KagemushaSubmissionFailureClassifier.classify(
                    ToriiClientError.httpStatus(
                        code: 400,
                        message: nil,
                        rejectCode: "PRTRY:ROUTE_UNRESOLVED"
                    ),
                    target: target
                ),
                .ambiguous
            )
            XCTAssertEqual(
                KagemushaSubmissionFailureClassifier.classify(
                    ToriiClientError.httpStatus(
                        code: 400,
                        message: nil,
                        rejectCode: "PRTRY:NTS_UNHEALTHY"
                    ),
                    target: target
                ),
                .ambiguous
            )
        }
    }

    func testInvalidOperationIDFailsBeforeAnyCallerSideEffect() async throws {
        let harness = Harness(
            operationId: "invalid",
            kind: .redeem,
            steps: []
        )
        do {
            _ = try await harness.run()
            XCTFail("invalid operation id must fail")
        } catch KagemushaOperationError.invalidField(let field) {
            XCTAssertEqual(field, "operation_id")
        }
        XCTAssertEqual(harness.trace, [])
    }

    func testOverallDeadlineBoundsStatusPollingWithMonotonicTime() async throws {
        let harness = Harness(
            operationId: id(0x0f),
            kind: .redeem,
            steps: [
                .failure(ToriiClientError.transport(URLError(.timedOut))),
            ]
        )
        harness.configuration = try .init(
            maximumPollAttempts: 1,
            pollingIntervalNanoseconds: 1_000_000_000,
            overallTimeoutNanoseconds: 1_000_000_000
        )

        do {
            _ = try await harness.run()
            XCTFail("overall deadline must stop before another network call")
        } catch KagemushaOperationFinalityError.overallDeadlineExceeded {
            // The injected monotonic clock advances by the scheduled sleep.
        }
        XCTAssertEqual(harness.statusFetchCount, 1)
        XCTAssertEqual(harness.trace, ["status", "sleep"])
        XCTAssertEqual(harness.sleepValues, [1_000_000_000])
    }

    func testOnlyAuthoritative404AllowsDurableMarkerThenSubmission() async throws {
        for kind in [KagemushaOperationKind.topUp, .redeem] {
            let operationId = id(kind == .topUp ? 0x11 : 0x12)
            let transactionHash = id(kind == .topUp ? 0x21 : 0x22)
            let harness = Harness(
                operationId: operationId,
                kind: kind,
                steps: [
                    .failure(notFound),
                    .status(try pending(operationId, kind, transactionHash)),
                ]
            )
            harness.submitHook = {
                XCTAssertTrue(harness.journal.attempted)
                return try self.reference(operationId, kind, transactionHash)
            }

            do {
                _ = try await harness.run()
                XCTFail("one pending poll must reach the configured deadline")
            } catch KagemushaOperationFinalityError.pollingDeadlineExceeded {
                // Expected.
            }

            XCTAssertEqual(harness.submissionCount, 1)
            XCTAssertEqual(harness.journal.attemptCount, 1)
            XCTAssertEqual(harness.journal.transactionHash, transactionHash)
            XCTAssertEqual(
                Array(harness.trace.prefix(5)),
                ["status", "revalidate", "persist_attempt", "submit", "persist_accept"]
            )
        }
    }

    func testUnclassifiedAndProxy404NeverAuthorizeSubmission() async throws {
        for rejectCode in [nil, "route_not_found"] {
            let operationId = id(0x13)
            let harness = Harness(
                operationId: operationId,
                kind: .redeem,
                steps: [
                    .failure(
                        ToriiClientError.httpStatus(
                            code: 404,
                            message: "not the canonical absence signal",
                            rejectCode: rejectCode
                        )
                    ),
                ]
            )

            do {
                _ = try await harness.run()
                XCTFail("unclassified 404 must remain terminal")
            } catch ToriiClientError.httpStatus(let code, _, let actualRejectCode) {
                XCTAssertEqual(code, 404)
                XCTAssertEqual(actualRejectCode, rejectCode)
            }

            XCTAssertEqual(harness.trace, ["status"])
            XCTAssertEqual(harness.submissionCount, 0)
            XCTAssertFalse(harness.journal.attempted)
        }
    }

    func testStatusFirstAppliedSkipsReadinessMarkerAndSubmission() async throws {
        let operationId = id(0x31)
        let transactionHash = id(0x32)
        let harness = Harness(
            operationId: operationId,
            kind: .redeem,
            steps: [.status(try appliedRedeem(operationId, transactionHash))]
        )

        let resolution = try await harness.run()

        guard case .applied = resolution.outcome else {
            return XCTFail("expected applied status")
        }
        XCTAssertEqual(harness.submissionCount, 0)
        XCTAssertFalse(harness.journal.attempted)
        XCTAssertEqual(harness.journal.transactionHash, transactionHash)
        XCTAssertEqual(harness.trace, ["status", "persist_observation"])
    }

    func testInitialTransientStatusFailureNeverAuthorizesSubmission() async throws {
        let operationId = id(0x41)
        let transactionHash = id(0x42)
        for initialFailure in [
            ToriiClientError.transport(URLError(.timedOut)),
            ToriiClientError.httpStatus(
                code: 503,
                message: nil,
                rejectCode: "offline_topup_finality_proof_unavailable"
            ),
            ToriiClientError.httpStatus(
                code: 500,
                message: nil,
                rejectCode: "internal_server_error"
            ),
            ToriiClientError.httpStatus(
                code: 502,
                message: nil,
                rejectCode: "bad_gateway"
            ),
            ToriiClientError.httpStatus(
                code: 504,
                message: nil,
                rejectCode: "gateway_timeout"
            ),
        ] {
            let harness = Harness(
                operationId: operationId,
                kind: .redeem,
                steps: [
                    .failure(initialFailure),
                    .status(try appliedRedeem(operationId, transactionHash)),
                ]
            )

            let resolution = try await harness.run()

            guard case .applied = resolution.outcome else {
                return XCTFail("status-only recovery should reach applied")
            }
            XCTAssertEqual(harness.submissionCount, 0)
            XCTAssertFalse(harness.journal.attempted)
            XCTAssertEqual(
                harness.trace,
                ["status", "sleep", "status", "persist_observation"]
            )
        }
    }

    func testAmbiguousSubmissionIsPolledWithoutSecondPost() async throws {
        let operationId = id(0x51)
        let transactionHash = id(0x52)
        let harness = Harness(
            operationId: operationId,
            kind: .redeem,
            steps: [
                .failure(notFound),
                .status(try appliedRedeem(operationId, transactionHash)),
            ]
        )
        harness.submitHook = {
            throw ToriiClientError.transport(
                URLError(.networkConnectionLost)
            )
        }

        let resolution = try await harness.run()

        guard case .applied = resolution.outcome else {
            return XCTFail("ambiguous POST must resolve through status")
        }
        XCTAssertEqual(harness.submissionCount, 1)
        XCTAssertEqual(harness.journal.attemptCount, 1)
        XCTAssertEqual(
            harness.trace,
            [
                "status", "revalidate", "persist_attempt", "submit",
                "sleep", "status", "persist_observation",
            ]
        )
    }

    func testTransientAdmissionCodesRemainStatusOnlyAndRecoverOnRestart() async throws {
        for (index, rejectCode) in [
            "PRTRY:NTS_UNHEALTHY",
            "PRTRY:ROUTE_UNRESOLVED",
        ].enumerated() {
            let operationId = id(UInt8(0x55 + index * 2))
            let transactionHash = id(UInt8(0x56 + index * 2))
            let first = Harness(
                operationId: operationId,
                kind: .redeem,
                steps: [
                    .failure(notFound),
                    .failure(
                        ToriiClientError.transport(URLError(.timedOut))
                    ),
                ]
            )
            first.submitHook = {
                throw ToriiClientError.httpStatus(
                    code: 400,
                    message: "transient pre-insertion admission failure",
                    rejectCode: rejectCode
                )
            }

            do {
                _ = try await first.run()
                XCTFail("status-only polling must remain bounded")
            } catch KagemushaOperationFinalityError.pollingDeadlineExceeded {
                // The durable attempt remains available for restart recovery.
            }
            XCTAssertEqual(first.submissionCount, 1)
            XCTAssertEqual(first.journal.attemptCount, 1)
            XCTAssertNil(first.journal.definitiveSubmissionFailure)
            XCTAssertEqual(
                first.trace,
                [
                    "status", "revalidate", "persist_attempt", "submit",
                    "sleep", "status",
                ]
            )

            let restarted = Harness(
                operationId: operationId,
                kind: .redeem,
                steps: [
                    .status(
                        try appliedRedeem(operationId, transactionHash)
                    ),
                ],
                initialJournal: first.journal
            )
            let resolution = try await restarted.run()

            guard case .applied = resolution.outcome else {
                return XCTFail("restart must reconcile status first")
            }
            XCTAssertEqual(restarted.submissionCount, 0)
            XCTAssertEqual(
                restarted.trace,
                ["status", "persist_observation"]
            )
        }
    }

    func testPostAcceptanceServerFailureIsStatusPolledWithoutReplay() async throws {
        let operationId = id(0x53)
        let transactionHash = id(0x54)
        for failure in [
            ToriiClientError.httpStatus(
                code: 503,
                message: "queue accepted but admission response was inconsistent",
                rejectCode: "offline_operation_admission_inconsistent"
            ),
            ToriiClientError.httpStatus(
                code: 500,
                message: "gateway lost the accepted response",
                rejectCode: nil
            ),
            ToriiClientError.httpStatus(
                code: 408,
                message: "gateway timeout",
                rejectCode: nil
            ),
            ToriiClientError.httpStatus(
                code: 429,
                message: "rate limited after admission",
                rejectCode: nil
            ),
            ToriiClientError.httpStatus(
                code: 200,
                message: "unexpected success framing",
                rejectCode: nil
            ),
            ToriiClientError.httpStatus(
                code: 302,
                message: "gateway redirect after forwarding",
                rejectCode: nil
            ),
            ToriiClientError.httpStatus(
                code: 499,
                message: "proxy closed the request",
                rejectCode: "client_closed_request"
            ),
        ] {
            let harness = Harness(
                operationId: operationId,
                kind: .redeem,
                steps: [
                    .failure(notFound),
                    .status(try appliedRedeem(operationId, transactionHash)),
                ]
            )
            harness.submitHook = { throw failure }

            let resolution = try await harness.run()

            guard case .applied = resolution.outcome else {
                return XCTFail("ambiguous HTTP failure must resolve by status")
            }
            XCTAssertEqual(harness.submissionCount, 1)
            XCTAssertEqual(harness.statusFetchCount, 2)
            XCTAssertNil(harness.journal.definitiveSubmissionFailure)
        }
    }

    func testDefinitiveHTTPSubmissionFailureIsPersistedAndNeverPolled() async throws {
        let operationId = id(0x61)
        let harness = Harness(
            operationId: operationId,
            kind: .redeem,
            steps: [.failure(notFound)]
        )
        harness.submitHook = {
            throw ToriiClientError.httpStatus(
                code: 400,
                message: "rejected",
                rejectCode: "offline_redeem_invalid"
            )
        }

        let resolution = try await harness.run()
        guard case let .definitiveSubmissionFailure(failure) = resolution.outcome else {
            return XCTFail("expected definitive pre-admission outcome")
        }
        XCTAssertEqual(failure.statusCode, 400)
        XCTAssertEqual(failure.rejectCode, "offline_redeem_invalid")
        XCTAssertEqual(failure.message, "rejected")
        XCTAssertEqual(resolution.state.definitiveSubmissionFailure, failure)
        XCTAssertEqual(harness.submissionCount, 1)
        XCTAssertEqual(
            harness.trace,
            [
                "status", "revalidate", "persist_attempt", "submit",
                "persist_definitive_failure",
            ]
        )
    }

    func testHostileDefinitiveFailureMetadataCannotBypassPersistence() async throws {
        let operationId = id(0x62)
        let harness = Harness(
            operationId: operationId,
            kind: .redeem,
            steps: [.failure(notFound)]
        )
        harness.submitHook = {
            throw ToriiClientError.httpStatus(
                code: 400,
                message: " \u{0000}" + String(repeating: "é", count: 2_000),
                rejectCode: "offline_redeem_invalid"
            )
        }

        let resolution = try await harness.run()
        guard case let .definitiveSubmissionFailure(failure) = resolution.outcome else {
            return XCTFail("hostile metadata must still produce a durable outcome")
        }
        XCTAssertEqual(failure.rejectCode, "offline_redeem_invalid")
        XCTAssertLessThanOrEqual(
            failure.message?.utf8.count ?? 0,
            KagemushaDefinitiveSubmissionFailure.maximumMessageUTF8Bytes
        )
        XCTAssertEqual(harness.journal.definitiveSubmissionFailure, failure)
        XCTAssertEqual(harness.trace.last, "persist_definitive_failure")
    }

    func testPersistedDefinitiveFailureShortCircuitsRestartBeforeNetwork() async throws {
        let operationId = id(0x63)
        let first = Harness(
            operationId: operationId,
            kind: .redeem,
            steps: [.failure(notFound)]
        )
        first.submitHook = {
            throw ToriiClientError.httpStatus(
                code: 409,
                message: "operation conflict",
                rejectCode: "operation_id_conflict"
            )
        }
        let firstResolution = try await first.run()
        guard case .definitiveSubmissionFailure = firstResolution.outcome else {
            return XCTFail("first invocation must freeze the failure")
        }

        let restarted = Harness(
            operationId: operationId,
            kind: .redeem,
            steps: [.failure(notFound)],
            initialJournal: firstResolution.state
        )
        restarted.submitHook = {
            XCTFail("persisted definitive failure must not be replayed")
            throw FinalityHarnessError.unexpectedSubmission
        }
        let restartedResolution = try await restarted.run()

        XCTAssertEqual(restartedResolution.outcome, firstResolution.outcome)
        XCTAssertEqual(restarted.trace, [])
        XCTAssertEqual(restarted.statusFetchCount, 0)
        XCTAssertEqual(restarted.submissionCount, 0)
    }

    func testSubstitutedAcceptedReferenceFailsBeforeAcceptanceOrPolling() async throws {
        let operationId = id(0x71)
        let otherOperationId = id(0x72)
        let transactionHash = id(0x73)
        let harness = Harness(
            operationId: operationId,
            kind: .topUp,
            steps: [.failure(notFound)]
        )
        harness.submitHook = {
            try self.reference(otherOperationId, .topUp, transactionHash)
        }

        do {
            _ = try await harness.run()
            XCTFail("substituted reference must fail continuity")
        } catch KagemushaOperationFinalityError.continuityViolation(let field) {
            XCTAssertEqual(field, "accepted operation reference")
        }
        XCTAssertNil(harness.journal.transactionHash)
        XCTAssertEqual(
            harness.trace,
            ["status", "revalidate", "persist_attempt", "submit"]
        )
    }

    func testAppliedWinnerMayAdvanceTransactionHash() async throws {
        let operationId = id(0x81)
        let firstHash = id(0x82)
        let winningHash = id(0x83)
        let harness = Harness(
            operationId: operationId,
            kind: .redeem,
            steps: [
                .status(try pending(operationId, .redeem, firstHash)),
                .status(try appliedRedeem(operationId, winningHash)),
            ]
        )

        let resolution = try await harness.run()

        guard case .applied = resolution.outcome else {
            return XCTFail("global Applied winner must be authoritative")
        }
        XCTAssertEqual(harness.journal.transactionHash, winningHash)
        XCTAssertEqual(
            harness.trace,
            [
                "status", "persist_observation", "sleep", "status",
                "persist_observation",
            ]
        )
    }

    func testSubmittedTimestampSubstitutionFailsBeforeSecondPersistence() async throws {
        let operationId = id(0x84)
        let transactionHash = id(0x85)
        let harness = Harness(
            operationId: operationId,
            kind: .redeem,
            steps: [
                .status(try pending(
                    operationId,
                    .redeem,
                    transactionHash,
                    submittedAtMs: 7
                )),
                .status(try pending(
                    operationId,
                    .redeem,
                    transactionHash,
                    submittedAtMs: 8
                )),
            ],
            expectedSubmittedAtMs: 7
        )

        do {
            _ = try await harness.run()
            XCTFail("submitted timestamp substitution must fail")
        } catch KagemushaOperationFinalityError.continuityViolation(let field) {
            XCTAssertEqual(field, "submitted timestamp")
        }
        XCTAssertEqual(harness.journal.transactionHash, transactionHash)
        XCTAssertEqual(harness.journal.submittedAtMs, 7)
        XCTAssertEqual(
            harness.trace,
            ["status", "persist_observation", "sleep", "status"]
        )
    }

    func testRestartContinuitySeedAllowsNewPendingCarrierHash() async throws {
        let operationId = id(0x86)
        let previousHash = id(0x87)
        let newerHash = id(0x88)
        let harness = Harness(
            operationId: operationId,
            kind: .redeem,
            steps: [
                .status(try pending(
                    operationId,
                    .redeem,
                    newerHash,
                    submittedAtMs: 9
                )),
                .status(try appliedRedeem(operationId, newerHash)),
            ],
            expectedSubmittedAtMs: 9
        )
        harness.continuity = .accepted(try reference(
            operationId,
            .redeem,
            previousHash,
            submittedAtMs: 9
        ))

        let resolution = try await harness.run()

        guard case .applied = resolution.outcome else {
            return XCTFail("new pending carrier hash must remain authoritative")
        }
        XCTAssertEqual(harness.journal.transactionHash, newerHash)
        XCTAssertEqual(harness.journal.submittedAtMs, 9)
    }

    func testRestartContinuitySeedRejectsNewHashTimestampSubstitution() async throws {
        let operationId = id(0x86)
        let previousHash = id(0x87)
        let newerHash = id(0x88)
        let harness = Harness(
            operationId: operationId,
            kind: .redeem,
            steps: [.status(try pending(
                operationId,
                .redeem,
                newerHash,
                submittedAtMs: 10
            ))],
            expectedSubmittedAtMs: 9
        )
        harness.continuity = .accepted(try reference(
            operationId,
            .redeem,
            previousHash,
            submittedAtMs: 9
        ))

        do {
            _ = try await harness.run()
            XCTFail("new carrier hash must not replace the submitted timestamp")
        } catch KagemushaOperationFinalityError.continuityViolation(let field) {
            XCTAssertEqual(field, "submitted timestamp")
        }
        XCTAssertEqual(harness.trace, ["status"])
        XCTAssertNil(harness.journal.transactionHash)
        XCTAssertNil(harness.journal.submittedAtMs)
    }

    func testRestartContinuitySeedRejectsSameHashTimestampSubstitution() async throws {
        let operationId = id(0x86)
        let transactionHash = id(0x87)
        let harness = Harness(
            operationId: operationId,
            kind: .redeem,
            steps: [.status(try pending(
                operationId,
                .redeem,
                transactionHash,
                submittedAtMs: 10
            ))],
            expectedSubmittedAtMs: 9
        )
        harness.continuity = .accepted(try reference(
            operationId,
            .redeem,
            transactionHash,
            submittedAtMs: 9
        ))

        do {
            _ = try await harness.run()
            XCTFail("same-attempt timestamp substitution must fail")
        } catch KagemushaOperationFinalityError.continuityViolation(let field) {
            XCTAssertEqual(field, "submitted timestamp")
        }
        XCTAssertEqual(harness.trace, ["status"])
        XCTAssertNil(harness.journal.transactionHash)
    }

    func testRestartContinuityReferenceIdentityFailsBeforeAnySideEffect() async throws {
        let operationId = id(0x89)
        let validHash = id(0x8a)
        let cases = [
            try reference(id(0x8b), .redeem, validHash),
            try reference(operationId, .topUp, validHash),
        ]
        for acceptedReference in cases {
            let harness = Harness(
                operationId: operationId,
                kind: .redeem,
                steps: []
            )
            harness.continuity = .accepted(acceptedReference)
            do {
                _ = try await harness.run()
                XCTFail("mismatched accepted reference must fail")
            } catch KagemushaOperationFinalityError.continuityViolation {
                // Expected.
            }
            XCTAssertEqual(harness.trace, [])
        }
    }

    func testAcceptedReferenceSupportsForeignAppliedWinner() async throws {
        let operationId = id(0x8b)
        let acceptedHash = id(0x8c)
        let winningHash = id(0x8d)
        let harness = Harness(
            operationId: operationId,
            kind: .redeem,
            steps: [.status(try appliedRedeem(operationId, winningHash))]
        )
        harness.continuity = .accepted(try reference(
            operationId,
            .redeem,
            acceptedHash
        ))

        let resolution = try await harness.run()

        guard case .applied = resolution.outcome else {
            return XCTFail("terminal status should bind to the accepted reference")
        }
        XCTAssertEqual(harness.journal.transactionHash, winningHash)
        XCTAssertNil(harness.journal.submittedAtMs)
    }

    func testAcceptedReferenceMakesCanonical404FailClosedWithoutPost() async throws {
        let harness = Harness(
            operationId: id(0x8d),
            kind: .redeem,
            steps: [.failure(notFound)]
        )
        harness.continuity = .accepted(try reference(
            harness.operationId,
            .redeem,
            id(0x8e)
        ))
        harness.submitHook = {
            XCTFail("accepted operation must never be submitted again")
            throw FinalityHarnessError.unexpectedSubmission
        }

        do {
            _ = try await harness.run()
            XCTFail("missing accepted status must fail continuity")
        } catch KagemushaOperationFinalityError.continuityViolation(let field) {
            XCTAssertEqual(field, "accepted operation status missing")
        }
        XCTAssertEqual(harness.trace, ["status"])
        XCTAssertEqual(harness.submissionCount, 0)
        XCTAssertFalse(harness.journal.attempted)
    }

    func testWrongStatusIdentityOrKindFailsWithoutPersistence() async throws {
        let operationId = id(0x91)
        let invalidStatuses = [
            try pending(id(0x93), .topUp, id(0x92)),
            try pending(operationId, .redeem, id(0x92)),
        ]

        for status in invalidStatuses {
            let harness = Harness(
                operationId: operationId,
                kind: .topUp,
                steps: [.status(status)]
            )
            do {
                _ = try await harness.run()
                XCTFail("wrong operation identity or kind must fail")
            } catch KagemushaOperationFinalityError.continuityViolation(let field) {
                XCTAssertEqual(field, "pending identity or kind")
            }
            XCTAssertEqual(harness.trace, ["status"])
            XCTAssertNil(harness.journal.transactionHash)
        }
    }

    func testPolledRejectedAttemptIsBoundedPersistedThenSurfaced() async throws {
        let operationId = id(0xa1)
        let transactionHash = id(0xa2)
        let rejectedHash = id(0xa3)
        let oversizedMessage = String(repeating: "é", count: 2_000)
        let harness = Harness(
            operationId: operationId,
            kind: .redeem,
            steps: [
                .status(try pending(operationId, .redeem, transactionHash)),
                .status(
                    try rejected(
                        operationId,
                        .redeem,
                        rejectedHash,
                        message: oversizedMessage
                    )
                ),
            ]
        )

        let resolution = try await harness.run()
        guard case let .rejectedAttempt(rejection) = resolution.outcome else {
            return XCTFail("expected retryable rejected-attempt resolution")
        }
        XCTAssertEqual(rejection.failure.code, "offline_operation_rejected")
        XCTAssertLessThanOrEqual(
            rejection.failure.message.utf8.count,
            KagemushaOperationAttemptFailure.maximumMessageUTF8Bytes
        )
        XCTAssertTrue(resolution.state.rejected)
        XCTAssertTrue(harness.journal.rejected)
        XCTAssertEqual(harness.journal.transactionHash, rejectedHash)
        XCTAssertEqual(harness.submissionCount, 0)
        XCTAssertEqual(
            harness.trace,
            [
                "status", "persist_observation", "sleep", "status",
                "persist_observation", "persist_rejection",
            ]
        )
    }

    func testInitialRejectedAttemptAuthorizesOneDeterministicRetry() async throws {
        let operationId = id(0xa4)
        let rejectedHash = id(0xa5)
        let retryHash = id(0xa6)
        let harness = Harness(
            operationId: operationId,
            kind: .redeem,
            steps: [
                .status(try rejected(operationId, .redeem, rejectedHash)),
                .status(try appliedRedeem(operationId, retryHash)),
            ]
        )
        harness.submitHook = {
            try self.reference(
                operationId,
                .redeem,
                retryHash,
                submittedAtMs: 1
            )
        }

        let resolution = try await harness.run()

        guard case .applied = resolution.outcome else {
            return XCTFail("exact retry must reconcile to Applied")
        }
        XCTAssertEqual(harness.submissionCount, 1)
        XCTAssertEqual(harness.journal.attemptCount, 1)
        XCTAssertEqual(harness.journal.transactionHash, retryHash)
        XCTAssertEqual(harness.journal.submittedAtMs, 1)
        XCTAssertTrue(harness.journal.rejected)
        XCTAssertEqual(
            harness.trace,
            [
                "status", "persist_observation", "persist_rejection",
                "revalidate", "persist_attempt", "submit", "persist_accept",
                "sleep", "status", "persist_observation",
            ]
        )
    }

    func testPollingIsExactlyBoundedAndSleepsOnlyBetweenAttempts() async throws {
        let operationId = id(0xb1)
        let transactionHash = id(0xb2)
        let status = try pending(operationId, .redeem, transactionHash)
        let harness = Harness(
            operationId: operationId,
            kind: .redeem,
            steps: [.status(status), .status(status), .status(status), .status(status)]
        )
        harness.configuration = try .init(
            maximumPollAttempts: 3,
            pollingIntervalNanoseconds: 1_000_000_000
        )

        do {
            _ = try await harness.run()
            XCTFail("pending operation must time out")
        } catch KagemushaOperationFinalityError.pollingDeadlineExceeded {
            // Expected.
        }
        XCTAssertEqual(harness.statusFetchCount, 4)
        XCTAssertEqual(
            harness.sleepValues,
            [1_000_000_000, 1_000_000_000, 1_000_000_000]
        )
        XCTAssertEqual(
            harness.trace.filter { $0 == "persist_observation" }.count,
            4
        )
    }

    func testCancellationDuringAttemptPersistencePreventsPost() async throws {
        let operationId = id(0xc1)
        let harness = Harness(
            operationId: operationId,
            kind: .redeem,
            steps: [.failure(notFound)]
        )
        harness.cancelWhileMarkingAttempt = true

        do {
            _ = try await harness.run()
            XCTFail("cancelled task must stop before POST")
        } catch is CancellationError {
            // Expected.
        }
        XCTAssertTrue(harness.journal.attempted)
        XCTAssertEqual(harness.submissionCount, 0)
        XCTAssertEqual(
            harness.trace,
            ["status", "revalidate", "persist_attempt"]
        )
    }

    func testRevalidationAndAttemptPersistenceFailuresPreventSubmission() async throws {
        for phase in 0..<2 {
            let harness = Harness(
                operationId: id(UInt8(0xc2 + phase)),
                kind: .redeem,
                steps: [.failure(notFound)]
            )
            if phase == 0 {
                harness.revalidationError = FinalityHarnessError.revalidationFailed
            } else {
                harness.attemptPersistenceError = FinalityHarnessError.persistenceFailed
            }
            do {
                _ = try await harness.run()
                XCTFail("local gate failure must escape")
            } catch FinalityHarnessError.revalidationFailed {
                XCTAssertEqual(phase, 0)
            } catch FinalityHarnessError.persistenceFailed {
                XCTAssertEqual(phase, 1)
            }
            XCTAssertEqual(harness.submissionCount, 0)
            XCTAssertNil(harness.journal.definitiveSubmissionFailure)
            XCTAssertEqual(
                harness.trace,
                phase == 0
                    ? ["status", "revalidate"]
                    : ["status", "revalidate", "persist_attempt"]
            )
        }
    }

    func testDefinitiveFailurePersistenceErrorEscapesWithoutPolling() async throws {
        let harness = Harness(
            operationId: id(0xc4),
            kind: .redeem,
            steps: [.failure(notFound)]
        )
        harness.submitHook = {
            throw ToriiClientError.httpStatus(
                code: 400,
                message: "invalid request",
                rejectCode: "offline_redeem_invalid"
            )
        }
        harness.definitiveFailurePersistenceError =
            FinalityHarnessError.persistenceFailed

        do {
            _ = try await harness.run()
            XCTFail("definitive failure persistence must be durable")
        } catch FinalityHarnessError.persistenceFailed {
            // Expected.
        }
        XCTAssertEqual(harness.statusFetchCount, 1)
        XCTAssertEqual(harness.submissionCount, 1)
        XCTAssertNil(harness.journal.definitiveSubmissionFailure)
        XCTAssertEqual(harness.trace.last, "persist_definitive_failure")
    }

    func testCancellationDuringSubmissionRecoversStatusFirstAfterRestart() async throws {
        let operationId = id(0xc5)
        let transactionHash = id(0xc6)
        let first = Harness(
            operationId: operationId,
            kind: .redeem,
            steps: [.failure(notFound)]
        )
        first.submitHook = { throw CancellationError() }

        do {
            _ = try await first.run()
            XCTFail("submission cancellation must escape")
        } catch is CancellationError {
            // Durable attempt marker remains for restart reconciliation.
        }
        XCTAssertTrue(first.journal.attempted)
        XCTAssertEqual(first.submissionCount, 1)

        let restarted = Harness(
            operationId: operationId,
            kind: .redeem,
            steps: [.status(try appliedRedeem(operationId, transactionHash))],
            initialJournal: first.journal
        )
        let resolution = try await restarted.run()

        guard case .applied = resolution.outcome else {
            return XCTFail("restart must reconcile status before submission")
        }
        XCTAssertEqual(restarted.submissionCount, 0)
        XCTAssertEqual(restarted.trace, ["status", "persist_observation"])
    }

    func testAcceptancePersistenceErrorsAreNeverMisclassifiedAsTransportAmbiguity() async throws {
        let operationId = id(0xd1)
        let transactionHash = id(0xd2)
        let harness = Harness(
            operationId: operationId,
            kind: .redeem,
            steps: [
                .failure(notFound),
                .status(try appliedRedeem(operationId, transactionHash)),
            ]
        )
        harness.submitHook = {
            try self.reference(operationId, .redeem, transactionHash)
        }
        harness.acceptanceError = ToriiClientError.transport(
            URLError(.cannotWriteToFile)
        )

        do {
            _ = try await harness.run()
            XCTFail("persistence failure must escape without polling")
        } catch ToriiClientError.transport {
            // Expected.
        }
        XCTAssertEqual(harness.statusFetchCount, 1)
        XCTAssertEqual(harness.submissionCount, 1)
        XCTAssertEqual(
            harness.trace,
            ["status", "revalidate", "persist_attempt", "submit", "persist_accept"]
        )
    }

    func testFailureClassifiersRemainFailClosed() {
        XCTAssertTrue(
            KagemushaOperationFinalityCoordinator.statusResourceIsMissing(
                after: notFound
            )
        )
        XCTAssertFalse(
            KagemushaOperationFinalityCoordinator.statusResourceIsMissing(
                after: ToriiClientError.transport(URLError(.timedOut))
            )
        )
        XCTAssertFalse(
            KagemushaOperationFinalityCoordinator.statusResourceIsMissing(
                after: ToriiClientError.httpStatus(
                    code: 404,
                    message: "proxy route missing",
                    rejectCode: nil
                )
            )
        )
        XCTAssertFalse(
            KagemushaOperationFinalityCoordinator.statusResourceIsMissing(
                after: ToriiClientError.httpStatus(
                    code: 404,
                    message: "different resource",
                    rejectCode: "route_not_found"
                )
            )
        )
        XCTAssertTrue(
            KagemushaOperationFinalityCoordinator.submissionMayHaveBeenAccepted(
                after: KagemushaOperationError.invalidNoritoArchive,
                operationKind: .redeem
            )
        )
        XCTAssertTrue(
            KagemushaOperationFinalityCoordinator.submissionMayHaveBeenAccepted(
                after: ToriiClientError.invalidPayload("malformed 202"),
                operationKind: .redeem
            )
        )
        XCTAssertTrue(
            KagemushaOperationFinalityCoordinator.submissionMayHaveBeenAccepted(
                after: ToriiClientError.httpStatus(
                    code: 500,
                    message: nil,
                    rejectCode: nil
                ),
                operationKind: .redeem
            )
        )
        XCTAssertTrue(
            KagemushaOperationFinalityCoordinator.submissionMayHaveBeenAccepted(
                after: ToriiClientError.httpStatus(
                    code: 429,
                    message: nil,
                    rejectCode: nil
                ),
                operationKind: .redeem
            )
        )
        XCTAssertTrue(
            KagemushaOperationFinalityCoordinator.submissionMayHaveBeenAccepted(
                after: ToriiClientError.httpStatus(
                    code: 408,
                    message: nil,
                    rejectCode: nil
                ),
                operationKind: .redeem
            )
        )
        XCTAssertFalse(
            KagemushaOperationFinalityCoordinator.submissionMayHaveBeenAccepted(
                after: ToriiClientError.httpStatus(
                    code: 400,
                    message: nil,
                    rejectCode: "offline_redeem_invalid"
                ),
                operationKind: .redeem
            )
        )
        for (code, rejectCode) in [
            (404, "route_not_found"),
            (425, "too_early"),
            (460, "proxy_custom"),
            (499, "client_closed_request"),
            (400, "unknown_bad_request"),
        ] {
            XCTAssertTrue(
                KagemushaOperationFinalityCoordinator.submissionMayHaveBeenAccepted(
                    after: ToriiClientError.httpStatus(
                        code: code,
                        message: nil,
                        rejectCode: rejectCode
                    ),
                    operationKind: .redeem
                )
            )
        }
        for code in [200, 302] {
            XCTAssertTrue(
                KagemushaOperationFinalityCoordinator.submissionMayHaveBeenAccepted(
                    after: ToriiClientError.httpStatus(
                        code: code,
                        message: nil,
                        rejectCode: nil
                    ),
                    operationKind: .redeem
                )
            )
        }
        XCTAssertFalse(
            KagemushaOperationFinalityCoordinator.submissionMayHaveBeenAccepted(
                after: CancellationError(),
                operationKind: .redeem
            )
        )
        XCTAssertTrue(
            KagemushaOperationFinalityCoordinator.statusFailureIsRetryable(
                ToriiClientError.transport(URLError(.networkConnectionLost))
            )
        )
        XCTAssertTrue(
            KagemushaOperationFinalityCoordinator.statusFailureIsRetryable(
                ToriiClientError.httpStatus(
                    code: 503,
                    message: nil,
                    rejectCode: "offline_topup_finality_proof_unavailable"
                )
            )
        )
        for code in [
            "offline_operation_pending_unavailable",
            "offline_operation_history_unavailable",
            "offline_operation_evidence_inconsistent",
        ] {
            XCTAssertTrue(
                KagemushaOperationFinalityCoordinator.statusFailureIsRetryable(
                    ToriiClientError.httpStatus(
                        code: 503,
                        message: nil,
                        rejectCode: code
                    )
                )
            )
        }
        XCTAssertTrue(
            KagemushaOperationFinalityCoordinator.statusFailureIsRetryable(
                ToriiClientError.httpStatus(
                    code: 429,
                    message: "rate limited",
                    rejectCode: nil
                )
            )
        )
        XCTAssertTrue(
            KagemushaOperationFinalityCoordinator.statusFailureIsRetryable(
                ToriiClientError.httpStatus(
                    code: 503,
                    message: nil,
                    rejectCode: "different_code"
                )
            )
        )
        XCTAssertTrue(
            KagemushaOperationFinalityCoordinator.statusFailureIsRetryable(
                ToriiClientError.httpStatus(
                    code: 503,
                    message: nil,
                    rejectCode: "unknown_future_retry_code"
                )
            )
        )
        XCTAssertFalse(
            KagemushaOperationFinalityCoordinator.statusFailureIsRetryable(
                ToriiClientError.invalidPayload("hostile status")
            )
        )
    }

    func testTypedOperationBindsStatusAndSubmissionToEmbeddedRequestIdentity() async throws {
        let operationBytes = Data(repeating: 0xe1, count: 32)
        let operationId = id(0xe1)
        let transactionHash = id(0xe2)
        let request = try KagemushaRedeemRequest(
            noritoArchive: requestArchive(
                schema: KagemushaRecursiveSpend.redeemRequestWireName,
                fieldCount: 10,
                operationIdFieldIndex: 8,
                operationId: operationBytes
            )
        )
        let operation = KagemushaOperationSubmission.redeem(request)
        let transport = TypedFinalityTransport(
            expectedOperation: operation,
            reference: try reference(
                operationId,
                .redeem,
                transactionHash
            ),
            terminalStatus: try appliedRedeem(operationId, transactionHash)
        )

        let resolution = try await KagemushaOperationFinalityCoordinator.resolve(
            operation: operation,
            transport: transport,
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1,
            initialState: FinalityJournal(),
            continuity: .unaccepted,
            configuration: try .init(
                maximumPollAttempts: 1,
                pollingIntervalNanoseconds: 1_000_000_000
            ),
            existingDefinitiveSubmissionFailure: {
                $0.definitiveSubmissionFailure
            },
            revalidateBeforeSubmission: { _ in },
            markSubmissionAttempt: { state in
                var state = state
                state.attempted = true
                state.attemptCount += 1
                return state
            },
            recordAcceptance: { reference, state in
                var state = state
                state.transactionHash = reference.transactionHash
                state.submittedAtMs = reference.submittedAtMs
                return state
            },
            recordObservation: { hash, timestamp, state in
                var state = state
                state.transactionHash = hash
                if let timestamp { state.submittedAtMs = timestamp }
                return state
            },
            recordRejection: { _, _, state in state },
            recordDefinitiveSubmissionFailure: { failure, state in
                var state = state
                state.definitiveSubmissionFailure = failure
                return state
            }
        )

        guard case .applied = resolution.outcome else {
            return XCTFail("typed transport lifecycle must reach finality")
        }
        let snapshot = await transport.snapshot()
        XCTAssertEqual(snapshot.requestedOperationIds, [operationId, operationId])
        XCTAssertEqual(snapshot.submittedOperations, [operation])
    }

    func testConcurrentSameOperationFailsFastAndSubmitsOnlyOnce() async throws {
        let operation = try redeemOperation(0xe3)
        let operationId = operation.operationId
        let transactionHash = id(0xe4)
        let transport = LeaseFinalityTransport(
            expectedOperation: operation,
            reference: try reference(operationId, .redeem, transactionHash),
            terminalStatus: try appliedRedeem(operationId, transactionHash)
        )
        let first = Task {
            try await self.resolveTyped(operation, transport: transport)
        }
        try await waitUntilSubmitted(transport)

        do {
            _ = try await resolveTyped(operation, transport: transport)
            XCTFail("same operation must not resolve concurrently")
        } catch KagemushaOperationFinalityError.alreadyResolving(let actualId) {
            XCTAssertEqual(actualId, operationId)
        }

        await transport.releaseSubmission()
        let resolution = try await first.value
        guard case .applied = resolution.outcome else {
            return XCTFail("first resolver must complete")
        }
        let submissionCount = await transport.submissionCount()
        XCTAssertEqual(submissionCount, 1)
    }

    func testOverallDeadlineReleasesLeaseWhenTransportIgnoresCancellation() async throws {
        let operation = try redeemOperation(0xec)
        let transactionHash = id(0xed)
        let configuration = try KagemushaOperationFinalityConfiguration(
            maximumPollAttempts: 1,
            pollingIntervalNanoseconds: 1_000_000_000,
            overallTimeoutNanoseconds: 1_000_000_000
        )

        do {
            _ = try await resolveTyped(
                operation,
                transport: DeadlineIgnoringFinalityTransport(),
                configuration: configuration
            )
            XCTFail("a cancellation-ignoring transport must still be bounded")
        } catch KagemushaOperationFinalityError.overallDeadlineExceeded {
            // Expected after one second of monotonic wall time.
        }

        let resolution = try await resolveTyped(
            operation,
            transport: AppliedOnlyFinalityTransport(
                status: try appliedRedeem(
                    operation.operationId,
                    transactionHash
                )
            ),
            configuration: configuration
        )
        guard case .applied = resolution.outcome else {
            return XCTFail("deadline must release the process-local operation lease")
        }
    }

    func testConcurrentDifferentOperationsHaveIndependentLeases() async throws {
        let firstOperation = try redeemOperation(0xe5)
        let secondOperation = try redeemOperation(0xe6)
        let firstTransport = LeaseFinalityTransport(
            expectedOperation: firstOperation,
            reference: try reference(
                firstOperation.operationId,
                .redeem,
                id(0xe7)
            ),
            terminalStatus: try appliedRedeem(
                firstOperation.operationId,
                id(0xe7)
            )
        )
        let secondTransport = LeaseFinalityTransport(
            expectedOperation: secondOperation,
            reference: try reference(
                secondOperation.operationId,
                .redeem,
                id(0xe8)
            ),
            terminalStatus: try appliedRedeem(
                secondOperation.operationId,
                id(0xe8)
            )
        )
        let first = Task {
            try await self.resolveTyped(firstOperation, transport: firstTransport)
        }
        let second = Task {
            try await self.resolveTyped(secondOperation, transport: secondTransport)
        }

        try await waitUntilSubmitted(firstTransport)
        try await waitUntilSubmitted(secondTransport)
        let firstSubmissionCount = await firstTransport.submissionCount()
        let secondSubmissionCount = await secondTransport.submissionCount()
        XCTAssertEqual(firstSubmissionCount, 1)
        XCTAssertEqual(secondSubmissionCount, 1)
        await firstTransport.releaseSubmission()
        await secondTransport.releaseSubmission()

        _ = try await first.value
        _ = try await second.value
    }

    private var notFound: ToriiClientError {
        .httpStatus(
            code: 404,
            message: "missing",
            rejectCode: "offline_operation_not_found"
        )
    }

    private func id(_ byte: UInt8) -> String {
        String(repeating: String(format: "%02x", byte), count: 31)
            + String(format: "%02x", byte | 1)
    }

    private func pending(
        _ operationId: String,
        _ kind: KagemushaOperationKind,
        _ transactionHash: String,
        submittedAtMs: UInt64 = 1
    ) throws -> KagemushaOperationStatus {
        .pending(
            try .init(
                operationId: operationId,
                kind: kind,
                transactionHash: transactionHash,
                submittedAtMs: submittedAtMs
            )
        )
    }

    private func appliedRedeem(
        _ operationId: String,
        _ transactionHash: String
    ) throws -> KagemushaOperationStatus {
        .applied(
            try .init(
                operationId: operationId,
                result: .redeem(
                    try .init(
                        transactionHash: transactionHash,
                        finalizedBlockHeight: 1
                    )
                )
            )
        )
    }

    private func rejected(
        _ operationId: String,
        _ kind: KagemushaOperationKind,
        _ transactionHash: String,
        message: String = "Rejected by policy."
    ) throws -> KagemushaOperationStatus {
        .rejected(
            try .init(
                operationId: operationId,
                kind: kind,
                transactionHash: transactionHash,
                error: try .init(
                    code: "offline_operation_rejected",
                    message: message
                )
            )
        )
    }

    private func reference(
        _ operationId: String,
        _ kind: KagemushaOperationKind,
        _ transactionHash: String,
        submittedAtMs: UInt64 = 1
    ) throws -> KagemushaOperationReference {
        try .init(
            operationId: operationId,
            kind: kind,
            state: .pending,
            transactionHash: transactionHash,
            statusUri: "/v1/offline/operations/\(operationId)",
            submittedAtMs: submittedAtMs
        )
    }

    private func requestArchive(
        schema: String,
        fieldCount: Int,
        operationIdFieldIndex: Int,
        operationId: Data,
        issuedAtMs: UInt64 = 1
    ) -> Data {
        var authorization = CompactNoritoWriter()
        for index in 0..<10 {
            let field: Data
            switch index {
            case 3:
                field = operationId
            case 4:
                field = CompactNorito.encodeUInt64(issuedAtMs)
            default:
                field = Data([UInt8(index + 1)])
            }
            authorization.writeField(field)
        }
        var payload = CompactNoritoWriter()
        for index in 0..<fieldCount {
            payload.writeField(
                index == 0
                    ? CompactNorito.encodeUInt16(KagemushaRecursiveSpend.wireVersionV4)
                    : index == operationIdFieldIndex
                    ? operationId
                    : index == fieldCount - 1
                    ? authorization.data
                    : Data([UInt8(index + 1)])
            )
        }
        return KagemushaRecursiveSpend.frameArchive(
            schema: schema,
            payload: payload.data
        )
    }

    private func redeemOperation(
        _ byte: UInt8
    ) throws -> KagemushaOperationSubmission {
        .redeem(try KagemushaRedeemRequest(
            noritoArchive: requestArchive(
                schema: KagemushaRecursiveSpend.redeemRequestWireName,
                fieldCount: 10,
                operationIdFieldIndex: 8,
                operationId: Data(repeating: byte, count: 32)
            )
        ))
    }

    private func resolveTyped<Transport>(
        _ operation: KagemushaOperationSubmission,
        transport: Transport,
        configuration: KagemushaOperationFinalityConfiguration? = nil
    ) async throws -> KagemushaOperationFinalityResolution<FinalityJournal>
    where Transport: KagemushaOperationFinalityTransport {
        try await KagemushaOperationFinalityCoordinator.resolve(
            operation: operation,
            transport: transport,
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1,
            initialState: FinalityJournal(),
            continuity: .unaccepted,
            configuration: try configuration ?? .init(
                maximumPollAttempts: 1,
                pollingIntervalNanoseconds: 1_000_000_000
            ),
            existingDefinitiveSubmissionFailure: {
                $0.definitiveSubmissionFailure
            },
            revalidateBeforeSubmission: { _ in },
            markSubmissionAttempt: { state in
                var state = state
                state.attempted = true
                state.attemptCount += 1
                return state
            },
            recordAcceptance: { reference, state in
                var state = state
                state.transactionHash = reference.transactionHash
                state.submittedAtMs = reference.submittedAtMs
                return state
            },
            recordObservation: { hash, timestamp, state in
                var state = state
                state.transactionHash = hash
                if let timestamp { state.submittedAtMs = timestamp }
                return state
            },
            recordRejection: { _, _, state in state },
            recordDefinitiveSubmissionFailure: { failure, state in
                var state = state
                state.definitiveSubmissionFailure = failure
                return state
            }
        )
    }

    private func waitUntilSubmitted(
        _ transport: LeaseFinalityTransport
    ) async throws {
        for _ in 0..<2_000 {
            if await transport.submissionCount() > 0 { return }
            try await Task.sleep(nanoseconds: 1_000_000)
        }
        throw FinalityHarnessError.exhaustedStatusScript
    }
}

private actor TypedFinalityTransport: KagemushaOperationFinalityTransport {
    struct Snapshot: Sendable {
        let requestedOperationIds: [String]
        let submittedOperations: [KagemushaOperationSubmission]
    }

    let expectedOperation: KagemushaOperationSubmission
    let reference: KagemushaOperationReference
    let terminalStatus: KagemushaOperationStatus
    var requestedOperationIds: [String] = []
    var submittedOperations: [KagemushaOperationSubmission] = []

    init(
        expectedOperation: KagemushaOperationSubmission,
        reference: KagemushaOperationReference,
        terminalStatus: KagemushaOperationStatus
    ) {
        self.expectedOperation = expectedOperation
        self.reference = reference
        self.terminalStatus = terminalStatus
    }

    func getKagemushaOperationStatus(
        operation: KagemushaOperationSubmission,
        acceptedReference: KagemushaOperationReference?,
        chainDiscriminant: UInt16
    ) async throws -> KagemushaOperationStatus {
        guard chainDiscriminant == SccpV1.tairaI105DiscriminantV1,
              operation == expectedOperation,
              acceptedReference == nil || acceptedReference == reference else {
            throw FinalityHarnessError.unexpectedSubmission
        }
        requestedOperationIds.append(operation.operationId)
        if requestedOperationIds.count == 1 {
            throw ToriiClientError.httpStatus(
                code: 404,
                message: "missing",
                rejectCode: "offline_operation_not_found"
            )
        }
        return terminalStatus
    }

    func submitKagemushaOperation(
        _ operation: KagemushaOperationSubmission
    ) async throws -> KagemushaOperationReference {
        guard operation == expectedOperation else {
            throw FinalityHarnessError.unexpectedSubmission
        }
        submittedOperations.append(operation)
        return reference
    }

    func snapshot() -> Snapshot {
        Snapshot(
            requestedOperationIds: requestedOperationIds,
            submittedOperations: submittedOperations
        )
    }
}

private struct AppliedOnlyFinalityTransport:
    KagemushaOperationFinalityTransport,
    Sendable
{
    let status: KagemushaOperationStatus

    func getKagemushaOperationStatus(
        operation: KagemushaOperationSubmission,
        acceptedReference: KagemushaOperationReference?,
        chainDiscriminant: UInt16
    ) async throws -> KagemushaOperationStatus {
        guard chainDiscriminant == SccpV1.tairaI105DiscriminantV1,
              status.operationId == operation.operationId,
              status.kind == operation.kind,
              acceptedReference == nil else {
            throw FinalityHarnessError.unexpectedSubmission
        }
        return status
    }

    func submitKagemushaOperation(
        _ operation: KagemushaOperationSubmission
    ) async throws -> KagemushaOperationReference {
        throw FinalityHarnessError.unexpectedSubmission
    }
}

private struct DeadlineIgnoringFinalityTransport:
    KagemushaOperationFinalityTransport,
    Sendable
{
    func getKagemushaOperationStatus(
        operation: KagemushaOperationSubmission,
        acceptedReference: KagemushaOperationReference?,
        chainDiscriminant: UInt16
    ) async throws -> KagemushaOperationStatus {
        guard chainDiscriminant == SccpV1.tairaI105DiscriminantV1,
              acceptedReference == nil,
              operation.kind == .redeem else {
            throw FinalityHarnessError.unexpectedSubmission
        }
        return try await withCheckedThrowingContinuation {
            (continuation: CheckedContinuation<KagemushaOperationStatus, Error>) in
            Task.detached {
                try? await Task.sleep(nanoseconds: 1_500_000_000)
                continuation.resume(
                    throwing: ToriiClientError.transport(
                        URLError(.networkConnectionLost)
                    )
                )
            }
        }
    }

    func submitKagemushaOperation(
        _ operation: KagemushaOperationSubmission
    ) async throws -> KagemushaOperationReference {
        throw FinalityHarnessError.unexpectedSubmission
    }
}

private actor LeaseFinalityTransport: KagemushaOperationFinalityTransport {
    let expectedOperation: KagemushaOperationSubmission
    let reference: KagemushaOperationReference
    let terminalStatus: KagemushaOperationStatus
    private var statusCount = 0
    private var submissions = 0
    private var submissionReleased = false
    private var releaseContinuation: CheckedContinuation<Void, Never>?

    init(
        expectedOperation: KagemushaOperationSubmission,
        reference: KagemushaOperationReference,
        terminalStatus: KagemushaOperationStatus
    ) {
        self.expectedOperation = expectedOperation
        self.reference = reference
        self.terminalStatus = terminalStatus
    }

    func getKagemushaOperationStatus(
        operation: KagemushaOperationSubmission,
        acceptedReference: KagemushaOperationReference?,
        chainDiscriminant: UInt16
    ) async throws -> KagemushaOperationStatus {
        guard chainDiscriminant == SccpV1.tairaI105DiscriminantV1,
              operation == expectedOperation,
              acceptedReference == nil || acceptedReference == reference else {
            throw FinalityHarnessError.unexpectedSubmission
        }
        statusCount += 1
        if statusCount == 1 {
            throw ToriiClientError.httpStatus(
                code: 404,
                message: "missing",
                rejectCode: "offline_operation_not_found"
            )
        }
        return terminalStatus
    }

    func submitKagemushaOperation(
        _ operation: KagemushaOperationSubmission
    ) async throws -> KagemushaOperationReference {
        guard operation == expectedOperation else {
            throw FinalityHarnessError.unexpectedSubmission
        }
        submissions += 1
        if !submissionReleased {
            await withCheckedContinuation { continuation in
                releaseContinuation = continuation
            }
        }
        return reference
    }

    func releaseSubmission() {
        submissionReleased = true
        releaseContinuation?.resume()
        releaseContinuation = nil
    }

    func submissionCount() -> Int {
        submissions
    }
}

private enum FinalityStatusStep {
    case status(KagemushaOperationStatus)
    case failure(Error)
}

private struct FinalityJournal: Equatable {
    var attempted = false
    var attemptCount = 0
    var transactionHash: String?
    var submittedAtMs: UInt64?
    var rejected = false
    var definitiveSubmissionFailure: KagemushaDefinitiveSubmissionFailure?
}

private final class Harness {
    let operationId: String
    let kind: KagemushaOperationKind
    var expectedSubmittedAtMs: UInt64
    var steps: [FinalityStatusStep]
    var journal = FinalityJournal()
    var trace: [String] = []
    var statusFetchCount = 0
    var submissionCount = 0
    var sleepValues: [UInt64] = []
    var monotonicNanoseconds: UInt64 = 1
    var configuration = try! KagemushaOperationFinalityConfiguration(
        maximumPollAttempts: 1,
        pollingIntervalNanoseconds: 1_000_000_000
    )
    var cancelWhileMarkingAttempt = false
    var acceptanceError: Error?
    var revalidationError: Error?
    var attemptPersistenceError: Error?
    var definitiveFailurePersistenceError: Error?
    var continuity = KagemushaOperationContinuity.unaccepted
    var submitHook: (() async throws -> KagemushaOperationReference)?

    init(
        operationId: String,
        kind: KagemushaOperationKind,
        steps: [FinalityStatusStep],
        initialJournal: FinalityJournal = FinalityJournal(),
        expectedSubmittedAtMs: UInt64 = 1
    ) {
        self.operationId = operationId
        self.kind = kind
        self.expectedSubmittedAtMs = expectedSubmittedAtMs
        self.steps = steps
        self.journal = initialJournal
    }

    func run() async throws -> KagemushaOperationFinalityResolution<FinalityJournal> {
        try await KagemushaOperationFinalityCoordinator.resolveForTesting(
            operationId: operationId,
            expectedKind: kind,
            expectedSubmittedAtMs: expectedSubmittedAtMs,
            initialState: journal,
            continuity: continuity,
            configuration: configuration,
            sleep: { value in
                self.trace.append("sleep")
                self.sleepValues.append(value)
                self.monotonicNanoseconds += value
            },
            monotonicNow: { self.monotonicNanoseconds },
            existingDefinitiveSubmissionFailure: {
                $0.definitiveSubmissionFailure
            },
            fetchStatus: { _ in
                self.trace.append("status")
                self.statusFetchCount += 1
                guard !self.steps.isEmpty else {
                    throw FinalityHarnessError.exhaustedStatusScript
                }
                switch self.steps.removeFirst() {
                case let .status(status): return status
                case let .failure(error): throw error
                }
            },
            revalidateBeforeSubmission: { _ in
                self.trace.append("revalidate")
                if let revalidationError = self.revalidationError {
                    throw revalidationError
                }
            },
            markSubmissionAttempt: { state in
                self.trace.append("persist_attempt")
                if let attemptPersistenceError = self.attemptPersistenceError {
                    throw attemptPersistenceError
                }
                var state = state
                state.attempted = true
                state.attemptCount += 1
                self.journal = state
                if self.cancelWhileMarkingAttempt {
                    withUnsafeCurrentTask { $0?.cancel() }
                }
                return state
            },
            submit: {
                self.trace.append("submit")
                self.submissionCount += 1
                guard let submitHook = self.submitHook else {
                    throw FinalityHarnessError.unexpectedSubmission
                }
                return try await submitHook()
            },
            recordAcceptance: { reference, state in
                self.trace.append("persist_accept")
                if let acceptanceError = self.acceptanceError {
                    throw acceptanceError
                }
                let state = try self.record(
                    reference.transactionHash,
                    submittedAtMs: reference.submittedAtMs,
                    in: state
                )
                self.journal = state
                return state
            },
            recordObservation: { transactionHash, submittedAtMs, state in
                self.trace.append("persist_observation")
                let state = try self.record(
                    transactionHash,
                    submittedAtMs: submittedAtMs,
                    in: state
                )
                self.journal = state
                return state
            },
            recordRejection: { transactionHash, _, state in
                self.trace.append("persist_rejection")
                var state = try self.record(
                    transactionHash,
                    submittedAtMs: nil,
                    in: state
                )
                state.rejected = true
                self.journal = state
                return state
            },
            recordDefinitiveSubmissionFailure: { failure, state in
                self.trace.append("persist_definitive_failure")
                if let error = self.definitiveFailurePersistenceError {
                    throw error
                }
                var state = state
                state.definitiveSubmissionFailure = failure
                self.journal = state
                return state
            }
        )
    }

    private func record(
        _ transactionHash: String,
        submittedAtMs: UInt64?,
        in state: FinalityJournal
    ) throws -> FinalityJournal {
        var state = state
        let hashChanged = state.transactionHash.map { $0 != transactionHash }
            ?? false
        if hashChanged {
            state.submittedAtMs = nil
        }
        state.transactionHash = transactionHash
        if let submittedAtMs {
            if let previous = state.submittedAtMs,
               previous != submittedAtMs {
                throw KagemushaOperationFinalityError.continuityViolation(
                    "durable submitted timestamp"
                )
            }
            state.submittedAtMs = submittedAtMs
        }
        return state
    }
}

private enum FinalityHarnessError: Error {
    case exhaustedStatusScript
    case unexpectedSubmission
    case revalidationFailed
    case persistenceFailed
}
