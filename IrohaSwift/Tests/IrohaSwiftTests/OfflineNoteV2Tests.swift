import XCTest
import CryptoKit
@testable import IrohaSwift

final class AttestedOfflineNoteTests: XCTestCase {
    private func assertRetiredOfflineNotePayment(
        _ expression: @autoclosure () throws -> SignedTransactionEnvelope,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        XCTAssertThrowsError(try expression(), file: file, line: line) { error in
            guard case SwiftTransactionEncoderError.retiredOfflineNotePayment = error else {
                return XCTFail("expected retiredOfflineNotePayment, got \(error)", file: file, line: line)
            }
        }
    }

    func testCertificateSigningBytesMatchRustVector() throws {
        let fixture = try Self.loadFixture()
        let sender = try Self.certificate(fixture.paymentToken.senderKeyCertificate)

        XCTAssertEqual(
            try sender.signingBytes().base64EncodedString(),
            fixture.chainVectors.certificates.senderPayloadBase64
        )
        XCTAssertEqual(
            try sender.payloadHash().hexLowercased(),
            fixture.chainVectors.certificates.senderPayloadHash
        )
    }

    func testAttestedOfflineNoteModelsMatchRustNoritoVectors() throws {
        let fixture = try Self.loadFixture()

        XCTAssertEqual(
            try Self.issue(fixture).noritoEncoded().base64EncodedString(),
            fixture.chainVectors.issue.noritoBase64
        )
        XCTAssertEqual(
            try Self.audit(fixture).noritoEncoded().base64EncodedString(),
            fixture.chainVectors.audit.noritoBase64
        )
        XCTAssertEqual(
            try Self.redeem(fixture).noritoEncoded().base64EncodedString(),
            fixture.chainVectors.redeem.noritoBase64
        )
    }

    func testOfflineNoteV2DecodersRoundTripRustNoritoVectors() throws {
        let fixture = try Self.loadFixture()
        let certificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let certificatePayload = try certificate.signingPayload()
        let issue = try Self.issue(fixture)
        let issuedClaim = try issue.issuedClaim()
        let audit = try Self.audit(fixture)
        let auditOutputClaim = audit.outputClaims[0]
        let auditPublicInputs = try audit.publicInputs()
        let redeem = try Self.redeem(fixture)
        let redeemPublicInputs = try redeem.publicInputs()

        XCTAssertEqual(
            try AttestedOfflineNoteDecoding.decodeCertificatePayload(certificatePayload.noritoEncoded()),
            certificatePayload
        )
        XCTAssertEqual(
            try AttestedOfflineNoteDecoding.decodeKeyCertificatePayload(certificatePayload.noritoEncoded()),
            certificatePayload
        )
        XCTAssertEqual(try AttestedOfflineNoteDecoding.decodeCertificate(certificate.noritoEncoded()), certificate)
        XCTAssertEqual(try AttestedOfflineNoteDecoding.decodeIssue(issue.noritoEncoded()), issue)
        XCTAssertEqual(try AttestedOfflineNoteDecoding.decodeIssuedClaim(issuedClaim.noritoEncoded()), issuedClaim)
        XCTAssertEqual(
            try AttestedOfflineNoteDecoding.decodeAuditOutputClaim(auditOutputClaim.noritoEncoded()),
            auditOutputClaim
        )
        XCTAssertEqual(
            try AttestedOfflineNoteDecoding.decodeRecursiveProof(audit.recursiveProof.noritoEncoded()),
            audit.recursiveProof
        )
        XCTAssertEqual(try AttestedOfflineNoteDecoding.decodeAudit(audit.noritoEncoded()), audit)
        XCTAssertEqual(
            try AttestedOfflineNoteDecoding.decodeAuditPublicInputs(auditPublicInputs.noritoEncoded()),
            auditPublicInputs
        )
        XCTAssertEqual(
            try AttestedOfflineNoteDecoding.decodeRecursiveProof(redeem.recursiveProof.noritoEncoded()),
            redeem.recursiveProof
        )
        XCTAssertEqual(try AttestedOfflineNoteDecoding.decodeRedeem(redeem.noritoEncoded()), redeem)
        XCTAssertEqual(
            try AttestedOfflineNoteDecoding.decodeRedeemPublicInputs(redeemPublicInputs.noritoEncoded()),
            redeemPublicInputs
        )

        XCTAssertEqual(
            try AttestedOfflineNoteDecoding.decodeIssue(try Self.base64(fixture.chainVectors.issue.noritoBase64)).noritoEncoded(),
            try issue.noritoEncoded()
        )
        XCTAssertEqual(
            try AttestedOfflineNoteDecoding.decodeAudit(try Self.base64(fixture.chainVectors.audit.noritoBase64)).noritoEncoded(),
            try audit.noritoEncoded()
        )
        XCTAssertEqual(
            try AttestedOfflineNoteDecoding.decodeRedeem(try Self.base64(fixture.chainVectors.redeem.noritoBase64)).noritoEncoded(),
            try redeem.noritoEncoded()
        )
    }

    func testOfflineNoteV2InstructionDecodersReadExplorerEnvelopeBytes() throws {
        let fixture = try Self.loadFixture()
        let issue = try Self.issue(fixture)
        let audit = try Self.audit(fixture)
        let redeem = try Self.redeem(fixture)

        XCTAssertEqual(AttestedOfflineNoteTypeNames.issueInstruction, OfflineNoteTypeNames.issueInstruction)
        XCTAssertEqual(AttestedOfflineNoteTypeNames.redeemInstruction, OfflineNoteTypeNames.redeemInstruction)
        XCTAssertEqual(AttestedOfflineNoteTypeNames.auditInstruction, OfflineNoteTypeNames.auditInstruction)
        XCTAssertFalse(AttestedOfflineNoteTypeNames.issueInstruction.hasSuffix("V2"))
        XCTAssertFalse(AttestedOfflineNoteTypeNames.redeemInstruction.hasSuffix("V2"))
        XCTAssertFalse(AttestedOfflineNoteTypeNames.auditInstruction.hasSuffix("V2"))

        let issueInstruction = ParsedAttestedOfflineNoteInstruction(
            wireName: AttestedOfflineNoteTypeNames.issueInstruction,
            archive: Self.instructionWirePayload(
                typeName: AttestedOfflineNoteTypeNames.issueInstruction,
                modelPayload: try AttestedOfflineNoteEncoding.encodeIssue(issue)
            )
        )
        XCTAssertFalse(issueInstruction.wireName.hasSuffix("V2"))
        let issueEnvelope = Self.rawInstructionPair(
            wireName: issueInstruction.wireName,
            wirePayload: issueInstruction.archive
        )
        XCTAssertEqual(
            try AttestedOfflineNoteDecoding.decodeIssueInstruction(issueEnvelope).noritoEncoded().base64EncodedString(),
            try issue.noritoEncoded().base64EncodedString()
        )
        XCTAssertEqual(
            try AttestedOfflineNoteDecoding.decodeIssueInstruction(issueInstruction.archive).noritoEncoded().base64EncodedString(),
            try issue.noritoEncoded().base64EncodedString()
        )

        let auditInstruction = ParsedAttestedOfflineNoteInstruction(
            wireName: AttestedOfflineNoteTypeNames.auditInstruction,
            archive: Self.instructionWirePayload(
                typeName: AttestedOfflineNoteTypeNames.auditInstruction,
                modelPayload: try AttestedOfflineNoteEncoding.encodeAudit(audit)
            )
        )
        XCTAssertFalse(auditInstruction.wireName.hasSuffix("V2"))
        let auditEnvelope = Self.rawInstructionPair(
            wireName: auditInstruction.wireName,
            wirePayload: auditInstruction.archive,
            compact: false
        )
        XCTAssertEqual(
            try AttestedOfflineNoteDecoding.decodeAuditInstruction(auditEnvelope).noritoEncoded().base64EncodedString(),
            try audit.noritoEncoded().base64EncodedString()
        )
        XCTAssertEqual(
            try AttestedOfflineNoteDecoding.decodeAuditInstruction(auditInstruction.archive).noritoEncoded().base64EncodedString(),
            try audit.noritoEncoded().base64EncodedString()
        )

        let redeemInstruction = ParsedAttestedOfflineNoteInstruction(
            wireName: AttestedOfflineNoteTypeNames.redeemInstruction,
            archive: Self.instructionWirePayload(
                typeName: AttestedOfflineNoteTypeNames.redeemInstruction,
                modelPayload: try AttestedOfflineNoteEncoding.encodeRedeem(redeem)
            )
        )
        XCTAssertFalse(redeemInstruction.wireName.hasSuffix("V2"))
        let redeemEnvelope = Self.rawInstructionPair(
            wireName: redeemInstruction.wireName,
            wirePayload: redeemInstruction.archive
        )
        XCTAssertEqual(
            try AttestedOfflineNoteDecoding.decodeRedeemInstruction(redeemEnvelope).noritoEncoded().base64EncodedString(),
            try redeem.noritoEncoded().base64EncodedString()
        )
        XCTAssertEqual(
            try AttestedOfflineNoteDecoding.decodeRedeemInstruction(redeemInstruction.archive).noritoEncoded().base64EncodedString(),
            try redeem.noritoEncoded().base64EncodedString()
        )
    }

    func testOfflineNoteV2InstructionDecodersRejectRetiredAliasEnvelopeBytes() throws {
        let fixture = try Self.loadFixture()
        let issue = try Self.issue(fixture)
        let audit = try Self.audit(fixture)
        let redeem = try Self.redeem(fixture)
        let retiredIssueInstructionAlias = "iroha_data_model::isi::offline::IssueOfflineNoteV2"
        let retiredAuditInstructionAlias = "iroha_data_model::isi::offline::AuditOfflineNoteV2"
        let retiredRedeemInstructionAlias = "iroha_data_model::isi::offline::RedeemOfflineNoteV2"
        let issueAliasWirePayload = Self.instructionWirePayload(
            typeName: retiredIssueInstructionAlias,
            modelPayload: try AttestedOfflineNoteEncoding.encodeIssue(issue)
        )
        let auditAliasWirePayload = Self.instructionWirePayload(
            typeName: retiredAuditInstructionAlias,
            modelPayload: try AttestedOfflineNoteEncoding.encodeAudit(audit)
        )
        let redeemAliasWirePayload = Self.instructionWirePayload(
            typeName: retiredRedeemInstructionAlias,
            modelPayload: try AttestedOfflineNoteEncoding.encodeRedeem(redeem)
        )

        XCTAssertThrowsError(try AttestedOfflineNoteDecoding.decodeIssueInstruction(issueAliasWirePayload))
        XCTAssertThrowsError(try AttestedOfflineNoteDecoding.decodeAuditInstruction(auditAliasWirePayload))
        XCTAssertThrowsError(try AttestedOfflineNoteDecoding.decodeRedeemInstruction(redeemAliasWirePayload))
        XCTAssertThrowsError(
            try AttestedOfflineNoteDecoding.decodeIssueInstruction(Self.rawInstructionPair(
                wireName: retiredIssueInstructionAlias,
                wirePayload: issueAliasWirePayload
            ))
        )
        XCTAssertThrowsError(
            try AttestedOfflineNoteDecoding.decodeAuditInstruction(Self.rawInstructionPair(
                wireName: retiredAuditInstructionAlias,
                wirePayload: auditAliasWirePayload
            ))
        )
        XCTAssertThrowsError(
            try AttestedOfflineNoteDecoding.decodeRedeemInstruction(Self.rawInstructionPair(
                wireName: retiredRedeemInstructionAlias,
                wirePayload: redeemAliasWirePayload
            ))
        )
    }

    func testOfflineNoteV2InstructionDecodersRejectWrongEnvelopeShapes() throws {
        let fixture = try Self.loadFixture()
        let issue = try Self.issue(fixture)
        let issueWirePayload = Self.instructionWirePayload(
            typeName: AttestedOfflineNoteTypeNames.issueInstruction,
            modelPayload: try AttestedOfflineNoteEncoding.encodeIssue(issue)
        )
        let issueEnvelope = Self.rawInstructionPair(
            wireName: AttestedOfflineNoteTypeNames.issueInstruction,
            wirePayload: issueWirePayload
        )
        let wrongWireEnvelope = Self.rawInstructionPair(
            wireName: AttestedOfflineNoteTypeNames.redeemInstruction,
            wirePayload: issueWirePayload
        )
        let wrongSchemaPayload = Self.instructionWirePayload(
            typeName: AttestedOfflineNoteTypeNames.redeemInstruction,
            modelPayload: try AttestedOfflineNoteEncoding.encodeIssue(issue)
        )
        let wrongSchemaEnvelope = Self.rawInstructionPair(
            wireName: AttestedOfflineNoteTypeNames.issueInstruction,
            wirePayload: wrongSchemaPayload
        )

        XCTAssertThrowsError(try AttestedOfflineNoteDecoding.decodeRedeemInstruction(issueEnvelope))
        XCTAssertThrowsError(try AttestedOfflineNoteDecoding.decodeIssueInstruction(wrongWireEnvelope))
        XCTAssertThrowsError(try AttestedOfflineNoteDecoding.decodeIssueInstruction(wrongSchemaEnvelope))
        XCTAssertThrowsError(try AttestedOfflineNoteDecoding.decodeIssueInstruction(Data(issueEnvelope.dropLast())))
    }

    func testOfflineNoteV2DecodersRejectMalformedPayloads() throws {
        let fixture = try Self.loadFixture()
        let certificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let issue = try Self.issue(fixture)
        let issueBytes = try issue.noritoEncoded()

        XCTAssertThrowsError(try AttestedOfflineNoteDecoding.decodeIssue(Data(issueBytes.dropLast())))
        XCTAssertThrowsError(try AttestedOfflineNoteDecoding.decodeIssue(try certificate.noritoEncoded()))
        XCTAssertThrowsError(try AttestedOfflineNoteDecoding.decodeCertificatePayload(try certificate.noritoEncoded()))

        var corruptedChecksum = issueBytes
        corruptedChecksum[corruptedChecksum.count - 1] ^= 0x01
        XCTAssertThrowsError(try AttestedOfflineNoteDecoding.decodeIssue(corruptedChecksum))

        let nonCompactIssue = noritoEncode(
            typeName: AttestedOfflineNoteTypeNames.issue,
            payload: try AttestedOfflineNoteEncoding.encodeIssue(issue)
        )
        XCTAssertThrowsError(try AttestedOfflineNoteDecoding.decodeIssue(nonCompactIssue))

        var trailingPayload = try AttestedOfflineNoteEncoding.encodeIssue(issue)
        trailingPayload.append(0)
        let trailingIssue = AttestedOfflineNoteEncoding.wrap(typeName: AttestedOfflineNoteTypeNames.issue, payload: trailingPayload)
        XCTAssertThrowsError(try AttestedOfflineNoteDecoding.decodeIssue(trailingIssue))

        let invalidProof = AttestedOfflineNoteEncoding.wrap(
            typeName: AttestedOfflineNoteTypeNames.recursiveProof,
            payload: Self.recursiveProofPayload(
                publicInputsHash: Data(repeating: 0x02, count: 32),
                proofBackend: AttestedOfflineNoteConstants.recursiveBackend,
                proofBytes: Data([0x01])
            )
        )
        XCTAssertThrowsError(try AttestedOfflineNoteDecoding.decodeRecursiveProof(invalidProof)) { error in
            XCTAssertEqual(error as? AttestedOfflineNoteError, .invalidHash(field: "public_inputs_hash"))
        }

        let emptyProofBytes = AttestedOfflineNoteEncoding.wrap(
            typeName: AttestedOfflineNoteTypeNames.recursiveProof,
            payload: Self.recursiveProofPayload(
                publicInputsHash: try issue.issuedClaim().claimHash(),
                proofBackend: AttestedOfflineNoteConstants.recursiveBackend,
                proofBytes: Data()
            )
        )
        XCTAssertThrowsError(try AttestedOfflineNoteDecoding.decodeRecursiveProof(emptyProofBytes)) { error in
            XCTAssertEqual(error as? AttestedOfflineNoteError, .emptyProofBytes)
        }
    }

    func testAttestedOfflineNotePublicInputHashesMatchRustVectors() throws {
        let fixture = try Self.loadFixture()
        let audit = try Self.audit(fixture)
        let redeem = try Self.redeem(fixture)

        XCTAssertEqual(try audit.publicInputsHash().hexLowercased(), fixture.chainVectors.audit.publicInputsHash)
        XCTAssertEqual(try redeem.publicInputsHash().hexLowercased(), fixture.chainVectors.redeem.publicInputsHash)
        XCTAssertNoThrow(try audit.validateProofBinding())
        XCTAssertNoThrow(try redeem.validateProofBinding())
    }

    func testOfflineDeviceAttestationRegistrationMatchesRustVectors() throws {
        let fixture = try Self.loadFixture()
        let registration = try Self.attestationRegistration(fixture)
        let vector = fixture.chainVectors.attestationRegistration

        XCTAssertEqual(try registration.canonicalChallengeHash().hexLowercased(), vector.challengeHash)
        XCTAssertEqual(registration.challengeHash.hexLowercased(), vector.challengeHash)
        XCTAssertEqual(registration.attestationReportHash.hexLowercased(), vector.attestationReportHash)
        XCTAssertEqual(registration.evidenceHash.hexLowercased(), vector.evidenceHash)
        let keyCertificatePayload = try registration.keyCertificatePayload()
        XCTAssertEqual(
            IrohaHash.hash(try keyCertificatePayload.noritoEncoded()).hexLowercased(),
            vector.keyCertificatePayloadHash
        )
        XCTAssertEqual(try registration.keyCertificatePayloadHash().hexLowercased(), vector.keyCertificatePayloadHash)
        XCTAssertEqual(try registration.noritoEncoded().base64EncodedString(), vector.noritoBase64)

        let changedReport = Data("other-report".utf8)
        let changed = try registration.replacingAttestationEvidence(
            attestationReport: changedReport,
            evidence: Self.attestationEvidence(attestationReportHash: IrohaHash.hash(changedReport))
        )
        XCTAssertEqual(try changed.canonicalChallengeHash(), try registration.canonicalChallengeHash())
        XCTAssertNotEqual(changed.attestationReportHash, registration.attestationReportHash)
        XCTAssertNotEqual(changed.evidenceHash, registration.evidenceHash)
    }

    func testOfflineDeviceAttestationRegistrationDraftBuildsChallengeBeforeEvidence() throws {
        let fixture = try Self.loadFixture()
        let vector = fixture.chainVectors.attestationRegistration
        let preAttestationChallenge = try OfflineDeviceAttestationRegistration
            .preAttestationChallengeHash(
                version: vector.version,
                platform: vector.platform,
                keyId: vector.keyId,
                deviceId: vector.deviceId,
                accountId: vector.accountId,
                assetDefinitionId: vector.assetDefinitionId,
                iosTeamId: vector.iosTeamId,
                iosBundleId: vector.iosBundleId,
                iosEnvironment: vector.iosEnvironment,
                androidPackageName: vector.androidPackageName,
                androidSigningCertificateSha256: try vector.androidSigningCertificateSha256.map(Self.hex),
                publicKey: try Self.base64(vector.publicKey),
                assertionScheme: vector.assertionScheme,
                assertionKeyAlgorithm: vector.assertionKeyAlgorithm,
                assertionUsageCountLimit: vector.assertionUsageCountLimit,
                oneUse: vector.oneUse,
                recentBlockHeight: vector.recentBlockHeight,
                recentBlockHash: try Self.hex(vector.recentBlockHash),
                expiresAtMs: vector.expiresAtMs
            )
        let draft = try OfflineDeviceAttestationRegistration(
            version: vector.version,
            platform: vector.platform,
            keyId: vector.keyId,
            deviceId: vector.deviceId,
            accountId: vector.accountId,
            assetDefinitionId: vector.assetDefinitionId,
            iosTeamId: vector.iosTeamId,
            iosBundleId: vector.iosBundleId,
            iosEnvironment: vector.iosEnvironment,
            androidPackageName: vector.androidPackageName,
            androidSigningCertificateSha256: try vector.androidSigningCertificateSha256.map(Self.hex),
            publicKey: try Self.base64(vector.publicKey),
            assertionScheme: vector.assertionScheme,
            assertionKeyAlgorithm: vector.assertionKeyAlgorithm,
            assertionPublicKey: try Self.base64(vector.assertionPublicKey),
            assertionUsageCountLimit: vector.assertionUsageCountLimit,
            oneUse: vector.oneUse,
            recentBlockHeight: vector.recentBlockHeight,
            recentBlockHash: try Self.hex(vector.recentBlockHash),
            expiresAtMs: vector.expiresAtMs
        )
        let emptyReportHash = IrohaHash.hash(Data())
        let expectedEvidence = Self.attestationEvidence(attestationReportHash: emptyReportHash)

        XCTAssertEqual(try draft.canonicalChallengeHash().hexLowercased(), vector.challengeHash)
        XCTAssertEqual(preAttestationChallenge, draft.challengeHash)
        XCTAssertEqual(draft.challengeHash.hexLowercased(), vector.challengeHash)
        XCTAssertEqual(draft.attestationReportHash, emptyReportHash)
        XCTAssertEqual(draft.attestationReport, Data())
        XCTAssertEqual(draft.evidence, expectedEvidence)
        XCTAssertEqual(draft.evidenceHash, IrohaHash.hash(expectedEvidence))
    }

    func testAttestedOfflineNotePaymentTransactionBuildersAreRetiredAndRegistrationStillSigns() throws {
        let fixture = try Self.loadFixture()
        let keypair = try Keypair(privateKeyBytes: Data(0..<32))
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let chainId = "00000000-0000-0000-0000-000000000000"
        let creationTimeMs: UInt64 = 1_706_000_000_000
        let issueModel = try Self.issue(fixture)
        let auditModel = try Self.audit(fixture)
        let redeemModel = try Self.redeem(fixture)
        let registrationModel = try Self.attestationRegistration(fixture)

        assertRetiredOfflineNotePayment(try SwiftTransactionEncoder.encodeAttestedOfflineNoteIssue(
            request: AttestedOfflineNoteIssueRequest(
                chainId: chainId,
                authority: authority,
                issue: issueModel,
                ttlMs: 60_000
            ),
            keypair: keypair,
            creationTimeMs: creationTimeMs
        ))
        assertRetiredOfflineNotePayment(try SwiftTransactionEncoder.encodeAttestedOfflineNoteAudit(
            request: AttestedOfflineNoteAuditRequest(
                chainId: chainId,
                authority: authority,
                audit: auditModel,
                ttlMs: 60_000
            ),
            keypair: keypair,
            creationTimeMs: creationTimeMs
        ))
        assertRetiredOfflineNotePayment(try SwiftTransactionEncoder.encodeAttestedOfflineNoteRedeem(
            request: AttestedOfflineNoteRedeemRequest(
                chainId: chainId,
                authority: authority,
                redemption: redeemModel,
                ttlMs: 60_000
            ),
            keypair: keypair,
            creationTimeMs: creationTimeMs
        ))
        let registrationRequest = RegisterOfflineDeviceAttestationRequest(
            chainId: chainId,
            authority: authority,
            registration: registrationModel,
            ttlMs: 60_000
        )
        let registerAttestation = try SwiftTransactionEncoder.encodeRegisterOfflineDeviceAttestation(
            request: registrationRequest,
            keypair: keypair,
            creationTimeMs: creationTimeMs
        )
        let unsigned = try SwiftTransactionEncoder.encodeUnsignedRegisterOfflineDeviceAttestation(
            request: registrationRequest,
            creationTimeMs: creationTimeMs
        )
        let externalSignature = try SigningKey.ed25519(
            privateKey: keypair.privateKeyBytes
        ).sign(unsigned.signingHash)
        let externallySigned = try unsigned.signed(signature: externalSignature)
        XCTAssertEqual(externallySigned.norito, registerAttestation.norito)
        XCTAssertEqual(externallySigned.transactionHash, registerAttestation.transactionHash)

        XCTAssertEqual(registerAttestation.norito.first, 1)
        XCTAssertEqual(Data(registerAttestation.norito.dropFirst()), registerAttestation.signedTransaction)
        XCTAssertEqual(registerAttestation.transactionHash.count, 32)
        XCTAssertNil(registerAttestation.payload)
        XCTAssertCanonicalExternalEntrypointHash(registerAttestation)

        let registerInstruction = try Self.parseSingleOfflineNoteV2Instruction(registerAttestation)
        XCTAssertEqual(registerInstruction.wireName, AttestedOfflineNoteTypeNames.registerDeviceAttestationInstruction)
        XCTAssertEqual(
            registerInstruction.archive.base64EncodedString(),
            fixture.chainVectors.attestationRegistration.instructionNoritoBase64
        )
    }

    func testRedeemBuilderRejectsMismatchedProofBinding() throws {
        let fixture = try Self.loadFixture()
        let redeem = try Self.redeem(fixture)
        let badProof = try AttestedOfflineNoteRecursiveProof(
            publicInputsHash: IrohaHash.hash(Data("wrong-public-inputs".utf8)),
            proofBytes: Data("offline-v2-vector-redeem-proof".utf8)
        )
        let forged = try AttestedOfflineNoteRedeem(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: redeem.inputNullifiers,
            senderKeyCertificate: redeem.senderKeyCertificate,
            recipient: redeem.recipient,
            assetId: redeem.assetId,
            amount: redeem.amount,
            recursiveProof: badProof
        )
        let keypair = try Keypair(privateKeyBytes: Data(0..<32))
        let authority = AccountId.make(publicKey: keypair.publicKey)

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeAttestedOfflineNoteRedeem(
                request: AttestedOfflineNoteRedeemRequest(
                    chainId: "00000000-0000-0000-0000-000000000000",
                    authority: authority,
                    redemption: forged
                ),
                keypair: keypair,
                creationTimeMs: 1
            )
        ) { error in
            guard case AttestedOfflineNoteError.proofPublicInputsHashMismatch = error else {
                return XCTFail("expected proofPublicInputsHashMismatch, got \(error)")
            }
        }
    }

    func testOfflineNoteV2ProofAndHashValidationRejectsMalformedValues() throws {
        let fixture = try Self.loadFixture()
        let publicInputsHash = try Self.hex(fixture.chainVectors.audit.publicInputsHash)

        let proof = try AttestedOfflineNoteProofBox(
            backend: AttestedOfflineNoteConstants.recursiveBackend,
            bytes: Data([0x01])
        )
        XCTAssertEqual(proof.backend, AttestedOfflineNoteConstants.recursiveBackend)

        XCTAssertThrowsError(try AttestedOfflineNoteProofBox(
            backend: "  \(AttestedOfflineNoteConstants.recursiveBackend)  ",
            bytes: Data([0x01])
        )) { error in
            XCTAssertEqual(
                error as? AttestedOfflineNoteError,
                .unsupportedRecursiveProofBackend(
                    expected: AttestedOfflineNoteConstants.recursiveBackend,
                    actual: "  \(AttestedOfflineNoteConstants.recursiveBackend)  "
                )
            )
        }

        XCTAssertThrowsError(try AttestedOfflineNoteProofBox(backend: " \n ", bytes: Data([0x01]))) { error in
            XCTAssertEqual(error as? AttestedOfflineNoteError, .emptyProofBackend)
        }
        XCTAssertThrowsError(try AttestedOfflineNoteProofBox(backend: "halo2/ipa", bytes: Data())) { error in
            XCTAssertEqual(error as? AttestedOfflineNoteError, .emptyProofBytes)
        }
        XCTAssertThrowsError(try AttestedOfflineNoteRecursiveProof(
            publicInputsHash: Data(repeating: 0x01, count: 31),
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(
                error as? AttestedOfflineNoteError,
                .invalidHashLength(field: "public_inputs_hash", expected: 32, actual: 31)
            )
        }

        var nonCanonicalHash = publicInputsHash
        nonCanonicalHash[31] &= 0xfe
        XCTAssertThrowsError(try AttestedOfflineNoteRecursiveProof(
            publicInputsHash: nonCanonicalHash,
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(error as? AttestedOfflineNoteError, .invalidHash(field: "public_inputs_hash"))
        }
    }

    func testAttestedOfflineNoteCertificateValidationRejectsMalformedValues() throws {
        let fixture = try Self.loadFixture()
        let cert = fixture.paymentToken.senderKeyCertificate
        let publicKey = try Self.base64(cert.publicKey)
        let assertionPublicKey = try Self.base64(cert.assertionPublicKey)
        let issuerSignature = try Self.base64(cert.issuerSignatureBase64)

        XCTAssertThrowsError(try AttestedOfflineNoteKeyCertificate(
            version: AttestedOfflineNoteConstants.keyCertificateVersion + 1,
            platform: cert.platform,
            keyId: cert.keyId,
            deviceId: cert.deviceId,
            accountId: cert.accountId,
            publicKey: publicKey,
            assertionScheme: cert.assertionScheme,
            assertionKeyAlgorithm: cert.assertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey,
            assertionUsageCountLimit: cert.assertionUsageCountLimit,
            oneUse: true,
            issuerSignature: issuerSignature
        )) { error in
            XCTAssertEqual(
                error as? AttestedOfflineNoteError,
                .invalidCertificateVersion(AttestedOfflineNoteConstants.keyCertificateVersion + 1)
            )
        }
        XCTAssertThrowsError(try AttestedOfflineNoteKeyCertificate(
            platform: cert.platform,
            keyId: cert.keyId,
            deviceId: cert.deviceId,
            accountId: cert.accountId,
            publicKey: publicKey,
            assertionScheme: cert.assertionScheme,
            assertionKeyAlgorithm: cert.assertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey,
            assertionUsageCountLimit: cert.assertionUsageCountLimit,
            oneUse: false,
            issuerSignature: issuerSignature
        )) { error in
            XCTAssertEqual(error as? AttestedOfflineNoteError, .certificateMustBeOneUse)
        }
        XCTAssertThrowsError(try AttestedOfflineNoteKeyCertificate(
            platform: cert.platform,
            keyId: cert.keyId,
            deviceId: cert.deviceId,
            accountId: cert.accountId,
            publicKey: Data(publicKey.dropLast()),
            assertionScheme: cert.assertionScheme,
            assertionKeyAlgorithm: cert.assertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey,
            assertionUsageCountLimit: cert.assertionUsageCountLimit,
            oneUse: true,
            issuerSignature: issuerSignature
        )) { error in
            XCTAssertEqual(
                error as? AttestedOfflineNoteError,
                .invalidNotePublicKeyLength(expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try AttestedOfflineNoteKeyCertificate(
            platform: cert.platform,
            keyId: cert.keyId,
            deviceId: cert.deviceId,
            accountId: cert.accountId,
            publicKey: publicKey,
            assertionScheme: cert.assertionScheme,
            assertionKeyAlgorithm: cert.assertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey,
            assertionUsageCountLimit: cert.assertionUsageCountLimit,
            oneUse: true,
            issuerSignature: Data(issuerSignature.dropLast())
        )) { error in
            XCTAssertEqual(
                error as? AttestedOfflineNoteError,
                .invalidIssuerSignatureLength(expected: 64, actual: 63)
            )
        }
        XCTAssertThrowsError(try AttestedOfflineNoteKeyCertificate(
            platform: cert.platform,
            keyId: cert.keyId,
            deviceId: cert.deviceId,
            accountId: cert.accountId,
            publicKey: publicKey,
            assertionScheme: cert.assertionScheme,
            assertionKeyAlgorithm: cert.assertionKeyAlgorithm,
            assertionPublicKey: Self.offCurveP256AssertionPublicKey(),
            assertionUsageCountLimit: cert.assertionUsageCountLimit,
            oneUse: true,
            issuerSignature: issuerSignature
        )) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try AttestedOfflineNoteKeyCertificate(
            platform: cert.platform,
            keyId: cert.keyId,
            deviceId: cert.deviceId,
            accountId: cert.accountId,
            publicKey: publicKey,
            assertionScheme: "apple-app-attest-v1",
            assertionKeyAlgorithm: cert.assertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey,
            assertionUsageCountLimit: cert.assertionUsageCountLimit,
            oneUse: true,
            issuerSignature: issuerSignature
        )) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
    }

    func testOfflineDeviceAttestationRegistrationValidationRejectsMalformedValues() throws {
        let fixture = try Self.loadFixture()
        let vector = fixture.chainVectors.attestationRegistration

        var badChallenge = try Self.hex(vector.challengeHash)
        badChallenge[0] ^= 0x01
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, challengeHash: badChallenge)) { error in
            guard case AttestedOfflineNoteError.deviceAttestationChallengeHashMismatch = error else {
                return XCTFail("expected deviceAttestationChallengeHashMismatch, got \(error)")
            }
        }

        var badReportHash = try Self.hex(vector.attestationReportHash)
        badReportHash[0] ^= 0x01
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, attestationReportHash: badReportHash)) { error in
            XCTAssertEqual(error as? AttestedOfflineNoteError, .deviceAttestationHashMismatch(field: "attestation_report_hash"))
        }

        var badEvidenceHash = try Self.hex(vector.evidenceHash)
        badEvidenceHash[0] ^= 0x01
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, evidenceHash: badEvidenceHash)) { error in
            XCTAssertEqual(error as? AttestedOfflineNoteError, .deviceAttestationHashMismatch(field: "evidence_hash"))
        }

        let forgedEvidence = Self.attestationEvidence(attestationReportHash: Data(repeating: 0xA5, count: 32))
        XCTAssertThrowsError(try Self.attestationRegistration(
            fixture,
            evidenceHash: IrohaHash.hash(forgedEvidence),
            evidence: forgedEvidence
        )) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }

        XCTAssertThrowsError(try Self.attestationRegistration(
            fixture,
            androidSigningCertificateSha256: Data(repeating: 0x01, count: 31)
        )) { error in
            XCTAssertEqual(
                error as? AttestedOfflineNoteError,
                .invalidDigestLength(field: "android_signing_certificate_sha256", expected: 32, actual: 31)
            )
        }

        XCTAssertThrowsError(try Self.attestationRegistration(fixture, publicKey: Data(repeating: 0x01, count: 31))) { error in
            XCTAssertEqual(error as? AttestedOfflineNoteError, .invalidNotePublicKeyLength(expected: 32, actual: 31))
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, keyId: "not standard base64!")) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, keyId: "AB==")) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, keyId: " \(vector.keyId) ")) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, deviceId: " \(vector.deviceId) ")) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, deviceId: "\u{00A0}\u{2003}")) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, iosTeamId: " \(vector.iosTeamId ?? "") ")) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, iosBundleId: "\(vector.iosBundleId ?? "")\n")) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, iosEnvironment: "\t\(vector.iosEnvironment ?? "")")) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, androidPackageName: " jp.co.soramitsu.iroha.offline ")) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(
            fixture,
            assertionPublicKey: Self.offCurveP256AssertionPublicKey()
        )) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, recentBlockHash: Data(repeating: 0x01, count: 31))) { error in
            XCTAssertEqual(
                error as? AttestedOfflineNoteError,
                .invalidHashLength(field: "recent_block_hash", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, oneUse: false)) { error in
            XCTAssertEqual(error as? AttestedOfflineNoteError, .certificateMustBeOneUse)
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, assetDefinitionId: "cash#bad"))
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, assertionUsageCountLimit: 1)) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(
            fixture,
            platform: AttestedOfflineNoteConstants.androidKeyMintPlatform,
            assertionScheme: AttestedOfflineNoteConstants.androidKeyMintAssertionScheme,
            assertionKeyAlgorithm: AttestedOfflineNoteConstants.androidKeyMintAssertionKeyAlgorithm
        )) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(
            fixture,
            platform: AttestedOfflineNoteConstants.androidKeyMintPlatform,
            assertionScheme: "android-keymint-ecdsa-p256-usage-limit",
            assertionKeyAlgorithm: AttestedOfflineNoteConstants.androidKeyMintAssertionKeyAlgorithm,
            assertionUsageCountLimit: 1
        )) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(
            fixture,
            keyId: String(repeating: "00", count: 32),
            platform: AttestedOfflineNoteConstants.androidKeyMintPlatform,
            assertionScheme: AttestedOfflineNoteConstants.androidKeyMintAssertionScheme,
            assertionKeyAlgorithm: AttestedOfflineNoteConstants.androidKeyMintAssertionKeyAlgorithm,
            assertionUsageCountLimit: 1
        )) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        let androidUppercaseKeyId = Data(SHA256.hash(data: try Self.base64(vector.assertionPublicKey)))
            .hexLowercased()
            .uppercased()
        XCTAssertThrowsError(try Self.attestationRegistration(
            fixture,
            keyId: androidUppercaseKeyId,
            platform: AttestedOfflineNoteConstants.androidKeyMintPlatform,
            assertionScheme: AttestedOfflineNoteConstants.androidKeyMintAssertionScheme,
            assertionKeyAlgorithm: AttestedOfflineNoteConstants.androidKeyMintAssertionKeyAlgorithm,
            assertionUsageCountLimit: 1
        )) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, platform: "ios-app-attest")) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
    }

    func testOfflineDeviceAttestationRegistrationUsesValueSemanticsForData() throws {
        let fixture = try Self.loadFixture()
        let vector = fixture.chainVectors.attestationRegistration
        var publicKey = try Self.base64(vector.publicKey)
        var assertionPublicKey = try Self.base64(vector.assertionPublicKey)
        var attestationReport = try Self.base64(vector.attestationReportBase64)
        var evidence = try Self.base64(vector.evidenceBase64)
        var recentBlockHash = try Self.hex(vector.recentBlockHash)
        let registration = try Self.attestationRegistration(
            fixture,
            publicKey: publicKey,
            assertionPublicKey: assertionPublicKey,
            attestationReport: attestationReport,
            evidence: evidence,
            recentBlockHash: recentBlockHash
        )
        let encoded = try registration.noritoEncoded()

        publicKey[0] ^= 0x01
        assertionPublicKey[0] ^= 0x01
        attestationReport[0] ^= 0x01
        evidence[0] ^= 0x01
        recentBlockHash[0] ^= 0x01
        XCTAssertEqual(encoded.base64EncodedString(), vector.noritoBase64)
        XCTAssertEqual(try registration.noritoEncoded(), encoded)

        var returnedPublicKey = registration.publicKey
        returnedPublicKey[0] ^= 0x01
        var returnedReport = registration.attestationReport
        returnedReport[0] ^= 0x01
        var returnedEvidence = registration.evidence
        returnedEvidence[0] ^= 0x01
        XCTAssertEqual(try registration.noritoEncoded(), encoded)
    }

    func testAttestedOfflineNoteAuditBundleRejectsInvalidShapes() throws {
        let fixture = try Self.loadFixture()
        let audit = try Self.audit(fixture)

        XCTAssertThrowsError(try AttestedOfflineNoteAuditBundle(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: [],
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: audit.outputClaims,
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(error as? AttestedOfflineNoteError, .emptyInputNullifiers)
        }
        XCTAssertThrowsError(try AttestedOfflineNoteAuditBundle(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: [],
            outputCommitments: audit.outputCommitments,
            outputClaims: audit.outputClaims,
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(error as? AttestedOfflineNoteError, .emptyInputClaims)
        }
        XCTAssertThrowsError(try AttestedOfflineNoteAuditBundle(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers + [audit.inputNullifiers[0]],
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: audit.outputClaims,
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(
                error as? AttestedOfflineNoteError,
                .auditInputCountMismatch(nullifiers: audit.inputNullifiers.count + 1, claims: audit.inputClaims.count)
            )
        }
        XCTAssertThrowsError(try AttestedOfflineNoteAuditBundle(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: [],
            outputClaims: audit.outputClaims,
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(error as? AttestedOfflineNoteError, .emptyOutputCommitments)
        }
        XCTAssertThrowsError(try AttestedOfflineNoteAuditBundle(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: [],
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(error as? AttestedOfflineNoteError, .emptyOutputClaims)
        }

        let uncommittedClaim = try AttestedOfflineNoteAuditOutputClaim(
            noteCommitment: Data(repeating: 0x03, count: 32),
            keyCertificate: audit.outputClaims[0].keyCertificate,
            assetId: audit.outputClaims[0].assetId,
            amount: audit.outputClaims[0].amount
        )
        XCTAssertThrowsError(try AttestedOfflineNoteAuditBundle(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: [uncommittedClaim],
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(
                error as? AttestedOfflineNoteError,
                .auditOutputClaimNotCommitted(uncommittedClaim.noteCommitment.hexLowercased())
            )
        }
    }

    func testOfflineNoteV2IssueAndClaimValidationCoversDerivedClaimAndFailures() throws {
        let fixture = try Self.loadFixture()
        let certificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let noteCommitment = try Self.hex(fixture.chainVectors.issue.noteCommitment)
        let issue = try AttestedOfflineNoteIssue(
            noteCommitment: noteCommitment,
            keyCertificate: certificate,
            assetId: fixture.chainVectors.issue.assetId,
            amount: "5.5000"
        )

        XCTAssertEqual(issue.amount, "5.5000")
        let claim = try issue.issuedClaim()
        XCTAssertEqual(claim.domain, AttestedOfflineNoteConstants.issuedClaimDomain)
        XCTAssertEqual(claim.noteCommitment, issue.noteCommitment)
        XCTAssertEqual(claim.keyCertificatePayloadHash, try certificate.payloadHash())
        XCTAssertEqual(claim.assetId, issue.assetId)
        XCTAssertEqual(claim.amount, "5.5000")
        XCTAssertEqual(try claim.claimHash().count, 32)
        XCTAssertThrowsError(try AttestedOfflineNoteIssuedClaim(
            noteCommitment: issue.noteCommitment,
            keyCertificatePayloadHash: try certificate.payloadHash(),
            assetId: issue.assetId,
            amount: "05.5000"
        )) { error in
            XCTAssertEqual(error as? AttestedOfflineNoteError, .nonCanonicalField(field: "amount"))
        }

        XCTAssertThrowsError(try AttestedOfflineNoteIssue(
            noteCommitment: Data(repeating: 0x01, count: 31),
            keyCertificate: certificate,
            assetId: fixture.chainVectors.issue.assetId,
            amount: fixture.chainVectors.issue.amount
        )) { error in
            XCTAssertEqual(
                error as? AttestedOfflineNoteError,
                .invalidHashLength(field: "note_commitment", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try AttestedOfflineNoteIssue(
            noteCommitment: noteCommitment,
            keyCertificate: certificate,
            assetId: "cash#branch.sbp",
            amount: fixture.chainVectors.issue.amount
        )) { error in
            guard case OfflineNoritoError.invalidAssetId("cash#branch.sbp") = error else {
                return XCTFail("expected invalidAssetId, got \(error)")
            }
        }
        XCTAssertThrowsError(try AttestedOfflineNoteIssue(
            noteCommitment: noteCommitment,
            keyCertificate: certificate,
            assetId: fixture.chainVectors.issue.assetId,
            amount: "not-a-number"
        )) { error in
            guard case OfflineNoritoError.invalidNumeric("not-a-number") = error else {
                return XCTFail("expected invalidNumeric, got \(error)")
            }
        }
    }

    func testAttestedOfflineNoteRedeemValidationRejectsBadInputsAndDerivesIssuedClaim() throws {
        let fixture = try Self.loadFixture()
        let redeem = try Self.redeem(fixture)
        let issuedClaim = try redeem.issuedClaim()

        XCTAssertEqual(issuedClaim.noteCommitment, redeem.sourceNoteCommitment)
        XCTAssertEqual(issuedClaim.keyCertificatePayloadHash, try redeem.senderKeyCertificate.payloadHash())
        XCTAssertEqual(issuedClaim.assetId, redeem.assetId)
        XCTAssertEqual(issuedClaim.amount, redeem.amount)

        XCTAssertThrowsError(try AttestedOfflineNoteRedeem(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: [],
            senderKeyCertificate: redeem.senderKeyCertificate,
            recipient: redeem.recipient,
            assetId: redeem.assetId,
            amount: redeem.amount,
            recursiveProof: redeem.recursiveProof
        )) { error in
            XCTAssertEqual(error as? AttestedOfflineNoteError, .emptyInputNullifiers)
        }
        XCTAssertThrowsError(try AttestedOfflineNoteRedeem(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: [Data(repeating: 0x01, count: 31)],
            senderKeyCertificate: redeem.senderKeyCertificate,
            recipient: redeem.recipient,
            assetId: redeem.assetId,
            amount: redeem.amount,
            recursiveProof: redeem.recursiveProof
        )) { error in
            XCTAssertEqual(
                error as? AttestedOfflineNoteError,
                .invalidHashLength(field: "input_nullifiers[0]", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try AttestedOfflineNoteRedeem(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: redeem.inputNullifiers,
            senderKeyCertificate: redeem.senderKeyCertificate,
            recipient: "\(redeem.recipient)@bad",
            assetId: redeem.assetId,
            amount: redeem.amount,
            recursiveProof: redeem.recursiveProof
        ))
    }

    func testAttestedOfflineNoteAuditValidateProofBindingReportsExpectedAndActualHashes() throws {
        let fixture = try Self.loadFixture()
        let audit = try Self.audit(fixture)
        var wrongPublicInputsHash = try audit.publicInputsHash()
        wrongPublicInputsHash[0] ^= 0x01
        let forgedProof = try AttestedOfflineNoteRecursiveProof(
            publicInputsHash: wrongPublicInputsHash,
            proofBytes: audit.recursiveProof.proof.bytes
        )
        let forgedAudit = try AttestedOfflineNoteAuditBundle(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: audit.outputClaims,
            recursiveProof: forgedProof
        )

        XCTAssertThrowsError(try forgedAudit.validateProofBinding()) { error in
            guard case let AttestedOfflineNoteError.proofPublicInputsHashMismatch(expected, actual) = error else {
                return XCTFail("expected proofPublicInputsHashMismatch, got \(error)")
            }
            XCTAssertEqual(expected, try? audit.publicInputsHash().hexLowercased())
            XCTAssertEqual(actual, forgedProof.publicInputsHash.hexLowercased())
        }
    }

    func testAttestedOfflineNoteTransactionBuilderCoversOptionalNonceAndInputValidation() throws {
        let fixture = try Self.loadFixture()
        let keypair = try Keypair(privateKeyBytes: Data(0..<32))
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let chainId = "00000000-0000-0000-0000-000000000000"
        let issue = try Self.issue(fixture)

        assertRetiredOfflineNotePayment(try SwiftTransactionEncoder.encodeAttestedOfflineNoteIssue(
            request: AttestedOfflineNoteIssueRequest(chainId: chainId, authority: authority, issue: issue),
            keypair: keypair,
            creationTimeMs: 1_706_000_000_000
        ))
        assertRetiredOfflineNotePayment(try SwiftTransactionEncoder.encodeAttestedOfflineNoteIssue(
            request: AttestedOfflineNoteIssueRequest(
                chainId: chainId,
                authority: authority,
                issue: issue,
                ttlMs: nil,
                nonce: 42
            ),
            keypair: keypair,
            creationTimeMs: 1_706_000_000_000
        ))

        XCTAssertThrowsError(try SwiftTransactionEncoder.encodeAttestedOfflineNoteIssue(
            request: AttestedOfflineNoteIssueRequest(chainId: "  \(chainId)  ", authority: authority, issue: issue),
            keypair: keypair,
            creationTimeMs: 1
        )) { error in
            XCTAssertEqual(error as? TransactionInputError, .invalidChainId("  \(chainId)  "))
        }
        XCTAssertThrowsError(try SwiftTransactionEncoder.encodeAttestedOfflineNoteIssue(
            request: AttestedOfflineNoteIssueRequest(chainId: chainId, authority: "  \(authority)  ", issue: issue),
            keypair: keypair,
            creationTimeMs: 1
        )) { error in
            XCTAssertEqual(
                error as? TransactionInputError,
                .malformedAccountId(field: "authority", value: "  \(authority)  ")
            )
        }
        XCTAssertThrowsError(try SwiftTransactionEncoder.encodeAttestedOfflineNoteIssue(
            request: AttestedOfflineNoteIssueRequest(chainId: " \n ", authority: authority, issue: issue),
            keypair: keypair,
            creationTimeMs: 1
        )) { error in
            XCTAssertEqual(error as? TransactionInputError, .emptyChainId)
        }
        XCTAssertThrowsError(try SwiftTransactionEncoder.encodeAttestedOfflineNoteIssue(
            request: AttestedOfflineNoteIssueRequest(chainId: chainId, authority: "\(authority)@bad", issue: issue),
            keypair: keypair,
            creationTimeMs: 1
        )) { error in
            XCTAssertEqual(
                error as? TransactionInputError,
                .malformedAccountId(field: "authority", value: "\(authority)@bad")
            )
        }
    }

    func testOfflineNoteV2RecursiveProofCoversCustomVerifierAndVerifierValidation() throws {
        let publicInputsHash = try Self.audit(Self.loadFixture()).publicInputsHash()
        let proof = try AttestedOfflineNoteRecursiveProof(
            verifierBackend: "custom_backend",
            verifierName: "custom_vk",
            publicInputsHash: publicInputsHash,
            proofBytes: Data([0x01, 0x02, 0x03]),
            proofBackend: "custom_proof_backend"
        )

        XCTAssertEqual(proof.verifierKeyId.backend, "custom_backend")
        XCTAssertEqual(proof.verifierKeyId.name, "custom_vk")
        XCTAssertEqual(proof.proof.backend, "custom_proof_backend")
        XCTAssertEqual(proof.proof.bytes, Data([0x01, 0x02, 0x03]))

        XCTAssertThrowsError(try AttestedOfflineNoteRecursiveProof(
            verifierBackend: "custom_backend",
            verifierName: "custom_vk",
            publicInputsHash: publicInputsHash,
            proofBytes: Data([0x01]),
            proofBackend: " custom_proof_backend "
        )) { error in
            XCTAssertEqual(
                error as? AttestedOfflineNoteError,
                .unsupportedRecursiveProofBackend(
                    expected: AttestedOfflineNoteConstants.recursiveBackend,
                    actual: " custom_proof_backend "
                )
            )
        }

        XCTAssertThrowsError(try AttestedOfflineNoteRecursiveProof(
            verifierBackend: "",
            verifierName: "custom_vk",
            publicInputsHash: publicInputsHash,
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(error as? VerifyingKeyIdError, .emptyBackend)
        }
        XCTAssertThrowsError(try AttestedOfflineNoteRecursiveProof(
            verifierBackend: "custom_backend",
            verifierName: "",
            publicInputsHash: publicInputsHash,
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(error as? VerifyingKeyIdError, .emptyName)
        }
        XCTAssertThrowsError(try AttestedOfflineNoteRecursiveProof(
            verifierBackend: "halo2:ipa",
            verifierName: "custom_vk",
            publicInputsHash: publicInputsHash,
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(error as? VerifyingKeyIdError, .invalidSeparator)
        }
        XCTAssertThrowsError(try AttestedOfflineNoteRecursiveProof(
            verifierBackend: " custom_backend ",
            verifierName: "custom_vk",
            publicInputsHash: publicInputsHash,
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(error as? VerifyingKeyIdError, .surroundingWhitespace)
        }
        XCTAssertThrowsError(try AttestedOfflineNoteRecursiveProof(
            verifierBackend: "custom_backend",
            verifierName: " custom_vk ",
            publicInputsHash: publicInputsHash,
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(error as? VerifyingKeyIdError, .surroundingWhitespace)
        }
    }

    func testAttestedOfflineNoteCertificatePayloadValidationAndEncodingBranches() throws {
        let certificate = try Self.certificate(Self.loadFixture().paymentToken.senderKeyCertificate)
        let payload = try certificate.signingPayload()

        XCTAssertEqual(payload.domain, AttestedOfflineNoteConstants.keyCertificatePayloadDomain)
        XCTAssertEqual(payload.version, certificate.version)
        XCTAssertEqual(payload.publicKey, certificate.publicKey)
        XCTAssertEqual(payload.oneUse, true)
        XCTAssertNotEqual(try payload.noritoEncoded(), try certificate.noritoEncoded())

        let noLimitPayload = try AttestedOfflineNoteKeyCertificatePayload(
            version: AttestedOfflineNoteConstants.keyCertificateVersion,
            platform: certificate.platform,
            keyId: certificate.keyId,
            deviceId: certificate.deviceId,
            accountId: certificate.accountId,
            publicKey: certificate.publicKey,
            assertionScheme: certificate.assertionScheme,
            assertionKeyAlgorithm: certificate.assertionKeyAlgorithm,
            assertionPublicKey: certificate.assertionPublicKey,
            assertionUsageCountLimit: nil,
            oneUse: true
        )
        let limitedPayload = try AttestedOfflineNoteKeyCertificatePayload(
            version: AttestedOfflineNoteConstants.keyCertificateVersion,
            platform: AttestedOfflineNoteConstants.androidKeyMintPlatform,
            keyId: Data(SHA256.hash(data: certificate.assertionPublicKey)).hexLowercased(),
            deviceId: certificate.deviceId,
            accountId: certificate.accountId,
            publicKey: certificate.publicKey,
            assertionScheme: AttestedOfflineNoteConstants.androidKeyMintAssertionScheme,
            assertionKeyAlgorithm: AttestedOfflineNoteConstants.androidKeyMintAssertionKeyAlgorithm,
            assertionPublicKey: certificate.assertionPublicKey,
            assertionUsageCountLimit: 1,
            oneUse: true
        )
        XCTAssertNil(noLimitPayload.assertionUsageCountLimit)
        XCTAssertEqual(limitedPayload.assertionUsageCountLimit, 1)
        XCTAssertNotEqual(try noLimitPayload.noritoEncoded(), try limitedPayload.noritoEncoded())

        XCTAssertThrowsError(try AttestedOfflineNoteKeyCertificatePayload(
            version: AttestedOfflineNoteConstants.keyCertificateVersion,
            platform: certificate.platform,
            keyId: certificate.keyId,
            deviceId: certificate.deviceId,
            accountId: "\(certificate.accountId)@bad",
            publicKey: certificate.publicKey,
            assertionScheme: certificate.assertionScheme,
            assertionKeyAlgorithm: certificate.assertionKeyAlgorithm,
            assertionPublicKey: certificate.assertionPublicKey,
            assertionUsageCountLimit: certificate.assertionUsageCountLimit,
            oneUse: true
        ))
        XCTAssertThrowsError(try AttestedOfflineNoteKeyCertificatePayload(
            version: AttestedOfflineNoteConstants.keyCertificateVersion,
            platform: certificate.platform,
            keyId: certificate.keyId,
            deviceId: certificate.deviceId,
            accountId: certificate.accountId,
            publicKey: Data(certificate.publicKey.dropLast()),
            assertionScheme: certificate.assertionScheme,
            assertionKeyAlgorithm: certificate.assertionKeyAlgorithm,
            assertionPublicKey: certificate.assertionPublicKey,
            assertionUsageCountLimit: certificate.assertionUsageCountLimit,
            oneUse: true
        )) { error in
            XCTAssertEqual(
                error as? AttestedOfflineNoteError,
                .invalidNotePublicKeyLength(expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try AttestedOfflineNoteKeyCertificatePayload(
            version: AttestedOfflineNoteConstants.keyCertificateVersion,
            platform: certificate.platform,
            keyId: certificate.keyId,
            deviceId: "\u{00A0}\u{2003}",
            accountId: certificate.accountId,
            publicKey: certificate.publicKey,
            assertionScheme: certificate.assertionScheme,
            assertionKeyAlgorithm: certificate.assertionKeyAlgorithm,
            assertionPublicKey: certificate.assertionPublicKey,
            assertionUsageCountLimit: certificate.assertionUsageCountLimit,
            oneUse: true
        )) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try AttestedOfflineNoteKeyCertificate(
            version: AttestedOfflineNoteConstants.keyCertificateVersion,
            platform: certificate.platform,
            keyId: "\u{00A0}\u{2003}",
            deviceId: certificate.deviceId,
            accountId: certificate.accountId,
            publicKey: certificate.publicKey,
            assertionScheme: certificate.assertionScheme,
            assertionKeyAlgorithm: certificate.assertionKeyAlgorithm,
            assertionPublicKey: certificate.assertionPublicKey,
            assertionUsageCountLimit: certificate.assertionUsageCountLimit,
            oneUse: true,
            issuerSignature: certificate.issuerSignature
        )) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try AttestedOfflineNoteKeyCertificatePayload(
            version: AttestedOfflineNoteConstants.keyCertificateVersion,
            platform: "ios-app-attest",
            keyId: certificate.keyId,
            deviceId: certificate.deviceId,
            accountId: certificate.accountId,
            publicKey: certificate.publicKey,
            assertionScheme: "apple-app-attest-v1",
            assertionKeyAlgorithm: "ecdsa-p256-sha256",
            assertionPublicKey: certificate.assertionPublicKey,
            assertionUsageCountLimit: nil,
            oneUse: true
        )) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try AttestedOfflineNoteKeyCertificate(
            version: AttestedOfflineNoteConstants.keyCertificateVersion,
            platform: "ios-app-attest",
            keyId: certificate.keyId,
            deviceId: certificate.deviceId,
            accountId: certificate.accountId,
            publicKey: certificate.publicKey,
            assertionScheme: "apple-app-attest-v1",
            assertionKeyAlgorithm: "ecdsa-p256-sha256",
            assertionPublicKey: certificate.assertionPublicKey,
            assertionUsageCountLimit: nil,
            oneUse: true,
            issuerSignature: certificate.issuerSignature
        )) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try AttestedOfflineNoteKeyCertificatePayload(
            version: AttestedOfflineNoteConstants.keyCertificateVersion,
            platform: certificate.platform,
            keyId: certificate.keyId,
            deviceId: certificate.deviceId,
            accountId: certificate.accountId,
            publicKey: certificate.publicKey,
            assertionScheme: certificate.assertionScheme,
            assertionKeyAlgorithm: certificate.assertionKeyAlgorithm,
            assertionPublicKey: Self.offCurveP256AssertionPublicKey(),
            assertionUsageCountLimit: certificate.assertionUsageCountLimit,
            oneUse: true
        )) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try AttestedOfflineNoteKeyCertificatePayload(
            version: AttestedOfflineNoteConstants.keyCertificateVersion,
            platform: AttestedOfflineNoteConstants.androidKeyMintPlatform,
            keyId: Data(SHA256.hash(data: certificate.assertionPublicKey)).hexLowercased(),
            deviceId: certificate.deviceId,
            accountId: certificate.accountId,
            publicKey: certificate.publicKey,
            assertionScheme: AttestedOfflineNoteConstants.androidKeyMintAssertionScheme,
            assertionKeyAlgorithm: AttestedOfflineNoteConstants.androidKeyMintAssertionKeyAlgorithm,
            assertionPublicKey: certificate.assertionPublicKey,
            assertionUsageCountLimit: 7,
            oneUse: true
        )) { error in
            guard case AttestedOfflineNoteError.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
    }

    private static func offCurveP256AssertionPublicKey() -> Data {
        var key = Data(repeating: 0, count: 65)
        key[0] = 0x04
        return key
    }

    func testAttestedOfflineNotePublicInputConstructorsRejectMalformedInputs() throws {
        let fixture = try Self.loadFixture()
        let audit = try Self.audit(fixture)
        let redeem = try Self.redeem(fixture)
        let auditOutputClaims = try audit.outputClaims.map(AttestedOfflineNoteIssuedClaim.fromAuditOutput)

        XCTAssertThrowsError(try AttestedOfflineNoteRedeemPublicInputs(
            sourceNoteCommitment: Data(repeating: 0x01, count: 31),
            inputNullifiers: redeem.inputNullifiers,
            keyCertificatePayloadHash: try redeem.senderKeyCertificate.payloadHash(),
            recipient: redeem.recipient,
            assetId: redeem.assetId,
            amount: redeem.amount
        )) { error in
            XCTAssertEqual(
                error as? AttestedOfflineNoteError,
                .invalidHashLength(field: "source_note_commitment", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try AttestedOfflineNoteRedeemPublicInputs(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: redeem.inputNullifiers,
            keyCertificatePayloadHash: Data(repeating: 0x01, count: 31),
            recipient: redeem.recipient,
            assetId: redeem.assetId,
            amount: redeem.amount
        )) { error in
            XCTAssertEqual(
                error as? AttestedOfflineNoteError,
                .invalidHashLength(field: "key_certificate_payload_hash", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try AttestedOfflineNoteRedeemPublicInputs(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: redeem.inputNullifiers,
            keyCertificatePayloadHash: try redeem.senderKeyCertificate.payloadHash(),
            recipient: "\(redeem.recipient)@bad",
            assetId: redeem.assetId,
            amount: redeem.amount
        ))

        XCTAssertThrowsError(try AttestedOfflineNoteAuditPublicInputs(
            tokenId: Data(repeating: 0x01, count: 31),
            keyCertificatePayloadHash: try audit.senderKeyCertificate.payloadHash(),
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: auditOutputClaims
        )) { error in
            XCTAssertEqual(
                error as? AttestedOfflineNoteError,
                .invalidHashLength(field: "token_id", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try AttestedOfflineNoteAuditPublicInputs(
            tokenId: audit.tokenId,
            keyCertificatePayloadHash: try audit.senderKeyCertificate.payloadHash(),
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: []
        )) { error in
            XCTAssertEqual(error as? AttestedOfflineNoteError, .emptyOutputClaims)
        }
    }

    func testAttestedOfflineNoteDomainsRejectSubstitutionAndPadding() throws {
        let fixture = try Self.loadFixture()
        let certificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let audit = try Self.audit(fixture)
        let redeem = try Self.redeem(fixture)
        let claim = audit.inputClaims[0]
        let auditPublic = try audit.publicInputs()
        let redeemPublic = try redeem.publicInputs()

        XCTAssertThrowsError(try AttestedOfflineNoteKeyCertificatePayload(
            domain: "\(AttestedOfflineNoteConstants.keyCertificatePayloadDomain) ",
            version: certificate.version,
            platform: certificate.platform,
            keyId: certificate.keyId,
            deviceId: certificate.deviceId,
            accountId: certificate.accountId,
            publicKey: certificate.publicKey,
            assertionScheme: certificate.assertionScheme,
            assertionKeyAlgorithm: certificate.assertionKeyAlgorithm,
            assertionPublicKey: certificate.assertionPublicKey,
            assertionUsageCountLimit: certificate.assertionUsageCountLimit,
            oneUse: certificate.oneUse
        )) { error in
            XCTAssertEqual(
                error as? AttestedOfflineNoteError,
                .unsupportedDomain(
                    field: "domain",
                    expected: AttestedOfflineNoteConstants.keyCertificatePayloadDomain,
                    actual: "\(AttestedOfflineNoteConstants.keyCertificatePayloadDomain) "
                )
            )
        }
        XCTAssertThrowsError(try AttestedOfflineNoteIssuedClaim(
            domain: "\(AttestedOfflineNoteConstants.issuedClaimDomain)\n",
            noteCommitment: claim.noteCommitment,
            keyCertificatePayloadHash: claim.keyCertificatePayloadHash,
            assetId: claim.assetId,
            amount: claim.amount
        )) { error in
            XCTAssertEqual(
                error as? AttestedOfflineNoteError,
                .unsupportedDomain(
                    field: "domain",
                    expected: AttestedOfflineNoteConstants.issuedClaimDomain,
                    actual: "\(AttestedOfflineNoteConstants.issuedClaimDomain)\n"
                )
            )
        }
        XCTAssertThrowsError(try AttestedOfflineNoteRedeemPublicInputs(
            domain: "forged:\(AttestedOfflineNoteConstants.redeemPublicInputsDomain)",
            sourceNoteCommitment: redeemPublic.sourceNoteCommitment,
            inputNullifiers: redeemPublic.inputNullifiers,
            keyCertificatePayloadHash: redeemPublic.keyCertificatePayloadHash,
            recipient: redeemPublic.recipient,
            assetId: redeemPublic.assetId,
            amount: redeemPublic.amount
        )) { error in
            XCTAssertEqual(
                error as? AttestedOfflineNoteError,
                .unsupportedDomain(
                    field: "domain",
                    expected: AttestedOfflineNoteConstants.redeemPublicInputsDomain,
                    actual: "forged:\(AttestedOfflineNoteConstants.redeemPublicInputsDomain)"
                )
            )
        }
        XCTAssertThrowsError(try AttestedOfflineNoteAuditPublicInputs(
            domain: " \(AttestedOfflineNoteConstants.auditPublicInputsDomain)",
            tokenId: auditPublic.tokenId,
            keyCertificatePayloadHash: auditPublic.keyCertificatePayloadHash,
            inputNullifiers: auditPublic.inputNullifiers,
            inputClaims: auditPublic.inputClaims,
            outputCommitments: auditPublic.outputCommitments,
            outputClaims: auditPublic.outputClaims
        )) { error in
            XCTAssertEqual(
                error as? AttestedOfflineNoteError,
                .unsupportedDomain(
                    field: "domain",
                    expected: AttestedOfflineNoteConstants.auditPublicInputsDomain,
                    actual: " \(AttestedOfflineNoteConstants.auditPublicInputsDomain)"
                )
            )
        }
    }

    private static func issue(_ fixture: OfflineInteropFixture) throws -> AttestedOfflineNoteIssue {
        try AttestedOfflineNoteIssue(
            noteCommitment: hex(fixture.chainVectors.issue.noteCommitment),
            keyCertificate: certificate(fixture.paymentToken.senderKeyCertificate),
            assetId: fixture.chainVectors.issue.assetId,
            amount: fixture.chainVectors.issue.amount
        )
    }

    private static func redeem(_ fixture: OfflineInteropFixture) throws -> AttestedOfflineNoteRedeem {
        let vector = fixture.chainVectors.redeem
        return try AttestedOfflineNoteRedeem(
            sourceNoteCommitment: hex(vector.sourceNoteCommitment),
            inputNullifiers: try vector.inputNullifiers.map(hex),
            senderKeyCertificate: certificate(fixture.paymentToken.recipientKeyCertificate),
            recipient: fixture.paymentToken.recipientAccountId,
            assetId: vector.assetId,
            amount: vector.amount,
            recursiveProof: AttestedOfflineNoteRecursiveProof(
                publicInputsHash: hex(vector.publicInputsHash),
                proofBytes: Data("offline-v2-vector-redeem-proof".utf8)
            )
        )
    }

    private static func audit(_ fixture: OfflineInteropFixture) throws -> AttestedOfflineNoteAuditBundle {
        let vector = fixture.chainVectors.audit
        return try AttestedOfflineNoteAuditBundle(
            tokenId: hex(vector.tokenId),
            senderKeyCertificate: certificate(fixture.paymentToken.senderKeyCertificate),
            inputNullifiers: try vector.inputNullifiers.map(hex),
            inputClaims: try fixture.paymentToken.inputClaims.map(issuedClaim),
            outputCommitments: try vector.outputCommitments.map(hex),
            outputClaims: try fixture.paymentToken.outputClaims.map(auditOutputClaim),
            recursiveProof: AttestedOfflineNoteRecursiveProof(
                publicInputsHash: hex(vector.publicInputsHash),
                proofBytes: Data("offline-v2-vector-audit-proof".utf8)
            )
        )
    }

    private static func attestationRegistration(
        _ fixture: OfflineInteropFixture,
        challengeHash: Data? = nil,
        attestationReportHash: Data? = nil,
        evidenceHash: Data? = nil,
        androidSigningCertificateSha256: Data? = nil,
        publicKey: Data? = nil,
        assertionPublicKey: Data? = nil,
        keyId: String? = nil,
        deviceId: String? = nil,
        platform: String? = nil,
        assertionScheme: String? = nil,
        assertionKeyAlgorithm: String? = nil,
        assertionUsageCountLimit: UInt32? = nil,
        iosTeamId: String? = nil,
        iosBundleId: String? = nil,
        iosEnvironment: String? = nil,
        androidPackageName: String? = nil,
        attestationReport: Data? = nil,
        evidence: Data? = nil,
        recentBlockHash: Data? = nil,
        oneUse: Bool? = nil,
        assetDefinitionId: String? = nil
    ) throws -> OfflineDeviceAttestationRegistration {
        let vector = fixture.chainVectors.attestationRegistration
        return try OfflineDeviceAttestationRegistration(
            version: vector.version,
            platform: platform ?? vector.platform,
            keyId: keyId ?? vector.keyId,
            deviceId: deviceId ?? vector.deviceId,
            accountId: vector.accountId,
            assetDefinitionId: assetDefinitionId ?? vector.assetDefinitionId,
            iosTeamId: iosTeamId ?? vector.iosTeamId,
            iosBundleId: iosBundleId ?? vector.iosBundleId,
            iosEnvironment: iosEnvironment ?? vector.iosEnvironment,
            androidPackageName: androidPackageName ?? vector.androidPackageName,
            androidSigningCertificateSha256: androidSigningCertificateSha256
                ?? vector.androidSigningCertificateSha256.map(hex),
            publicKey: publicKey ?? (try base64(vector.publicKey)),
            assertionScheme: assertionScheme ?? vector.assertionScheme,
            assertionKeyAlgorithm: assertionKeyAlgorithm ?? vector.assertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey ?? (try base64(vector.assertionPublicKey)),
            assertionUsageCountLimit: assertionUsageCountLimit ?? vector.assertionUsageCountLimit,
            oneUse: oneUse ?? vector.oneUse,
            challengeHash: challengeHash ?? hex(vector.challengeHash),
            attestationReportHash: attestationReportHash ?? hex(vector.attestationReportHash),
            attestationReport: attestationReport ?? (try base64(vector.attestationReportBase64)),
            evidenceHash: evidenceHash ?? (try hex(vector.evidenceHash)),
            evidence: evidence ?? (try base64(vector.evidenceBase64)),
            recentBlockHeight: vector.recentBlockHeight,
            recentBlockHash: recentBlockHash ?? (try hex(vector.recentBlockHash)),
            expiresAtMs: vector.expiresAtMs
        )
    }

    private static func certificate(_ json: OfflineCertificateJSON) throws -> AttestedOfflineNoteKeyCertificate {
        try AttestedOfflineNoteKeyCertificate(
            version: json.version,
            platform: json.platform,
            keyId: json.keyId,
            deviceId: json.deviceId,
            accountId: json.accountId,
            publicKey: base64(json.publicKey),
            assertionScheme: json.assertionScheme,
            assertionKeyAlgorithm: json.assertionKeyAlgorithm,
            assertionPublicKey: base64(json.assertionPublicKey),
            assertionUsageCountLimit: json.assertionUsageCountLimit,
            oneUse: json.oneUse,
            issuerSignature: base64(json.issuerSignatureBase64)
        )
    }

    private static func issuedClaim(_ json: OfflineInputClaimJSON) throws -> AttestedOfflineNoteIssuedClaim {
        try AttestedOfflineNoteIssuedClaim(
            domain: json.domain,
            noteCommitment: hex(json.noteCommitment),
            keyCertificatePayloadHash: hex(json.keyCertificatePayloadHash),
            assetId: json.assetId,
            amount: json.amount
        )
    }

    private static func auditOutputClaim(_ json: OfflineOutputClaimJSON) throws -> AttestedOfflineNoteAuditOutputClaim {
        try AttestedOfflineNoteAuditOutputClaim(
            noteCommitment: hex(json.noteCommitment),
            keyCertificate: certificate(json.keyCertificate),
            assetId: "\(json.assetDefinitionId)#\(json.accountId)",
            amount: json.amount
        )
    }

    private static func loadFixture() throws -> OfflineInteropFixture {
        let testFile = URL(fileURLWithPath: #filePath)
        let fixtureURL = testFile
            .deletingLastPathComponent()
            .appendingPathComponent("../../../fixtures/offline/interop_contract_v2.json")
            .standardizedFileURL
        let data = try Data(contentsOf: fixtureURL)
        return try JSONDecoder().decode(OfflineInteropFixture.self, from: data)
    }

    private static func hex(_ value: String) throws -> Data {
        guard let data = Data(hexString: value) else {
            throw AttestedOfflineNoteFixtureError.invalidHex(value)
        }
        return data
    }

    private static func base64(_ value: String) throws -> Data {
        guard let data = Data(base64Encoded: value) else {
            throw AttestedOfflineNoteFixtureError.invalidBase64
        }
        return data
    }

    private static func attestationEvidence(attestationReportHash: Data) -> Data {
        var evidence = Data(AttestedOfflineNoteConstants.deviceAttestationEvidencePrefix.utf8)
        evidence.append(attestationReportHash)
        return evidence
    }

    private static func recursiveProofPayload(
        publicInputsHash: Data,
        proofBackend: String,
        proofBytes: Data
    ) -> Data {
        var proofWriter = OfflineCompactNoritoWriter()
        proofWriter.writeField(OfflineCompactNorito.encodeString(AttestedOfflineNoteConstants.recursiveBackend))
        proofWriter.writeField(OfflineCompactNorito.encodeString(AttestedOfflineNoteConstants.recursiveVerifierName))

        var proofBoxWriter = OfflineCompactNoritoWriter()
        proofBoxWriter.writeField(OfflineCompactNorito.encodeString(proofBackend))
        proofBoxWriter.writeField(OfflineNorito.encodeBytesVec(proofBytes))

        var writer = OfflineCompactNoritoWriter()
        writer.writeField(proofWriter.data)
        writer.writeField(publicInputsHash)
        writer.writeField(proofBoxWriter.data)
        return writer.data
    }

    private static func instructionWirePayload(typeName: String, modelPayload: Data) -> Data {
        var payload = OfflineNoritoWriter()
        payload.writeField(modelPayload)
        return noritoEncode(typeName: typeName, payload: payload.data, flags: 0)
    }

    private static func rawInstructionPair(wireName: String, wirePayload: Data, compact: Bool = true) -> Data {
        var data = Data()
        writeInstructionField(encodeInstructionString(wireName, compact: compact), to: &data, compact: compact)
        writeInstructionField(encodeInstructionBytesVec(wirePayload), to: &data, compact: compact)
        return data
    }

    private static func encodeInstructionString(_ value: String, compact: Bool) -> Data {
        let bytes = Data(value.utf8)
        var data = Data()
        appendInstructionLength(UInt64(bytes.count), to: &data, compact: compact)
        data.append(bytes)
        return data
    }

    private static func encodeInstructionBytesVec(_ value: Data) -> Data {
        var data = Data()
        appendInstructionLength(UInt64(value.count), to: &data, compact: false)
        data.append(value)
        return data
    }

    private static func writeInstructionField(_ payload: Data, to data: inout Data, compact: Bool) {
        appendInstructionLength(UInt64(payload.count), to: &data, compact: compact)
        data.append(payload)
    }

    private static func appendInstructionLength(_ value: UInt64, to data: inout Data, compact: Bool) {
        if compact {
            var remaining = value
            while remaining >= 0x80 {
                data.append(UInt8(remaining & 0x7f) | 0x80)
                remaining >>= 7
            }
            data.append(UInt8(remaining))
        } else {
            var littleEndian = value.littleEndian
            data.append(contentsOf: withUnsafeBytes(of: &littleEndian, Array.init))
        }
    }

    private struct ParsedAttestedOfflineNoteInstruction {
        let wireName: String
        let archive: Data
    }

    private static func parseSingleOfflineNoteV2Instruction(
        _ envelope: SignedTransactionEnvelope
    ) throws -> ParsedAttestedOfflineNoteInstruction {
        var signed = OfflineNoritoReader(data: envelope.signedTransaction)
        _ = try signed.readField()
        let transactionPayload = try signed.readField()
        XCTAssertEqual(try signed.readField(), Data([0]))
        XCTAssertEqual(try signed.readField(), Data([0]))
        XCTAssertEqual(signed.remaining(), 0)

        var transaction = OfflineNoritoReader(data: transactionPayload)
        _ = try transaction.readField()
        _ = try transaction.readField()
        _ = try transaction.readField()
        let executablePayload = try transaction.readField()
        _ = try transaction.readField()
        _ = try transaction.readField()
        _ = try transaction.readField()
        XCTAssertEqual(transaction.remaining(), 0)

        let instructionPayload = try singleInstructionPayload(fromExecutablePayload: executablePayload)
        var instruction = OfflineNoritoReader(data: instructionPayload)
        let wireName = try readFieldString(&instruction)
        let archive = try readFieldBytesVec(&instruction)
        XCTAssertEqual(instruction.remaining(), 0)
        return ParsedAttestedOfflineNoteInstruction(wireName: wireName, archive: archive)
    }

    private static func singleInstructionPayload(fromExecutablePayload payload: Data) throws -> Data {
        var executable = OfflineNoritoReader(data: payload)
        XCTAssertEqual(try executable.readUInt32LE(), 0)
        let instructionsPayload = try executable.readField()
        XCTAssertEqual(executable.remaining(), 0)

        var instructions = OfflineNoritoReader(data: instructionsPayload)
        XCTAssertEqual(try instructions.readUInt64LE(), 1)
        let instructionPayload = try instructions.readField()
        XCTAssertEqual(instructions.remaining(), 0)
        return instructionPayload
    }

    private static func readFieldString(_ reader: inout OfflineNoritoReader) throws -> String {
        var child = OfflineNoritoReader(data: try reader.readField())
        let length = try child.readUInt64LE()
        guard length <= UInt64(Int.max) else {
            throw OfflineNoritoDecodingError.invalidField("string length overflow")
        }
        let bytes = try child.readBytes(Int(length))
        XCTAssertEqual(child.remaining(), 0)
        guard let value = String(data: bytes, encoding: .utf8) else {
            throw OfflineNoritoDecodingError.invalidField("invalid UTF-8")
        }
        return value
    }

    private static func readFieldBytesVec(_ reader: inout OfflineNoritoReader) throws -> Data {
        var child = OfflineNoritoReader(data: try reader.readField())
        let length = try child.readUInt64LE()
        guard length <= UInt64(Int.max) else {
            throw OfflineNoritoDecodingError.invalidField("byte vector length overflow")
        }
        let bytes = try child.readBytes(Int(length))
        XCTAssertEqual(child.remaining(), 0)
        return bytes
    }
}

private enum AttestedOfflineNoteFixtureError: Error {
    case invalidHex(String)
    case invalidBase64
}

private struct OfflineInteropFixture: Decodable {
    let chainVectors: OfflineChainVectors
    let paymentToken: OfflinePaymentTokenJSON

    private enum CodingKeys: String, CodingKey {
        case chainVectors = "chain_vectors"
        case paymentToken = "payment_token"
    }
}

private struct OfflineChainVectors: Decodable {
    let certificates: OfflineCertificateVectors
    let issue: OfflineIssueVector
    let audit: OfflineAuditVector
    let redeem: OfflineRedeemVector
    let attestationRegistration: OfflineAttestationRegistrationVector

    private enum CodingKeys: String, CodingKey {
        case certificates
        case issue
        case audit
        case redeem
        case attestationRegistration = "attestation_registration"
    }
}

private struct OfflineCertificateVectors: Decodable {
    let senderPayloadBase64: String
    let senderPayloadHash: String

    private enum CodingKeys: String, CodingKey {
        case senderPayloadBase64 = "sender_payload_base64"
        case senderPayloadHash = "sender_payload_hash"
    }
}

private struct OfflineIssueVector: Decodable {
    let noteCommitment: String
    let assetId: String
    let amount: String
    let noritoBase64: String

    private enum CodingKeys: String, CodingKey {
        case noteCommitment = "note_commitment"
        case assetId = "asset_id"
        case amount
        case noritoBase64 = "norito_base64"
    }
}

private struct OfflineAuditVector: Decodable {
    let tokenId: String
    let inputNullifiers: [String]
    let outputCommitments: [String]
    let publicInputsHash: String
    let noritoBase64: String

    private enum CodingKeys: String, CodingKey {
        case tokenId = "token_id"
        case inputNullifiers = "input_nullifiers"
        case outputCommitments = "output_commitments"
        case publicInputsHash = "public_inputs_hash"
        case noritoBase64 = "norito_base64"
    }
}

private struct OfflineRedeemVector: Decodable {
    let sourceNoteCommitment: String
    let inputNullifiers: [String]
    let assetId: String
    let amount: String
    let publicInputsHash: String
    let noritoBase64: String

    private enum CodingKeys: String, CodingKey {
        case sourceNoteCommitment = "source_note_commitment"
        case inputNullifiers = "input_nullifiers"
        case assetId = "asset_id"
        case amount
        case publicInputsHash = "public_inputs_hash"
        case noritoBase64 = "norito_base64"
    }
}

private struct OfflineAttestationRegistrationVector: Decodable {
    let version: UInt16
    let platform: String
    let keyId: String
    let deviceId: String
    let accountId: String
    let assetDefinitionId: String?
    let iosTeamId: String?
    let iosBundleId: String?
    let iosEnvironment: String?
    let androidPackageName: String?
    let androidSigningCertificateSha256: String?
    let publicKey: String
    let assertionScheme: String
    let assertionKeyAlgorithm: String
    let assertionPublicKey: String
    let assertionUsageCountLimit: UInt32?
    let oneUse: Bool
    let challengeHash: String
    let attestationReportHash: String
    let attestationReportBase64: String
    let evidenceHash: String
    let evidenceBase64: String
    let recentBlockHeight: UInt64
    let recentBlockHash: String
    let expiresAtMs: UInt64
    let keyCertificatePayloadHash: String
    let noritoBase64: String
    let instructionNoritoBase64: String

    private enum CodingKeys: String, CodingKey {
        case version
        case platform
        case keyId = "key_id"
        case deviceId = "device_id"
        case accountId = "account_id"
        case assetDefinitionId = "asset_definition_id"
        case iosTeamId = "ios_team_id"
        case iosBundleId = "ios_bundle_id"
        case iosEnvironment = "ios_environment"
        case androidPackageName = "android_package_name"
        case androidSigningCertificateSha256 = "android_signing_certificate_sha256"
        case publicKey = "public_key"
        case assertionScheme = "assertion_scheme"
        case assertionKeyAlgorithm = "assertion_key_algorithm"
        case assertionPublicKey = "assertion_public_key"
        case assertionUsageCountLimit = "assertion_usage_count_limit"
        case oneUse = "one_use"
        case challengeHash = "challenge_hash"
        case attestationReportHash = "attestation_report_hash"
        case attestationReportBase64 = "attestation_report_base64"
        case evidenceHash = "evidence_hash"
        case evidenceBase64 = "evidence_base64"
        case recentBlockHeight = "recent_block_height"
        case recentBlockHash = "recent_block_hash"
        case expiresAtMs = "expires_at_ms"
        case keyCertificatePayloadHash = "key_certificate_payload_hash"
        case noritoBase64 = "norito_base64"
        case instructionNoritoBase64 = "instruction_norito_base64"
    }
}

private struct OfflinePaymentTokenJSON: Decodable {
    let senderAccountId: String
    let recipientAccountId: String
    let senderKeyCertificate: OfflineCertificateJSON
    let recipientKeyCertificate: OfflineCertificateJSON
    let inputClaims: [OfflineInputClaimJSON]
    let outputClaims: [OfflineOutputClaimJSON]

    private enum CodingKeys: String, CodingKey {
        case senderAccountId = "sender_account_id"
        case recipientAccountId = "recipient_account_id"
        case senderKeyCertificate = "sender_key_certificate"
        case recipientKeyCertificate = "recipient_key_certificate"
        case inputClaims = "input_claims"
        case outputClaims = "output_claims"
    }
}

private struct OfflineCertificateJSON: Decodable {
    let version: UInt16
    let platform: String
    let keyId: String
    let deviceId: String
    let accountId: String
    let publicKey: String
    let assertionScheme: String
    let assertionKeyAlgorithm: String
    let assertionPublicKey: String
    let assertionUsageCountLimit: UInt32?
    let oneUse: Bool
    let issuerSignatureBase64: String

    private enum CodingKeys: String, CodingKey {
        case version
        case platform
        case keyId = "key_id"
        case deviceId = "device_id"
        case accountId = "account_id"
        case publicKey = "public_key"
        case assertionScheme = "assertion_scheme"
        case assertionKeyAlgorithm = "assertion_key_algorithm"
        case assertionPublicKey = "assertion_public_key"
        case assertionUsageCountLimit = "assertion_usage_count_limit"
        case oneUse = "one_use"
        case issuerSignatureBase64 = "issuer_signature_base64"
    }
}

private struct OfflineInputClaimJSON: Decodable {
    let domain: String
    let noteCommitment: String
    let keyCertificatePayloadHash: String
    let assetId: String
    let amount: String

    private enum CodingKeys: String, CodingKey {
        case domain
        case noteCommitment = "note_commitment"
        case keyCertificatePayloadHash = "key_certificate_payload_hash"
        case assetId = "asset_id"
        case amount
    }
}

private struct OfflineOutputClaimJSON: Decodable {
    let accountId: String
    let noteCommitment: String
    let keyCertificate: OfflineCertificateJSON
    let assetDefinitionId: String
    let amount: String

    private enum CodingKeys: String, CodingKey {
        case accountId = "account_id"
        case noteCommitment = "note_commitment"
        case keyCertificate = "key_certificate"
        case assetDefinitionId = "asset_definition_id"
        case amount
    }
}
