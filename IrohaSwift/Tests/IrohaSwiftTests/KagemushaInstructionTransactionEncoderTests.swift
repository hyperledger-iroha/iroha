import XCTest
@testable import IrohaSwift

final class KagemushaInstructionTransactionEncoderTests: XCTestCase {
    func testBuildRecursiveRedeemInstructionTransactionWrapsNativeInstructionArchive() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x42, count: 32))
        let authority = try Self.canonicalAuthorityLiteral(from: signingKey)
        let instructionArchive = Self.instructionArchive(
            type: .redeemRecursive,
            payload: Data([0xA1, 0xB2, 0xC3])
        )
        XCTAssertEqual(
            try KagemushaInstructionType.validatedArchiveType(for: instructionArchive),
            .redeemRecursive
        )

        let envelope = try SwiftTransactionEncoder.encodeKagemushaInstruction(
            request: KagemushaInstructionTransactionRequest(
                chainId: "00000000-0000-0000-0000-000000000000",
                authority: authority,
                ttlMs: 90_000,
                nonce: 7,
                instructionArchive: instructionArchive
            ),
            signingKey: signingKey,
            creationTimeMs: 1_717_777_000
        )

        XCTAssertEqual(envelope.norito.first, 1)
        XCTAssertFalse(envelope.signedTransaction.isEmpty)
        XCTAssertFalse(envelope.transactionHash.isEmpty)

        let parsed = try Self.parseSingleInstructionEnvelope(envelope)
        XCTAssertEqual(parsed.chainId, "00000000-0000-0000-0000-000000000000")
        XCTAssertEqual(parsed.authority, authority)
        XCTAssertEqual(parsed.creationTimeMs, 1_717_777_000)
        XCTAssertEqual(parsed.ttlMs, 90_000)
        XCTAssertEqual(parsed.nonce, 7)
        XCTAssertEqual(parsed.metadataCount, 0)
        XCTAssertEqual(parsed.instructionWireName, KagemushaInstructionType.redeemRecursive.wireName)
        XCTAssertEqual(parsed.instructionArchive, instructionArchive)
    }

    func testBuildKagemushaTransferInstructionTransactionUsesTransferWireName() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x43, count: 32))
        let authority = try Self.canonicalAuthorityLiteral(from: signingKey)
        let instructionArchive = Self.instructionArchive(
            type: .transfer,
            payload: Data([0x01, 0x02, 0x03, 0x04])
        )
        XCTAssertEqual(
            try KagemushaInstructionType.validatedArchiveType(for: instructionArchive),
            .transfer
        )

        let envelope = try SwiftTransactionEncoder.encodeKagemushaInstruction(
            request: KagemushaInstructionTransactionRequest(
                chainId: "chain",
                authority: authority,
                instructionArchive: instructionArchive
            ),
            signingKey: signingKey,
            creationTimeMs: 10
        )

        let parsed = try Self.parseSingleInstructionEnvelope(envelope)
        XCTAssertEqual(parsed.instructionWireName, KagemushaInstructionType.transfer.wireName)
        XCTAssertEqual(parsed.instructionArchive, instructionArchive)
        XCTAssertNil(parsed.ttlMs)
        XCTAssertNil(parsed.nonce)
    }

    func testKagemushaArchiveValidationAcceptsSharedAbi7Fixtures() throws {
        let redeemRequest = try Self.sharedRecursiveSpendAbi7Archive(named: "redeem_request")
        let redeemInstruction = try Self.sharedRecursiveSpendAbi7Archive(named: "redeem_instruction")

        XCTAssertNoThrow(try KagemushaRecursiveRedeemRequestArchive.validate(redeemRequest))
        XCTAssertEqual(
            try KagemushaInstructionType.validatedArchiveType(for: redeemInstruction),
            .redeemRecursive
        )
    }

    func testBuildKagemushaRecursiveRedeemTransactionDerivesInstructionBeforeSigning() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x44, count: 32))
        let authority = try Self.canonicalAuthorityLiteral(from: signingKey)
        let requestArchive = Self.redeemRequestArchive(payload: Data([0x10, 0x11, 0x12]))
        let instructionArchive = Self.instructionArchive(
            type: .redeemRecursive,
            payload: Data([0xA0, 0xA1, 0xA2, 0xA3])
        )
        var redeemedRequests: [Data] = []

        let envelope = try SwiftTransactionEncoder.encodeKagemushaRecursiveRedeem(
            request: KagemushaRecursiveRedeemTransactionRequest(
                chainId: "chain",
                authority: authority,
                ttlMs: 123,
                nonce: 9,
                metadata: ["kagemusha": .string("redeem")],
                redeemRequestArchive: requestArchive
            ),
            signingKey: signingKey,
            creationTimeMs: 20,
            redeem: { archive in
                redeemedRequests.append(archive)
                return instructionArchive
            }
        )

        XCTAssertEqual(redeemedRequests, [requestArchive])
        let parsed = try Self.parseSingleInstructionEnvelope(envelope)
        XCTAssertEqual(parsed.chainId, "chain")
        XCTAssertEqual(parsed.authority, authority)
        XCTAssertEqual(parsed.creationTimeMs, 20)
        XCTAssertEqual(parsed.ttlMs, 123)
        XCTAssertEqual(parsed.nonce, 9)
        XCTAssertEqual(parsed.metadataCount, 1)
        XCTAssertEqual(parsed.instructionWireName, KagemushaInstructionType.redeemRecursive.wireName)
        XCTAssertEqual(parsed.instructionArchive, instructionArchive)
    }

    func testKagemushaInstructionRequestsPreserveDataAfterCallerMutation() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x48, count: 32))
        let authority = try Self.canonicalAuthorityLiteral(from: signingKey)
        var instructionArchive = Self.instructionArchive(
            type: .transfer,
            payload: Data([0x31, 0x32, 0x33])
        )
        let expectedInstructionArchive = instructionArchive
        let instructionRequest = KagemushaInstructionTransactionRequest(
            chainId: "chain",
            authority: authority,
            instructionArchive: instructionArchive
        )
        instructionArchive[0] = 0

        let instructionEnvelope = try SwiftTransactionEncoder.encodeKagemushaInstruction(
            request: instructionRequest,
            signingKey: signingKey,
            creationTimeMs: 50
        )
        let parsedInstruction = try Self.parseSingleInstructionEnvelope(instructionEnvelope)
        XCTAssertEqual(parsedInstruction.instructionArchive, expectedInstructionArchive)

        var redeemRequestArchive = Self.redeemRequestArchive(payload: Data([0x41, 0x42, 0x43]))
        let expectedRedeemRequestArchive = redeemRequestArchive
        var nativeInstructionArchive = Self.instructionArchive(
            type: .redeemRecursive,
            payload: Data([0x51, 0x52, 0x53])
        )
        let expectedNativeInstructionArchive = nativeInstructionArchive
        let redeemRequest = KagemushaRecursiveRedeemTransactionRequest(
            chainId: "chain",
            authority: authority,
            redeemRequestArchive: redeemRequestArchive
        )
        redeemRequestArchive[0] = 0
        var redeemedRequests: [Data] = []

        let redeemEnvelope = try SwiftTransactionEncoder.encodeKagemushaRecursiveRedeem(
            request: redeemRequest,
            signingKey: signingKey,
            creationTimeMs: 51,
            redeem: { archive in
                redeemedRequests.append(archive)
                return nativeInstructionArchive
            }
        )
        nativeInstructionArchive[0] = 0

        XCTAssertEqual(redeemedRequests, [expectedRedeemRequestArchive])
        let parsedRedeem = try Self.parseSingleInstructionEnvelope(redeemEnvelope)
        XCTAssertEqual(parsedRedeem.instructionArchive, expectedNativeInstructionArchive)
    }

    func testKagemushaInstructionRequestsRejectPaddedIdsBeforeArchiveOrRedeem() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x49, count: 32))
        let authority = try Self.canonicalAuthorityLiteral(from: signingKey)

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeKagemushaInstruction(
                request: KagemushaInstructionTransactionRequest(
                    chainId: " chain",
                    authority: authority,
                    instructionArchive: Data()
                ),
                signingKey: signingKey,
                creationTimeMs: 60
            )
        ) { error in
            XCTAssertEqual(error as? TransactionInputError, .invalidChainId(" chain"))
        }

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeKagemushaInstruction(
                request: KagemushaInstructionTransactionRequest(
                    chainId: "chain",
                    authority: " \(authority) ",
                    instructionArchive: Data()
                ),
                signingKey: signingKey,
                creationTimeMs: 61
            )
        ) { error in
            XCTAssertEqual(
                error as? TransactionInputError,
                .malformedAccountId(field: "authority", value: " \(authority) ")
            )
        }

        var redeemedRequests: [Data] = []
        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeKagemushaRecursiveRedeem(
                request: KagemushaRecursiveRedeemTransactionRequest(
                    chainId: " chain",
                    authority: authority,
                    redeemRequestArchive: Data()
                ),
                signingKey: signingKey,
                creationTimeMs: 62,
                redeem: { archive in
                    redeemedRequests.append(archive)
                    return archive
                }
            )
        ) { error in
            XCTAssertEqual(error as? TransactionInputError, .invalidChainId(" chain"))
        }

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeKagemushaRecursiveRedeem(
                request: KagemushaRecursiveRedeemTransactionRequest(
                    chainId: "chain",
                    authority: " \(authority) ",
                    redeemRequestArchive: Data()
                ),
                signingKey: signingKey,
                creationTimeMs: 63,
                redeem: { archive in
                    redeemedRequests.append(archive)
                    return archive
                }
            )
        ) { error in
            XCTAssertEqual(
                error as? TransactionInputError,
                .malformedAccountId(field: "authority", value: " \(authority) ")
            )
        }
        XCTAssertEqual(redeemedRequests, [])
    }

    func testNativeBridgeRejectsPaddedAuthorityBeforeChainDiscriminantInference() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x4A, count: 32))
        let authority = try Self.canonicalAuthorityLiteral(from: signingKey)

        XCTAssertEqual(
            try NoritoNativeBridge.shared.validatedAuthorityChainDiscriminant(authority: authority),
            AccountId.defaultNetworkPrefix
        )

        for invalidAuthority in [
            " \(authority)",
            "\(authority) ",
            "\t\(authority)",
            "\(authority)\n",
            "\(authority)@banka.dataspace",
            "not an account",
        ] {
            XCTAssertThrowsError(
                try NoritoNativeBridge.shared.validatedAuthorityChainDiscriminant(authority: invalidAuthority)
            ) { error in
                XCTAssertEqual(error as? NativeBridgeError, .authority)
            }
        }
    }

    func testKagemushaInstructionTransactionRejectsAdversarialArchives() throws {
        XCTAssertThrowsError(try KagemushaInstructionTransactionEncoder.validateInstructionArchive(Data())) { error in
            XCTAssertEqual(error as? KagemushaInstructionTransactionError, .emptyInstructionArchive)
        }

        XCTAssertThrowsError(
            try KagemushaInstructionTransactionEncoder.validateInstructionArchive(Data([0x4E, 0x52, 0x54, 0x30]))
        ) { error in
            XCTAssertEqual(error as? KagemushaInstructionTransactionError, .invalidInstructionArchive)
        }

        let zeroPayloadArchive = Self.instructionArchive(type: .redeemRecursive, payload: Data())
        XCTAssertThrowsError(
            try KagemushaInstructionTransactionEncoder.validateInstructionArchive(zeroPayloadArchive)
        ) { error in
            XCTAssertEqual(error as? KagemushaInstructionTransactionError, .invalidInstructionArchive)
        }

        let wrongTypeArchive = noritoEncode(
            typeName: "RedeemOfflineNoteV2",
            payload: Data([0x01]),
            flags: 0
        )
        XCTAssertThrowsError(
            try KagemushaInstructionTransactionEncoder.validateInstructionArchive(wrongTypeArchive)
        ) { error in
            XCTAssertEqual(error as? KagemushaInstructionTransactionError, .unsupportedInstructionArchiveType)
        }

        var invalidCompressionArchive = Self.instructionArchive(type: .redeemRecursive, payload: Data([0x01]))
        invalidCompressionArchive[22] = 0xff
        XCTAssertThrowsError(
            try KagemushaInstructionTransactionEncoder.validateInstructionArchive(invalidCompressionArchive)
        ) { error in
            XCTAssertEqual(error as? KagemushaInstructionTransactionError, .invalidInstructionArchive)
        }

        var checksumTamperedArchive = Self.instructionArchive(type: .redeemRecursive, payload: Data([0x01]))
        checksumTamperedArchive[31] ^= 0xff
        XCTAssertThrowsError(
            try KagemushaInstructionTransactionEncoder.validateInstructionArchive(checksumTamperedArchive)
        ) { error in
            XCTAssertEqual(error as? KagemushaInstructionTransactionError, .invalidInstructionArchive)
        }

        var unsupportedFlagsArchive = Self.instructionArchive(type: .redeemRecursive, payload: Data([0x01]))
        unsupportedFlagsArchive[39] = NoritoHeader.varintOffsets
        XCTAssertThrowsError(
            try KagemushaInstructionTransactionEncoder.validateInstructionArchive(unsupportedFlagsArchive)
        ) { error in
            XCTAssertEqual(error as? KagemushaInstructionTransactionError, .invalidInstructionArchive)
        }

        var invalidFieldBitsetArchive = Self.instructionArchive(type: .redeemRecursive, payload: Data([0x01]))
        invalidFieldBitsetArchive[39] = NoritoHeader.fieldBitset
        XCTAssertThrowsError(
            try KagemushaInstructionTransactionEncoder.validateInstructionArchive(invalidFieldBitsetArchive)
        ) { error in
            XCTAssertEqual(error as? KagemushaInstructionTransactionError, .invalidInstructionArchive)
        }

        var nonZeroPaddingArchive = Self.instructionArchive(type: .redeemRecursive, payload: Data([0x01]))
        nonZeroPaddingArchive.insert(0xff, at: NoritoHeader.encodedLength)
        XCTAssertThrowsError(
            try KagemushaInstructionTransactionEncoder.validateInstructionArchive(nonZeroPaddingArchive)
        ) { error in
            XCTAssertEqual(error as? KagemushaInstructionTransactionError, .invalidInstructionArchive)
        }

        var excessivePaddingArchive = Self.instructionArchive(type: .redeemRecursive, payload: Data([0x01]))
        excessivePaddingArchive.insert(contentsOf: Data(repeating: 0, count: 65), at: NoritoHeader.encodedLength)
        XCTAssertThrowsError(
            try KagemushaInstructionTransactionEncoder.validateInstructionArchive(excessivePaddingArchive)
        ) { error in
            XCTAssertEqual(error as? KagemushaInstructionTransactionError, .invalidInstructionArchive)
        }

        let oversizedArchive = Data(count: KagemushaRecursiveCompactPaymentTokenProver.nativeArchiveMaxBytes + 1)
        XCTAssertThrowsError(
            try KagemushaInstructionTransactionEncoder.validateInstructionArchive(oversizedArchive)
        ) { error in
            XCTAssertEqual(error as? KagemushaInstructionTransactionError, .oversizedInstructionArchive)
        }
    }

    func testKagemushaRecursiveRedeemTransactionRejectsMalformedRequestBeforeNativeRedeem() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x46, count: 32))
        let authority = try Self.canonicalAuthorityLiteral(from: signingKey)
        var nativeRedeemCalled = false

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeKagemushaRecursiveRedeem(
                request: KagemushaRecursiveRedeemTransactionRequest(
                    chainId: "chain",
                    authority: authority,
                    redeemRequestArchive: Data()
                ),
                signingKey: signingKey,
                creationTimeMs: 30,
                redeem: { _ in
                    nativeRedeemCalled = true
                    return Self.instructionArchive(type: .redeemRecursive, payload: Data([0x01]))
                }
            )
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveRedeemRequestArchiveError, .emptyRequestArchive)
        }
        XCTAssertFalse(nativeRedeemCalled)

        let wrongTypeArchive = Self.instructionArchive(type: .redeemRecursive, payload: Data([0x01]))
        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeKagemushaRecursiveRedeem(
                request: KagemushaRecursiveRedeemTransactionRequest(
                    chainId: "chain",
                    authority: authority,
                    redeemRequestArchive: wrongTypeArchive
                ),
                signingKey: signingKey,
                creationTimeMs: 30,
                redeem: { _ in
                    nativeRedeemCalled = true
                    return Self.instructionArchive(type: .redeemRecursive, payload: Data([0x01]))
                }
            )
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveRedeemRequestArchiveError, .unsupportedRequestArchiveType)
        }
        XCTAssertFalse(nativeRedeemCalled)
    }

    func testKagemushaRecursiveRedeemTransactionRejectsAdversarialNativeInstructionArchives() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x47, count: 32))
        let authority = try Self.canonicalAuthorityLiteral(from: signingKey)
        let requestArchive = Self.redeemRequestArchive(payload: Data([0x01]))

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeKagemushaRecursiveRedeem(
                request: KagemushaRecursiveRedeemTransactionRequest(
                    chainId: "chain",
                    authority: authority,
                    redeemRequestArchive: requestArchive
                ),
                signingKey: signingKey,
                creationTimeMs: 40,
                redeem: { _ in Data([0x4E, 0x52, 0x54, 0x30]) }
            )
        ) { error in
            XCTAssertEqual(error as? KagemushaInstructionTransactionError, .invalidInstructionArchive)
        }

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeKagemushaRecursiveRedeem(
                request: KagemushaRecursiveRedeemTransactionRequest(
                    chainId: "chain",
                    authority: authority,
                    redeemRequestArchive: requestArchive
                ),
                signingKey: signingKey,
                creationTimeMs: 40,
                redeem: { _ in Self.instructionArchive(type: .transfer, payload: Data([0x01])) }
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaInstructionTransactionError,
                .unexpectedInstructionArchiveType(expected: .redeemRecursive, actual: .transfer)
            )
        }

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeKagemushaRecursiveRedeem(
                request: KagemushaRecursiveRedeemTransactionRequest(
                    chainId: "chain",
                    authority: authority,
                    redeemRequestArchive: requestArchive
                ),
                signingKey: signingKey,
                creationTimeMs: 40,
                redeem: { _ in throw KagemushaRecursiveSpendProverError.proofRejected }
            )
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveSpendProverError, .proofRejected)
        }
    }

    func testKagemushaRecursiveRedeemRequestArchiveValidationRejectsAdversarialInputs() throws {
        let validArchive = Self.redeemRequestArchive(payload: Data([0x01, 0x02, 0x03]))
        XCTAssertNoThrow(try KagemushaRecursiveRedeemRequestArchive.validate(validArchive))

        XCTAssertThrowsError(
            try KagemushaRecursiveRedeemRequestArchive.validate(Data())
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveRedeemRequestArchiveError, .emptyRequestArchive)
        }
        XCTAssertThrowsError(
            try KagemushaRecursiveRedeemRequestArchive.validate(Data([0x4E, 0x52, 0x54, 0x30]))
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveRedeemRequestArchiveError, .invalidRequestArchive)
        }

        let emptyPayloadArchive = Self.redeemRequestArchive(payload: Data())
        XCTAssertThrowsError(
            try KagemushaRecursiveRedeemRequestArchive.validate(emptyPayloadArchive)
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveRedeemRequestArchiveError, .invalidRequestArchive)
        }

        let wrongTypeArchive = noritoEncode(
            typeName: KagemushaInstructionType.redeemRecursive.rawValue,
            payload: Data([0x01]),
            flags: 0
        )
        XCTAssertThrowsError(
            try KagemushaRecursiveRedeemRequestArchive.validate(wrongTypeArchive)
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveRedeemRequestArchiveError, .unsupportedRequestArchiveType)
        }

        var invalidCompressionArchive = validArchive
        invalidCompressionArchive[22] = 0xff
        XCTAssertThrowsError(
            try KagemushaRecursiveRedeemRequestArchive.validate(invalidCompressionArchive)
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveRedeemRequestArchiveError, .invalidRequestArchive)
        }

        var checksumTamperedArchive = validArchive
        checksumTamperedArchive[31] ^= 0xff
        XCTAssertThrowsError(
            try KagemushaRecursiveRedeemRequestArchive.validate(checksumTamperedArchive)
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveRedeemRequestArchiveError, .invalidRequestArchive)
        }

        var unsupportedFlagsArchive = validArchive
        unsupportedFlagsArchive[39] = NoritoHeader.varintOffsets
        XCTAssertThrowsError(
            try KagemushaRecursiveRedeemRequestArchive.validate(unsupportedFlagsArchive)
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveRedeemRequestArchiveError, .invalidRequestArchive)
        }

        var invalidFieldBitsetArchive = validArchive
        invalidFieldBitsetArchive[39] = NoritoHeader.fieldBitset
        XCTAssertThrowsError(
            try KagemushaRecursiveRedeemRequestArchive.validate(invalidFieldBitsetArchive)
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveRedeemRequestArchiveError, .invalidRequestArchive)
        }

        var nonZeroPaddingArchive = validArchive
        nonZeroPaddingArchive.insert(0xff, at: NoritoHeader.encodedLength)
        XCTAssertThrowsError(
            try KagemushaRecursiveRedeemRequestArchive.validate(nonZeroPaddingArchive)
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveRedeemRequestArchiveError, .invalidRequestArchive)
        }

        var excessivePaddingArchive = validArchive
        excessivePaddingArchive.insert(contentsOf: Data(repeating: 0, count: 65), at: NoritoHeader.encodedLength)
        XCTAssertThrowsError(
            try KagemushaRecursiveRedeemRequestArchive.validate(excessivePaddingArchive)
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveRedeemRequestArchiveError, .invalidRequestArchive)
        }

        let oversizedArchive = Data(count: KagemushaRecursiveCompactPaymentTokenProver.nativeArchiveMaxBytes + 1)
        XCTAssertThrowsError(
            try KagemushaRecursiveRedeemRequestArchive.validate(oversizedArchive)
        ) { error in
            XCTAssertEqual(error as? KagemushaRecursiveRedeemRequestArchiveError, .oversizedRequestArchive)
        }
    }

    func testKagemushaInstructionRequestValidationRejectsInvalidInputsBeforeSigning() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x45, count: 32))
        let authority = try Self.canonicalAuthorityLiteral(from: signingKey)

        XCTAssertNoThrow(
            try KagemushaInstructionTransactionRequest.validateInputs(
                chainId: "chain",
                authority: authority
            )
        )
        XCTAssertThrowsError(
            try KagemushaInstructionTransactionRequest.validateInputs(
                chainId: "bad chain",
                authority: authority
            )
        ) { error in
            XCTAssertEqual(error as? TransactionInputError, .invalidChainId("bad chain"))
        }
        XCTAssertThrowsError(
            try KagemushaInstructionTransactionRequest.validateInputs(
                chainId: "chain",
                authority: "not an account"
            )
        ) { error in
            XCTAssertEqual(
                error as? TransactionInputError,
                .malformedAccountId(field: "authority", value: "not an account")
            )
        }
    }

    private static func instructionArchive(
        type: KagemushaInstructionType,
        payload: Data
    ) -> Data {
        noritoEncode(typeName: type.wireName, payload: payload, flags: 0)
    }

    private static func redeemRequestArchive(payload: Data) -> Data {
        noritoEncode(typeName: KagemushaRecursiveRedeemRequestArchive.schemaName, payload: payload, flags: 0)
    }

    private static func sharedRecursiveSpendAbi7Archive(named archiveName: String) throws -> Data {
        let fixture = try sharedRecursiveSpendAbi7Archives()
        let archives = try XCTUnwrap(fixture["archives"] as? [[String: Any]])
        let archive = try XCTUnwrap(archives.first { $0["name"] as? String == archiveName })
        let encoded = try XCTUnwrap(archive["bytes_base64"] as? String)
        return try XCTUnwrap(Data(base64Encoded: encoded))
    }

    private static func sharedRecursiveSpendAbi7Archives() throws -> [String: Any] {
        var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        for _ in 0..<10 {
            let candidate = directory
                .appendingPathComponent("fixtures")
                .appendingPathComponent("kagemusha_recursive_spend_abi7")
                .appendingPathComponent("archives.json")
            if FileManager.default.fileExists(atPath: candidate.path) {
                let data = try Data(contentsOf: candidate)
                return try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
            }
            directory.deleteLastPathComponent()
        }
        throw NSError(
            domain: "KagemushaInstructionTransactionEncoderTests",
            code: 1,
            userInfo: [NSLocalizedDescriptionKey: "missing shared recursive spend ABI-7 archives fixture"]
        )
    }

    private static func canonicalAuthorityLiteral(from signingKey: SigningKey) throws -> String {
        let publicKey = try signingKey.publicKey()
        let address = try AccountAddress.fromAccount(publicKey: publicKey)
        return try address.toI105(networkPrefix: 0x02F1)
    }

    private struct ParsedEnvelope {
        let chainId: String
        let authority: String
        let creationTimeMs: UInt64
        let ttlMs: UInt64?
        let nonce: UInt32?
        let metadataCount: UInt64
        let instructionWireName: String
        let instructionArchive: Data
    }

    private static func parseSingleInstructionEnvelope(_ envelope: SignedTransactionEnvelope) throws -> ParsedEnvelope {
        var signed = OfflineNoritoReader(data: envelope.signedTransaction)
        _ = try signed.readField()
        let transactionPayload = try signed.readField()
        XCTAssertEqual(try signed.readField(), Data([0]))
        XCTAssertEqual(try signed.readField(), Data([0]))
        XCTAssertEqual(signed.remaining(), 0)

        var transaction = OfflineNoritoReader(data: transactionPayload)
        let chainId = try readFieldString(&transaction)
        let authority = try readFieldString(&transaction)
        let creationTimeMs = try readFieldUInt64(&transaction)
        let executablePayload = try transaction.readField()
        let ttlMs = try readFieldOptionalUInt64(&transaction)
        let nonce = try readFieldOptionalUInt32(&transaction)
        let metadataCount = try readFieldMetadataCount(&transaction)
        XCTAssertEqual(transaction.remaining(), 0)

        let instructionPayload = try singleInstructionPayload(fromExecutablePayload: executablePayload)
        var instruction = OfflineNoritoReader(data: instructionPayload)
        let instructionWireName = try readFieldString(&instruction)
        let instructionArchive = try readFieldBytesVec(&instruction)
        XCTAssertEqual(instruction.remaining(), 0)

        return ParsedEnvelope(
            chainId: chainId,
            authority: authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            nonce: nonce,
            metadataCount: metadataCount,
            instructionWireName: instructionWireName,
            instructionArchive: instructionArchive
        )
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
        try readStringPayload(reader.readField())
    }

    private static func readFieldUInt64(_ reader: inout OfflineNoritoReader) throws -> UInt64 {
        var child = OfflineNoritoReader(data: try reader.readField())
        let value = try child.readUInt64LE()
        XCTAssertEqual(child.remaining(), 0)
        return value
    }

    private static func readFieldOptionalUInt64(_ reader: inout OfflineNoritoReader) throws -> UInt64? {
        var child = OfflineNoritoReader(data: try reader.readField())
        let tag = try child.readUInt8()
        switch tag {
        case 0:
            XCTAssertEqual(child.remaining(), 0)
            return nil
        case 1:
            let valuePayload = try child.readField()
            XCTAssertEqual(child.remaining(), 0)
            var valueReader = OfflineNoritoReader(data: valuePayload)
            let value = try valueReader.readUInt64LE()
            XCTAssertEqual(valueReader.remaining(), 0)
            return value
        default:
            throw OfflineNoritoDecodingError.invalidField("invalid optional UInt64 tag")
        }
    }

    private static func readFieldOptionalUInt32(_ reader: inout OfflineNoritoReader) throws -> UInt32? {
        var child = OfflineNoritoReader(data: try reader.readField())
        let tag = try child.readUInt8()
        switch tag {
        case 0:
            XCTAssertEqual(child.remaining(), 0)
            return nil
        case 1:
            let valuePayload = try child.readField()
            XCTAssertEqual(child.remaining(), 0)
            var valueReader = OfflineNoritoReader(data: valuePayload)
            let value = try valueReader.readUInt32LE()
            XCTAssertEqual(valueReader.remaining(), 0)
            return value
        default:
            throw OfflineNoritoDecodingError.invalidField("invalid optional UInt32 tag")
        }
    }

    private static func readFieldMetadataCount(_ reader: inout OfflineNoritoReader) throws -> UInt64 {
        var child = OfflineNoritoReader(data: try reader.readField())
        let count = try child.readUInt64LE()
        for _ in 0..<count {
            let entryLength = try child.readUInt64LE()
            guard entryLength <= UInt64(Int.max) else {
                throw OfflineNoritoDecodingError.invalidField("metadata entry length overflow")
            }
            _ = try child.readBytes(Int(entryLength))
        }
        XCTAssertEqual(child.remaining(), 0)
        return count
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

    private static func readStringPayload(_ payload: Data) throws -> String {
        var reader = OfflineNoritoReader(data: payload)
        let length = try reader.readUInt64LE()
        guard length <= UInt64(Int.max) else {
            throw OfflineNoritoDecodingError.invalidField("string length overflow")
        }
        let bytes = try reader.readBytes(Int(length))
        XCTAssertEqual(reader.remaining(), 0)
        guard let value = String(data: bytes, encoding: .utf8) else {
            throw OfflineNoritoDecodingError.invalidField("invalid UTF-8")
        }
        return value
    }
}
