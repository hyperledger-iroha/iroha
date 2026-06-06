import XCTest
@testable import IrohaSwift

final class OfflineNoteV2Tests: XCTestCase {
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

    func testOfflineNoteV2PublicInputHashesMatchRustVectors() throws {
        let fixture = try Self.loadFixture()
        let audit = try Self.audit(fixture)
        let redeem = try Self.redeem(fixture)

        XCTAssertEqual(try audit.publicInputsHash().hexLowercased(), fixture.chainVectors.audit.publicInputsHash)
        XCTAssertEqual(try redeem.publicInputsHash().hexLowercased(), fixture.chainVectors.redeem.publicInputsHash)
        XCTAssertNoThrow(try audit.validateProofBinding())
        XCTAssertNoThrow(try redeem.validateProofBinding())
    }

    func testOfflineNoteV2TransactionBuildersProduceSignedEnvelopes() throws {
        let fixture = try Self.loadFixture()
        let keypair = try Keypair(privateKeyBytes: Data(0..<32))
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let chainId = "00000000-0000-0000-0000-000000000000"
        let creationTimeMs: UInt64 = 1_706_000_000_000

        let issue = try SwiftTransactionEncoder.encodeIssueOfflineNoteV2(
            request: IssueOfflineNoteV2Request(
                chainId: chainId,
                authority: authority,
                issue: Self.issue(fixture),
                ttlMs: 60_000
            ),
            keypair: keypair,
            creationTimeMs: creationTimeMs
        )
        let audit = try SwiftTransactionEncoder.encodeAuditOfflineNoteV2(
            request: AuditOfflineNoteV2Request(
                chainId: chainId,
                authority: authority,
                audit: Self.audit(fixture),
                ttlMs: 60_000
            ),
            keypair: keypair,
            creationTimeMs: creationTimeMs
        )
        let redeem = try SwiftTransactionEncoder.encodeRedeemOfflineNoteV2(
            request: RedeemOfflineNoteV2Request(
                chainId: chainId,
                authority: authority,
                redemption: Self.redeem(fixture),
                ttlMs: 60_000
            ),
            keypair: keypair,
            creationTimeMs: creationTimeMs
        )

        for envelope in [issue, audit, redeem] {
            XCTAssertEqual(envelope.norito.first, 1)
            XCTAssertEqual(Data(envelope.norito.dropFirst()), envelope.signedTransaction)
            XCTAssertEqual(envelope.transactionHash.count, 32)
            XCTAssertNil(envelope.payload)
        }
        XCTAssertNotEqual(issue.transactionHash, audit.transactionHash)
        XCTAssertNotEqual(audit.transactionHash, redeem.transactionHash)
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

        let trimmedProof = try OfflineNoteProofBoxV2(
            backend: "  \(OfflineNoteV2Constants.recursiveBackend)  ",
            bytes: Data([0x01])
        )
        XCTAssertEqual(trimmedProof.backend, OfflineNoteV2Constants.recursiveBackend)

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

        let defaultEnvelope = try SwiftTransactionEncoder.encodeIssueOfflineNoteV2(
            request: IssueOfflineNoteV2Request(chainId: chainId, authority: authority, issue: issue),
            keypair: keypair,
            creationTimeMs: 1_706_000_000_000
        )
        let nonceEnvelope = try SwiftTransactionEncoder.encodeIssueOfflineNoteV2(
            request: IssueOfflineNoteV2Request(
                chainId: "  \(chainId)  ",
                authority: "  \(authority)  ",
                issue: issue,
                ttlMs: nil,
                nonce: 42
            ),
            keypair: keypair,
            creationTimeMs: 1_706_000_000_000
        )

        XCTAssertNotEqual(defaultEnvelope.signedTransaction, nonceEnvelope.signedTransaction)
        XCTAssertNotEqual(defaultEnvelope.transactionHash, nonceEnvelope.transactionHash)

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
            proofBackend: " custom_proof_backend "
        )

        XCTAssertEqual(proof.verifierKeyId.backend, "custom_backend")
        XCTAssertEqual(proof.verifierKeyId.name, "custom_vk")
        XCTAssertEqual(proof.proof.backend, "custom_proof_backend")
        XCTAssertEqual(proof.proof.bytes, Data([0x01, 0x02, 0x03]))

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
            platform: certificate.platform,
            keyId: certificate.keyId,
            deviceId: certificate.deviceId,
            accountId: certificate.accountId,
            publicKey: certificate.publicKey,
            assertionScheme: certificate.assertionScheme,
            assertionKeyAlgorithm: certificate.assertionKeyAlgorithm,
            assertionPublicKey: certificate.assertionPublicKey,
            assertionUsageCountLimit: 7,
            oneUse: true
        )
        XCTAssertNil(noLimitPayload.assertionUsageCountLimit)
        XCTAssertEqual(limitedPayload.assertionUsageCountLimit, 7)
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
