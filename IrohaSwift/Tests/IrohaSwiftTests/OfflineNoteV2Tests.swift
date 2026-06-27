import XCTest
import CryptoKit
@testable import IrohaSwift

final class OfflineNoteV2Tests: XCTestCase {
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

    func testOfflineNoteV2ModelsMatchRustNoritoVectors() throws {
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
            try OfflineNoteV2Decoding.decodeCertificatePayload(certificatePayload.noritoEncoded()),
            certificatePayload
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeKeyCertificatePayload(certificatePayload.noritoEncoded()),
            certificatePayload
        )
        XCTAssertEqual(try OfflineNoteV2Decoding.decodeCertificate(certificate.noritoEncoded()), certificate)
        XCTAssertEqual(try OfflineNoteV2Decoding.decodeIssue(issue.noritoEncoded()), issue)
        XCTAssertEqual(try OfflineNoteV2Decoding.decodeIssuedClaim(issuedClaim.noritoEncoded()), issuedClaim)
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeAuditOutputClaim(auditOutputClaim.noritoEncoded()),
            auditOutputClaim
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeRecursiveProof(audit.recursiveProof.noritoEncoded()),
            audit.recursiveProof
        )
        XCTAssertEqual(try OfflineNoteV2Decoding.decodeAudit(audit.noritoEncoded()), audit)
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeAuditPublicInputs(auditPublicInputs.noritoEncoded()),
            auditPublicInputs
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeRecursiveProof(redeem.recursiveProof.noritoEncoded()),
            redeem.recursiveProof
        )
        XCTAssertEqual(try OfflineNoteV2Decoding.decodeRedeem(redeem.noritoEncoded()), redeem)
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeRedeemPublicInputs(redeemPublicInputs.noritoEncoded()),
            redeemPublicInputs
        )

        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeIssue(try Self.base64(fixture.chainVectors.issue.noritoBase64)).noritoEncoded(),
            try issue.noritoEncoded()
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeAudit(try Self.base64(fixture.chainVectors.audit.noritoBase64)).noritoEncoded(),
            try audit.noritoEncoded()
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeRedeem(try Self.base64(fixture.chainVectors.redeem.noritoBase64)).noritoEncoded(),
            try redeem.noritoEncoded()
        )
    }

    func testOfflineNoteV2InstructionDecodersReadExplorerEnvelopeBytes() throws {
        let fixture = try Self.loadFixture()
        let issue = try Self.issue(fixture)
        let audit = try Self.audit(fixture)
        let redeem = try Self.redeem(fixture)

        XCTAssertEqual(OfflineNoteV2TypeNames.issueInstruction, OfflineNoteTypeNames.issueInstruction)
        XCTAssertEqual(OfflineNoteV2TypeNames.redeemInstruction, OfflineNoteTypeNames.redeemInstruction)
        XCTAssertEqual(OfflineNoteV2TypeNames.auditInstruction, OfflineNoteTypeNames.auditInstruction)
        XCTAssertFalse(OfflineNoteV2TypeNames.issueInstruction.hasSuffix("V2"))
        XCTAssertFalse(OfflineNoteV2TypeNames.redeemInstruction.hasSuffix("V2"))
        XCTAssertFalse(OfflineNoteV2TypeNames.auditInstruction.hasSuffix("V2"))

        let issueInstruction = ParsedOfflineNoteV2Instruction(
            wireName: OfflineNoteV2TypeNames.issueInstruction,
            archive: Self.instructionWirePayload(
                typeName: OfflineNoteV2TypeNames.issueInstruction,
                modelPayload: try OfflineNoteV2Encoding.encodeIssue(issue)
            )
        )
        XCTAssertFalse(issueInstruction.wireName.hasSuffix("V2"))
        let issueEnvelope = Self.rawInstructionPair(
            wireName: issueInstruction.wireName,
            wirePayload: issueInstruction.archive
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeIssueInstruction(issueEnvelope).noritoEncoded().base64EncodedString(),
            try issue.noritoEncoded().base64EncodedString()
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeIssueInstruction(issueInstruction.archive).noritoEncoded().base64EncodedString(),
            try issue.noritoEncoded().base64EncodedString()
        )

        let auditInstruction = ParsedOfflineNoteV2Instruction(
            wireName: OfflineNoteV2TypeNames.auditInstruction,
            archive: Self.instructionWirePayload(
                typeName: OfflineNoteV2TypeNames.auditInstruction,
                modelPayload: try OfflineNoteV2Encoding.encodeAudit(audit)
            )
        )
        XCTAssertFalse(auditInstruction.wireName.hasSuffix("V2"))
        let auditEnvelope = Self.rawInstructionPair(
            wireName: auditInstruction.wireName,
            wirePayload: auditInstruction.archive,
            compact: false
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeAuditInstruction(auditEnvelope).noritoEncoded().base64EncodedString(),
            try audit.noritoEncoded().base64EncodedString()
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeAuditInstruction(auditInstruction.archive).noritoEncoded().base64EncodedString(),
            try audit.noritoEncoded().base64EncodedString()
        )

        let redeemInstruction = ParsedOfflineNoteV2Instruction(
            wireName: OfflineNoteV2TypeNames.redeemInstruction,
            archive: Self.instructionWirePayload(
                typeName: OfflineNoteV2TypeNames.redeemInstruction,
                modelPayload: try OfflineNoteV2Encoding.encodeRedeem(redeem)
            )
        )
        XCTAssertFalse(redeemInstruction.wireName.hasSuffix("V2"))
        let redeemEnvelope = Self.rawInstructionPair(
            wireName: redeemInstruction.wireName,
            wirePayload: redeemInstruction.archive
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeRedeemInstruction(redeemEnvelope).noritoEncoded().base64EncodedString(),
            try redeem.noritoEncoded().base64EncodedString()
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeRedeemInstruction(redeemInstruction.archive).noritoEncoded().base64EncodedString(),
            try redeem.noritoEncoded().base64EncodedString()
        )
    }

    func testOfflineNoteV2InstructionDecodersReadLegacyAliasEnvelopeBytes() throws {
        let fixture = try Self.loadFixture()
        let issue = try Self.issue(fixture)
        let audit = try Self.audit(fixture)
        let redeem = try Self.redeem(fixture)
        let issueAliasWirePayload = Self.instructionWirePayload(
            typeName: OfflineNoteV2TypeNames.issueInstructionAlias,
            modelPayload: try OfflineNoteV2Encoding.encodeIssue(issue)
        )
        let auditAliasWirePayload = Self.instructionWirePayload(
            typeName: OfflineNoteV2TypeNames.auditInstructionAlias,
            modelPayload: try OfflineNoteV2Encoding.encodeAudit(audit)
        )
        let redeemAliasWirePayload = Self.instructionWirePayload(
            typeName: OfflineNoteV2TypeNames.redeemInstructionAlias,
            modelPayload: try OfflineNoteV2Encoding.encodeRedeem(redeem)
        )

        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeIssueInstruction(issueAliasWirePayload).noritoEncoded().base64EncodedString(),
            try issue.noritoEncoded().base64EncodedString()
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeIssueInstruction(Self.rawInstructionPair(
                wireName: OfflineNoteV2TypeNames.issueInstructionAlias,
                wirePayload: issueAliasWirePayload
            )).noritoEncoded().base64EncodedString(),
            try issue.noritoEncoded().base64EncodedString()
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeAuditInstruction(Self.rawInstructionPair(
                wireName: OfflineNoteV2TypeNames.auditInstructionAlias,
                wirePayload: auditAliasWirePayload
            )).noritoEncoded().base64EncodedString(),
            try audit.noritoEncoded().base64EncodedString()
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeRedeemInstruction(Self.rawInstructionPair(
                wireName: OfflineNoteV2TypeNames.redeemInstructionAlias,
                wirePayload: redeemAliasWirePayload
            )).noritoEncoded().base64EncodedString(),
            try redeem.noritoEncoded().base64EncodedString()
        )
    }

    func testOfflineNoteV2InstructionDecodersRejectWrongEnvelopeShapes() throws {
        let fixture = try Self.loadFixture()
        let issue = try Self.issue(fixture)
        let issueWirePayload = Self.instructionWirePayload(
            typeName: OfflineNoteV2TypeNames.issueInstruction,
            modelPayload: try OfflineNoteV2Encoding.encodeIssue(issue)
        )
        let issueEnvelope = Self.rawInstructionPair(
            wireName: OfflineNoteV2TypeNames.issueInstruction,
            wirePayload: issueWirePayload
        )
        let wrongWireEnvelope = Self.rawInstructionPair(
            wireName: OfflineNoteV2TypeNames.redeemInstruction,
            wirePayload: issueWirePayload
        )
        let wrongSchemaPayload = Self.instructionWirePayload(
            typeName: OfflineNoteV2TypeNames.redeemInstruction,
            modelPayload: try OfflineNoteV2Encoding.encodeIssue(issue)
        )
        let wrongSchemaEnvelope = Self.rawInstructionPair(
            wireName: OfflineNoteV2TypeNames.issueInstruction,
            wirePayload: wrongSchemaPayload
        )

        XCTAssertThrowsError(try OfflineNoteV2Decoding.decodeRedeemInstruction(issueEnvelope))
        XCTAssertThrowsError(try OfflineNoteV2Decoding.decodeIssueInstruction(wrongWireEnvelope))
        XCTAssertThrowsError(try OfflineNoteV2Decoding.decodeIssueInstruction(wrongSchemaEnvelope))
        XCTAssertThrowsError(try OfflineNoteV2Decoding.decodeIssueInstruction(Data(issueEnvelope.dropLast())))
    }

    func testOfflineNoteV2DecodersRejectMalformedPayloads() throws {
        let fixture = try Self.loadFixture()
        let certificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let issue = try Self.issue(fixture)
        let issueBytes = try issue.noritoEncoded()

        XCTAssertThrowsError(try OfflineNoteV2Decoding.decodeIssue(Data(issueBytes.dropLast())))
        XCTAssertThrowsError(try OfflineNoteV2Decoding.decodeIssue(try certificate.noritoEncoded()))
        XCTAssertThrowsError(try OfflineNoteV2Decoding.decodeCertificatePayload(try certificate.noritoEncoded()))

        var corruptedChecksum = issueBytes
        corruptedChecksum[corruptedChecksum.count - 1] ^= 0x01
        XCTAssertThrowsError(try OfflineNoteV2Decoding.decodeIssue(corruptedChecksum))

        let nonCompactIssue = noritoEncode(
            typeName: OfflineNoteV2TypeNames.issue,
            payload: try OfflineNoteV2Encoding.encodeIssue(issue)
        )
        XCTAssertThrowsError(try OfflineNoteV2Decoding.decodeIssue(nonCompactIssue))

        var trailingPayload = try OfflineNoteV2Encoding.encodeIssue(issue)
        trailingPayload.append(0)
        let trailingIssue = OfflineNoteV2Encoding.wrap(typeName: OfflineNoteV2TypeNames.issue, payload: trailingPayload)
        XCTAssertThrowsError(try OfflineNoteV2Decoding.decodeIssue(trailingIssue))

        let invalidProof = OfflineNoteV2Encoding.wrap(
            typeName: OfflineNoteV2TypeNames.recursiveProof,
            payload: Self.recursiveProofPayload(
                publicInputsHash: Data(repeating: 0x02, count: 32),
                proofBackend: OfflineNoteV2Constants.recursiveBackend,
                proofBytes: Data([0x01])
            )
        )
        XCTAssertThrowsError(try OfflineNoteV2Decoding.decodeRecursiveProof(invalidProof)) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .invalidHash(field: "public_inputs_hash"))
        }

        let emptyProofBytes = OfflineNoteV2Encoding.wrap(
            typeName: OfflineNoteV2TypeNames.recursiveProof,
            payload: Self.recursiveProofPayload(
                publicInputsHash: try issue.issuedClaim().claimHash(),
                proofBackend: OfflineNoteV2Constants.recursiveBackend,
                proofBytes: Data()
            )
        )
        XCTAssertThrowsError(try OfflineNoteV2Decoding.decodeRecursiveProof(emptyProofBytes)) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .emptyProofBytes)
        }
    }

    func testOfflineNoteV2PublicInputHashesMatchRustVectors() throws {
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
        XCTAssertEqual(draft.challengeHash.hexLowercased(), vector.challengeHash)
        XCTAssertEqual(draft.attestationReportHash, emptyReportHash)
        XCTAssertEqual(draft.attestationReport, Data())
        XCTAssertEqual(draft.evidence, expectedEvidence)
        XCTAssertEqual(draft.evidenceHash, IrohaHash.hash(expectedEvidence))
    }

    func testOfflineNoteV2PaymentTransactionBuildersAreRetiredAndRegistrationStillSigns() throws {
        let fixture = try Self.loadFixture()
        let keypair = try Keypair(privateKeyBytes: Data(0..<32))
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let chainId = "00000000-0000-0000-0000-000000000000"
        let creationTimeMs: UInt64 = 1_706_000_000_000
        let issueModel = try Self.issue(fixture)
        let auditModel = try Self.audit(fixture)
        let redeemModel = try Self.redeem(fixture)
        let registrationModel = try Self.attestationRegistration(fixture)

        assertRetiredOfflineNotePayment(try SwiftTransactionEncoder.encodeIssueOfflineNoteV2(
            request: IssueOfflineNoteV2Request(
                chainId: chainId,
                authority: authority,
                issue: issueModel,
                ttlMs: 60_000
            ),
            keypair: keypair,
            creationTimeMs: creationTimeMs
        ))
        assertRetiredOfflineNotePayment(try SwiftTransactionEncoder.encodeAuditOfflineNoteV2(
            request: AuditOfflineNoteV2Request(
                chainId: chainId,
                authority: authority,
                audit: auditModel,
                ttlMs: 60_000
            ),
            keypair: keypair,
            creationTimeMs: creationTimeMs
        ))
        assertRetiredOfflineNotePayment(try SwiftTransactionEncoder.encodeRedeemOfflineNoteV2(
            request: RedeemOfflineNoteV2Request(
                chainId: chainId,
                authority: authority,
                redemption: redeemModel,
                ttlMs: 60_000
            ),
            keypair: keypair,
            creationTimeMs: creationTimeMs
        ))
        let registerAttestation = try SwiftTransactionEncoder.encodeRegisterOfflineDeviceAttestation(
            request: RegisterOfflineDeviceAttestationRequest(
                chainId: chainId,
                authority: authority,
                registration: registrationModel,
                ttlMs: 60_000
            ),
            keypair: keypair,
            creationTimeMs: creationTimeMs
        )

        XCTAssertEqual(registerAttestation.norito.first, 1)
        XCTAssertEqual(Data(registerAttestation.norito.dropFirst()), registerAttestation.signedTransaction)
        XCTAssertEqual(registerAttestation.transactionHash.count, 32)
        XCTAssertNil(registerAttestation.payload)

        let registerInstruction = try Self.parseSingleOfflineNoteV2Instruction(registerAttestation)
        XCTAssertEqual(registerInstruction.wireName, OfflineNoteV2TypeNames.registerDeviceAttestationInstruction)
        XCTAssertEqual(
            registerInstruction.archive.base64EncodedString(),
            fixture.chainVectors.attestationRegistration.instructionNoritoBase64
        )
    }

    func testRedeemBuilderRejectsMismatchedProofBinding() throws {
        let fixture = try Self.loadFixture()
        let redeem = try Self.redeem(fixture)
        let badProof = try OfflineNoteRecursiveProofV2(
            publicInputsHash: IrohaHash.hash(Data("wrong-public-inputs".utf8)),
            proofBytes: Data("offline-v2-vector-redeem-proof".utf8)
        )
        let forged = try OfflineNoteRedeemV2(
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
            try SwiftTransactionEncoder.encodeRedeemOfflineNoteV2(
                request: RedeemOfflineNoteV2Request(
                    chainId: "00000000-0000-0000-0000-000000000000",
                    authority: authority,
                    redemption: forged
                ),
                keypair: keypair,
                creationTimeMs: 1
            )
        ) { error in
            guard case OfflineNoteV2Error.proofPublicInputsHashMismatch = error else {
                return XCTFail("expected proofPublicInputsHashMismatch, got \(error)")
            }
        }
    }

    func testOfflineNoteV2ProofAndHashValidationRejectsMalformedValues() throws {
        let fixture = try Self.loadFixture()
        let publicInputsHash = try Self.hex(fixture.chainVectors.audit.publicInputsHash)

        let proof = try OfflineNoteProofBoxV2(
            backend: OfflineNoteV2Constants.recursiveBackend,
            bytes: Data([0x01])
        )
        XCTAssertEqual(proof.backend, OfflineNoteV2Constants.recursiveBackend)

        XCTAssertThrowsError(try OfflineNoteProofBoxV2(
            backend: "  \(OfflineNoteV2Constants.recursiveBackend)  ",
            bytes: Data([0x01])
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .unsupportedRecursiveProofBackend(
                    expected: OfflineNoteV2Constants.recursiveBackend,
                    actual: "  \(OfflineNoteV2Constants.recursiveBackend)  "
                )
            )
        }

        XCTAssertThrowsError(try OfflineNoteProofBoxV2(backend: " \n ", bytes: Data([0x01]))) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .emptyProofBackend)
        }
        XCTAssertThrowsError(try OfflineNoteProofBoxV2(backend: "halo2/ipa", bytes: Data())) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .emptyProofBytes)
        }
        XCTAssertThrowsError(try OfflineNoteRecursiveProofV2(
            publicInputsHash: Data(repeating: 0x01, count: 31),
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .invalidHashLength(field: "public_inputs_hash", expected: 32, actual: 31)
            )
        }

        var nonCanonicalHash = publicInputsHash
        nonCanonicalHash[31] &= 0xfe
        XCTAssertThrowsError(try OfflineNoteRecursiveProofV2(
            publicInputsHash: nonCanonicalHash,
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .invalidHash(field: "public_inputs_hash"))
        }
    }

    func testOfflineNoteV2CertificateValidationRejectsMalformedValues() throws {
        let fixture = try Self.loadFixture()
        let cert = fixture.paymentToken.senderKeyCertificate
        let publicKey = try Self.base64(cert.publicKey)
        let assertionPublicKey = try Self.base64(cert.assertionPublicKey)
        let issuerSignature = try Self.base64(cert.issuerSignatureBase64)

        XCTAssertThrowsError(try OfflineNoteKeyCertificateV2(
            version: OfflineNoteV2Constants.keyCertificateVersion + 1,
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
                error as? OfflineNoteV2Error,
                .invalidCertificateVersion(OfflineNoteV2Constants.keyCertificateVersion + 1)
            )
        }
        XCTAssertThrowsError(try OfflineNoteKeyCertificateV2(
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
            XCTAssertEqual(error as? OfflineNoteV2Error, .certificateMustBeOneUse)
        }
        XCTAssertThrowsError(try OfflineNoteKeyCertificateV2(
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
                error as? OfflineNoteV2Error,
                .invalidNotePublicKeyLength(expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try OfflineNoteKeyCertificateV2(
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
                error as? OfflineNoteV2Error,
                .invalidIssuerSignatureLength(expected: 64, actual: 63)
            )
        }
        XCTAssertThrowsError(try OfflineNoteKeyCertificateV2(
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
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try OfflineNoteKeyCertificateV2(
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
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
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
            guard case OfflineNoteV2Error.deviceAttestationChallengeHashMismatch = error else {
                return XCTFail("expected deviceAttestationChallengeHashMismatch, got \(error)")
            }
        }

        var badReportHash = try Self.hex(vector.attestationReportHash)
        badReportHash[0] ^= 0x01
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, attestationReportHash: badReportHash)) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .deviceAttestationHashMismatch(field: "attestation_report_hash"))
        }

        var badEvidenceHash = try Self.hex(vector.evidenceHash)
        badEvidenceHash[0] ^= 0x01
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, evidenceHash: badEvidenceHash)) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .deviceAttestationHashMismatch(field: "evidence_hash"))
        }

        let forgedEvidence = Self.attestationEvidence(attestationReportHash: Data(repeating: 0xA5, count: 32))
        XCTAssertThrowsError(try Self.attestationRegistration(
            fixture,
            evidenceHash: IrohaHash.hash(forgedEvidence),
            evidence: forgedEvidence
        )) { error in
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }

        XCTAssertThrowsError(try Self.attestationRegistration(
            fixture,
            androidSigningCertificateSha256: Data(repeating: 0x01, count: 31)
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .invalidDigestLength(field: "android_signing_certificate_sha256", expected: 32, actual: 31)
            )
        }

        XCTAssertThrowsError(try Self.attestationRegistration(fixture, publicKey: Data(repeating: 0x01, count: 31))) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .invalidNotePublicKeyLength(expected: 32, actual: 31))
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, keyId: "not standard base64!")) { error in
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, keyId: "AB==")) { error in
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, keyId: " \(vector.keyId) ")) { error in
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, deviceId: " \(vector.deviceId) ")) { error in
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, deviceId: "\u{00A0}\u{2003}")) { error in
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, iosTeamId: " \(vector.iosTeamId ?? "") ")) { error in
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, iosBundleId: "\(vector.iosBundleId ?? "")\n")) { error in
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, iosEnvironment: "\t\(vector.iosEnvironment ?? "")")) { error in
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, androidPackageName: " jp.co.soramitsu.iroha.offline ")) { error in
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(
            fixture,
            assertionPublicKey: Self.offCurveP256AssertionPublicKey()
        )) { error in
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, recentBlockHash: Data(repeating: 0x01, count: 31))) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .invalidHashLength(field: "recent_block_hash", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, oneUse: false)) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .certificateMustBeOneUse)
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, assetDefinitionId: "cash#bad"))
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, assertionUsageCountLimit: 1)) { error in
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(
            fixture,
            platform: OfflineNoteV2Constants.androidKeyMintPlatform,
            assertionScheme: OfflineNoteV2Constants.androidKeyMintAssertionScheme,
            assertionKeyAlgorithm: OfflineNoteV2Constants.androidKeyMintAssertionKeyAlgorithm
        )) { error in
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(
            fixture,
            platform: OfflineNoteV2Constants.androidKeyMintPlatform,
            assertionScheme: "android-keymint-ecdsa-p256-usage-limit",
            assertionKeyAlgorithm: OfflineNoteV2Constants.androidKeyMintAssertionKeyAlgorithm,
            assertionUsageCountLimit: 1
        )) { error in
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(
            fixture,
            keyId: String(repeating: "00", count: 32),
            platform: OfflineNoteV2Constants.androidKeyMintPlatform,
            assertionScheme: OfflineNoteV2Constants.androidKeyMintAssertionScheme,
            assertionKeyAlgorithm: OfflineNoteV2Constants.androidKeyMintAssertionKeyAlgorithm,
            assertionUsageCountLimit: 1
        )) { error in
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        let androidUppercaseKeyId = Data(SHA256.hash(data: try Self.base64(vector.assertionPublicKey)))
            .hexLowercased()
            .uppercased()
        XCTAssertThrowsError(try Self.attestationRegistration(
            fixture,
            keyId: androidUppercaseKeyId,
            platform: OfflineNoteV2Constants.androidKeyMintPlatform,
            assertionScheme: OfflineNoteV2Constants.androidKeyMintAssertionScheme,
            assertionKeyAlgorithm: OfflineNoteV2Constants.androidKeyMintAssertionKeyAlgorithm,
            assertionUsageCountLimit: 1
        )) { error in
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try Self.attestationRegistration(fixture, platform: "ios-app-attest")) { error in
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
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

    func testOfflineNoteV2AuditBundleRejectsInvalidShapes() throws {
        let fixture = try Self.loadFixture()
        let audit = try Self.audit(fixture)

        XCTAssertThrowsError(try OfflineNoteAuditBundleV2(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: [],
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: audit.outputClaims,
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .emptyInputNullifiers)
        }
        XCTAssertThrowsError(try OfflineNoteAuditBundleV2(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: [],
            outputCommitments: audit.outputCommitments,
            outputClaims: audit.outputClaims,
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .emptyInputClaims)
        }
        XCTAssertThrowsError(try OfflineNoteAuditBundleV2(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers + [audit.inputNullifiers[0]],
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: audit.outputClaims,
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .auditInputCountMismatch(nullifiers: audit.inputNullifiers.count + 1, claims: audit.inputClaims.count)
            )
        }
        XCTAssertThrowsError(try OfflineNoteAuditBundleV2(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: [],
            outputClaims: audit.outputClaims,
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .emptyOutputCommitments)
        }
        XCTAssertThrowsError(try OfflineNoteAuditBundleV2(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: [],
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .emptyOutputClaims)
        }

        let uncommittedClaim = try OfflineNoteAuditOutputClaimV2(
            noteCommitment: Data(repeating: 0x03, count: 32),
            keyCertificate: audit.outputClaims[0].keyCertificate,
            assetId: audit.outputClaims[0].assetId,
            amount: audit.outputClaims[0].amount
        )
        XCTAssertThrowsError(try OfflineNoteAuditBundleV2(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: [uncommittedClaim],
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .auditOutputClaimNotCommitted(uncommittedClaim.noteCommitment.hexLowercased())
            )
        }
    }

    func testOfflineNoteV2IssueAndClaimValidationCoversDerivedClaimAndFailures() throws {
        let fixture = try Self.loadFixture()
        let certificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let noteCommitment = try Self.hex(fixture.chainVectors.issue.noteCommitment)
        let issue = try OfflineNoteIssueV2(
            noteCommitment: noteCommitment,
            keyCertificate: certificate,
            assetId: fixture.chainVectors.issue.assetId,
            amount: "5.5000"
        )

        XCTAssertEqual(issue.amount, "5.5000")
        let claim = try issue.issuedClaim()
        XCTAssertEqual(claim.domain, OfflineNoteV2Constants.issuedClaimDomain)
        XCTAssertEqual(claim.noteCommitment, issue.noteCommitment)
        XCTAssertEqual(claim.keyCertificatePayloadHash, try certificate.payloadHash())
        XCTAssertEqual(claim.assetId, issue.assetId)
        XCTAssertEqual(claim.amount, "5.5000")
        XCTAssertEqual(try claim.claimHash().count, 32)

        XCTAssertThrowsError(try OfflineNoteIssueV2(
            noteCommitment: Data(repeating: 0x01, count: 31),
            keyCertificate: certificate,
            assetId: fixture.chainVectors.issue.assetId,
            amount: fixture.chainVectors.issue.amount
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .invalidHashLength(field: "note_commitment", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try OfflineNoteIssueV2(
            noteCommitment: noteCommitment,
            keyCertificate: certificate,
            assetId: "cash#branch.sbp",
            amount: fixture.chainVectors.issue.amount
        )) { error in
            guard case OfflineNoritoError.invalidAssetId("cash#branch.sbp") = error else {
                return XCTFail("expected invalidAssetId, got \(error)")
            }
        }
        XCTAssertThrowsError(try OfflineNoteIssueV2(
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

    func testOfflineNoteV2RedeemValidationRejectsBadInputsAndDerivesIssuedClaim() throws {
        let fixture = try Self.loadFixture()
        let redeem = try Self.redeem(fixture)
        let issuedClaim = try redeem.issuedClaim()

        XCTAssertEqual(issuedClaim.noteCommitment, redeem.sourceNoteCommitment)
        XCTAssertEqual(issuedClaim.keyCertificatePayloadHash, try redeem.senderKeyCertificate.payloadHash())
        XCTAssertEqual(issuedClaim.assetId, redeem.assetId)
        XCTAssertEqual(issuedClaim.amount, redeem.amount)

        XCTAssertThrowsError(try OfflineNoteRedeemV2(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: [],
            senderKeyCertificate: redeem.senderKeyCertificate,
            recipient: redeem.recipient,
            assetId: redeem.assetId,
            amount: redeem.amount,
            recursiveProof: redeem.recursiveProof
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .emptyInputNullifiers)
        }
        XCTAssertThrowsError(try OfflineNoteRedeemV2(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: [Data(repeating: 0x01, count: 31)],
            senderKeyCertificate: redeem.senderKeyCertificate,
            recipient: redeem.recipient,
            assetId: redeem.assetId,
            amount: redeem.amount,
            recursiveProof: redeem.recursiveProof
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .invalidHashLength(field: "input_nullifiers[0]", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try OfflineNoteRedeemV2(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: redeem.inputNullifiers,
            senderKeyCertificate: redeem.senderKeyCertificate,
            recipient: "\(redeem.recipient)@bad",
            assetId: redeem.assetId,
            amount: redeem.amount,
            recursiveProof: redeem.recursiveProof
        ))
    }

    func testOfflineNoteV2AuditValidateProofBindingReportsExpectedAndActualHashes() throws {
        let fixture = try Self.loadFixture()
        let audit = try Self.audit(fixture)
        var wrongPublicInputsHash = try audit.publicInputsHash()
        wrongPublicInputsHash[0] ^= 0x01
        let forgedProof = try OfflineNoteRecursiveProofV2(
            publicInputsHash: wrongPublicInputsHash,
            proofBytes: audit.recursiveProof.proof.bytes
        )
        let forgedAudit = try OfflineNoteAuditBundleV2(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: audit.outputClaims,
            recursiveProof: forgedProof
        )

        XCTAssertThrowsError(try forgedAudit.validateProofBinding()) { error in
            guard case let OfflineNoteV2Error.proofPublicInputsHashMismatch(expected, actual) = error else {
                return XCTFail("expected proofPublicInputsHashMismatch, got \(error)")
            }
            XCTAssertEqual(expected, try? audit.publicInputsHash().hexLowercased())
            XCTAssertEqual(actual, forgedProof.publicInputsHash.hexLowercased())
        }
    }

    func testOfflineNoteV2TransactionBuilderCoversOptionalNonceAndInputValidation() throws {
        let fixture = try Self.loadFixture()
        let keypair = try Keypair(privateKeyBytes: Data(0..<32))
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let chainId = "00000000-0000-0000-0000-000000000000"
        let issue = try Self.issue(fixture)

        assertRetiredOfflineNotePayment(try SwiftTransactionEncoder.encodeIssueOfflineNoteV2(
            request: IssueOfflineNoteV2Request(chainId: chainId, authority: authority, issue: issue),
            keypair: keypair,
            creationTimeMs: 1_706_000_000_000
        ))
        assertRetiredOfflineNotePayment(try SwiftTransactionEncoder.encodeIssueOfflineNoteV2(
            request: IssueOfflineNoteV2Request(
                chainId: chainId,
                authority: authority,
                issue: issue,
                ttlMs: nil,
                nonce: 42
            ),
            keypair: keypair,
            creationTimeMs: 1_706_000_000_000
        ))

        XCTAssertThrowsError(try SwiftTransactionEncoder.encodeIssueOfflineNoteV2(
            request: IssueOfflineNoteV2Request(chainId: "  \(chainId)  ", authority: authority, issue: issue),
            keypair: keypair,
            creationTimeMs: 1
        )) { error in
            XCTAssertEqual(error as? TransactionInputError, .invalidChainId("  \(chainId)  "))
        }
        XCTAssertThrowsError(try SwiftTransactionEncoder.encodeIssueOfflineNoteV2(
            request: IssueOfflineNoteV2Request(chainId: chainId, authority: "  \(authority)  ", issue: issue),
            keypair: keypair,
            creationTimeMs: 1
        )) { error in
            XCTAssertEqual(
                error as? TransactionInputError,
                .malformedAccountId(field: "authority", value: "  \(authority)  ")
            )
        }
        XCTAssertThrowsError(try SwiftTransactionEncoder.encodeIssueOfflineNoteV2(
            request: IssueOfflineNoteV2Request(chainId: " \n ", authority: authority, issue: issue),
            keypair: keypair,
            creationTimeMs: 1
        )) { error in
            XCTAssertEqual(error as? TransactionInputError, .emptyChainId)
        }
        XCTAssertThrowsError(try SwiftTransactionEncoder.encodeIssueOfflineNoteV2(
            request: IssueOfflineNoteV2Request(chainId: chainId, authority: "\(authority)@bad", issue: issue),
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
        let proof = try OfflineNoteRecursiveProofV2(
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

        XCTAssertThrowsError(try OfflineNoteRecursiveProofV2(
            verifierBackend: "custom_backend",
            verifierName: "custom_vk",
            publicInputsHash: publicInputsHash,
            proofBytes: Data([0x01]),
            proofBackend: " custom_proof_backend "
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .unsupportedRecursiveProofBackend(
                    expected: OfflineNoteV2Constants.recursiveBackend,
                    actual: " custom_proof_backend "
                )
            )
        }

        XCTAssertThrowsError(try OfflineNoteRecursiveProofV2(
            verifierBackend: "",
            verifierName: "custom_vk",
            publicInputsHash: publicInputsHash,
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(error as? VerifyingKeyIdError, .emptyBackend)
        }
        XCTAssertThrowsError(try OfflineNoteRecursiveProofV2(
            verifierBackend: "custom_backend",
            verifierName: "",
            publicInputsHash: publicInputsHash,
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(error as? VerifyingKeyIdError, .emptyName)
        }
        XCTAssertThrowsError(try OfflineNoteRecursiveProofV2(
            verifierBackend: "halo2:ipa",
            verifierName: "custom_vk",
            publicInputsHash: publicInputsHash,
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(error as? VerifyingKeyIdError, .invalidSeparator)
        }
        XCTAssertThrowsError(try OfflineNoteRecursiveProofV2(
            verifierBackend: " custom_backend ",
            verifierName: "custom_vk",
            publicInputsHash: publicInputsHash,
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(error as? VerifyingKeyIdError, .surroundingWhitespace)
        }
        XCTAssertThrowsError(try OfflineNoteRecursiveProofV2(
            verifierBackend: "custom_backend",
            verifierName: " custom_vk ",
            publicInputsHash: publicInputsHash,
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(error as? VerifyingKeyIdError, .surroundingWhitespace)
        }
    }

    func testOfflineNoteV2CertificatePayloadValidationAndEncodingBranches() throws {
        let certificate = try Self.certificate(Self.loadFixture().paymentToken.senderKeyCertificate)
        let payload = try certificate.signingPayload()

        XCTAssertEqual(payload.domain, OfflineNoteV2Constants.keyCertificatePayloadDomain)
        XCTAssertEqual(payload.version, certificate.version)
        XCTAssertEqual(payload.publicKey, certificate.publicKey)
        XCTAssertEqual(payload.oneUse, true)
        XCTAssertNotEqual(try payload.noritoEncoded(), try certificate.noritoEncoded())

        let noLimitPayload = try OfflineNoteKeyCertificatePayloadV2(
            version: OfflineNoteV2Constants.keyCertificateVersion,
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
        let limitedPayload = try OfflineNoteKeyCertificatePayloadV2(
            version: OfflineNoteV2Constants.keyCertificateVersion,
            platform: OfflineNoteV2Constants.androidKeyMintPlatform,
            keyId: Data(SHA256.hash(data: certificate.assertionPublicKey)).hexLowercased(),
            deviceId: certificate.deviceId,
            accountId: certificate.accountId,
            publicKey: certificate.publicKey,
            assertionScheme: OfflineNoteV2Constants.androidKeyMintAssertionScheme,
            assertionKeyAlgorithm: OfflineNoteV2Constants.androidKeyMintAssertionKeyAlgorithm,
            assertionPublicKey: certificate.assertionPublicKey,
            assertionUsageCountLimit: 1,
            oneUse: true
        )
        XCTAssertNil(noLimitPayload.assertionUsageCountLimit)
        XCTAssertEqual(limitedPayload.assertionUsageCountLimit, 1)
        XCTAssertNotEqual(try noLimitPayload.noritoEncoded(), try limitedPayload.noritoEncoded())

        XCTAssertThrowsError(try OfflineNoteKeyCertificatePayloadV2(
            version: OfflineNoteV2Constants.keyCertificateVersion,
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
        XCTAssertThrowsError(try OfflineNoteKeyCertificatePayloadV2(
            version: OfflineNoteV2Constants.keyCertificateVersion,
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
                error as? OfflineNoteV2Error,
                .invalidNotePublicKeyLength(expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try OfflineNoteKeyCertificatePayloadV2(
            version: OfflineNoteV2Constants.keyCertificateVersion,
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
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try OfflineNoteKeyCertificateV2(
            version: OfflineNoteV2Constants.keyCertificateVersion,
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
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try OfflineNoteKeyCertificatePayloadV2(
            version: OfflineNoteV2Constants.keyCertificateVersion,
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
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
        XCTAssertThrowsError(try OfflineNoteKeyCertificatePayloadV2(
            version: OfflineNoteV2Constants.keyCertificateVersion,
            platform: OfflineNoteV2Constants.androidKeyMintPlatform,
            keyId: Data(SHA256.hash(data: certificate.assertionPublicKey)).hexLowercased(),
            deviceId: certificate.deviceId,
            accountId: certificate.accountId,
            publicKey: certificate.publicKey,
            assertionScheme: OfflineNoteV2Constants.androidKeyMintAssertionScheme,
            assertionKeyAlgorithm: OfflineNoteV2Constants.androidKeyMintAssertionKeyAlgorithm,
            assertionPublicKey: certificate.assertionPublicKey,
            assertionUsageCountLimit: 7,
            oneUse: true
        )) { error in
            guard case OfflineNoteV2Error.unsupportedDeviceAttestationProfile = error else {
                return XCTFail("expected unsupportedDeviceAttestationProfile, got \(error)")
            }
        }
    }

    private static func offCurveP256AssertionPublicKey() -> Data {
        var key = Data(repeating: 0, count: 65)
        key[0] = 0x04
        return key
    }

    func testOfflineNoteV2PublicInputConstructorsRejectMalformedInputs() throws {
        let fixture = try Self.loadFixture()
        let audit = try Self.audit(fixture)
        let redeem = try Self.redeem(fixture)
        let auditOutputClaims = try audit.outputClaims.map(OfflineNoteIssuedClaimV2.fromAuditOutput)

        XCTAssertThrowsError(try OfflineNoteRedeemPublicInputsV2(
            sourceNoteCommitment: Data(repeating: 0x01, count: 31),
            inputNullifiers: redeem.inputNullifiers,
            keyCertificatePayloadHash: try redeem.senderKeyCertificate.payloadHash(),
            recipient: redeem.recipient,
            assetId: redeem.assetId,
            amount: redeem.amount
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .invalidHashLength(field: "source_note_commitment", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try OfflineNoteRedeemPublicInputsV2(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: redeem.inputNullifiers,
            keyCertificatePayloadHash: Data(repeating: 0x01, count: 31),
            recipient: redeem.recipient,
            assetId: redeem.assetId,
            amount: redeem.amount
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .invalidHashLength(field: "key_certificate_payload_hash", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try OfflineNoteRedeemPublicInputsV2(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: redeem.inputNullifiers,
            keyCertificatePayloadHash: try redeem.senderKeyCertificate.payloadHash(),
            recipient: "\(redeem.recipient)@bad",
            assetId: redeem.assetId,
            amount: redeem.amount
        ))

        XCTAssertThrowsError(try OfflineNoteAuditPublicInputsV2(
            tokenId: Data(repeating: 0x01, count: 31),
            keyCertificatePayloadHash: try audit.senderKeyCertificate.payloadHash(),
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: auditOutputClaims
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .invalidHashLength(field: "token_id", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try OfflineNoteAuditPublicInputsV2(
            tokenId: audit.tokenId,
            keyCertificatePayloadHash: try audit.senderKeyCertificate.payloadHash(),
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: []
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .emptyOutputClaims)
        }
    }

    func testOfflineNoteV2DomainsRejectSubstitutionAndPadding() throws {
        let fixture = try Self.loadFixture()
        let certificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let audit = try Self.audit(fixture)
        let redeem = try Self.redeem(fixture)
        let claim = audit.inputClaims[0]
        let auditPublic = try audit.publicInputs()
        let redeemPublic = try redeem.publicInputs()

        XCTAssertThrowsError(try OfflineNoteKeyCertificatePayloadV2(
            domain: "\(OfflineNoteV2Constants.keyCertificatePayloadDomain) ",
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
                error as? OfflineNoteV2Error,
                .unsupportedDomain(
                    field: "domain",
                    expected: OfflineNoteV2Constants.keyCertificatePayloadDomain,
                    actual: "\(OfflineNoteV2Constants.keyCertificatePayloadDomain) "
                )
            )
        }
        XCTAssertThrowsError(try OfflineNoteIssuedClaimV2(
            domain: "\(OfflineNoteV2Constants.issuedClaimDomain)\n",
            noteCommitment: claim.noteCommitment,
            keyCertificatePayloadHash: claim.keyCertificatePayloadHash,
            assetId: claim.assetId,
            amount: claim.amount
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .unsupportedDomain(
                    field: "domain",
                    expected: OfflineNoteV2Constants.issuedClaimDomain,
                    actual: "\(OfflineNoteV2Constants.issuedClaimDomain)\n"
                )
            )
        }
        XCTAssertThrowsError(try OfflineNoteRedeemPublicInputsV2(
            domain: "forged:\(OfflineNoteV2Constants.redeemPublicInputsDomain)",
            sourceNoteCommitment: redeemPublic.sourceNoteCommitment,
            inputNullifiers: redeemPublic.inputNullifiers,
            keyCertificatePayloadHash: redeemPublic.keyCertificatePayloadHash,
            recipient: redeemPublic.recipient,
            assetId: redeemPublic.assetId,
            amount: redeemPublic.amount
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .unsupportedDomain(
                    field: "domain",
                    expected: OfflineNoteV2Constants.redeemPublicInputsDomain,
                    actual: "forged:\(OfflineNoteV2Constants.redeemPublicInputsDomain)"
                )
            )
        }
        XCTAssertThrowsError(try OfflineNoteAuditPublicInputsV2(
            domain: " \(OfflineNoteV2Constants.auditPublicInputsDomain)",
            tokenId: auditPublic.tokenId,
            keyCertificatePayloadHash: auditPublic.keyCertificatePayloadHash,
            inputNullifiers: auditPublic.inputNullifiers,
            inputClaims: auditPublic.inputClaims,
            outputCommitments: auditPublic.outputCommitments,
            outputClaims: auditPublic.outputClaims
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .unsupportedDomain(
                    field: "domain",
                    expected: OfflineNoteV2Constants.auditPublicInputsDomain,
                    actual: " \(OfflineNoteV2Constants.auditPublicInputsDomain)"
                )
            )
        }
    }

    private static func issue(_ fixture: OfflineInteropFixture) throws -> OfflineNoteIssueV2 {
        try OfflineNoteIssueV2(
            noteCommitment: hex(fixture.chainVectors.issue.noteCommitment),
            keyCertificate: certificate(fixture.paymentToken.senderKeyCertificate),
            assetId: fixture.chainVectors.issue.assetId,
            amount: fixture.chainVectors.issue.amount
        )
    }

    private static func redeem(_ fixture: OfflineInteropFixture) throws -> OfflineNoteRedeemV2 {
        let vector = fixture.chainVectors.redeem
        return try OfflineNoteRedeemV2(
            sourceNoteCommitment: hex(vector.sourceNoteCommitment),
            inputNullifiers: try vector.inputNullifiers.map(hex),
            senderKeyCertificate: certificate(fixture.paymentToken.recipientKeyCertificate),
            recipient: fixture.paymentToken.recipientAccountId,
            assetId: vector.assetId,
            amount: vector.amount,
            recursiveProof: OfflineNoteRecursiveProofV2(
                publicInputsHash: hex(vector.publicInputsHash),
                proofBytes: Data("offline-v2-vector-redeem-proof".utf8)
            )
        )
    }

    private static func audit(_ fixture: OfflineInteropFixture) throws -> OfflineNoteAuditBundleV2 {
        let vector = fixture.chainVectors.audit
        return try OfflineNoteAuditBundleV2(
            tokenId: hex(vector.tokenId),
            senderKeyCertificate: certificate(fixture.paymentToken.senderKeyCertificate),
            inputNullifiers: try vector.inputNullifiers.map(hex),
            inputClaims: try fixture.paymentToken.inputClaims.map(issuedClaim),
            outputCommitments: try vector.outputCommitments.map(hex),
            outputClaims: try fixture.paymentToken.outputClaims.map(auditOutputClaim),
            recursiveProof: OfflineNoteRecursiveProofV2(
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

    private static func certificate(_ json: OfflineCertificateJSON) throws -> OfflineNoteKeyCertificateV2 {
        try OfflineNoteKeyCertificateV2(
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

    private static func issuedClaim(_ json: OfflineInputClaimJSON) throws -> OfflineNoteIssuedClaimV2 {
        try OfflineNoteIssuedClaimV2(
            domain: json.domain,
            noteCommitment: hex(json.noteCommitment),
            keyCertificatePayloadHash: hex(json.keyCertificatePayloadHash),
            assetId: json.assetId,
            amount: json.amount
        )
    }

    private static func auditOutputClaim(_ json: OfflineOutputClaimJSON) throws -> OfflineNoteAuditOutputClaimV2 {
        try OfflineNoteAuditOutputClaimV2(
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
            throw OfflineNoteV2FixtureError.invalidHex(value)
        }
        return data
    }

    private static func base64(_ value: String) throws -> Data {
        guard let data = Data(base64Encoded: value) else {
            throw OfflineNoteV2FixtureError.invalidBase64
        }
        return data
    }

    private static func attestationEvidence(attestationReportHash: Data) -> Data {
        var evidence = Data(OfflineNoteV2Constants.deviceAttestationEvidencePrefix.utf8)
        evidence.append(attestationReportHash)
        return evidence
    }

    private static func recursiveProofPayload(
        publicInputsHash: Data,
        proofBackend: String,
        proofBytes: Data
    ) -> Data {
        var proofWriter = OfflineCompactNoritoWriter()
        proofWriter.writeField(OfflineCompactNorito.encodeString(OfflineNoteV2Constants.recursiveBackend))
        proofWriter.writeField(OfflineCompactNorito.encodeString(OfflineNoteV2Constants.recursiveVerifierName))

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

    private struct ParsedOfflineNoteV2Instruction {
        let wireName: String
        let archive: Data
    }

    private static func parseSingleOfflineNoteV2Instruction(
        _ envelope: SignedTransactionEnvelope
    ) throws -> ParsedOfflineNoteV2Instruction {
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
        return ParsedOfflineNoteV2Instruction(wireName: wireName, archive: archive)
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

private enum OfflineNoteV2FixtureError: Error {
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
