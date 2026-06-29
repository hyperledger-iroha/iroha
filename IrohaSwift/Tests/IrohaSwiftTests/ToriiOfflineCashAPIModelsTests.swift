import XCTest
@testable import IrohaSwift

final class ToriiOfflineCashAPIModelsTests: XCTestCase {
    func testEndpointConstantsUseCurrentOfflineNoteRoutes() {
        XCTAssertEqual(ToriiOfflineCashAPI.Endpoint.keyRefill.path, "/v1/offline/v2/keys/refill")
        XCTAssertEqual(ToriiOfflineCashAPI.Endpoint.noteIssue.path, "/v1/offline/v2/notes/issue")
        XCTAssertEqual(ToriiOfflineCashAPI.Endpoint.noteRedeem.path, "/v1/offline/v2/notes/redeem")
        XCTAssertEqual(ToriiOfflineCashAPI.Endpoint.audit.path, "/v1/offline/v2/audit")
        XCTAssertEqual(ToriiOfflineCashAPI.Endpoint.revocationBundle.path, "/v1/offline/revocations/bundle")
        XCTAssertEqual(ToriiOfflineCashAPI.Endpoint.telemetry.path, "/v1/offline/telemetry")
    }

    func testIssuerEndpointConstantsDoNotRegressToRetiredRoutes() {
        let retiredRoutes = Set([
            "/v1/offline/keys/refill",
            "/v1/offline/notes/issue",
            "/v1/offline/notes/redeem",
            "/v1/offline/audit",
        ])
        let issuerRoutes = [
            ToriiOfflineCashAPI.Endpoint.keyRefill.path,
            ToriiOfflineCashAPI.Endpoint.noteIssue.path,
            ToriiOfflineCashAPI.Endpoint.noteRedeem.path,
            ToriiOfflineCashAPI.Endpoint.audit.path,
        ]

        XCTAssertEqual(Set(issuerRoutes).count, issuerRoutes.count)
        for route in issuerRoutes {
            XCTAssertTrue(route.hasPrefix("/v1/offline/v2/"))
            XCTAssertFalse(retiredRoutes.contains(route))
        }
    }

    func testCompactKeyCertificateDefaultsUseCanonicalAttestationProfiles() throws {
        let iosCertificate = try Self.certificate(platform: "ios-appattest").offlineNoteKeyCertificate()
        XCTAssertEqual(iosCertificate.assertionScheme, "apple-appattest-counter-v1")
        XCTAssertEqual(iosCertificate.assertionKeyAlgorithm, "app-attest-p256")
        XCTAssertNil(iosCertificate.assertionUsageCountLimit)

        let androidCertificate = try Self.certificate(
            platform: "android-keymint",
            assertionUsageCountLimit: 1
        ).offlineNoteKeyCertificate()
        XCTAssertEqual(androidCertificate.assertionScheme, "android-keymint-ecdsa-p256-usage-limit-v1")
        XCTAssertEqual(androidCertificate.assertionKeyAlgorithm, "ecdsa-p256-sha256")
        XCTAssertEqual(androidCertificate.assertionUsageCountLimit, 1)
    }

    func testCompactKeyCertificateRejectsNonCanonicalCompatibilityFields() throws {
        XCTAssertThrowsError(try Self.certificate(
            platform: "android-keymint",
            assertionScheme: "android-keymint-ecdsa-p256-usage-limit",
            assertionUsageCountLimit: 1
        ).offlineNoteKeyCertificate()) { error in
            XCTAssertEqual(error as? OfflineNoteCompatibilityError, .invalidField("assertion_scheme"))
        }

        XCTAssertThrowsError(try Self.certificate(
            platform: "ios-appattest",
            assertionKeyAlgorithm: "ed25519"
        ).offlineNoteKeyCertificate()) { error in
            XCTAssertEqual(error as? OfflineNoteCompatibilityError, .invalidField("assertion_key_algorithm"))
        }

        XCTAssertThrowsError(try Self.certificate(
            platform: "ios-appattest",
            issuerSignatureBase64: "issuer-signature"
        ).offlineNoteKeyCertificate()) { error in
            XCTAssertEqual(error as? OfflineNoteCompatibilityError, .invalidField("issuer_signature_base64"))
        }

        for invalidPlatform in ["android", "android-keymint ", "Android-keymint", "ios-appattest-android"] {
            XCTAssertThrowsError(try Self.certificate(
                platform: invalidPlatform,
                assertionScheme: "android-keymint-ecdsa-p256-usage-limit-v1",
                assertionKeyAlgorithm: "ecdsa-p256-sha256",
                assertionUsageCountLimit: 1
            ).offlineNoteKeyCertificate()) { error in
                XCTAssertEqual(error as? OfflineNoteCompatibilityError, .invalidField("platform"))
            }
        }
    }

    func testCompactKeyCertificateRejectsRetiredAssertionPublicKeyAlias() throws {
        let retiredAlias = Data(repeating: 3, count: 65).base64EncodedString()
        XCTAssertThrowsError(try Self.certificate(
            platform: "ios-appattest",
            assertionPublicKey: nil,
            appAttestPublicKeyBase64: retiredAlias
        ).offlineNoteKeyCertificate()) { error in
            XCTAssertEqual(error as? OfflineNoteCompatibilityError, .invalidField("app_attest_public_key_base64"))
        }

        XCTAssertThrowsError(try Self.certificate(
            platform: "ios-appattest",
            assertionPublicKey: nil
        ).offlineNoteKeyCertificate()) { error in
            XCTAssertEqual(error as? OfflineNoteCompatibilityError, .invalidField("assertion_public_key"))
        }
    }

    func testCompactKeyCertificateRejectsNonCanonicalBase64Encodings() throws {
        let hexPublicKey = Data(repeating: 1, count: 33).map { String(format: "%02x", $0) }.joined()
        XCTAssertThrowsError(try Self.certificate(
            platform: "ios-appattest",
            publicKey: hexPublicKey
        ).offlineNoteKeyCertificate()) { error in
            XCTAssertEqual(error as? OfflineNoteCompatibilityError, .invalidField("public_key"))
        }

        let urlSafeAssertionKey = Data(repeating: 0xFF, count: 32)
            .base64EncodedString()
            .replacingOccurrences(of: "/", with: "_")
        XCTAssertThrowsError(try Self.certificate(
            platform: "ios-appattest",
            assertionPublicKey: urlSafeAssertionKey
        ).offlineNoteKeyCertificate()) { error in
            XCTAssertEqual(error as? OfflineNoteCompatibilityError, .invalidField("assertion_public_key"))
        }

        let canonicalSignature = Data(repeating: 2, count: 64).base64EncodedString()
        XCTAssertThrowsError(try Self.certificate(
            platform: "ios-appattest",
            issuerSignatureBase64: " \(canonicalSignature)"
        ).offlineNoteKeyCertificate()) { error in
            XCTAssertEqual(error as? OfflineNoteCompatibilityError, .invalidField("issuer_signature_base64"))
        }
        XCTAssertThrowsError(try Self.certificate(
            platform: "ios-appattest",
            issuerSignatureBase64: canonicalSignature.replacingOccurrences(of: "=", with: "")
        ).offlineNoteKeyCertificate()) { error in
            XCTAssertEqual(error as? OfflineNoteCompatibilityError, .invalidField("issuer_signature_base64"))
        }
    }

    func testKeyRefillRequestEncodesSnakeCaseAndRejectsRetiredAttestKeyAlias() throws {
        let request = ToriiOfflineKeyRefillRequest(
            operationId: "op-refill",
            accountId: "alice@hbl.sbp",
            deviceId: "device-1",
            offlinePublicKey: "offline-public-key",
            attestationKeyId: "attest-key",
            assetDefinitionId: "pkr#sbp",
            existingLineageId: "lineage-1",
            lineageState: try Self.lineageState(),
            localRevision: 3,
            localStateHash: "state-3",
            deviceBinding: Self.binding(),
            deviceProof: Self.proof()
        )

        XCTAssertEqual(ToriiOfflineCashAPI.idempotencyKey(for: request), "op-refill")
        let json = try Self.jsonObject(ToriiOfflineCashAPI.canonicalBody(request))
        XCTAssertEqual(json["operation_id"] as? String, "op-refill")
        XCTAssertNil(json["app_attest_key_id"])
        XCTAssertEqual(json["attestation_key_id"] as? String, "attest-key")
        let keyCertificateBindings = try XCTUnwrap(json["key_certificate_bindings"] as? [[String: Any]])
        XCTAssertEqual(keyCertificateBindings.count, 1)
        XCTAssertEqual(keyCertificateBindings.first?["attestation_key_id"] as? String, "attest-key")
        XCTAssertEqual(keyCertificateBindings.first?["assertion_public_key"] as? String, "assertion-public-key")
        XCTAssertEqual((json["lineage_state"] as? [String: Any])?["lineage_id"] as? String, "lineage-1")
        XCTAssertNil(json["operationId"])
        let proof = try XCTUnwrap(json["device_proof"] as? [String: Any])
        XCTAssertEqual(proof["challenge_hash_hex"] as? String, "abc123")
        XCTAssertNil(proof["challengeHashHex"])

        let retiredAliasPayload = """
        {
          "operation_id":"op-refill",
          "account_id":"alice@hbl.sbp",
          "device_id":"device-1",
          "offline_public_key":"offline-public-key",
          "app_attest_key_id":"retired-attest-key",
          "asset_definition_id":"pkr#sbp",
          "local_revision":3,
          "local_state_hash":"state-3",
          "device_binding":\(try Self.jsonString(Self.binding())),
          "device_proof":\(try Self.jsonString(Self.proof()))
        }
        """
        XCTAssertThrowsError(try JSONDecoder().decode(
            ToriiOfflineKeyRefillRequest.self,
            from: Data(retiredAliasPayload.utf8)
        )) { error in
            guard case DecodingError.keyNotFound(let key, _) = error else {
                return XCTFail("expected missing attestation_key_id, got \(error)")
            }
            XCTAssertEqual(key.stringValue, "attestation_key_id")
        }
    }

    func testIssueSettlementRequestCarriesLineageState() throws {
        let request = try ToriiOfflineNoteIssueSettlementRequest(
            operationId: "op-issue",
            accountId: "alice@hbl.sbp",
            deviceId: "device-1",
            offlinePublicKey: "offline-public-key",
            lineageId: "lineage-1",
            assetDefinitionId: "pkr#sbp",
            amount: "50.00",
            noteCommitment: Self.hashHex(9),
            lineageState: try Self.lineageState(),
            localBalance: "100.00",
            localRevision: 4,
            localStateHash: "state-4",
            deviceBinding: Self.binding(),
            deviceProof: Self.proof()
        )

        XCTAssertEqual(ToriiOfflineCashAPI.idempotencyKey(for: request), "op-issue")
        let json = try Self.jsonObject(ToriiOfflineCashAPI.canonicalBody(request))
        XCTAssertEqual(json["operation_id"] as? String, "op-issue")
        XCTAssertEqual(json["offline_public_key"] as? String, "offline-public-key")
        XCTAssertEqual(json["note_commitment"] as? String, Self.hashHex(9))
        let keyCertificateBindings = try XCTUnwrap(json["key_certificate_bindings"] as? [[String: Any]])
        XCTAssertEqual(keyCertificateBindings.count, 1)
        XCTAssertEqual(keyCertificateBindings.first?["attestation_key_id"] as? String, "attest-key")
        XCTAssertEqual((json["lineage_state"] as? [String: Any])?["lineage_id"] as? String, "lineage-1")
        XCTAssertNil(json["offlinePublicKey"])
        XCTAssertNil(json["lineageState"])
    }

    func testRedeemSettlementRequestEncodesSnakeCaseContract() throws {
        let redemption = try ToriiOfflineRedemptionProof(
            sourceNoteCommitment: Self.hashHex(1),
            inputNullifiers: [Self.hashHex(3)],
            senderKeyCertificate: try Self.certificate(),
            recipientAccountId: "alice@hbl.sbp",
            assetDefinitionId: "pkr#sbp",
            amount: "25.00",
            recursiveProof: OfflineRecursiveProof(
                publicInputsHashHex: Self.hashHex(5),
                proofBytesBase64: Data("proof".utf8).base64EncodedString()
            )
        )
        let request = try ToriiOfflineNoteRedeemSettlementRequest(
            operationId: "op-redeem",
            accountId: "alice@hbl.sbp",
            deviceId: "device-1",
            lineageId: "lineage-1",
            assetDefinitionId: "pkr#sbp",
            amount: "25.00",
            localBalance: "100.00",
            localRevision: 4,
            localStateHash: "state-4",
            pendingReceipts: [],
            paymentTokens: [],
            paymentTokensNoritoBase64: ["native-token"],
            deviceBinding: Self.binding(),
            deviceProof: Self.proof(),
            redemption: redemption
        )

        XCTAssertEqual(ToriiOfflineCashAPI.idempotencyKey(for: request), "op-redeem")
        let json = try Self.jsonObject(ToriiOfflineCashAPI.canonicalBody(request))
        XCTAssertEqual(json["operation_id"] as? String, "op-redeem")
        XCTAssertEqual(json["amount"] as? String, "25.00")
        XCTAssertEqual(json["local_balance"] as? String, "100.00")
        XCTAssertEqual(json["payment_tokens_norito_base64"] as? [String], ["native-token"])
        XCTAssertNil(json["operationId"])
        XCTAssertNil(json["localBalance"])
        XCTAssertNil(json["paymentTokensNoritoBase64"])
        let redemptionJSON = try XCTUnwrap(json["redemption"] as? [String: Any])
        XCTAssertEqual(redemptionJSON["source_note_commitment"] as? String, Self.hashHex(1))
        XCTAssertNil(redemptionJSON["sourceNoteCommitment"])
        let recursiveProof = try XCTUnwrap(redemptionJSON["recursive_proof"] as? [String: Any])
        XCTAssertEqual(recursiveProof["public_inputs_hash_hex"] as? String, Self.hashHex(5))
    }

    func testRecursiveProofRejectsNonCanonicalBase64Encodings() throws {
        let canonicalProofBytes = Data(repeating: 3, count: 64).base64EncodedString()
        XCTAssertNoThrow(try OfflineRecursiveProof(
            publicInputsHashHex: Self.hashHex(7),
            proofBytesBase64: canonicalProofBytes
        ).offlineNoteRecursiveProof())

        let hexProofBytes = Data(repeating: 4, count: 33).map { String(format: "%02x", $0) }.joined()
        XCTAssertThrowsError(try OfflineRecursiveProof(
            publicInputsHashHex: Self.hashHex(7),
            proofBytesBase64: hexProofBytes
        ).offlineNoteRecursiveProof()) { error in
            XCTAssertEqual(error as? OfflineNoteCompatibilityError, .invalidField("proof_bytes_base64"))
        }

        let urlSafeProofBytes = Data(repeating: 0xFF, count: 64)
            .base64EncodedString()
            .replacingOccurrences(of: "/", with: "_")
        XCTAssertThrowsError(try OfflineRecursiveProof(
            publicInputsHashHex: Self.hashHex(7),
            proofBytesBase64: urlSafeProofBytes
        ).offlineNoteRecursiveProof()) { error in
            XCTAssertEqual(error as? OfflineNoteCompatibilityError, .invalidField("proof_bytes_base64"))
        }

        XCTAssertThrowsError(try OfflineRecursiveProof(
            publicInputsHashHex: Self.hashHex(7),
            proofBytesBase64: " \(canonicalProofBytes)"
        ).offlineNoteRecursiveProof()) { error in
            XCTAssertEqual(error as? OfflineNoteCompatibilityError, .invalidField("proof_bytes_base64"))
        }

        XCTAssertThrowsError(try OfflineRecursiveProof(
            publicInputsHashHex: Self.hashHex(7),
            proofBytesBase64: canonicalProofBytes.replacingOccurrences(of: "=", with: "")
        ).offlineNoteRecursiveProof()) { error in
            XCTAssertEqual(error as? OfflineNoteCompatibilityError, .invalidField("proof_bytes_base64"))
        }
    }

    func testSettlementResponseDecodesCanonicalAmounts() throws {
        let response = try JSONDecoder().decode(
            ToriiOfflineNoteRedeemSettlementResponse.self,
            from: Data("""
            {
              "operation_id":"op-redeem",
              "settlement":{
                "operation_id":"op-redeem",
                "kind":"redeem",
                "account_id":"alice@hbl.sbp",
                "device_id":"device-1",
                "asset_definition_id":"pkr#sbp",
                "amount":"25.00",
                "pre_balance":"100.00",
                "post_balance":"75.00",
                "entry_hash":"entry",
                "chain_tx_hash":"tx",
                "block_height":7,
                "issued_at_ms":1700000000000,
                "issuer_signature_base64":"signature"
              },
              "local_balance":"75.00",
              "locked_balance":"0",
              "local_revision":5,
              "local_state_hash":"state-5",
              "accepted_receipt_ids":["receipt-1"]
            }
            """.utf8)
        )

        XCTAssertEqual(response.operationId, "op-redeem")
        XCTAssertEqual(response.settlement.kind, .redeem)
        XCTAssertEqual(response.settlement.amount, "25.00")
        XCTAssertEqual(response.settlement.postBalance, "75.00")
        XCTAssertEqual(response.acceptedReceiptIds, ["receipt-1"])
    }

    private static func binding() -> ToriiOfflineDeviceBinding {
        ToriiOfflineDeviceBinding(
            platform: "ios",
            attestationKeyId: "attest-key",
            deviceId: "device-1",
            offlinePublicKey: "offline-public-key",
            assertionPublicKey: "assertion-public-key",
            attestationReportBase64: "report"
        )
    }

    private static func proof() -> ToriiOfflineDeviceProof {
        ToriiOfflineDeviceProof(
            platform: "ios",
            attestationKeyId: "attest-key",
            challengeHashHex: "abc123",
            assertionBase64: "assertion",
            counter: 1
        )
    }

    private static func certificate(
        platform: String = "ios-appattest",
        assertionScheme: String? = nil,
        assertionKeyAlgorithm: String? = nil,
        assertionUsageCountLimit: Int? = nil,
        publicKey: String = Data(repeating: 1, count: 32).base64EncodedString(),
        assertionPublicKey: String? = Data(repeating: 2, count: 65).base64EncodedString(),
        appAttestPublicKeyBase64: String? = nil,
        issuerSignatureBase64: String = Data(repeating: 2, count: 64).base64EncodedString()
    ) throws -> OfflineCompactKeyCertificate {
        let keypair = try Keypair(privateKeyBytes: Data(0..<32))
        return OfflineCompactKeyCertificate(
            platform: platform,
            keyId: "attest-key",
            deviceId: "device-1",
            accountId: AccountId.make(publicKey: keypair.publicKey),
            publicKey: publicKey,
            assertionScheme: assertionScheme,
            assertionKeyAlgorithm: assertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey,
            assertionUsageCountLimit: assertionUsageCountLimit,
            appAttestPublicKeyBase64: appAttestPublicKeyBase64,
            issuerSignatureBase64: issuerSignatureBase64
        )
    }

    private static func lineageState() throws -> ToriiOfflineCashState {
        try ToriiOfflineCashState(
            lineageId: "lineage-1",
            accountId: "alice@hbl.sbp",
            deviceId: "device-1",
            offlinePublicKey: "offline-public-key",
            assetDefinitionId: "pkr#sbp",
            balance: "100.00",
            lockedBalance: "0",
            serverRevision: 4,
            serverStateHash: "server-state-4",
            pendingLocalRevision: 4,
            authorization: ToriiOfflineSpendAuthorization(
                authorizationId: "authorization-1",
                lineageId: "lineage-1",
                accountId: "alice@hbl.sbp",
                verdictId: "verdict-1",
                policyMaxBalance: "1000",
                policyMaxTxValue: "250",
                issuedAtMs: 1_700_000_000_000,
                refreshAtMs: 1_700_000_100_000,
                expiresAtMs: 1_700_000_200_000,
                deviceBinding: Self.binding(),
                issuerSignatureBase64: "issuer-signature"
            ),
            issuerSignatureBase64: "issuer-signature"
        )
    }

    private static func hashHex(_ lastByte: UInt8) -> String {
        (Data(repeating: 0, count: 31) + Data([lastByte])).map { String(format: "%02x", $0) }.joined()
    }

    private static func jsonObject(_ data: Data) throws -> [String: Any] {
        try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    private static func jsonString<T: Encodable>(_ value: T) throws -> String {
        String(data: try ToriiOfflineCashAPI.canonicalBody(value), encoding: .utf8) ?? "{}"
    }
}
