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
        XCTAssertEqual(KagemushaToriiAPI.Endpoint.capability.path, "/v1/offline/readiness")
        XCTAssertEqual(KagemushaToriiAPI.Endpoint.topUp.path, "/v1/offline/top-up")
        XCTAssertEqual(KagemushaToriiAPI.Endpoint.redeem.path, "/v1/offline/redeem")
        XCTAssertEqual(KagemushaToriiAPI.Endpoint.operations.path, "/v1/offline/operations")
        XCTAssertEqual(
            try KagemushaToriiAPI.operationPath(Self.operationId),
            "/v1/offline/operations/\(Self.operationId)"
        )
    }

    func testOperationIdentityRequiresMarkedDigestsAndBoundLifetime() throws {
        XCTAssertNoThrow(try operationIdentity(kind: .topUp))
        for (operationID, authorityDigest, requestDigest, issuedAt, expiresAt, field) in [
            (String(repeating: "10", count: 32), Self.authorityDigest, Self.requestDigest, 1, 2, "identity.operation_id"),
            (Self.operationId, String(repeating: "20", count: 32), Self.requestDigest, 1, 2, "identity.request_authority_digest"),
            (Self.operationId, Self.authorityDigest, String(repeating: "40", count: 32), 1, 2, "identity.canonical_request_digest"),
            (Self.operationId, Self.authorityDigest, Self.requestDigest, 0, 1, "identity.issued_at_ms"),
            (Self.operationId, Self.authorityDigest, Self.requestDigest, 2, 2, "identity.expires_at_ms"),
            (
                Self.operationId,
                Self.authorityDigest,
                Self.requestDigest,
                1,
                KagemushaRecursiveSpend.maximumAuthorizationTTLMilliseconds + 2,
                "identity.expires_at_ms"
            ),
        ] {
            XCTAssertThrowsError(try KagemushaOperationIdentity(
                operationID: operationID,
                requestAuthorityDigest: authorityDigest,
                canonicalRequestDigest: requestDigest,
                kind: .topUp,
                issuedAtMs: issuedAt,
                expiresAtMs: expiresAt
            )) { error in
                XCTAssertEqual(
                    error as? KagemushaOperationError,
                    .invalidField(field)
                )
            }
        }

        let encoded = try JSONEncoder().encode(operationIdentity(kind: .redeem))
        let object = try XCTUnwrap(
            JSONSerialization.jsonObject(with: encoded) as? [String: Any]
        )
        XCTAssertEqual(
            Set(object.keys),
            [
                "operation_id", "request_authority_digest",
                "canonical_request_digest", "kind", "issued_at_ms",
                "expires_at_ms",
            ]
        )
        XCTAssertEqual(
            try JSONDecoder().decode(KagemushaOperationIdentity.self, from: encoded),
            try operationIdentity(kind: .redeem)
        )
    }

    func testOperationReferenceNoritoRoundTrips() throws {
        for kind in [KagemushaOperationKind.topUp, .redeem] {
            let identity = try operationIdentity(kind: kind)
            let expected = try KagemushaOperationReference(
                identity: identity,
                state: .pending,
                transactionHash: Self.transactionHash,
                statusUri: "/v1/offline/operations/\(identity.operationID)"
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

    func testLegacyFlatOperationReferenceVectorIsRejected() throws {
        let archive = try XCTUnwrap(Data(hexString: Self.rustOperationReferenceArchiveHex))
        XCTAssertThrowsError(try KagemushaOperationCodec.decodeReference(archive))
    }

    func testLegacyFlatPendingStatusVectorIsRejected() throws {
        let archive = try XCTUnwrap(Data(hexString: Self.rustPendingStatusArchiveHex))
        XCTAssertThrowsError(try KagemushaOperationCodec.decodeStatus(
            archive,
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ))
    }

    func testNativeWholeStatusValidatorRejectsZeroSubmittedTime() throws {
        #if canImport(Darwin)
        try requireNativeTestCapability(
            KagemushaRecursiveSpend.hasRequiredNativeSymbols,
            "whole-status validation bridge is not linked in this test host"
        )
        let invalidIdentity = try KagemushaOperationIdentity(
            operationID: Self.operationId,
            requestAuthorityDigest: Self.authorityDigest,
            canonicalRequestDigest: Self.requestDigest,
            kind: .topUp,
            issuedAtMs: 1,
            expiresAtMs: 2
        )
        var status = CompactNoritoWriter()
        status.writeUInt32LE(0)
        status.writeField(encodedIdentity(invalidIdentity, issuedAtMs: 0))
        status.writeField(CompactNorito.encodeString(Self.transactionHash))
        let invalid = noritoEncode(
            typeName: "iroha_torii_shared::offline_api::OfflineOperationStatus",
            payload: status.data,
            flags: NoritoHeader.compactLen,
            payloadAlignment: 16
        )
        XCTAssertThrowsError(
            try NoritoNativeBridge.shared
                .kagemushaOfflineOperationStatusValidateV2(statusArchive: invalid)
        ) { error in
            XCTAssertEqual(error as? NativeBridgeError, .kagemushaProve)
        }
        #endif
    }

    func testLegacyFlatRejectedStatusVectorIsRejected() throws {
        let archive = try XCTUnwrap(Data(hexString: Self.rustRejectedStatusArchiveHex))
        XCTAssertThrowsError(try KagemushaOperationCodec.decodeStatus(
            archive,
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ))
    }

    func testRejectedOperationErrorDetailsDecodeEntrypointBeforeTransactionHash() throws {
        let status = try KagemushaOperationCodec.decodeStatus(
            try rejectedStatusWithHashDetailsArchive(),
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        )
        guard case .rejected(let rejected) = status else {
            return XCTFail("expected rejected operation status")
        }
        let details = try XCTUnwrap(rejected.error.details)
        XCTAssertEqual(details.entrypointHash, Self.entrypointHash)
        XCTAssertEqual(details.transactionHash, Self.transactionHash)
    }

    func testLegacyFlatAppliedStatusVectorIsRejected() throws {
        let archive = try XCTUnwrap(Data(hexString: Self.rustAppliedRedeemStatusArchiveHex))
        XCTAssertThrowsError(try KagemushaOperationCodec.decodeStatus(
            archive,
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ))
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
            networkID: request.payload.networkID,
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
        try requireNativeTestCapability(
            KagemushaRecursiveSpend.hasRequiredNativeSymbols,
            "ABI-23 bridge is not linked in this test host"
        )
        let request = try KagemushaPeerTransportTestFixtures.paymentRequest()

        let query = try NoritoNativeBridge.shared.withChainDiscriminant(
            AccountId.defaultNetworkPrefix
        ) {
            try KagemushaRecipientLineageQueryV2(
                networkID: request.payload.networkID,
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
        try requireNativeTestCapability(
            KagemushaRecursiveSpend.hasRequiredNativeSymbols,
            "ABI-23 bridge is not linked in this test host"
        )
        let request = try KagemushaPeerTransportTestFixtures.paymentRequest()
        let address = try AccountAddress.parseEncodedSwiftOnly(
            request.payload.recipient,
            expectedPrefix: SccpV1.tairaI105DiscriminantV1
        )
        let networkID = request.payload.networkID
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
                networkID: networkID,
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
                    networkID: networkID,
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
        let finalityProof = try KagemushaTopUpFinalityProofArchive(
            noritoArchive: canonicalTopUpFinalityProofArchive()
        )
        XCTAssertEqual(anchor.noritoArchive(), archive)
        XCTAssertEqual(anchor.networkId, TestNetworkIds.canonical)
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
        let lastValidLeaf = KagemushaRecursiveSpend.topUpShieldInsertionCapacityV2 - 1
        XCTAssertNoThrow(try KagemushaRecursiveSpendCodecs.decodeTopUpAnchorV4(
            canonicalTopUpAnchorArchive(shieldLeafIndex: lastValidLeaf),
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ))
        XCTAssertThrowsError(try KagemushaRecursiveSpendCodecs.decodeTopUpAnchorV4(
            canonicalTopUpAnchorArchive(
                shieldLeafIndex: KagemushaRecursiveSpend.topUpShieldInsertionCapacityV2
            ),
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ))
        XCTAssertEqual(anchor, try KagemushaTopUpAnchor(
            noritoArchive: archive,
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ))

        let result = try KagemushaTopUpResult(
            transactionHash: String(repeating: "d7", count: 32),
            finalizedBlockHeight: 1,
            anchor: anchor,
            finalityProof: finalityProof
        )
        XCTAssertEqual(result.anchor.noritoArchive(), archive)
        XCTAssertEqual(
            result.finalityProof.noritoArchive,
            canonicalTopUpFinalityProofArchive()
        )
        XCTAssertNoThrow(try KagemushaOperationStatus.Applied(
            identity: operationIdentity(
                kind: .topUp,
                operationID: String(repeating: "d5", count: 32)
            ),
            result: .topUp(result)
        ))
        XCTAssertThrowsError(try KagemushaOperationStatus.Applied(
            identity: operationIdentity(kind: .topUp),
            result: .topUp(result)
        ))
        XCTAssertThrowsError(try KagemushaTopUpResult(
            transactionHash: Self.transactionHash,
            finalizedBlockHeight: 1,
            anchor: anchor,
            finalityProof: finalityProof
        ))
        XCTAssertThrowsError(try KagemushaTopUpResult(
            transactionHash: String(repeating: "d7", count: 32),
            finalizedBlockHeight: 7,
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
        let identity = try operationIdentity(kind: .topUp)
        for invalidHash in [
            "",
            String(repeating: "22", count: 32),
            String(repeating: "2", count: 63),
            String(repeating: "2", count: 65),
            String(repeating: "A", count: 64),
            String(repeating: "g", count: 64),
            String(repeating: "0", count: 64),
            " \(Self.transactionHash)",
        ] {
            XCTAssertThrowsError(try KagemushaOperationReference(
                identity: identity,
                state: .pending,
                transactionHash: invalidHash,
                statusUri: "/v1/offline/operations/\(identity.operationID)"
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
                identity: identity,
                state: .pending,
                transactionHash: Self.transactionHash,
                statusUri: invalidUri
            )) { error in
                XCTAssertEqual(error as? KagemushaOperationError, .invalidField("status_uri"))
            }
        }

    }

    func testTaggedOperationStatePayloadsCannotBypassValidation() throws {
        XCTAssertThrowsError(try KagemushaOperationIdentity(
            operationID: String(repeating: "0", count: 64),
            requestAuthorityDigest: Self.authorityDigest,
            canonicalRequestDigest: Self.requestDigest,
            kind: .topUp,
            issuedAtMs: 1,
            expiresAtMs: 2
        )) { error in
            XCTAssertEqual(
                error as? KagemushaOperationError,
                .invalidField("identity.operation_id")
            )
        }

        XCTAssertThrowsError(try KagemushaOperationStatus.Pending(
            identity: operationIdentity(kind: .topUp),
            transactionHash: String(repeating: "F", count: 64)
        )) { error in
            XCTAssertEqual(error as? KagemushaOperationError, .invalidField("transaction_hash"))
        }

        XCTAssertThrowsError(try KagemushaOperationIdentity(
            operationID: Self.operationId,
            requestAuthorityDigest: Self.authorityDigest,
            canonicalRequestDigest: Self.requestDigest,
            kind: .topUp,
            issuedAtMs: 0,
            expiresAtMs: 1
        )) { error in
            XCTAssertEqual(
                error as? KagemushaOperationError,
                .invalidField("identity.issued_at_ms")
            )
        }

        XCTAssertThrowsError(try KagemushaRedeemResult(
            transactionHash: "not-a-hash",
            finalizedBlockHeight: 1
        )) { error in
            XCTAssertEqual(error as? KagemushaOperationError, .invalidField("transaction_hash"))
        }

        let anchor = try KagemushaTopUpAnchor(
            noritoArchive: canonicalTopUpAnchorArchive(),
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        )
        let finalityProof = try KagemushaTopUpFinalityProofArchive(
            noritoArchive: canonicalTopUpFinalityProofArchive()
        )
        XCTAssertThrowsError(try KagemushaTopUpResult(
            transactionHash: Self.transactionHash,
            finalizedBlockHeight: 0,
            anchor: anchor,
            finalityProof: finalityProof
        )) { error in
            XCTAssertEqual(
                error as? KagemushaOperationError,
                .invalidField("finalized_block_height")
            )
        }
        XCTAssertThrowsError(try KagemushaRedeemResult(
            transactionHash: Self.transactionHash,
            finalizedBlockHeight: 0
        )) { error in
            XCTAssertEqual(
                error as? KagemushaOperationError,
                .invalidField("finalized_block_height")
            )
        }

        let valid = try KagemushaOperationStatus.Pending(
            identity: operationIdentity(kind: .redeem),
            transactionHash: Self.transactionHash
        )
        XCTAssertEqual(KagemushaOperationStatus.pending(valid).identity.operationID, Self.operationId)
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
        let details = try KagemushaOperationErrorDetails(
            rejectCode: "TX_QUEUE_FULL",
            entrypointHash: Self.entrypointHash,
            transactionHash: Self.transactionHash
        )
        XCTAssertEqual(details.entrypointHash, Self.entrypointHash)
        XCTAssertEqual(details.transactionHash, Self.transactionHash)
        XCTAssertThrowsError(try KagemushaOperationErrorDetails(
            rejectCode: " TX_QUEUE_FULL",
            transactionHash: Self.transactionHash
        ))
        XCTAssertThrowsError(try KagemushaOperationErrorDetails(
            entrypointHash: String(repeating: "A", count: 64),
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

    func testAxtErrorDetailsExposeExactActiveEraAndNextCounter() throws {
        let details = try KagemushaAxtErrorDetails(
            code: "handle_sequence_mismatch",
            activeHandleEra: 9,
            nextHandleCounter: 4
        )

        XCTAssertEqual(details.activeHandleEra, 9)
        XCTAssertEqual(details.nextHandleCounter, 4)
    }

    func testOperationReferenceRejectsInvalidUtf8AndNonCanonicalFraming() throws {
        var invalidString = CompactNoritoWriter()
        invalidString.writeLength(1)
        invalidString.writeBytes(Data([0xff]))

        let identity = try operationIdentity(kind: .topUp)
        var payload = CompactNoritoWriter()
        payload.writeField(encodedIdentity(identity))
        payload.writeField(CompactNorito.encodeUInt32(0))
        payload.writeField(invalidString.data)
        payload.writeField(
            CompactNorito.encodeString("/v1/offline/operations/\(Self.operationId)")
        )

        XCTAssertThrowsError(try KagemushaOperationCodec.decodeReference(noritoEncode(
            typeName: "iroha_torii_shared::offline_api::OfflineOperationReference",
            payload: payload.data,
            flags: NoritoHeader.compactLen
        ))) { error in
            XCTAssertEqual(error as? KagemushaOperationError, .invalidField("string"))
        }

        let valid = try KagemushaOperationReference(
            identity: identity,
            state: .pending,
            transactionHash: Self.transactionHash,
            statusUri: "/v1/offline/operations/\(identity.operationID)"
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
        let expectedOperationId = try KagemushaOperationIdentityDerivation.operationID(
            compactAuthorityPayload: Self.authorityPayload,
            nonce: Self.authorizationNonce
        )
        let topUpArchive = requestArchive(
            schema: KagemushaRecursiveSpend.topUpRequestWireName,
            fieldCount: 8,
            operationIdFieldIndex: 6
        )
        let redeemArchive = requestArchive(
            schema: KagemushaRecursiveSpend.redeemRequestWireName,
            fieldCount: 10,
            operationIdFieldIndex: 8
        )

        let topUp = try KagemushaTopUpRequest(noritoArchive: topUpArchive)
        let redeem = try KagemushaRedeemRequest(noritoArchive: redeemArchive)

        XCTAssertEqual(topUp.identity.operationID, expectedOperationId)
        XCTAssertEqual(topUp.identity.issuedAtMs, 1)
        XCTAssertEqual(topUp.identity.expiresAtMs, 2)
        XCTAssertEqual(topUp.identity.kind, .topUp)
        XCTAssertEqual(
            topUp.identity.requestAuthorityDigest,
            try KagemushaOperationIdentityDerivation.requestAuthorityDigest(
                compactAuthorityPayload: Self.authorityPayload
            )
        )
        XCTAssertEqual(
            topUp.identity.canonicalRequestDigest,
            KagemushaOperationIdentityDerivation.canonicalRequestDigest(
                requestArchive: topUpArchive,
                kind: .topUp
            )
        )
        XCTAssertEqual(topUp.noritoArchive(), topUpArchive)
        XCTAssertEqual(redeem.identity.operationID, expectedOperationId)
        XCTAssertEqual(redeem.identity.issuedAtMs, 1)
        XCTAssertEqual(redeem.identity.expiresAtMs, 2)
        XCTAssertEqual(redeem.identity.kind, .redeem)
        XCTAssertNotEqual(
            topUp.identity.canonicalRequestDigest,
            redeem.identity.canonicalRequestDigest
        )
        XCTAssertEqual(redeem.noritoArchive(), redeemArchive)
    }

    func testRequestsRejectNonceOperationAuthorityAndLifetimeSubstitution() throws {
        let validOperationID = Data(hexString: try KagemushaOperationIdentityDerivation
            .operationID(
                compactAuthorityPayload: Self.authorityPayload,
                nonce: Self.authorizationNonce
            ))!
        for nonce in [
            Data(),
            Data([1]),
            Data(repeating: 1, count: 31),
            Data(repeating: 1, count: 33),
            Data(repeating: 0, count: 32),
        ] {
            XCTAssertThrowsError(try KagemushaTopUpRequest(noritoArchive: requestArchive(
                schema: KagemushaRecursiveSpend.topUpRequestWireName,
                fieldCount: 8,
                operationIdFieldIndex: 6,
                operationId: validOperationID,
                nonce: nonce
            ))) { error in
                XCTAssertEqual(
                    error as? KagemushaOperationError,
                    .invalidField("authorization.nonce")
                )
            }
        }

        var mismatchedOperationID = validOperationID
        mismatchedOperationID[0] ^= 1
        XCTAssertThrowsError(try KagemushaTopUpRequest(noritoArchive: requestArchive(
            schema: KagemushaRecursiveSpend.topUpRequestWireName,
            fieldCount: 8,
            operationIdFieldIndex: 6,
            operationId: mismatchedOperationID
        ))) { error in
            XCTAssertEqual(
                error as? KagemushaOperationError,
                .invalidField("authorization.operation_id")
            )
        }

        let alternateKeypair = try Keypair(
            privateKeyBytes: Data(repeating: 0x43, count: 32)
        )
        let alternateAuthority = try AccountAddress
            .fromAccount(publicKey: alternateKeypair.publicKey)
            .compactNoritoAccountControllerPayload()
        XCTAssertThrowsError(try KagemushaTopUpRequest(noritoArchive: requestArchive(
            schema: KagemushaRecursiveSpend.topUpRequestWireName,
            fieldCount: 8,
            operationIdFieldIndex: 6,
            operationId: validOperationID,
            authorityPayload: alternateAuthority
        ))) { error in
            XCTAssertEqual(
                error as? KagemushaOperationError,
                .invalidField("authorization.operation_id")
            )
        }

        XCTAssertThrowsError(try KagemushaTopUpRequest(noritoArchive: requestArchive(
            schema: KagemushaRecursiveSpend.topUpRequestWireName,
            fieldCount: 8,
            operationIdFieldIndex: 6,
            expiresAtMs: KagemushaRecursiveSpend.maximumAuthorizationTTLMilliseconds + 2
        ))) { error in
            XCTAssertEqual(
                error as? KagemushaOperationError,
                .invalidField("authorization.expires_at_ms")
            )
        }
    }

    func testRequestsRequireAuthorizationIdentityAndPositiveIssuedAt() throws {
        let operationId = Data(repeating: 0x31, count: 32)
        let otherOperationId = Data(repeating: 0x32, count: 32)
        for (schema, fieldCount, operationIdFieldIndex) in [
            (KagemushaRecursiveSpend.topUpRequestWireName, 8, 6),
            (KagemushaRecursiveSpend.redeemRequestWireName, 10, 8),
        ] {
            let mismatched = requestArchive(
                schema: schema,
                fieldCount: fieldCount,
                operationIdFieldIndex: operationIdFieldIndex,
                operationId: operationId,
                authorizationOperationId: otherOperationId
            )
            let zeroIssuedAt = requestArchive(
                schema: schema,
                fieldCount: fieldCount,
                operationIdFieldIndex: operationIdFieldIndex,
                operationId: operationId,
                issuedAtMs: 0
            )
            let construct: (Data) throws -> Void = schema
                == KagemushaRecursiveSpend.topUpRequestWireName
                ? { _ = try KagemushaTopUpRequest(noritoArchive: $0) }
                : { _ = try KagemushaRedeemRequest(noritoArchive: $0) }

            XCTAssertThrowsError(try construct(mismatched)) { error in
                XCTAssertEqual(
                    error as? KagemushaOperationError,
                    .invalidField("authorization.operation_id")
                )
            }
            XCTAssertThrowsError(try construct(zeroIssuedAt)) { error in
                XCTAssertEqual(
                    error as? KagemushaOperationError,
                    .invalidField("authorization.issued_at_ms")
                )
            }
        }
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

    private func operationIdentity(
        kind: KagemushaOperationKind,
        operationID: String = ToriiKagemushaAPIModelsTests.operationId,
        requestAuthorityDigest: String = ToriiKagemushaAPIModelsTests.authorityDigest,
        canonicalRequestDigest: String = ToriiKagemushaAPIModelsTests.requestDigest,
        issuedAtMs: UInt64 = 1,
        expiresAtMs: UInt64 = 2
    ) throws -> KagemushaOperationIdentity {
        try KagemushaOperationIdentity(
            operationID: operationID,
            requestAuthorityDigest: requestAuthorityDigest,
            canonicalRequestDigest: canonicalRequestDigest,
            kind: kind,
            issuedAtMs: issuedAtMs,
            expiresAtMs: expiresAtMs
        )
    }

    private func encodedIdentity(
        _ identity: KagemushaOperationIdentity,
        issuedAtMs: UInt64? = nil
    ) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(CompactNorito.encodeString(identity.operationID))
        writer.writeField(CompactNorito.encodeString(identity.requestAuthorityDigest))
        writer.writeField(CompactNorito.encodeString(identity.canonicalRequestDigest))
        writer.writeField(CompactNorito.encodeUInt32(identity.kind == .topUp ? 0 : 1))
        writer.writeField(CompactNorito.encodeUInt64(issuedAtMs ?? identity.issuedAtMs))
        writer.writeField(CompactNorito.encodeUInt64(identity.expiresAtMs))
        return writer.data
    }

    private func requestArchive(
        schema: String,
        fieldCount: Int,
        operationIdFieldIndex: Int,
        operationId: Data? = nil,
        authorizationOperationId: Data? = nil,
        issuedAtMs: UInt64 = 1,
        expiresAtMs: UInt64 = 2,
        nonce: Data = ToriiKagemushaAPIModelsTests.authorizationNonce,
        authorityPayload: Data = ToriiKagemushaAPIModelsTests.authorityPayload
    ) -> Data {
        KagemushaRecursiveSpend.frameArchive(
            schema: schema,
            payload: requestPayload(
                fieldCount: fieldCount,
                operationIdFieldIndex: operationIdFieldIndex,
                operationId: operationId,
                authorizationOperationId: authorizationOperationId,
                issuedAtMs: issuedAtMs,
                expiresAtMs: expiresAtMs,
                nonce: nonce,
                authorityPayload: authorityPayload
            )
        )
    }

    private func rejectedStatusWithHashDetailsArchive() throws -> Data {
        func optionalString(_ value: String?) throws -> Data {
            try CompactNorito.encodeOption(value, encode: CompactNorito.encodeString)
        }

        let none = Data([0])
        var details = CompactNoritoWriter()
        details.writeField(try optionalString(nil)) // layer
        details.writeField(try optionalString(nil)) // reject_code
        details.writeField(none) // queue
        details.writeField(none) // retry_after_seconds
        details.writeField(try optionalString(nil)) // endpoint
        details.writeField(try optionalString(nil)) // field
        details.writeField(try optionalString(nil)) // expected
        details.writeField(try optionalString(nil)) // actual
        details.writeField(try optionalString(nil)) // profile
        details.writeField(none) // chain_discriminant
        details.writeField(try optionalString(Self.entrypointHash))
        details.writeField(try optionalString(Self.transactionHash))
        details.writeField(try optionalString(nil)) // last_status
        details.writeField(try optionalString(nil)) // hint
        details.writeField(none) // axt

        var detailsOption = CompactNoritoWriter()
        detailsOption.writeUInt8(1)
        detailsOption.writeField(details.data)

        var error = CompactNoritoWriter()
        error.writeField(CompactNorito.encodeString("offline_operation_rejected"))
        error.writeField(CompactNorito.encodeString("rejected"))
        error.writeField(detailsOption.data)

        var status = CompactNoritoWriter()
        status.writeUInt32LE(2)
        status.writeField(encodedIdentity(try operationIdentity(kind: .redeem)))
        status.writeField(CompactNorito.encodeString(Self.transactionHash))
        status.writeField(error.data)
        return noritoEncode(
            typeName: "iroha_torii_shared::offline_api::OfflineOperationStatus",
            payload: status.data,
            flags: NoritoHeader.compactLen,
            payloadAlignment: 16
        )
    }

    private func assertExactRequestBodyCeiling(
        schema: String,
        fieldCount: Int,
        operationIdFieldIndex: Int,
        maximumBytes: Int,
        construct: (Data) throws -> Void
    ) throws {
        let operationId = Data(hexString: try KagemushaOperationIdentityDerivation
            .operationID(
                compactAuthorityPayload: Self.authorityPayload,
                nonce: Self.authorizationNonce
            ))!
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
                } else if index == fieldCount - 1 {
                    field = requestAuthorization(
                        operationId: operationId,
                        issuedAtMs: 1,
                        expiresAtMs: 2,
                        nonce: Self.authorizationNonce
                    )
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
        operationId: Data? = nil,
        authorizationOperationId: Data? = nil,
        issuedAtMs: UInt64 = 1,
        expiresAtMs: UInt64 = 2,
        nonce: Data = ToriiKagemushaAPIModelsTests.authorizationNonce,
        authorityPayload: Data = ToriiKagemushaAPIModelsTests.authorityPayload
    ) -> Data {
        let derivedOperationID = Data(hexString: try! KagemushaOperationIdentityDerivation
            .operationID(
                compactAuthorityPayload: Self.authorityPayload,
                nonce: Self.authorizationNonce
            ))!
        let outerOperationID = operationId ?? derivedOperationID
        var payload = CompactNoritoWriter()
        for index in 0..<fieldCount {
            let field: Data
            if index == 0 {
                field = CompactNorito.encodeUInt16(KagemushaRecursiveSpend.wireVersionV4)
            } else if index == operationIdFieldIndex {
                field = outerOperationID
            } else if index == fieldCount - 1 {
                field = requestAuthorization(
                    operationId: authorizationOperationId ?? outerOperationID,
                    issuedAtMs: issuedAtMs,
                    expiresAtMs: expiresAtMs,
                    nonce: nonce,
                    authorityPayload: authorityPayload
                )
            } else {
                field = Data([UInt8(index + 1)])
            }
            payload.writeField(field)
        }
        return payload.data
    }

    private func requestAuthorization(
        operationId: Data,
        issuedAtMs: UInt64,
        expiresAtMs: UInt64,
        nonce: Data,
        authorityPayload: Data = ToriiKagemushaAPIModelsTests.authorityPayload
    ) -> Data {
        var authorization = CompactNoritoWriter()
        for index in 0..<10 {
            let field: Data
            switch index {
            case 0:
                field = authorityPayload
            case 1:
                field = CompactNorito.encodeString("swift-kagemusha-fixture")
            case 3:
                field = operationId
            case 4:
                field = CompactNorito.encodeUInt64(issuedAtMs)
            case 5:
                field = CompactNorito.encodeUInt64(expiresAtMs)
            case 6:
                field = nonce
            case 7:
                field = Data(repeating: 0x77, count: 32)
            case 8:
                field = Data(repeating: 0x99, count: 32)
            default:
                field = Data([UInt8(index + 1)])
            }
            authorization.writeField(field)
        }
        return authorization.data
    }

    private func canonicalTopUpAnchorArchive(shieldLeafIndex: UInt32 = 7) throws -> Data {
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
        let payerPublicKey = try Keypair(
            privateKeyBytes: fixed32(0xc0)
        ).publicKey
        let payer = try AccountAddress
            .fromAccount(publicKey: payerPublicKey)
            .toI105(networkPrefix: 0x02f1)
        let account = try AccountAddress
            .parseEncoded(payer, expectedPrefix: 0x02f1)
            .compactNoritoAccountControllerPayload()

        func network() -> Data { TestNetworkIds.canonical.bytes }
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
            writer.writeField(network())
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
            network(),
            account,
            assetID(),
            uint32(2),
            amount(),
            fixed32(0xd2),
            fixed32(0xd3),
            uint32(shieldLeafIndex),
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

    private static let authorityPayload: Data = {
        let keypair = try! Keypair(privateKeyBytes: Data(repeating: 0x42, count: 32))
        let address = try! AccountAddress.fromAccount(publicKey: keypair.publicKey)
        return try! address.compactNoritoAccountControllerPayload()
    }()
    private static let authorizationNonce = Data(repeating: 0x51, count: 32)
    private static let authorityDigest = String(repeating: "33", count: 32)
    private static let requestDigest = String(repeating: "55", count: 32)
    private static let operationId = String(repeating: "11", count: 32)
    private static let entrypointHash = String(repeating: "33", count: 32)
    private static let transactionHash = String(repeating: "22", count: 31) + "23"
    private static let rustOperationReferenceArchiveHex =
        "4e5254300000e8e2244e45e4be2a975e34957141128b00f0000000000000001b2c5ec0e2d4dc42024140313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131310400000000040000000041403232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323358572f76312f6f66666c696e652f6f7065726174696f6e732f3131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313108ffffffffffffffff"
    private static let rustPendingStatusArchiveHex =
        "4e5254300000fb04214104df1bdcd39249bddd4db23a0096000000000000008b9a6668d701e20402000000000000000000000000414031313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131040000000041403232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323308ffffffffffffffff"
    private static let rustRejectedStatusArchiveHex =
        "4e5254300000fb04214104df1bdcd39249bddd4db23a00b600000000000000fc930af6e00cccbe020000000000000000020000004140313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131310401000000414032323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323233281b1a6f66666c696e655f6f7065726174696f6e5f72656a6563746564090872656a65637465640100"
    private static let rustAppliedRedeemStatusArchiveHex =
        "4e5254300000fb04214104df1bdcd39249bddd4db23a009700000000000000ab260b446c2573b20200000000000000000100000041403131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313159010000005441403232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323308ffffffffffffffff"
}
