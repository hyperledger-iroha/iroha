import XCTest
@testable import IrohaSwift

final class ToriiKagemushaAPIModelsTests: XCTestCase {
    func testPublicRequestSchemaNamesMatchRust() {
        XCTAssertEqual(
            KagemushaRecursiveSpend.topUpRequestWireName,
            "iroha.torii.v1.offline.top_up.request"
        )
        XCTAssertEqual(
            KagemushaRecursiveSpend.redeemRequestWireName,
            "iroha.torii.v1.offline.redeem.request"
        )
    }

    func testEndpointConstantsUseSharpFirstReleaseRoutes() throws {
        XCTAssertEqual(KagemushaToriiAPI.Endpoint.readiness.path, "/v1/offline/readiness")
        XCTAssertEqual(KagemushaToriiAPI.Endpoint.topUp.path, "/v1/offline/top-up")
        XCTAssertEqual(KagemushaToriiAPI.Endpoint.redeem.path, "/v1/offline/redeem")
        XCTAssertEqual(KagemushaToriiAPI.Endpoint.operations.path, "/v1/offline/operations")
        XCTAssertEqual(
            try KagemushaToriiAPI.operationPath(Self.operationId),
            "/v1/offline/operations/\(Self.operationId)"
        )
    }

    func testOperationReferenceNoritoRoundTrips() throws {
        for kind in [KagemushaOperationKind.topUp, .redeem] {
            let expected = try KagemushaOperationReference(
                operationId: Self.operationId,
                kind: kind,
                state: .pending,
                transactionHash: Self.transactionHash,
                statusUri: "/v1/offline/operations/\(Self.operationId)",
                submittedAtMs: UInt64.max
            )

            XCTAssertEqual(
                try KagemushaOperationCodec.decodeReference(
                    KagemushaOperationCodec.encodeReference(expected)
                ),
                expected
            )
        }
    }

    func testOperationCodecsRejectOversizedArchivesBeforeParsing() {
        XCTAssertThrowsError(
            try KagemushaOperationCodec.decodeReference(
                Data(
                    repeating: 0xa5,
                    count: KagemushaOperationCodec.referenceMaximumArchiveBytes + 1
                )
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaOperationError,
                .invalidNoritoArchive
            )
        }
        XCTAssertThrowsError(
            try KagemushaOperationCodec.decodeStatus(
                Data(
                    repeating: 0xa5,
                    count: KagemushaOperationCodec.statusMaximumArchiveBytes + 1
                ),
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaOperationError,
                .invalidNoritoArchive
            )
        }
    }

    func testOperationReferenceMatchesRustNoritoGoldenVector() throws {
        let archive = try XCTUnwrap(Data(hexString: Self.rustOperationReferenceArchiveHex))
        let expected = try KagemushaOperationReference(
            operationId: Self.operationId,
            kind: .topUp,
            state: .pending,
            transactionHash: Self.transactionHash,
            statusUri: "/v1/offline/operations/\(Self.operationId)",
            submittedAtMs: UInt64.max
        )

        XCTAssertEqual(try KagemushaOperationCodec.decodeReference(archive), expected)
        XCTAssertEqual(KagemushaOperationCodec.encodeReference(expected), archive)
    }

    func testPendingOperationStatusMatchesRustNoritoGoldenVector() throws {
        let archive = try XCTUnwrap(Data(hexString: Self.rustPendingStatusArchiveHex))

        XCTAssertEqual(
            try KagemushaOperationCodec.decodeStatus(
                archive,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            ),
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
            try KagemushaOperationCodec.decodeStatus(
                archive,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            ),
            .rejected(try .init(
                operationId: Self.operationId,
                kind: .redeem,
                transactionHash: Self.transactionHash,
                error: try KagemushaOperationErrorEnvelope(
                    code: "offline_operation_rejected",
                    message: "rejected"
                )
            ))
        )
    }

    func testAppliedRedeemStatusMatchesRustNoritoGoldenVector() throws {
        let archive = try XCTUnwrap(Data(hexString: Self.rustAppliedRedeemStatusArchiveHex))

        XCTAssertEqual(
            try KagemushaOperationCodec.decodeStatus(
                archive,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            ),
            .applied(try .init(
                operationId: Self.operationId,
                result: .redeem(try KagemushaRedeemResult(
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
        XCTAssertThrowsError(try KagemushaOperationCodec.decodeStatus(
            referenceArchive,
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ))
    }

    func testReceiveOfferBindsProjectionToItsExplicitTairaContext() throws {
        let offer = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let projected = try offer.project(
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        )

        XCTAssertEqual(
            try AccountAddress.inspectI105NetworkPrefix(
                projected.request.payload.recipient
            ).chainDiscriminant,
            SccpV1.tairaI105DiscriminantV1
        )
        XCTAssertThrowsError(try offer.project(
            chainDiscriminant: AccountId.defaultNetworkPrefix
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("recipientReceiveOffer.chainDiscriminant")
            )
        }
    }

    func testRecipientLineageQueryRejectsWrongNetworkBeforeNativeProjection() throws {
        let request = try KagemushaPeerTransportTestFixtures.paymentRequest()

        XCTAssertThrowsError(try KagemushaRecipientLineageQueryV2(
            chainID: request.payload.chainID,
            recipient: request.payload.recipient,
            chainDiscriminant: AccountId.defaultNetworkPrefix,
            receiverDeviceID: request.payload.receiverDeviceID,
            assetDefinitionID: request.payload.assetDefinitionID,
            trustedCheckpointHeight: 1
        )) { error in
            XCTAssertEqual(
                error as? KagemushaRecursiveSpendError,
                .invalidField("lineageQuery.recipient")
            )
        }
    }

    func testRecipientLineageQueryOverridesAmbient753WithExplicitTairaContext() throws {
        #if canImport(Darwin)
        guard KagemushaRecursiveSpend.hasRequiredNativeSymbols else {
            throw XCTSkip("ABI-21 bridge is not linked in this source-only test host")
        }
        let request = try KagemushaPeerTransportTestFixtures.paymentRequest()

        let query = try NoritoNativeBridge.shared.withChainDiscriminant(
            AccountId.defaultNetworkPrefix
        ) {
            try KagemushaRecipientLineageQueryV2(
                chainID: request.payload.chainID,
                recipient: request.payload.recipient,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1,
                receiverDeviceID: request.payload.receiverDeviceID,
                assetDefinitionID: request.payload.assetDefinitionID,
                trustedCheckpointHeight: 1
            )
        }

        XCTAssertFalse(query.noritoArchive.isEmpty)
        XCTAssertEqual(query.trustedCheckpointHeight, 1)
        #endif
    }

    func testRecipientLineageQueriesIsolateConcurrentTairaAndSoraContexts() throws {
        #if canImport(Darwin)
        guard KagemushaRecursiveSpend.hasRequiredNativeSymbols else {
            throw XCTSkip("ABI-21 bridge is not linked in this source-only test host")
        }
        let request = try KagemushaPeerTransportTestFixtures.paymentRequest()
        let address = try AccountAddress.parseEncodedSwiftOnly(
            request.payload.recipient,
            expectedPrefix: SccpV1.tairaI105DiscriminantV1
        )
        let chainID = request.payload.chainID
        let receiverDeviceID = request.payload.receiverDeviceID
        let assetDefinitionID = request.payload.assetDefinitionID
        let contexts: [(discriminant: UInt16, recipient: String)] = [
            (
                SccpV1.tairaI105DiscriminantV1,
                try address.toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
            ),
            (
                AccountId.defaultNetworkPrefix,
                try address.toI105(networkPrefix: AccountId.defaultNetworkPrefix)
            ),
        ]
        let expected = try contexts.map { context in
            try KagemushaRecipientLineageQueryV2(
                chainID: chainID,
                recipient: context.recipient,
                chainDiscriminant: context.discriminant,
                receiverDeviceID: receiverDeviceID,
                assetDefinitionID: assetDefinitionID,
                trustedCheckpointHeight: 1
            ).noritoArchive
        }

        DispatchQueue.concurrentPerform(iterations: 128) { index in
            let contextIndex = index % contexts.count
            let context = contexts[contextIndex]
            do {
                let query = try KagemushaRecipientLineageQueryV2(
                    chainID: chainID,
                    recipient: context.recipient,
                    chainDiscriminant: context.discriminant,
                    receiverDeviceID: receiverDeviceID,
                    assetDefinitionID: assetDefinitionID,
                    trustedCheckpointHeight: 1
                )
                XCTAssertEqual(query.noritoArchive, expected[contextIndex])
            } catch {
                XCTFail("concurrent lineage query failed: \(error)")
            }
        }
        #endif
    }

    func testOfflineTopUpAnchorUsesCurrentPublicNameAndRetainsCanonicalWire() throws {
        let archive = try canonicalTopUpAnchorArchive()
        let anchor = try KagemushaTopUpAnchor(
            noritoArchive: archive,
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        )
        let finalityProof = try KagemushaTopUpFinalityProof(
            noritoArchive: canonicalTopUpFinalityProofArchive()
        )
        XCTAssertEqual(anchor.noritoArchive(), archive)
        XCTAssertEqual(anchor.digest, Data(repeating: 0xd8, count: 32))
        XCTAssertEqual(anchor.operationId, String(repeating: "d5", count: 32))
        XCTAssertEqual(
            anchor.finalizedTransactionHash,
            String(repeating: "d7", count: 32)
        )
        XCTAssertEqual(anchor.finalizedBlockHeight, 1)
        XCTAssertEqual(
            anchor.digest,
            try KagemushaRecursiveSpendCodecs.decodeTopUpAnchorV4(
                archive,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            ).anchorDigest
        )
        XCTAssertEqual(anchor, try KagemushaTopUpAnchor(
            noritoArchive: archive,
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ))

        let result = try KagemushaTopUpResult(
            transactionHash: String(repeating: "d7", count: 32),
            finalizedBlockHeight: 1,
            serverTimeMs: 8,
            anchor: anchor,
            finalityProof: finalityProof
        )
        XCTAssertEqual(result.anchor.noritoArchive(), archive)
        XCTAssertEqual(
            result.finalityProof.noritoArchive,
            canonicalTopUpFinalityProofArchive()
        )
        XCTAssertNoThrow(try KagemushaOperationStatus.Applied(
            operationId: String(repeating: "d5", count: 32),
            result: .topUp(result)
        ))
        XCTAssertThrowsError(try KagemushaOperationStatus.Applied(
            operationId: Self.operationId,
            result: .topUp(result)
        ))
        XCTAssertThrowsError(try KagemushaTopUpResult(
            transactionHash: Self.transactionHash,
            finalizedBlockHeight: 1,
            serverTimeMs: 8,
            anchor: anchor,
            finalityProof: finalityProof
        ))
        XCTAssertThrowsError(try KagemushaTopUpResult(
            transactionHash: String(repeating: "d7", count: 32),
            finalizedBlockHeight: 7,
            serverTimeMs: 8,
            anchor: anchor,
            finalityProof: finalityProof
        ))

        XCTAssertThrowsError(try KagemushaTopUpAnchor(
            noritoArchive: Data(),
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ))
        XCTAssertThrowsError(try KagemushaTopUpAnchor(noritoArchive: noritoEncode(
            typeName: KagemushaRecursiveSpend.topUpAnchorWireNameV4,
            payload: Data(
                repeating: 0xa4,
                count: KagemushaRecursiveSpend.topUpFinalityAnchorMaximumArchiveBytes
            ),
            flags: NoritoHeader.compactLen
        ), chainDiscriminant: SccpV1.tairaI105DiscriminantV1))
        XCTAssertThrowsError(try KagemushaTopUpAnchor(noritoArchive: noritoEncode(
            typeName: "wrong.anchor.schema",
            payload: try XCTUnwrap(noritoDecodeFrame(archive)).payload,
            flags: NoritoHeader.compactLen
        ), chainDiscriminant: SccpV1.tairaI105DiscriminantV1))

        var corrupted = archive
        corrupted[corrupted.index(before: corrupted.endIndex)] ^= 0xff
        XCTAssertThrowsError(try KagemushaTopUpAnchor(
            noritoArchive: corrupted,
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ))
    }

    func testOperationReferencesRequireCanonicalHashAndBoundStatusUri() throws {
        for invalidHash in [
            "",
            String(repeating: "2", count: 63),
            String(repeating: "2", count: 65),
            String(repeating: "A", count: 64),
            String(repeating: "g", count: 64),
            String(repeating: "0", count: 64),
            " \(Self.transactionHash)",
        ] {
            XCTAssertThrowsError(try KagemushaOperationReference(
                operationId: Self.operationId,
                kind: .topUp,
                state: .pending,
                transactionHash: invalidHash,
                statusUri: "/v1/offline/operations/\(Self.operationId)",
                submittedAtMs: 1
            )) { error in
                XCTAssertEqual(error as? KagemushaOperationError, .invalidField("transaction_hash"))
            }
        }

        for invalidUri in [
            "/v1/offline/operations/\(String(repeating: "33", count: 32))",
            "https://example.test/v1/offline/operations/\(Self.operationId)",
            "/v1/offline/operations/\(Self.operationId)/",
            " /v1/offline/operations/\(Self.operationId)",
        ] {
            XCTAssertThrowsError(try KagemushaOperationReference(
                operationId: Self.operationId,
                kind: .topUp,
                state: .pending,
                transactionHash: Self.transactionHash,
                statusUri: invalidUri,
                submittedAtMs: 1
            )) { error in
                XCTAssertEqual(error as? KagemushaOperationError, .invalidField("status_uri"))
            }
        }

        XCTAssertThrowsError(try KagemushaOperationReference(
            operationId: Self.operationId,
            kind: .topUp,
            state: .pending,
            transactionHash: Self.transactionHash,
            statusUri: "/v1/offline/operations/\(Self.operationId)",
            submittedAtMs: 0
        )) { error in
            XCTAssertEqual(
                error as? KagemushaOperationError,
                .invalidField("submitted_at_ms")
            )
        }
    }

    func testTaggedOperationStatePayloadsCannotBypassValidation() throws {
        XCTAssertThrowsError(try KagemushaOperationStatus.Pending(
            operationId: String(repeating: "0", count: 64),
            kind: .topUp,
            transactionHash: Self.transactionHash,
            submittedAtMs: 1
        )) { error in
            XCTAssertEqual(error as? KagemushaOperationError, .invalidField("operation_id"))
        }

        XCTAssertThrowsError(try KagemushaOperationStatus.Pending(
            operationId: Self.operationId,
            kind: .topUp,
            transactionHash: String(repeating: "F", count: 64),
            submittedAtMs: 1
        )) { error in
            XCTAssertEqual(error as? KagemushaOperationError, .invalidField("transaction_hash"))
        }

        XCTAssertThrowsError(try KagemushaOperationStatus.Pending(
            operationId: Self.operationId,
            kind: .topUp,
            transactionHash: Self.transactionHash,
            submittedAtMs: 0
        )) { error in
            XCTAssertEqual(
                error as? KagemushaOperationError,
                .invalidField("submitted_at_ms")
            )
        }

        XCTAssertThrowsError(try KagemushaRedeemResult(
            transactionHash: "not-a-hash",
            finalizedBlockHeight: 1,
            serverTimeMs: 2
        )) { error in
            XCTAssertEqual(error as? KagemushaOperationError, .invalidField("transaction_hash"))
        }

        let anchor = try KagemushaTopUpAnchor(
            noritoArchive: canonicalTopUpAnchorArchive(),
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        )
        let finalityProof = try KagemushaTopUpFinalityProof(
            noritoArchive: canonicalTopUpFinalityProofArchive()
        )
        for (finalizedBlockHeight, serverTimeMs, field) in [
            (UInt64(0), UInt64(1), "finalized_block_height"),
            (UInt64(1), UInt64(0), "server_time_ms"),
        ] {
            XCTAssertThrowsError(try KagemushaTopUpResult(
                transactionHash: Self.transactionHash,
                finalizedBlockHeight: finalizedBlockHeight,
                serverTimeMs: serverTimeMs,
                anchor: anchor,
                finalityProof: finalityProof
            )) { error in
                XCTAssertEqual(error as? KagemushaOperationError, .invalidField(field))
            }
            XCTAssertThrowsError(try KagemushaRedeemResult(
                transactionHash: Self.transactionHash,
                finalizedBlockHeight: finalizedBlockHeight,
                serverTimeMs: serverTimeMs
            )) { error in
                XCTAssertEqual(error as? KagemushaOperationError, .invalidField(field))
            }
        }

        let valid = try KagemushaOperationStatus.Pending(
            operationId: Self.operationId,
            kind: .redeem,
            transactionHash: Self.transactionHash,
            submittedAtMs: UInt64.max
        )
        XCTAssertEqual(KagemushaOperationStatus.pending(valid).operationId, Self.operationId)
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
            XCTAssertThrowsError(try KagemushaOperationErrorEnvelope(
                code: invalidCode,
                message: "rejected"
            )) { error in
                XCTAssertEqual(error as? KagemushaOperationError, .invalidField("error.code"))
            }
        }

        for invalidMessage in ["", " rejected", "rejected ", "bad\nmessage", "bad\0message"] {
            XCTAssertThrowsError(try KagemushaOperationErrorEnvelope(
                code: "offline_operation_rejected",
                message: invalidMessage
            )) { error in
                XCTAssertEqual(error as? KagemushaOperationError, .invalidField("error.message"))
            }
        }

        XCTAssertNoThrow(try KagemushaOperationErrorEnvelope(
            code: "1_valid_code_",
            message: "Human-readable detail."
        ))
        XCTAssertThrowsError(try KagemushaOperationErrorEnvelope(
            code: "offline_operation_rejected",
            message: String(
                repeating: "a",
                count: KagemushaOperationCodec.maximumTextFieldUTF8Bytes + 1
            )
        )) { error in
            XCTAssertEqual(
                error as? KagemushaOperationError,
                .invalidField("error.message")
            )
        }
        XCTAssertNoThrow(try KagemushaOperationErrorDetails(
            rejectCode: "TX_QUEUE_FULL",
            transactionHash: Self.transactionHash
        ))
        XCTAssertThrowsError(try KagemushaOperationErrorDetails(
            rejectCode: " TX_QUEUE_FULL",
            transactionHash: Self.transactionHash
        ))
        XCTAssertThrowsError(try KagemushaOperationErrorDetails(
            rejectCode: "valid_code",
            transactionHash: String(repeating: "A", count: 64)
        ))
        XCTAssertThrowsError(try KagemushaQueueErrorSnapshot(
            state: "queue full",
            queued: 1,
            capacity: 1,
            saturated: true
        ))
    }

    func testOperationReferenceRejectsInvalidUtf8AndNonCanonicalFraming() throws {
        var invalidString = CompactNoritoWriter()
        invalidString.writeLength(1)
        invalidString.writeBytes(Data([0xff]))

        var payload = CompactNoritoWriter()
        payload.writeField(CompactNorito.encodeString(Self.operationId))
        payload.writeField(CompactNorito.encodeUInt32(0))
        payload.writeField(CompactNorito.encodeUInt32(0))
        payload.writeField(invalidString.data)
        payload.writeField(
            CompactNorito.encodeString("/v1/offline/operations/\(Self.operationId)")
        )
        payload.writeField(CompactNorito.encodeUInt64(1))

        XCTAssertThrowsError(try KagemushaOperationCodec.decodeReference(noritoEncode(
            typeName: "iroha_torii_shared::offline_api::OfflineOperationReference",
            payload: payload.data,
            flags: NoritoHeader.compactLen
        ))) { error in
            XCTAssertEqual(error as? KagemushaOperationError, .invalidField("string"))
        }

        let valid = try KagemushaOperationReference(
            operationId: Self.operationId,
            kind: .topUp,
            state: .pending,
            transactionHash: Self.transactionHash,
            statusUri: "/v1/offline/operations/\(Self.operationId)",
            submittedAtMs: 1
        )
        let compactPayload = try XCTUnwrap(
            noritoDecodeFrame(KagemushaOperationCodec.encodeReference(valid))
        ).payload
        XCTAssertThrowsError(try KagemushaOperationCodec.decodeReference(noritoEncode(
            typeName: "iroha_torii_shared::offline_api::OfflineOperationReference",
            payload: compactPayload,
            flags: 0
        )))

        var padded = KagemushaOperationCodec.encodeReference(valid)
        padded.insert(0, at: NoritoHeader.encodedLength)
        XCTAssertThrowsError(try KagemushaOperationCodec.decodeReference(padded))
    }

    func testRequestsDeriveLowercaseOperationIdsFromCanonicalArchives() throws {
        let operationId = Data(repeating: 0xab, count: 32)
        let expectedOperationId = String(repeating: "ab", count: 32)
        let topUpArchive = requestArchive(
            schema: KagemushaRecursiveSpend.topUpRequestWireName,
            fieldCount: 8,
            operationIdFieldIndex: 6,
            operationId: operationId
        )
        let redeemArchive = requestArchive(
            schema: KagemushaRecursiveSpend.redeemRequestWireName,
            fieldCount: 10,
            operationIdFieldIndex: 8,
            operationId: operationId
        )

        let topUp = try KagemushaTopUpRequest(noritoArchive: topUpArchive)
        let redeem = try KagemushaRedeemRequest(noritoArchive: redeemArchive)

        XCTAssertEqual(topUp.operationId, expectedOperationId)
        XCTAssertEqual(topUp.noritoArchive(), topUpArchive)
        XCTAssertEqual(redeem.operationId, expectedOperationId)
        XCTAssertEqual(redeem.noritoArchive(), redeemArchive)
    }

    func testRequestsEnforceExactToriiRequestBodyCeiling() throws {
        XCTAssertEqual(
            KagemushaTopUpRequest.maximumArchiveBytes,
            512 * 1_024
        )
        XCTAssertEqual(
            KagemushaRedeemRequest.maximumArchiveBytes,
            48 * 1_024 * 1_024
        )
        try assertExactRequestBodyCeiling(
            schema: KagemushaRecursiveSpend.topUpRequestWireName,
            fieldCount: 8,
            operationIdFieldIndex: 6,
            maximumBytes: KagemushaTopUpRequest.maximumArchiveBytes,
            construct: { archive in
                _ = try KagemushaTopUpRequest(noritoArchive: archive)
            }
        )
        try assertExactRequestBodyCeiling(
            schema: KagemushaRecursiveSpend.redeemRequestWireName,
            fieldCount: 10,
            operationIdFieldIndex: 8,
            maximumBytes: KagemushaRedeemRequest.maximumArchiveBytes,
            construct: { archive in
                _ = try KagemushaRedeemRequest(noritoArchive: archive)
            }
        )
    }

    func testRequestsRequireTheirExactSchemaAndOperationIdField() throws {
        let operationId = Data(repeating: 0x11, count: 32)
        let topUpArchive = requestArchive(
            schema: KagemushaRecursiveSpend.topUpRequestWireName,
            fieldCount: 8,
            operationIdFieldIndex: 6,
            operationId: operationId
        )
        let redeemArchive = requestArchive(
            schema: KagemushaRecursiveSpend.redeemRequestWireName,
            fieldCount: 10,
            operationIdFieldIndex: 8,
            operationId: operationId
        )

        XCTAssertThrowsError(try KagemushaTopUpRequest(noritoArchive: redeemArchive))
        XCTAssertThrowsError(try KagemushaRedeemRequest(noritoArchive: topUpArchive))
        XCTAssertThrowsError(
            try KagemushaTopUpRequest(noritoArchive: requestArchive(
                schema: KagemushaRecursiveSpend.topUpRequestWireName,
                fieldCount: 8,
                operationIdFieldIndex: 5,
                operationId: operationId
            ))
        )
        XCTAssertThrowsError(
            try KagemushaRedeemRequest(noritoArchive: requestArchive(
                schema: KagemushaRecursiveSpend.redeemRequestWireName,
                fieldCount: 10,
                operationIdFieldIndex: 7,
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
            XCTAssertThrowsError(try KagemushaTopUpRequest(noritoArchive: requestArchive(
                schema: KagemushaRecursiveSpend.topUpRequestWireName,
                fieldCount: 8,
                operationIdFieldIndex: 6,
                operationId: operationId
            ))) { error in
                XCTAssertEqual(error as? KagemushaOperationError, .invalidField("operation_id"))
            }
            XCTAssertThrowsError(try KagemushaRedeemRequest(noritoArchive: requestArchive(
                schema: KagemushaRecursiveSpend.redeemRequestWireName,
                fieldCount: 10,
                operationIdFieldIndex: 8,
                operationId: operationId
            ))) { error in
                XCTAssertEqual(error as? KagemushaOperationError, .invalidField("operation_id"))
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
        let schema = KagemushaRecursiveSpend.topUpRequestWireName

        var trailingBytePayload = canonicalPayload
        trailingBytePayload.append(0xff)
        XCTAssertThrowsError(try KagemushaTopUpRequest(noritoArchive: KagemushaRecursiveSpend.frameArchive(
            schema: schema,
            payload: trailingBytePayload
        )))

        XCTAssertThrowsError(try KagemushaTopUpRequest(noritoArchive: requestArchive(
            schema: schema,
            fieldCount: 9,
            operationIdFieldIndex: 6,
            operationId: operationId
        )))

        var nonCanonicalPayload = Data([0x81, 0x00, 0x01])
        nonCanonicalPayload.append(canonicalPayload.dropFirst(2))
        XCTAssertThrowsError(try KagemushaTopUpRequest(noritoArchive: KagemushaRecursiveSpend.frameArchive(
            schema: schema,
            payload: nonCanonicalPayload
        )))

        XCTAssertThrowsError(try KagemushaTopUpRequest(noritoArchive: noritoEncode(
            typeName: schema,
            payload: canonicalPayload,
            flags: 0
        )))

        var paddedArchive = KagemushaRecursiveSpend.frameArchive(
            schema: schema,
            payload: canonicalPayload
        )
        paddedArchive.insert(0, at: NoritoHeader.encodedLength)
        XCTAssertThrowsError(
            try KagemushaTopUpRequest(noritoArchive: paddedArchive)
        )
        XCTAssertThrowsError(try KagemushaTopUpRequest(noritoArchive: Data()))
    }

    private func requestArchive(
        schema: String,
        fieldCount: Int,
        operationIdFieldIndex: Int,
        operationId: Data
    ) -> Data {
        KagemushaRecursiveSpend.frameArchive(
            schema: schema,
            payload: requestPayload(
                fieldCount: fieldCount,
                operationIdFieldIndex: operationIdFieldIndex,
                operationId: operationId
            )
        )
    }

    private func assertExactRequestBodyCeiling(
        schema: String,
        fieldCount: Int,
        operationIdFieldIndex: Int,
        maximumBytes: Int,
        construct: (Data) throws -> Void
    ) throws {
        let operationId = Data(repeating: 0xa7, count: 32)
        var fillerBytes = maximumBytes - 1_024
        var exactArchive: Data?
        for _ in 0..<8 {
            var payload = CompactNoritoWriter()
            for index in 0..<fieldCount {
                let field: Data
                if index == 0 {
                    field = CompactNorito.encodeUInt16(KagemushaRecursiveSpend.wireVersionV4)
                } else if index == operationIdFieldIndex {
                    field = operationId
                } else if index == 1 {
                    field = Data(repeating: 0x5a, count: fillerBytes)
                } else {
                    field = Data([UInt8(index + 1)])
                }
                payload.writeField(field)
            }
            let archive = KagemushaRecursiveSpend.frameArchive(
                schema: schema,
                payload: payload.data
            )
            let delta = maximumBytes - archive.count
            if delta == 0 {
                exactArchive = archive
                break
            }
            fillerBytes += delta
            guard fillerBytes >= 0 else {
                return XCTFail("could not construct exact-limit archive")
            }
        }
        let archive = try XCTUnwrap(exactArchive)
        XCTAssertEqual(archive.count, maximumBytes)
        try construct(archive)

        var oversized = archive
        oversized.append(0)
        XCTAssertEqual(oversized.count, maximumBytes + 1)
        XCTAssertThrowsError(try construct(oversized)) { error in
            XCTAssertEqual(
                error as? KagemushaOperationError,
                .invalidNoritoArchive
            )
        }
    }

    private func requestPayload(
        fieldCount: Int,
        operationIdFieldIndex: Int,
        operationId: Data
    ) -> Data {
        var payload = CompactNoritoWriter()
        for index in 0..<fieldCount {
            let field: Data
            if index == 0 {
                field = CompactNorito.encodeUInt16(KagemushaRecursiveSpend.wireVersionV4)
            } else if index == operationIdFieldIndex {
                field = operationId
            } else {
                field = Data([UInt8(index + 1)])
            }
            payload.writeField(field)
        }
        return payload.data
    }

    private func canonicalTopUpAnchorArchive() throws -> Data {
        func fields(_ values: [Data]) -> Data {
            var writer = CompactNoritoWriter()
            values.forEach { writer.writeField($0) }
            return writer.data
        }
        func constVector(_ value: Data) -> Data {
            fields(value.map { Data([$0]) })
        }
        func uint16(_ value: UInt16) -> Data {
            var writer = CompactNoritoWriter()
            writer.writeUInt16LE(value)
            return writer.data
        }
        func uint32(_ value: UInt32) -> Data {
            var writer = CompactNoritoWriter()
            writer.writeUInt32LE(value)
            return writer.data
        }
        func uint64(_ value: UInt64) -> Data {
            var writer = CompactNoritoWriter()
            writer.writeUInt64LE(value)
            return writer.data
        }

        var assetBytes = Data((0..<16).map { UInt8($0 + 1) })
        assetBytes[6] = (assetBytes[6] & 0x0f) | 0x40
        assetBytes[8] = (assetBytes[8] & 0x3f) | 0x80
        let fixed32: (UInt8) -> Data = { Data(repeating: $0, count: 32) }
        let payer = try AccountAddress
            .fromAccount(publicKey: fixed32(0xc0))
            .toI105(networkPrefix: 0x02f1)
        let account = try AccountAddress
            .parseEncoded(payer, expectedPrefix: 0x02f1)
            .compactNoritoAccountControllerPayload()

        func chain(_ value: String) -> Data {
            var writer = CompactNoritoWriter()
            writer.writeField(CompactNorito.encodeString(value))
            return writer.data
        }
        func amount() -> Data {
            var atomic = Data(repeating: 0, count: 16)
            atomic[0] = 1
            var writer = CompactNoritoWriter()
            writer.writeField(atomic)
            writer.writeField(uint32(2))
            return writer.data
        }
        func note() -> Data {
            var writer = CompactNoritoWriter()
            writer.writeField(chain("swift-offline-api"))
            writer.writeField(constVector(assetBytes))
            writer.writeField(fixed32(0xd0))
            writer.writeField(fixed32(0xd1))
            writer.writeField(amount())
            return writer.data
        }
        func assetID() -> Data {
            var writer = CompactNoritoWriter()
            writer.writeField(account)
            writer.writeField(constVector(assetBytes))
            writer.writeField(uint32(0))
            return writer.data
        }
        func verifierKeyID() -> Data {
            var writer = CompactNoritoWriter()
            writer.writeField(CompactNorito.encodeString("halo2/ipa"))
            writer.writeField(CompactNorito.encodeString("fixture-topup-shield"))
            return writer.data
        }
        func binding() -> Data {
            var writer = CompactNoritoWriter()
            writer.writeField(uint16(KagemushaRecursiveSpend.wireVersionV4))
            writer.writeField(CompactNorito.encodeString("generation-v4-test"))
            writer.writeField(fixed32(0xd9))
            return writer.data
        }

        let payload = fields([
            uint16(KagemushaRecursiveSpend.wireVersionV4),
            chain("swift-offline-api"),
            account,
            assetID(),
            uint32(2),
            amount(),
            fixed32(0xd2),
            fixed32(0xd3),
            uint32(7),
            note(),
            fixed32(0xd5),
            verifierKeyID(),
            fixed32(0xd6),
            binding(),
            uint64(1),
            fixed32(0xd7),
            fixed32(0xd8),
        ])
        return KagemushaRecursiveSpend.frameArchive(
            schema: KagemushaRecursiveSpend.topUpAnchorWireNameV4,
            payload: payload
        )
    }

    private func canonicalTopUpFinalityProofArchive() -> Data {
        KagemushaRecursiveSpend.frameArchive(
            schema: KagemushaRecursiveSpend.topUpFinalityProofWireName,
            payload: Data([0x02])
        )
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
