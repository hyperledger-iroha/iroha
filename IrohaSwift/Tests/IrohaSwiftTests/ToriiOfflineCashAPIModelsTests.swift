import XCTest
@testable import IrohaSwift

final class ToriiOfflineCashAPIModelsTests: XCTestCase {
    func testPublicRequestSchemaNamesMatchRust() {
        XCTAssertEqual(
            KagemushaRecursiveSpendV2.topUpRequestWireName,
            "iroha.torii.v1.offline.top_up.request"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpendV2.redeemRequestWireName,
            "iroha.torii.v1.offline.redeem.request"
        )
    }

    func testEndpointConstantsUseSharpFirstReleaseRoutes() throws {
        XCTAssertEqual(OfflineAPI.Endpoint.readiness.path, "/v1/offline/readiness")
        XCTAssertEqual(OfflineAPI.Endpoint.topUp.path, "/v1/offline/top-up")
        XCTAssertEqual(OfflineAPI.Endpoint.redeem.path, "/v1/offline/redeem")
        XCTAssertEqual(OfflineAPI.Endpoint.operations.path, "/v1/offline/operations")
        XCTAssertEqual(
            try OfflineAPI.operationPath(Self.operationId),
            "/v1/offline/operations/\(Self.operationId)"
        )
    }

    func testOperationReferenceNoritoRoundTrips() throws {
        for kind in [OfflineOperationKind.topUp, .redeem] {
            let expected = try OfflineOperationReference(
                operationId: Self.operationId,
                kind: kind,
                state: .pending,
                transactionHash: Self.transactionHash,
                statusUri: "/v1/offline/operations/\(Self.operationId)",
                submittedAtMs: UInt64.max
            )

            XCTAssertEqual(
                try OfflineOperationCodec.decodeReference(
                    OfflineOperationCodec.encodeReference(expected)
                ),
                expected
            )
        }
    }

    func testOperationReferenceMatchesRustNoritoGoldenVector() throws {
        let archive = try XCTUnwrap(Data(hexString: Self.rustOperationReferenceArchiveHex))
        let expected = try OfflineOperationReference(
            operationId: Self.operationId,
            kind: .topUp,
            state: .pending,
            transactionHash: Self.transactionHash,
            statusUri: "/v1/offline/operations/\(Self.operationId)",
            submittedAtMs: UInt64.max
        )

        XCTAssertEqual(try OfflineOperationCodec.decodeReference(archive), expected)
        XCTAssertEqual(OfflineOperationCodec.encodeReference(expected), archive)
    }

    func testPendingOperationStatusMatchesRustNoritoGoldenVector() throws {
        let archive = try XCTUnwrap(Data(hexString: Self.rustPendingStatusArchiveHex))

        XCTAssertEqual(
            try OfflineOperationCodec.decodeStatus(archive),
            .pending(try .init(
                operationId: Self.operationId,
                kind: .topUp,
                transactionHash: Self.transactionHash,
                submittedAtMs: UInt64.max
            ))
        )
    }

    func testRejectedOperationStatusMatchesRustNoritoGoldenVector() throws {
        let archive = try XCTUnwrap(Data(hexString: Self.rustRejectedStatusArchiveHex))

        XCTAssertEqual(
            try OfflineOperationCodec.decodeStatus(archive),
            .rejected(try .init(
                operationId: Self.operationId,
                kind: .redeem,
                transactionHash: Self.transactionHash,
                error: try OfflineOperationErrorEnvelope(
                    code: "offline_operation_rejected",
                    message: "rejected"
                )
            ))
        )
    }

    func testAppliedRedeemStatusMatchesRustNoritoGoldenVector() throws {
        let archive = try XCTUnwrap(Data(hexString: Self.rustAppliedRedeemStatusArchiveHex))

        XCTAssertEqual(
            try OfflineOperationCodec.decodeStatus(archive),
            .applied(try .init(
                operationId: Self.operationId,
                result: .redeem(try OfflineRedeemResult(
                    transactionHash: Self.transactionHash,
                    finalizedBlockHeight: UInt64.max,
                    serverTimeMs: 42
                ))
            ))
        )
    }

    func testOperationStatusRequiresExactSharedSchema() throws {
        let referenceArchive = try XCTUnwrap(
            Data(hexString: Self.rustOperationReferenceArchiveHex)
        )
        XCTAssertThrowsError(try OfflineOperationCodec.decodeStatus(referenceArchive))
    }

    func testOfflineTopUpAnchorUsesCurrentPublicNameAndRetainsCanonicalWire() throws {
        let archive = try canonicalTopUpAnchorArchive()
        let anchor = try OfflineTopUpAnchor(noritoArchive: archive)
        XCTAssertEqual(anchor.noritoArchive(), archive)
        XCTAssertEqual(anchor.digest, Data(repeating: 0xd8, count: 32))
        XCTAssertEqual(
            anchor.digest,
            try KagemushaRecursiveSpendV2Codecs.decodeTopUpAnchor(archive).anchorDigest
        )
        XCTAssertEqual(anchor, try OfflineTopUpAnchor(noritoArchive: archive))

        let result = try OfflineTopUpResult(
            transactionHash: Self.transactionHash,
            finalizedBlockHeight: 7,
            serverTimeMs: 8,
            anchor: anchor
        )
        XCTAssertEqual(result.anchor.noritoArchive(), archive)

        XCTAssertThrowsError(try OfflineTopUpAnchor(noritoArchive: Data()))
        XCTAssertThrowsError(try OfflineTopUpAnchor(noritoArchive: noritoEncode(
            typeName: "wrong.anchor.schema",
            payload: try XCTUnwrap(noritoDecodeFrame(archive)).payload,
            flags: NoritoHeader.compactLen
        )))

        var corrupted = archive
        corrupted[corrupted.index(before: corrupted.endIndex)] ^= 0xff
        XCTAssertThrowsError(try OfflineTopUpAnchor(noritoArchive: corrupted))
    }

    func testOperationReferencesRequireCanonicalHashAndBoundStatusUri() throws {
        for invalidHash in [
            "",
            String(repeating: "2", count: 63),
            String(repeating: "2", count: 65),
            String(repeating: "A", count: 64),
            String(repeating: "g", count: 64),
            " \(Self.transactionHash)",
        ] {
            XCTAssertThrowsError(try OfflineOperationReference(
                operationId: Self.operationId,
                kind: .topUp,
                state: .pending,
                transactionHash: invalidHash,
                statusUri: "/v1/offline/operations/\(Self.operationId)",
                submittedAtMs: 1
            )) { error in
                XCTAssertEqual(error as? OfflineOperationError, .invalidField("transaction_hash"))
            }
        }

        for invalidUri in [
            "/v1/offline/operations/\(String(repeating: "33", count: 32))",
            "https://example.test/v1/offline/operations/\(Self.operationId)",
            "/v1/offline/operations/\(Self.operationId)/",
            " /v1/offline/operations/\(Self.operationId)",
        ] {
            XCTAssertThrowsError(try OfflineOperationReference(
                operationId: Self.operationId,
                kind: .topUp,
                state: .pending,
                transactionHash: Self.transactionHash,
                statusUri: invalidUri,
                submittedAtMs: 1
            )) { error in
                XCTAssertEqual(error as? OfflineOperationError, .invalidField("status_uri"))
            }
        }
    }

    func testTaggedOperationStatePayloadsCannotBypassValidation() throws {
        XCTAssertThrowsError(try OfflineOperationStatus.Pending(
            operationId: String(repeating: "0", count: 64),
            kind: .topUp,
            transactionHash: Self.transactionHash,
            submittedAtMs: 1
        )) { error in
            XCTAssertEqual(error as? OfflineOperationError, .invalidField("operation_id"))
        }

        XCTAssertThrowsError(try OfflineOperationStatus.Pending(
            operationId: Self.operationId,
            kind: .topUp,
            transactionHash: String(repeating: "F", count: 64),
            submittedAtMs: 1
        )) { error in
            XCTAssertEqual(error as? OfflineOperationError, .invalidField("transaction_hash"))
        }

        XCTAssertThrowsError(try OfflineRedeemResult(
            transactionHash: "not-a-hash",
            finalizedBlockHeight: 1,
            serverTimeMs: 2
        )) { error in
            XCTAssertEqual(error as? OfflineOperationError, .invalidField("transaction_hash"))
        }

        let valid = try OfflineOperationStatus.Pending(
            operationId: Self.operationId,
            kind: .redeem,
            transactionHash: Self.transactionHash,
            submittedAtMs: UInt64.max
        )
        XCTAssertEqual(OfflineOperationStatus.pending(valid).operationId, Self.operationId)
    }

    func testTypedErrorsRequireStableCodesAndExactText() throws {
        for invalidCode in [
            "",
            "_leading",
            "Uppercase",
            "has-hyphen",
            "has space",
            String(repeating: "a", count: 65),
        ] {
            XCTAssertThrowsError(try OfflineOperationErrorEnvelope(
                code: invalidCode,
                message: "rejected"
            )) { error in
                XCTAssertEqual(error as? OfflineOperationError, .invalidField("error.code"))
            }
        }

        for invalidMessage in ["", " rejected", "rejected ", "bad\nmessage", "bad\0message"] {
            XCTAssertThrowsError(try OfflineOperationErrorEnvelope(
                code: "offline_operation_rejected",
                message: invalidMessage
            )) { error in
                XCTAssertEqual(error as? OfflineOperationError, .invalidField("error.message"))
            }
        }

        XCTAssertNoThrow(try OfflineOperationErrorEnvelope(
            code: "1_valid_code_",
            message: "Human-readable detail."
        ))
        XCTAssertNoThrow(try OfflineOperationErrorDetails(
            rejectCode: "TX_QUEUE_FULL",
            transactionHash: Self.transactionHash
        ))
        XCTAssertThrowsError(try OfflineOperationErrorDetails(
            rejectCode: " TX_QUEUE_FULL",
            transactionHash: Self.transactionHash
        ))
        XCTAssertThrowsError(try OfflineOperationErrorDetails(
            rejectCode: "valid_code",
            transactionHash: String(repeating: "A", count: 64)
        ))
        XCTAssertThrowsError(try OfflineQueueErrorSnapshot(
            state: "queue full",
            queued: 1,
            capacity: 1,
            saturated: true
        ))
    }

    func testOperationReferenceRejectsInvalidUtf8AndNonCanonicalFraming() throws {
        var invalidString = OfflineCompactNoritoWriter()
        invalidString.writeLength(1)
        invalidString.writeBytes(Data([0xff]))

        var payload = OfflineCompactNoritoWriter()
        payload.writeField(OfflineCompactNorito.encodeString(Self.operationId))
        payload.writeField(OfflineCompactNorito.encodeUInt32(0))
        payload.writeField(OfflineCompactNorito.encodeUInt32(0))
        payload.writeField(invalidString.data)
        payload.writeField(
            OfflineCompactNorito.encodeString("/v1/offline/operations/\(Self.operationId)")
        )
        payload.writeField(OfflineCompactNorito.encodeUInt64(1))

        XCTAssertThrowsError(try OfflineOperationCodec.decodeReference(noritoEncode(
            typeName: "iroha_torii_shared::offline_api::OfflineOperationReference",
            payload: payload.data,
            flags: NoritoHeader.compactLen
        ))) { error in
            XCTAssertEqual(error as? OfflineOperationError, .invalidField("string"))
        }

        let valid = try OfflineOperationReference(
            operationId: Self.operationId,
            kind: .topUp,
            state: .pending,
            transactionHash: Self.transactionHash,
            statusUri: "/v1/offline/operations/\(Self.operationId)",
            submittedAtMs: 1
        )
        let compactPayload = try XCTUnwrap(
            noritoDecodeFrame(OfflineOperationCodec.encodeReference(valid))
        ).payload
        XCTAssertThrowsError(try OfflineOperationCodec.decodeReference(noritoEncode(
            typeName: "iroha_torii_shared::offline_api::OfflineOperationReference",
            payload: compactPayload,
            flags: 0
        )))

        var padded = OfflineOperationCodec.encodeReference(valid)
        padded.insert(0, at: NoritoHeader.encodedLength)
        XCTAssertThrowsError(try OfflineOperationCodec.decodeReference(padded))
    }

    func testRequestsDeriveLowercaseOperationIdsFromCanonicalArchives() throws {
        let operationId = Data(repeating: 0xab, count: 32)
        let expectedOperationId = String(repeating: "ab", count: 32)
        let topUpArchive = requestArchive(
            schema: KagemushaRecursiveSpendV2.topUpRequestWireName,
            fieldCount: 8,
            operationIdFieldIndex: 6,
            operationId: operationId
        )
        let redeemArchive = requestArchive(
            schema: KagemushaRecursiveSpendV2.redeemRequestWireName,
            fieldCount: 11,
            operationIdFieldIndex: 9,
            operationId: operationId
        )

        let topUp = try OfflineTopUpRequest(noritoArchive: topUpArchive)
        let redeem = try OfflineRedeemRequest(noritoArchive: redeemArchive)

        XCTAssertEqual(topUp.operationId, expectedOperationId)
        XCTAssertEqual(topUp.noritoArchive(), topUpArchive)
        XCTAssertEqual(redeem.operationId, expectedOperationId)
        XCTAssertEqual(redeem.noritoArchive(), redeemArchive)
    }

    func testRequestsRequireTheirExactSchemaAndOperationIdField() throws {
        let operationId = Data(repeating: 0x11, count: 32)
        let topUpArchive = requestArchive(
            schema: KagemushaRecursiveSpendV2.topUpRequestWireName,
            fieldCount: 8,
            operationIdFieldIndex: 6,
            operationId: operationId
        )
        let redeemArchive = requestArchive(
            schema: KagemushaRecursiveSpendV2.redeemRequestWireName,
            fieldCount: 11,
            operationIdFieldIndex: 9,
            operationId: operationId
        )

        XCTAssertThrowsError(try OfflineTopUpRequest(noritoArchive: redeemArchive))
        XCTAssertThrowsError(try OfflineRedeemRequest(noritoArchive: topUpArchive))
        XCTAssertThrowsError(
            try OfflineTopUpRequest(noritoArchive: requestArchive(
                schema: KagemushaRecursiveSpendV2.topUpRequestWireName,
                fieldCount: 8,
                operationIdFieldIndex: 5,
                operationId: operationId
            ))
        )
        XCTAssertThrowsError(
            try OfflineRedeemRequest(noritoArchive: requestArchive(
                schema: KagemushaRecursiveSpendV2.redeemRequestWireName,
                fieldCount: 11,
                operationIdFieldIndex: 8,
                operationId: operationId
            ))
        )
    }

    func testRequestsRejectZeroOrWrongLengthOperationIds() throws {
        for operationId in [
            Data(repeating: 0, count: 32),
            Data(repeating: 0x11, count: 31),
            Data(repeating: 0x11, count: 33),
        ] {
            XCTAssertThrowsError(try OfflineTopUpRequest(noritoArchive: requestArchive(
                schema: KagemushaRecursiveSpendV2.topUpRequestWireName,
                fieldCount: 8,
                operationIdFieldIndex: 6,
                operationId: operationId
            ))) { error in
                XCTAssertEqual(error as? OfflineOperationError, .invalidField("operation_id"))
            }
            XCTAssertThrowsError(try OfflineRedeemRequest(noritoArchive: requestArchive(
                schema: KagemushaRecursiveSpendV2.redeemRequestWireName,
                fieldCount: 11,
                operationIdFieldIndex: 9,
                operationId: operationId
            ))) { error in
                XCTAssertEqual(error as? OfflineOperationError, .invalidField("operation_id"))
            }
        }
    }

    func testRequestsRejectNonCanonicalFramingAndTrailingPayload() throws {
        let operationId = Data(repeating: 0x11, count: 32)
        let canonicalPayload = requestPayload(
            fieldCount: 8,
            operationIdFieldIndex: 6,
            operationId: operationId
        )
        let schema = KagemushaRecursiveSpendV2.topUpRequestWireName

        var trailingBytePayload = canonicalPayload
        trailingBytePayload.append(0xff)
        XCTAssertThrowsError(try OfflineTopUpRequest(noritoArchive: noritoEncode(
            typeName: schema,
            payload: trailingBytePayload,
            flags: NoritoHeader.compactLen
        )))

        XCTAssertThrowsError(try OfflineTopUpRequest(noritoArchive: requestArchive(
            schema: schema,
            fieldCount: 9,
            operationIdFieldIndex: 6,
            operationId: operationId
        )))

        var nonCanonicalPayload = Data([0x81, 0x00, 0x01])
        nonCanonicalPayload.append(canonicalPayload.dropFirst(2))
        XCTAssertThrowsError(try OfflineTopUpRequest(noritoArchive: noritoEncode(
            typeName: schema,
            payload: nonCanonicalPayload,
            flags: NoritoHeader.compactLen
        )))

        XCTAssertThrowsError(try OfflineTopUpRequest(noritoArchive: noritoEncode(
            typeName: schema,
            payload: canonicalPayload,
            flags: 0
        )))

        var paddedArchive = noritoEncode(
            typeName: schema,
            payload: canonicalPayload,
            flags: NoritoHeader.compactLen
        )
        paddedArchive.insert(0, at: NoritoHeader.encodedLength)
        XCTAssertThrowsError(
            try OfflineTopUpRequest(noritoArchive: paddedArchive)
        )
        XCTAssertThrowsError(try OfflineTopUpRequest(noritoArchive: Data()))
    }

    private func requestArchive(
        schema: String,
        fieldCount: Int,
        operationIdFieldIndex: Int,
        operationId: Data
    ) -> Data {
        noritoEncode(
            typeName: schema,
            payload: requestPayload(
                fieldCount: fieldCount,
                operationIdFieldIndex: operationIdFieldIndex,
                operationId: operationId
            ),
            flags: NoritoHeader.compactLen
        )
    }

    private func requestPayload(
        fieldCount: Int,
        operationIdFieldIndex: Int,
        operationId: Data
    ) -> Data {
        var payload = OfflineCompactNoritoWriter()
        for index in 0..<fieldCount {
            payload.writeField(
                index == operationIdFieldIndex ? operationId : Data([UInt8(index + 1)])
            )
        }
        return payload.data
    }

    private func canonicalTopUpAnchorArchive() throws -> Data {
        var assetBytes = Data((0..<16).map { UInt8($0 + 1) })
        assetBytes[6] = (assetBytes[6] & 0x0f) | 0x40
        assetBytes[8] = (assetBytes[8] & 0x3f) | 0x80
        let assetDefinitionId = try XCTUnwrap(
            AssetDefinitionAddress.encode(uuidBytes: assetBytes)
        )
        let fixed32: (UInt8) -> Data = { Data(repeating: $0, count: 32) }
        let payer = try AccountAddress
            .fromAccount(publicKey: fixed32(0xc0))
            .toI105(networkPrefix: 0x02f1)
        let amount = try KagemushaScaledAmount(atomicUnits: "1", scale: 2)
        let note = try KagemushaSpendableNoteDescriptorV2(
            chainID: "swift-offline-api",
            assetDefinitionID: assetDefinitionId,
            noteCommitment: fixed32(0xd0),
            spendNullifier: fixed32(0xd1),
            amount: amount
        )
        let draft = try KagemushaRecursiveSpendTopUpAnchorV2(
            version: 2,
            chainID: note.chainID,
            payer: payer,
            assetID: "\(assetDefinitionId)#\(payer)",
            assetScale: 2,
            amount: amount,
            initialRoot: fixed32(0xd2),
            finalizedRoot: fixed32(0xd3),
            topUpAnchorNullifiers: [fixed32(0xd4)],
            currentNote: note,
            topUpOperationID: fixed32(0xd5),
            transferVerifierID: "halo2:fixture-transfer",
            transferVerifierCommitment: fixed32(0xd6),
            artifactGeneration: "generation-v2-test",
            finalizedHeight: 1,
            finalizedTransactionHash: fixed32(0xd7),
            anchorDigest: fixed32(0xd8),
            archive: Data([1])
        )
        return try KagemushaRecursiveSpendV2Codecs.encodeTopUpAnchor(draft)
    }

    private static let operationId = String(repeating: "11", count: 32)
    private static let transactionHash = String(repeating: "22", count: 32)
    private static let rustOperationReferenceArchiveHex =
        "4e5254300000e8e2244e45e4be2a975e34957141128b00f0000000000000001f5b5402d6dc2092024140313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131310400000000040000000041403232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323258572f76312f6f66666c696e652f6f7065726174696f6e732f3131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313108ffffffffffffffff"
    private static let rustPendingStatusArchiveHex =
        "4e5254300000fb04214104df1bdcd39249bddd4db23a009600000000000000bdfee2508f80055702000000000000000000000000414031313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131040000000041403232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323208ffffffffffffffff"
    private static let rustRejectedStatusArchiveHex =
        "4e5254300000fb04214104df1bdcd39249bddd4db23a00b6000000000000009322104cda8e602a020000000000000000020000004140313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131310401000000414032323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232281b1a6f66666c696e655f6f7065726174696f6e5f72656a6563746564090872656a65637465640100"
    private static let rustAppliedRedeemStatusArchiveHex =
        "4e5254300000fb04214104df1bdcd39249bddd4db23a00a00000000000000092cd6b32b062b3d30200000000000000000100000041403131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313159010000005441403232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323208ffffffffffffffff082a00000000000000"
}
