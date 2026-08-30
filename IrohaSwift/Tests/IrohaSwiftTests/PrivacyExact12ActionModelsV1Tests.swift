import Foundation
import XCTest
@testable import IrohaSwift

final class PrivacyExact12ActionModelsV1Tests: XCTestCase {
    func testClosedOperationProtocolAndEffectMappings() {
        let operations = PrivacyExact12ActionOperationV1.allCases
        XCTAssertEqual(operations.count, 13)
        XCTAssertEqual(PrivacyLedgerEffectKindV1.allCases.count, 10)
        XCTAssertEqual(
            operations.map(\.canonicalLabel),
            [
                "zk_ace_authorization_action_v1",
                "anonymous_pgc_payment_action_v1",
                "verange_range_proof_v1",
                "zk_ams_batch_admission_action_v1",
                "zk_ams_provision_account_action_v1",
                "vega_credential_presentation_v1",
                "zk_x509_identity_presentation_v1",
                "jindo_polynomial_evaluation_v1",
                "bootle_lantern_credential_presentation_v1",
                "orchard_note_action_v1",
                "fcmp_membership_payment_v1",
                "ivm_private_note_action_v1",
                "pq_masp_note_action_v1",
            ]
        )
        XCTAssertEqual(
            operations.map(\.protocolId),
            [
                .zkAcePqAuthorizationV0,
                .anonymousPgcKOutOfNV1,
                .veRangeTransparentRangeV1,
                .irohaZkAmsV1,
                .irohaZkAmsV1,
                .vegaExistingCredentialZkV0,
                .irohaZkX509StarkP256V0,
                .irohaJindoPolynomialCommitmentV0,
                .irohaBootleLanternAnoncredV1,
                .orchardHalo2ActionsV1,
                .moneroFcmpPlusPlusV1,
                .irohaIvmPrivateNoteStarkV1,
                .pqMaspStarkV0,
            ]
        )
        XCTAssertEqual(
            operations.map(\.ledgerEffectKind),
            [
                .zkAceTransparentTransfer,
                .anonymousPgcAccountStateTransition,
                .verificationOnly,
                .zkAmsBatchAdmission,
                .zkAmsProvisionAccount,
                .verificationOnly,
                .zkX509CertificateNullifier,
                .verificationOnly,
                .verificationOnly,
                .orchardNoteStateTransition,
                .fcmpMembershipPayment,
                .ivmPrivateNoteStateTransition,
                .pqMaspNoteStateTransition,
            ]
        )
        XCTAssertEqual(
            Set(PrivacyLedgerEffectKindV1.allCases.map(\.canonicalLabel)),
            Set(operations.map { $0.ledgerEffectKind.canonicalLabel })
        )
    }

    func testRequestBoundsAndSnapshotsWireAndOptionalManifestDigest() throws {
        var wire = Data([0x01, 0x02])
        var digest = fixed32(0x21)
        let request = try PrivacyExact12ActionRequestV1(
            operation: .zkAmsProvisionAccountActionV1,
            signedTransactionVersioned: wire,
            expectedManifestDigest: digest
        )
        wire[0] = 0xff
        digest[0] = 0xff
        XCTAssertEqual(request.signedTransactionVersioned, Data([0x01, 0x02]))
        XCTAssertEqual(request.expectedManifestDigest, fixed32(0x21))

        XCTAssertNoThrow(try PrivacyExact12ActionRequestV1(
            operation: .veRangeRangeProofV1,
            signedTransactionVersioned: Data(
                repeating: 0x01,
                count: PrivacyExact12ActionRequestV1.maximumSignedTransactionBytes
            )
        ))
        XCTAssertThrowsError(try PrivacyExact12ActionRequestV1(
            operation: .veRangeRangeProofV1,
            signedTransactionVersioned: Data()
        ))
        XCTAssertThrowsError(try PrivacyExact12ActionRequestV1(
            operation: .veRangeRangeProofV1,
            signedTransactionVersioned: Data(
                repeating: 0x01,
                count: PrivacyExact12ActionRequestV1.maximumSignedTransactionBytes + 1
            )
        ))
        XCTAssertThrowsError(try PrivacyExact12ActionRequestV1(
            operation: .veRangeRangeProofV1,
            signedTransactionVersioned: Data([0x01]),
            expectedManifestDigest: Data(repeating: 0, count: 32)
        ))
        XCTAssertThrowsError(try PrivacyExact12ActionRequestV1(
            operation: .veRangeRangeProofV1,
            signedTransactionVersioned: Data([0x01]),
            expectedManifestDigest: Data(repeating: 1, count: 31)
        ))
    }

    func testValidSubmittedAndTerminalViews() throws {
        let submitted = try makeView(
            localState: .submitted,
            terminalChainState: nil,
            committedHeight: nil,
            rejectionReason: nil
        )
        XCTAssertEqual(submitted.localState, .submitted)
        XCTAssertNil(submitted.terminalChainState)

        let committed = try makeView(
            localState: .terminal,
            terminalChainState: .committed,
            committedHeight: 42,
            rejectionReason: nil
        )
        XCTAssertEqual(committed.committedHeight, 42)
        XCTAssertNil(committed.executionCapabilityManifestDigest)

        let applied = try makeView(
            localState: .terminal,
            terminalChainState: .applied,
            committedHeight: 42,
            rejectionReason: nil,
            executionCapabilityManifestDigest: fixed32(0x31),
            executionCapabilityCommittedHeight: 41,
            executionReceiptFinalizedHeight: 43,
            executionReceiptFinalizedBlockHash: fixed32(0x32)
        )
        XCTAssertEqual(applied.committedHeight, 42)
        XCTAssertEqual(applied.terminalChainState, .applied)
        XCTAssertEqual(applied.executionCapabilityManifestDigest, fixed32(0x31))
        XCTAssertEqual(applied.executionCapabilityCommittedHeight, 41)
        XCTAssertEqual(applied.executionReceiptFinalizedHeight, 43)
        XCTAssertEqual(applied.executionReceiptFinalizedBlockHash, fixed32(0x32))

        let rejected = try makeView(
            localState: .terminal,
            terminalChainState: .rejected,
            committedHeight: 43,
            rejectionReason: "proof envelope expired"
        )
        XCTAssertEqual(rejected.rejectionReason, "proof envelope expired")

        let expired = try makeView(
            localState: .terminal,
            terminalChainState: .expired,
            committedHeight: nil,
            rejectionReason: nil
        )
        XCTAssertNil(expired.committedHeight)
    }

    func testImpossibleViewStateShapesFailClosed() {
        let hostile: [() throws -> Void] = [
            {
                _ = try self.makeView(
                    localState: .submitted,
                    terminalChainState: .committed,
                    committedHeight: nil,
                    rejectionReason: nil
                )
            },
            {
                _ = try self.makeView(
                    localState: .submitted,
                    terminalChainState: nil,
                    committedHeight: 1,
                    rejectionReason: nil
                )
            },
            {
                _ = try self.makeView(
                    localState: .terminal,
                    terminalChainState: nil,
                    committedHeight: nil,
                    rejectionReason: nil
                )
            },
            {
                _ = try self.makeView(
                    localState: .terminal,
                    terminalChainState: .committed,
                    committedHeight: nil,
                    rejectionReason: nil
                )
            },
            {
                _ = try self.makeView(
                    localState: .terminal,
                    terminalChainState: .applied,
                    committedHeight: 1,
                    rejectionReason: "unexpected"
                )
            },
            {
                _ = try self.makeView(
                    localState: .terminal,
                    terminalChainState: .applied,
                    committedHeight: 12,
                    rejectionReason: nil,
                    executionCapabilityManifestDigest: self.fixed32(0x31),
                    executionCapabilityCommittedHeight: 11,
                    executionReceiptFinalizedHeight: 12,
                    executionReceiptFinalizedBlockHash: nil
                )
            },
            {
                _ = try self.makeView(
                    localState: .terminal,
                    terminalChainState: .applied,
                    committedHeight: 12,
                    rejectionReason: nil,
                    executionCapabilityManifestDigest: self.fixed32(0x31),
                    executionCapabilityCommittedHeight: 13,
                    executionReceiptFinalizedHeight: 13,
                    executionReceiptFinalizedBlockHash: self.fixed32(0x32)
                )
            },
            {
                _ = try self.makeView(
                    localState: .terminal,
                    terminalChainState: .applied,
                    committedHeight: 12,
                    rejectionReason: nil,
                    executionCapabilityManifestDigest: self.fixed32(0x31),
                    executionCapabilityCommittedHeight: 11,
                    executionReceiptFinalizedHeight: 11,
                    executionReceiptFinalizedBlockHash: self.fixed32(0x32)
                )
            },
            {
                _ = try self.makeView(
                    localState: .terminal,
                    terminalChainState: .rejected,
                    committedHeight: nil,
                    rejectionReason: "rejected"
                )
            },
            {
                _ = try self.makeView(
                    localState: .terminal,
                    terminalChainState: .rejected,
                    committedHeight: 1,
                    rejectionReason: " rejected "
                )
            },
            {
                _ = try self.makeView(
                    localState: .terminal,
                    terminalChainState: .rejected,
                    committedHeight: 1,
                    rejectionReason: "policy\u{0001}rejected"
                )
            },
            {
                _ = try self.makeView(
                    localState: .terminal,
                    terminalChainState: .rejected,
                    committedHeight: 1,
                    rejectionReason: String(repeating: "é", count: 513)
                )
            },
            {
                _ = try self.makeView(
                    localState: .terminal,
                    terminalChainState: .expired,
                    committedHeight: 1,
                    rejectionReason: nil
                )
            },
            {
                _ = try self.makeView(
                    localState: .submitted,
                    terminalChainState: nil,
                    committedHeight: nil,
                    rejectionReason: nil,
                    executionCapabilityManifestDigest: self.fixed32(0x31)
                )
            },
        ]
        for (index, construct) in hostile.enumerated() {
            XCTAssertThrowsError(try construct(), "accepted hostile state shape \(index)")
        }
    }

    func testViewRejectsMappingHashesAndHeightsThatCannotBeAuthenticated() {
        XCTAssertThrowsError(try makeView(
            protocolId: .irohaZkAmsV1,
            localState: .submitted,
            terminalChainState: nil,
            committedHeight: nil,
            rejectionReason: nil
        ))
        XCTAssertThrowsError(try makeView(
            ledgerEffectKind: .verificationOnly,
            localState: .submitted,
            terminalChainState: nil,
            committedHeight: nil,
            rejectionReason: nil
        ))
        XCTAssertThrowsError(try makeView(
            transactionHash: Data(repeating: 0, count: 32),
            localState: .submitted,
            terminalChainState: nil,
            committedHeight: nil,
            rejectionReason: nil
        ))
        XCTAssertThrowsError(try makeView(
            capabilityManifestDigest: Data(repeating: 1, count: 31),
            localState: .submitted,
            terminalChainState: nil,
            committedHeight: nil,
            rejectionReason: nil
        ))
        XCTAssertThrowsError(try makeView(
            capabilityCommittedHeight: 0,
            localState: .submitted,
            terminalChainState: nil,
            committedHeight: nil,
            rejectionReason: nil
        ))
        XCTAssertThrowsError(try makeView(
            localState: .terminal,
            terminalChainState: .committed,
            committedHeight: 0,
            rejectionReason: nil
        ))
        XCTAssertThrowsError(try makeView(
            capabilityCommittedHeight: 10,
            localState: .terminal,
            terminalChainState: .committed,
            committedHeight: 9,
            rejectionReason: nil
        ))
        XCTAssertThrowsError(try makeView(
            capabilityCommittedHeight: 10,
            localState: .terminal,
            terminalChainState: .rejected,
            committedHeight: 9,
            rejectionReason: "rejected"
        ))
        XCTAssertThrowsError(try makeView(
            capabilityCommittedHeight: 10,
            localState: .terminal,
            terminalChainState: .applied,
            committedHeight: 9,
            rejectionReason: nil,
            executionCapabilityManifestDigest: fixed32(0x31),
            executionCapabilityCommittedHeight: 8,
            executionReceiptFinalizedHeight: 9,
            executionReceiptFinalizedBlockHash: fixed32(0x32)
        ))
    }

    func testNativeInspectionProjectionIsExactAndSnapshotsBytes() throws {
        var projection = Data()
        for byte in UInt8(1)...UInt8(4) {
            projection.append(Data(repeating: byte, count: 32))
        }
        let inspection = try PrivacyExact12ActionInspectionV1(
            nativeProjection: projection
        )
        projection[0] = 0xff

        XCTAssertEqual(inspection.transactionHash, fixed32(0x01))
        XCTAssertEqual(inspection.transactionIntentDigest, fixed32(0x02))
        XCTAssertEqual(inspection.statementDigest, fixed32(0x03))
        XCTAssertEqual(inspection.proofEnvelopeHash, fixed32(0x04))
        XCTAssertThrowsError(try PrivacyExact12ActionInspectionV1(
            nativeProjection: Data(repeating: 1, count: 127)
        ))

        var zeroField = Data(repeating: 1, count: 128)
        zeroField.replaceSubrange(64..<96, with: Data(repeating: 0, count: 32))
        XCTAssertThrowsError(try PrivacyExact12ActionInspectionV1(
            nativeProjection: zeroField
        ))
    }

    func testAuthenticatedCommittedResultDecodesExactSuccessAndRejection() throws {
        let success = try JSONDecoder().decode(
            AuthenticatedCommittedTransactionResultV1.self,
            from: committedResultJSON(resultOK: true, reasonJSON: "null", heightJSON: "\"7\"")
        )
        XCTAssertTrue(success.resultOk)
        XCTAssertNil(success.rejectionMessage)
        XCTAssertEqual(success.committedBlockHeight, 7)

        let rejection = try JSONDecoder().decode(
            AuthenticatedCommittedTransactionResultV1.self,
            from: committedResultJSON(
                resultOK: false,
                reasonJSON: "\"policy epoch is stale\"",
                heightJSON: "\"8\""
            )
        )
        XCTAssertFalse(rejection.resultOk)
        XCTAssertEqual(rejection.rejectionMessage, "policy epoch is stale")
        XCTAssertEqual(rejection.committedBlockHeight, 8)
    }

    func testAuthenticatedCommittedResultRejectsSubstitutionAndNoncanonicalJSON() {
        let hostile = [
            committedResultJSON(resultOK: true, reasonJSON: "\"contradiction\"", heightJSON: "\"7\""),
            committedResultJSON(resultOK: false, reasonJSON: "null", heightJSON: "\"7\""),
            committedResultJSON(resultOK: true, reasonJSON: "null", heightJSON: "7"),
            committedResultJSON(resultOK: true, reasonJSON: "null", heightJSON: "\"07\""),
            committedResultJSON(resultOK: true, reasonJSON: "null", heightJSON: "\"0\""),
            committedResultJSON(
                resultOK: false,
                reasonJSON: "\"policy\\u0001rejected\"",
                heightJSON: "\"7\""
            ),
            committedResultJSON(
                resultOK: false,
                reasonJSON: "\"\(String(repeating: "é", count: 513))\"",
                heightJSON: "\"7\""
            ),
            committedResultJSON(resultOK: true, reasonJSON: "null", heightJSON: "\"7\"", version: 2),
            committedResultJSON(
                resultOK: true,
                reasonJSON: "null",
                heightJSON: "\"7\"",
                extraField: ",\"unexpected\":true"
            ),
        ]
        for (index, json) in hostile.enumerated() {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    AuthenticatedCommittedTransactionResultV1.self,
                    from: json
                ),
                "accepted hostile committed-result projection \(index)"
            )
        }
    }

    func testAuthenticatedOfflineDeviceRegistrationResultDecodesClosedTerminalShapes() throws {
        let applied = try JSONDecoder().decode(
            AuthenticatedOfflineDeviceRegistrationResultV1.self,
            from: registrationResultJSON()
        )
        XCTAssertEqual(applied.terminalState, .applied)
        XCTAssertEqual(applied.committedBlockHeight, 7)
        XCTAssertNil(applied.eligibilityOutcome)
        XCTAssertNil(applied.rejectionMessage)

        let vulnerable = try JSONDecoder().decode(
            AuthenticatedOfflineDeviceRegistrationResultV1.self,
            from: registrationResultJSON(
                height: "18446744073709551615",
                terminalState: "eligibility_rejected",
                eligibilityOutcomeJSON: "\"drain_only\"",
                eligibilityReasonJSON: "\"vulnerable_firmware\"",
                matchedRulesJSON: "[\"CVE-2026-21046\",\"samsung-keymaster-2021\"]",
                rejectionCodeJSON: "\"offline_device_eligibility\"",
                rejectionMessageJSON: "\"device firmware is governed drain-only\""
            )
        )
        XCTAssertEqual(vulnerable.terminalState, .eligibilityRejected)
        XCTAssertEqual(vulnerable.eligibilityOutcome, .drainOnly)
        XCTAssertEqual(vulnerable.eligibilityReason, .vulnerableFirmware)
        XCTAssertEqual(
            vulnerable.matchedRuleIds,
            ["CVE-2026-21046", "samsung-keymaster-2021"]
        )
        XCTAssertEqual(vulnerable.committedBlockHeight, UInt64.max)

        let cryptographic = try JSONDecoder().decode(
            AuthenticatedOfflineDeviceRegistrationResultV1.self,
            from: registrationResultJSON(
                terminalState: "eligibility_rejected",
                eligibilityOutcomeJSON: "\"cryptographically_rejected\"",
                eligibilityReasonJSON: "\"cryptographic_attestation_rejected\"",
                rejectionCodeJSON: "\"offline_device_eligibility\"",
                rejectionMessageJSON: "\"attestation chain was rejected\""
            )
        )
        XCTAssertEqual(cryptographic.eligibilityOutcome, .cryptographicallyRejected)
        XCTAssertEqual(cryptographic.eligibilityReason, .cryptographicAttestationRejected)
        XCTAssertTrue(cryptographic.matchedRuleIds.isEmpty)

        let other = try JSONDecoder().decode(
            AuthenticatedOfflineDeviceRegistrationResultV1.self,
            from: registrationResultJSON(
                terminalState: "other_rejected",
                rejectionCodeJSON: "\"validation\"",
                rejectionMessageJSON: "\"authority is not permitted\""
            )
        )
        XCTAssertEqual(other.terminalState, .otherRejected)
        XCTAssertNil(other.eligibilityOutcome)
        XCTAssertEqual(other.rejectionCode, "validation")
    }

    func testAuthenticatedOfflineDeviceRegistrationResultRejectsSubstitution() {
        let hostile = [
            registrationResultJSON(version: 2),
            registrationResultJSON(height: "07"),
            registrationResultJSON(height: "18446744073709551616"),
            registrationResultJSON(transactionHash: String(repeating: "AB", count: 32)),
            registrationResultJSON(rejectionMessageJSON: "\"contradiction\""),
            registrationResultJSON(
                terminalState: "eligibility_rejected",
                eligibilityOutcomeJSON: "\"eligible\"",
                eligibilityReasonJSON: "\"policy_satisfied\"",
                rejectionCodeJSON: "\"offline_device_eligibility\"",
                rejectionMessageJSON: "\"contradiction\""
            ),
            registrationResultJSON(
                terminalState: "eligibility_rejected",
                eligibilityOutcomeJSON: "\"drain_only\"",
                eligibilityReasonJSON: "\"vulnerable_firmware\"",
                rejectionCodeJSON: "\"offline_device_eligibility\"",
                rejectionMessageJSON: "\"missing matched rule\""
            ),
            registrationResultJSON(
                terminalState: "eligibility_rejected",
                eligibilityOutcomeJSON: "\"drain_only\"",
                eligibilityReasonJSON: "\"policy_not_fresh\"",
                matchedRulesJSON: "[\"unexpected-rule\"]",
                rejectionCodeJSON: "\"offline_device_eligibility\"",
                rejectionMessageJSON: "\"unexpected matched rule\""
            ),
            registrationResultJSON(
                terminalState: "eligibility_rejected",
                eligibilityOutcomeJSON: "\"drain_only\"",
                eligibilityReasonJSON: "\"vulnerable_firmware\"",
                matchedRulesJSON: "[\"z-rule\",\"a-rule\"]",
                rejectionCodeJSON: "\"offline_device_eligibility\"",
                rejectionMessageJSON: "\"unsorted rules\""
            ),
            registrationResultJSON(
                terminalState: "other_rejected",
                rejectionCodeJSON: "\"invented\"",
                rejectionMessageJSON: "\"unknown code\""
            ),
            registrationResultJSON(extraField: ",\"unexpected\":true"),
        ]
        for (index, json) in hostile.enumerated() {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    AuthenticatedOfflineDeviceRegistrationResultV1.self,
                    from: json
                ),
                "accepted hostile registration projection \(index)"
            )
        }
    }

    func testAuthenticatedExecutionReceiptDecodesExactCanonicalProjection() throws {
        let receipt = try JSONDecoder().decode(
            AuthenticatedPrivacyActionExecutionReceiptV1.self,
            from: executionReceiptJSON()
        )
        XCTAssertEqual(receipt.networkId, fixed32(0x11))
        XCTAssertEqual(receipt.transactionHash, fixed32(0x01))
        XCTAssertEqual(receipt.capabilityManifestDigest, fixed32(0x16))
        XCTAssertEqual(receipt.capabilityCommittedHeight, 40)
        XCTAssertEqual(receipt.admittedAtHeight, 41)
        XCTAssertEqual(receipt.finalizedHeight, 42)
        XCTAssertEqual(receipt.finalizedBlockHash, fixed32(0x17))
    }

    func testAuthenticatedExecutionReceiptRejectsNoncanonicalOrIncompleteProjection() {
        let hostile = [
            executionReceiptJSON(version: 2),
            executionReceiptJSON(actionIndex: 1),
            executionReceiptJSON(capabilityHeight: "040"),
            executionReceiptJSON(capabilityHeight: "0"),
            executionReceiptJSON(capabilityHeight: "42", admittedHeight: "41"),
            executionReceiptJSON(admittedHeight: "42", finalizedHeight: "41"),
            executionReceiptJSON(networkIdHex: String(repeating: "0", count: 64)),
            executionReceiptJSON(networkIdHex: String(repeating: "A", count: 64)),
            executionReceiptJSON(extraField: ",\"unexpected\":true"),
        ]
        for (index, json) in hostile.enumerated() {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    AuthenticatedPrivacyActionExecutionReceiptV1.self,
                    from: json
                ),
                "accepted hostile receipt projection \(index)"
            )
        }
    }

    func testStatusResolverKeepsCommittedAndCacheExpiredNonterminalWithoutQueries() async throws {
        let operation = try makeView(
            localState: .submitted,
            terminalChainState: nil,
            committedHeight: nil,
            rejectionReason: nil
        )
        var detailsCalls = 0
        var receiptCalls = 0
        for status in [
            try pipelineStatus(kind: "Committed", blockHeight: 41, resolvedFrom: "state"),
            try pipelineStatus(kind: "Expired", blockHeight: nil, resolvedFrom: "cache"),
        ] {
            let resolved = try await ToriiClient.resolvePrivacyActionStatusV1(
                operation,
                status: status,
                loadDetails: {
                    detailsCalls += 1
                    return nil
                },
                loadReceipt: {
                    receiptCalls += 1
                    return nil
                }
            )
            XCTAssertEqual(resolved, operation)
            XCTAssertEqual(resolved.localState, .submitted)
        }
        XCTAssertEqual(detailsCalls, 0)
        XCTAssertEqual(receiptCalls, 0)
    }

    func testStatusResolverRetriesAppliedWhileEitherLocalEvidenceIndexLags() async throws {
        let operation = try makeView(
            localState: .submitted,
            terminalChainState: nil,
            committedHeight: nil,
            rejectionReason: nil
        )
        let status = try pipelineStatus(kind: "Applied", blockHeight: 41, resolvedFrom: "state")
        let details = try JSONDecoder().decode(
            AuthenticatedCommittedTransactionResultV1.self,
            from: committedResultJSON(resultOK: true, reasonJSON: "null", heightJSON: "\"41\"")
        )
        let receipt = try JSONDecoder().decode(
            AuthenticatedPrivacyActionExecutionReceiptV1.self,
            from: executionReceiptJSON()
        )

        let missingDetails = try await ToriiClient.resolvePrivacyActionStatusV1(
            operation,
            status: status,
            loadDetails: { nil },
            loadReceipt: { receipt }
        )
        XCTAssertEqual(missingDetails, operation)

        let missingReceipt = try await ToriiClient.resolvePrivacyActionStatusV1(
            operation,
            status: status,
            loadDetails: { details },
            loadReceipt: { nil }
        )
        XCTAssertEqual(missingReceipt, operation)
    }

    func testStatusResolverRequiresReceiptBeforeAppliedTerminalization() async throws {
        let operation = try makeView(
            localState: .submitted,
            terminalChainState: nil,
            committedHeight: nil,
            rejectionReason: nil
        )
        let status = try pipelineStatus(kind: "Applied", blockHeight: 41, resolvedFrom: "state")
        let details = try JSONDecoder().decode(
            AuthenticatedCommittedTransactionResultV1.self,
            from: committedResultJSON(resultOK: true, reasonJSON: "null", heightJSON: "\"41\"")
        )
        let receipt = try JSONDecoder().decode(
            AuthenticatedPrivacyActionExecutionReceiptV1.self,
            from: executionReceiptJSON()
        )
        let resolved = try await ToriiClient.resolvePrivacyActionStatusV1(
            operation,
            status: status,
            loadDetails: { details },
            loadReceipt: { receipt }
        )
        XCTAssertEqual(resolved.localState, .terminal)
        XCTAssertEqual(resolved.terminalChainState, .applied)
        XCTAssertEqual(resolved.committedHeight, 41)
        XCTAssertEqual(resolved.executionCapabilityManifestDigest, fixed32(0x16))
        XCTAssertEqual(resolved.executionCapabilityCommittedHeight, 40)
        XCTAssertEqual(resolved.executionReceiptFinalizedHeight, 42)
        XCTAssertEqual(resolved.executionReceiptFinalizedBlockHash, fixed32(0x17))
    }

    func testStatusResolverRejectsReceiptContradictions() async throws {
        let operation = try makeView(
            localState: .submitted,
            terminalChainState: nil,
            committedHeight: nil,
            rejectionReason: nil
        )
        let receipt = try JSONDecoder().decode(
            AuthenticatedPrivacyActionExecutionReceiptV1.self,
            from: executionReceiptJSON()
        )
        let rejection = try JSONDecoder().decode(
            AuthenticatedCommittedTransactionResultV1.self,
            from: committedResultJSON(
                resultOK: false,
                reasonJSON: "\"rejected\"",
                heightJSON: "\"41\""
            )
        )
        do {
            _ = try await ToriiClient.resolvePrivacyActionStatusV1(
                operation,
                status: pipelineStatus(
                    kind: "Rejected",
                    blockHeight: 41,
                    resolvedFrom: "state"
                ),
                loadDetails: { rejection },
                loadReceipt: { receipt }
            )
            XCTFail("rejected action accepted a finalized execution receipt")
        } catch {
            XCTAssertTrue(error.localizedDescription.contains("execution receipt"))
        }

        do {
            _ = try await ToriiClient.resolvePrivacyActionStatusV1(
                operation,
                status: pipelineStatus(
                    kind: "Applied",
                    blockHeight: 42,
                    resolvedFrom: "state"
                ),
                loadDetails: { nil },
                loadReceipt: { receipt }
            )
            XCTFail("applied action accepted a status/receipt height contradiction")
        } catch {
            XCTAssertTrue(error.localizedDescription.contains("finalized execution receipt"))
        }
    }

    func testStatusRejectsDetachedOperationViewBeforeNetwork() async throws {
        PrivacyDetachedOperationURLProtocol.counter.reset()
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [PrivacyDetachedOperationURLProtocol.self]
        let session = URLSession(configuration: configuration)
        defer { session.invalidateAndCancel() }
        let client = ToriiClient(
            baseURL: URL(string: "https://torii.example")!,
            session: session,
            localSigningContext: ToriiLocalSigningContext(
                networkId: TestNetworkIds.canonical
            )
        )
        let detached = try makeView(
            localState: .submitted,
            terminalChainState: nil,
            committedHeight: nil,
            rejectionReason: nil
        )
        let auth = ToriiCanonicalRequestAuth(
            accountId: "alice@universal",
            privateKey: Data(repeating: 0x41, count: 32)
        )

        do {
            _ = try await client.getPrivacyActionStatusV1(
                detached,
                canonicalAuth: auth
            )
            XCTFail("detached Exact12 operation view was accepted")
        } catch {
            XCTAssertTrue(
                error.localizedDescription.contains("authenticated submission")
            )
        }
        XCTAssertEqual(PrivacyDetachedOperationURLProtocol.counter.value, 0)
    }

    func testAuthenticatedProvenanceBindsClientNetworkAndSurvivesTerminalCopy() throws {
        let detached = try makeView(
            localState: .submitted,
            terminalChainState: nil,
            committedHeight: nil,
            rejectionReason: nil
        )
        let owner = PrivacyActionOperationProvenanceOwnerV1()
        let otherOwner = PrivacyActionOperationProvenanceOwnerV1()
        XCTAssertFalse(detached.hasAuthenticatedProvenance(
            owner: owner,
            networkId: TestNetworkIds.canonical
        ))

        let bound = detached.bindingAuthenticatedSubmission(
            owner: owner,
            networkId: TestNetworkIds.canonical
        )
        XCTAssertTrue(bound.hasAuthenticatedProvenance(
            owner: owner,
            networkId: TestNetworkIds.canonical
        ))
        XCTAssertFalse(bound.hasAuthenticatedProvenance(
            owner: otherOwner,
            networkId: TestNetworkIds.canonical
        ))
        XCTAssertFalse(bound.hasAuthenticatedProvenance(
            owner: owner,
            networkId: TestNetworkIds.other
        ))

        let terminal = try bound.replacingTerminalState(
            .applied,
            committedHeight: 17,
            rejectionReason: nil,
            executionCapabilityManifestDigest: fixed32(0x31),
            executionCapabilityCommittedHeight: 16,
            executionReceiptFinalizedHeight: 18,
            executionReceiptFinalizedBlockHash: fixed32(0x32)
        )
        XCTAssertTrue(terminal.hasAuthenticatedProvenance(
            owner: owner,
            networkId: TestNetworkIds.canonical
        ))
    }

    private func makeView(
        protocolId: PrivacyProtocolIdV1? = nil,
        ledgerEffectKind: PrivacyLedgerEffectKindV1? = nil,
        transactionHash: Data? = nil,
        capabilityManifestDigest: Data? = nil,
        capabilityCommittedHeight: UInt64 = 10,
        localState: PrivacyActionLocalStateV1,
        terminalChainState: PrivacyActionTerminalChainStateV1?,
        committedHeight: UInt64?,
        rejectionReason: String?,
        executionCapabilityManifestDigest: Data? = nil,
        executionCapabilityCommittedHeight: UInt64? = nil,
        executionReceiptFinalizedHeight: UInt64? = nil,
        executionReceiptFinalizedBlockHash: Data? = nil
    ) throws -> PrivacyActionOperationViewV1 {
        let operation = PrivacyExact12ActionOperationV1.orchardNoteActionV1
        return try PrivacyActionOperationViewV1(
            protocolId: protocolId ?? operation.protocolId,
            operationSchema: operation,
            transactionHash: transactionHash ?? fixed32(0x01),
            transactionIntentDigest: fixed32(0x02),
            statementDigest: fixed32(0x03),
            proofEnvelopeHash: fixed32(0x04),
            localState: localState,
            terminalChainState: terminalChainState,
            committedHeight: committedHeight,
            rejectionReason: rejectionReason,
            ledgerEffectKind: ledgerEffectKind ?? operation.ledgerEffectKind,
            capabilityManifestDigest: capabilityManifestDigest ?? fixed32(0x05),
            capabilityCommittedHeight: capabilityCommittedHeight,
            executionCapabilityManifestDigest: executionCapabilityManifestDigest,
            executionCapabilityCommittedHeight: executionCapabilityCommittedHeight,
            executionReceiptFinalizedHeight: executionReceiptFinalizedHeight,
            executionReceiptFinalizedBlockHash: executionReceiptFinalizedBlockHash
        )
    }

    private func fixed32(_ byte: UInt8) -> Data {
        Data(repeating: byte, count: 32)
    }

    private func committedResultJSON(
        resultOK: Bool,
        reasonJSON: String,
        heightJSON: String,
        version: UInt32 = 1,
        extraField: String = ""
    ) -> Data {
        Data(
            """
            {"version":\(version),"transaction_hash_hex":"\(String(repeating: "ab", count: 32))","transaction_authority":"canonical-authority","block_hash_hex":"\(String(repeating: "cd", count: 32))","result_hash_hex":"\(String(repeating: "ef", count: 32))","result_ok":\(resultOK ? "true" : "false"),"rejection_message":\(reasonJSON),"committed_block_height":\(heightJSON)\(extraField)}
            """.utf8
        )
    }

    private func registrationResultJSON(
        version: UInt32 = 1,
        transactionHash: String? = nil,
        height: String = "7",
        terminalState: String = "applied",
        eligibilityOutcomeJSON: String = "null",
        eligibilityReasonJSON: String = "null",
        matchedRulesJSON: String = "[]",
        rejectionCodeJSON: String = "null",
        rejectionMessageJSON: String = "null",
        extraField: String = ""
    ) -> Data {
        Data(
            """
            {"version":\(version),"transaction_hash_hex":"\(transactionHash ?? String(repeating: "ab", count: 32))","transaction_authority":"canonical-authority","block_hash_hex":"\(String(repeating: "cd", count: 32))","result_hash_hex":"\(String(repeating: "ef", count: 32))","committed_block_height":"\(height)","terminal_state":"\(terminalState)","eligibility_outcome":\(eligibilityOutcomeJSON),"eligibility_reason":\(eligibilityReasonJSON),"matched_rule_ids":\(matchedRulesJSON),"rejection_code":\(rejectionCodeJSON),"rejection_message":\(rejectionMessageJSON)\(extraField)}
            """.utf8
        )
    }

    private func executionReceiptJSON(
        version: UInt16 = 1,
        actionIndex: UInt32 = 0,
        capabilityHeight: String = "40",
        admittedHeight: String = "41",
        finalizedHeight: String = "42",
        networkIdHex: String? = nil,
        extraField: String = ""
    ) -> Data {
        let hex: (UInt8) -> String = { String(repeating: String(format: "%02x", $0), count: 32) }
        return Data(
            """
            {"version":\(version),"network_id":"\(networkIdHex ?? hex(0x11))","protocol_id":"orchard-halo2-actions-v1","operation_schema":"orchard_note_action_v1","ledger_effect_kind":"orchard_note_state_transition","transaction_hash":"\(hex(0x01))","action_index":\(actionIndex),"transaction_intent_digest":"\(hex(0x02))","statement_digest":"\(hex(0x03))","proof_envelope_hash":"\(hex(0x04))","capability_manifest_digest":"\(hex(0x16))","capability_committed_height":"\(capabilityHeight)","admitted_at_height":"\(admittedHeight)","finalized_height":"\(finalizedHeight)","finalized_block_hash":"\(hex(0x17))"\(extraField)}
            """.utf8
        )
    }

    private func pipelineStatus(
        kind: String,
        blockHeight: UInt64?,
        resolvedFrom: String
    ) throws -> ToriiPipelineTransactionStatus {
        let heightField = blockHeight.map { ",\"block_height\":\($0)" } ?? ""
        return try JSONDecoder().decode(
            ToriiPipelineTransactionStatus.self,
            from: Data(
                """
                {"hash":"\(String(repeating: "01", count: 32))","status":{"kind":"\(kind)"\(heightField)},"scope":"global","resolved_from":"\(resolvedFrom)"}
                """.utf8
            )
        )
    }
}

private final class PrivacyDetachedOperationURLProtocol: URLProtocol {
    static let counter = PrivacyDetachedOperationDispatchCounter()

    override class func canInit(with request: URLRequest) -> Bool { true }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        Self.counter.increment()
        client?.urlProtocol(
            self,
            didFailWithError: NSError(domain: "unexpected-exact12-dispatch", code: 1)
        )
    }

    override func stopLoading() {}
}

private final class PrivacyDetachedOperationDispatchCounter: @unchecked Sendable {
    private let lock = NSLock()
    private var storage = 0

    var value: Int {
        lock.lock()
        defer { lock.unlock() }
        return storage
    }

    func increment() {
        lock.lock()
        storage += 1
        lock.unlock()
    }

    func reset() {
        lock.lock()
        storage = 0
        lock.unlock()
    }
}
