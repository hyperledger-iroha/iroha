import XCTest
@testable import IrohaSwift

final class ToriiOfflineCashAPIModelsTests: XCTestCase {
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
                transactionHash: "transaction-hash",
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
            transactionHash: "transaction-hash",
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
            .pending(
                operationId: Self.operationId,
                kind: .topUp,
                transactionHash: "transaction-hash",
                submittedAtMs: UInt64.max
            )
        )
    }

    func testRejectedOperationStatusMatchesRustNoritoGoldenVector() throws {
        let archive = try XCTUnwrap(Data(hexString: Self.rustRejectedStatusArchiveHex))

        XCTAssertEqual(
            try OfflineOperationCodec.decodeStatus(archive),
            .rejected(
                operationId: Self.operationId,
                kind: .redeem,
                transactionHash: "transaction-hash",
                error: OfflineOperationErrorEnvelope(
                    code: "offline_operation_rejected",
                    message: "rejected"
                )
            )
        )
    }

    func testAppliedRedeemStatusMatchesRustNoritoGoldenVector() throws {
        let archive = try XCTUnwrap(Data(hexString: Self.rustAppliedRedeemStatusArchiveHex))

        XCTAssertEqual(
            try OfflineOperationCodec.decodeStatus(archive),
            .applied(
                operationId: Self.operationId,
                result: .redeem(OfflineRedeemResult(
                    transactionHash: "transaction-hash",
                    finalizedBlockHeight: UInt64.max,
                    serverTimeMs: 42
                ))
            )
        )
    }

    func testOperationStatusRequiresExactSharedSchema() throws {
        let referenceArchive = try XCTUnwrap(
            Data(hexString: Self.rustOperationReferenceArchiveHex)
        )
        XCTAssertThrowsError(try OfflineOperationCodec.decodeStatus(referenceArchive))
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

    private static let operationId = String(repeating: "11", count: 32)
    private static let rustOperationReferenceArchiveHex =
        "4e5254300000e8e2244e45e4be2a975e34957141128b00c000000000000000fe8a8b6e958d2447024140313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131310400000000040000000011107472616e73616374696f6e2d6861736858572f76312f6f66666c696e652f6f7065726174696f6e732f3131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313108ffffffffffffffff"
    private static let rustPendingStatusArchiveHex =
        "4e5254300000fb04214104df1bdcd39249bddd4db23a006600000000000000b3fae818809b7b8e02000000000000000000000000414031313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131040000000011107472616e73616374696f6e2d6861736808ffffffffffffffff"
    private static let rustRejectedStatusArchiveHex =
        "4e5254300000fb04214104df1bdcd39249bddd4db23a0086000000000000008878a32fe86d887302000000000000000002000000414031313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131040100000011107472616e73616374696f6e2d68617368281b1a6f66666c696e655f6f7065726174696f6e5f72656a6563746564090872656a65637465640100"
    private static let rustAppliedRedeemStatusArchiveHex =
        "4e5254300000fb04214104df1bdcd39249bddd4db23a007000000000000000451e52608aefd9710200000000000000000100000041403131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313129010000002411107472616e73616374696f6e2d6861736808ffffffffffffffff082a00000000000000"
}
