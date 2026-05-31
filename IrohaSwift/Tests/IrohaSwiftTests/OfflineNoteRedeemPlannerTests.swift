import XCTest
@testable import IrohaSwift

final class OfflineNoteRedeemPlannerTests: XCTestCase {
    func testPartialRedeemDraftSplitsIssuedNoteAndRedeemsIssuedOutput() throws {
        let source = try makeOwnedInput(amount: "500")

        let draft = try OfflineNoteRedeemPlanner.partialRedeemDraft(
            input: source,
            redeemAmount: "80",
            paymentRequestId: "redeem-split-op-1",
            createdAtMs: 1_800_000_000,
            tokenNonce: bytes(0x70),
            redeemNoteSecret: bytes(0x80),
            changeNoteSecret: bytes(0x42)
        )

        XCTAssertEqual(draft.redeemAmount, "80")
        XCTAssertEqual(draft.changeAmount, "420")
        XCTAssertEqual(draft.audit.inputClaims, [try source.issuedClaim()])
        XCTAssertEqual(draft.audit.inputNullifiers, [try source.inputNullifier()])
        XCTAssertEqual(draft.audit.outputClaims.map(\.amount), ["80", "420"])
        XCTAssertEqual(draft.audit.outputCommitments, [
            draft.redeemOutput.noteCommitment,
            draft.changeOutput.noteCommitment
        ])
        XCTAssertEqual(
            try OfflineNoteIssuedClaim.fromAuditOutput(draft.audit.outputClaims[0]),
            try draft.redemption.issuedClaim()
        )
        XCTAssertEqual(draft.redemption.sourceNoteCommitment, draft.redeemOutput.noteCommitment)
        XCTAssertEqual(draft.redemption.amount, "80")
        XCTAssertEqual(draft.auditInstanceValues.inputAmounts[0], 500)
        XCTAssertEqual(draft.auditInstanceValues.outputAmounts[0], 80)
        XCTAssertEqual(draft.auditInstanceValues.outputAmounts[1], 420)
        XCTAssertEqual(draft.redemptionInstanceValues.inputAmounts[0], 80)
        XCTAssertEqual(draft.redemptionInstanceValues.outputAmounts[0], 80)
        XCTAssertThrowsError(try draft.audit.validateProofBinding()) { error in
            XCTAssertEqual(
                error as? OfflineNoteError,
                .unsupportedRecursiveProofBackend(
                    expected: OfflineNoteConstants.recursiveBackend,
                    actual: "offline-note/draft-placeholder"
                )
            )
        }
        XCTAssertThrowsError(try draft.redemption.validateProofBinding()) { error in
            XCTAssertEqual(
                error as? OfflineNoteError,
                .unsupportedRecursiveProofBackend(
                    expected: OfflineNoteConstants.recursiveBackend,
                    actual: "offline-note/draft-placeholder"
                )
            )
        }

        let auditProof = try proof(publicInputsHash: draft.audit.publicInputsHash(), marker: 0xA1)
        let redeemProof = try proof(publicInputsHash: draft.redemption.publicInputsHash(), marker: 0xA2)
        let plan = try OfflineNoteRedeemPlanner.finalizePartialRedeem(
            draft,
            auditProof: auditProof,
            redeemProof: redeemProof
        )

        XCTAssertEqual(plan.redemption, try draft.redemption.replacingRecursiveProof(redeemProof))
        XCTAssertEqual(plan.changeOutput?.amount, "420")
        XCTAssertEqual(plan.redeemOutput?.amount, "80")
        let splitToken = try XCTUnwrap(plan.splitPaymentToken)
        XCTAssertEqual(splitToken.audit, try draft.audit.replacingRecursiveProof(auditProof))
        XCTAssertEqual(splitToken.bearerAuditTrail.map(\.tokenId), [splitToken.tokenId])
        XCTAssertEqual(plan.bearerAuditTrail.map(\.tokenId), [splitToken.tokenId])

        let decoded = try OfflineNotePaymentTokenCodec.decodeNorito(
            OfflineNotePaymentTokenCodec.encodeNorito(splitToken)
        )
        XCTAssertEqual(decoded.tokenId, splitToken.tokenId)
        XCTAssertEqual(decoded.bearerAuditTrail.map(\.tokenId), [splitToken.tokenId])
    }

    func testExactRedeemDraftUsesSourceNoteWithoutSplitToken() throws {
        let source = try makeOwnedInput(amount: "80")
        let draft = try OfflineNoteRedeemPlanner.exactRedeemDraft(input: source)

        XCTAssertEqual(draft.redemption.sourceNoteCommitment, source.noteCommitment)
        XCTAssertEqual(draft.redemption.inputNullifiers, [try source.inputNullifier()])
        XCTAssertEqual(draft.redemption.amount, "80")
        XCTAssertEqual(draft.instanceValues.inputAmounts[0], 80)
        XCTAssertEqual(draft.instanceValues.outputAmounts[0], 80)

        let recursiveProof = try proof(publicInputsHash: draft.redemption.publicInputsHash(), marker: 0xE1)
        let plan = try OfflineNoteRedeemPlanner.finalizeExactRedeem(draft, recursiveProof: recursiveProof)

        XCTAssertEqual(plan.redemption, try draft.redemption.replacingRecursiveProof(recursiveProof))
        XCTAssertTrue(plan.bearerAuditTrail.isEmpty)
        XCTAssertNil(plan.splitPaymentToken)
        XCTAssertNil(plan.changeOutput)
    }

    func testPartialRedeemRejectsEqualOrOversizedAmount() throws {
        let source = try makeOwnedInput(amount: "500")

        XCTAssertThrowsError(
            try OfflineNoteRedeemPlanner.partialRedeemDraft(
                input: source,
                redeemAmount: "500",
                paymentRequestId: "redeem-split-op-equal",
                createdAtMs: 1,
                tokenNonce: bytes(0x01),
                redeemNoteSecret: bytes(0x02),
                changeNoteSecret: bytes(0x03)
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteRedeemPlannerError, .exactRedeemRequired)
        }

        XCTAssertThrowsError(
            try OfflineNoteRedeemPlanner.partialRedeemDraft(
                input: source,
                redeemAmount: "501",
                paymentRequestId: "redeem-split-op-large",
                createdAtMs: 1,
                tokenNonce: bytes(0x04),
                redeemNoteSecret: bytes(0x05),
                changeNoteSecret: bytes(0x06)
            )
        ) { error in
            XCTAssertEqual(
                error as? OfflineNoteRedeemPlannerError,
                .insufficientAmount(requested: "501", available: "500")
            )
        }
    }

    func testOwnedInputRejectsWrongNoteSecretForOrigin() throws {
        let source = try makeOwnedInput(amount: "500")

        XCTAssertThrowsError(
            try OfflineNoteOwnedInput(
                chainId: source.chainId,
                accountId: source.accountId,
                assetId: source.assetId,
                amount: source.amount,
                keyCertificate: source.keyCertificate,
                noteCommitment: source.noteCommitment,
                noteSecret: bytes(0x99),
                origin: source.origin
            )
        ) { error in
            guard case .commitmentMismatch = error as? OfflineNoteRedeemPlannerError else {
                return XCTFail("Expected commitmentMismatch, got \(error)")
            }
        }
    }

    func testFinalizeRejectsTamperedProofBindings() throws {
        let source = try makeOwnedInput(amount: "500")
        let draft = try OfflineNoteRedeemPlanner.partialRedeemDraft(
            input: source,
            redeemAmount: "80",
            paymentRequestId: "redeem-split-op-proof",
            createdAtMs: 1_800_000_100,
            tokenNonce: bytes(0x10),
            redeemNoteSecret: bytes(0x11),
            changeNoteSecret: bytes(0x12)
        )
        let auditProof = try proof(publicInputsHash: draft.audit.publicInputsHash(), marker: 0x01)
        let redeemProof = try proof(publicInputsHash: draft.redemption.publicInputsHash(), marker: 0x02)
        let wrongProof = try proof(publicInputsHash: IrohaHash.hash(Data("wrong-proof-binding".utf8)), marker: 0x03)

        XCTAssertThrowsError(
            try OfflineNoteRedeemPlanner.finalizePartialRedeem(
                draft,
                auditProof: wrongProof,
                redeemProof: redeemProof
            )
        ) { error in
            XCTAssertTrue(error is OfflineNoteError)
        }

        XCTAssertThrowsError(
            try OfflineNoteRedeemPlanner.finalizePartialRedeem(
                draft,
                auditProof: auditProof,
                redeemProof: wrongProof
            )
        ) { error in
            XCTAssertTrue(error is OfflineNoteError)
        }
    }

    private func makeOwnedInput(amount: String) throws -> OfflineNoteOwnedInput {
        let accountId = try accountId(seed: 1)
        let certificate = try OfflineNoteKeyCertificate(
            platform: "ios-appattest",
            keyId: "key-1",
            deviceId: "device-1",
            accountId: accountId,
            publicKey: bytes(0x31),
            assertionScheme: "apple-appattest-counter",
            assertionKeyAlgorithm: "app-attest-p256",
            assertionPublicKey: bytes(0x32),
            assertionUsageCountLimit: 1,
            issuerSignature: Data(repeating: 0x33, count: 64)
        )
        let assetId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM#\(accountId)"
        let origin = try OfflineNoteCommitmentOrigin.issuerLoad(
            OfflineNoteIssuerLoadOrigin(
                operationId: "issue-op-1",
                lineageId: "lineage-1",
                localRevision: 7
            )
        )
        let noteSecret = bytes(0x44)
        let noteCommitment = try OfflineNoteCommitmentPreimage(
            chainId: "chain-1",
            ownerKeyCertificatePayloadHash: certificate.payloadHash(),
            assetId: assetId,
            amount: amount,
            noteSecret: noteSecret,
            origin: origin
        ).deriveNoteCommitment()
        return try OfflineNoteOwnedInput(
            chainId: "chain-1",
            accountId: accountId,
            assetId: assetId,
            amount: amount,
            keyCertificate: certificate,
            noteCommitment: noteCommitment,
            noteSecret: noteSecret,
            origin: origin
        )
    }

    private func accountId(seed: UInt8) throws -> String {
        let keypair = try Keypair(privateKeyBytes: Data(repeating: seed, count: 32))
        return try AccountAddress.fromAccount(publicKey: keypair.publicKey).toI105(networkPrefix: 0x02F1)
    }

    private func bytes(_ value: UInt8) -> Data {
        Data(repeating: value, count: 32)
    }

    private func proof(publicInputsHash: Data, marker: UInt8) throws -> OfflineNoteRecursiveProof {
        try OfflineNoteRecursiveProof(
            publicInputsHash: publicInputsHash,
            proofBytes: Data([marker, 0xAA])
        )
    }
}
