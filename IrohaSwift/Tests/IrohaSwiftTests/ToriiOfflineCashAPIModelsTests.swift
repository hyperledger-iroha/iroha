import XCTest
@testable import IrohaSwift

final class ToriiOfflineCashAPIModelsTests: XCTestCase {
    func testEndpointConstantsUseCurrentOfflineNoteRoutes() {
        XCTAssertEqual(ToriiOfflineCashAPI.Endpoint.keyRefill.path, "/v1/offline/keys/refill")
        XCTAssertEqual(ToriiOfflineCashAPI.Endpoint.noteIssue.path, "/v1/offline/notes/issue")
        XCTAssertEqual(ToriiOfflineCashAPI.Endpoint.noteRedeem.path, "/v1/offline/notes/redeem")
        XCTAssertEqual(ToriiOfflineCashAPI.Endpoint.audit.path, "/v1/offline/audit")
        XCTAssertEqual(ToriiOfflineCashAPI.Endpoint.revocationBundle.path, "/v1/offline/revocations/bundle")
        XCTAssertEqual(ToriiOfflineCashAPI.Endpoint.telemetry.path, "/v1/offline/telemetry")
    }

    func testKeyRefillRequestEncodesSnakeCaseAndLegacyAttestKeyAliasDecodes() throws {
        let request = ToriiOfflineKeyRefillRequest(
            operationId: "op-refill",
            accountId: "alice@hbl.sbp",
            deviceId: "device-1",
            offlinePublicKey: "offline-public-key",
            appAttestKeyId: "attest-key",
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
        XCTAssertEqual(json["attestation_key_id"] as? String, "attest-key")
        XCTAssertEqual((json["lineage_state"] as? [String: Any])?["lineage_id"] as? String, "lineage-1")
        XCTAssertNil(json["operationId"])
        XCTAssertNil(json["app_attest_key_id"])
        let proof = try XCTUnwrap(json["device_proof"] as? [String: Any])
        XCTAssertEqual(proof["challenge_hash_hex"] as? String, "abc123")
        XCTAssertNil(proof["challengeHashHex"])

        let legacy = """
        {
          "operation_id":"op-refill",
          "account_id":"alice@hbl.sbp",
          "device_id":"device-1",
          "offline_public_key":"offline-public-key",
          "app_attest_key_id":"legacy-attest-key",
          "asset_definition_id":"pkr#sbp",
          "local_revision":3,
          "local_state_hash":"state-3",
          "device_binding":\(try Self.jsonString(Self.binding())),
          "device_proof":\(try Self.jsonString(Self.proof()))
        }
        """
        let decoded = try JSONDecoder().decode(ToriiOfflineKeyRefillRequest.self, from: Data(legacy.utf8))
        XCTAssertEqual(decoded.appAttestKeyId, "legacy-attest-key")
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
        XCTAssertEqual((json["lineage_state"] as? [String: Any])?["lineage_id"] as? String, "lineage-1")
        XCTAssertNil(json["offlinePublicKey"])
        XCTAssertNil(json["lineageState"])
    }

    func testRedeemSettlementRequestEncodesSnakeCaseContract() throws {
        let redemption = try ToriiOfflineRedemptionProof(
            sourceNoteCommitment: Self.hashHex(1),
            inputNullifiers: [Self.hashHex(3)],
            senderKeyCertificate: Self.certificate(),
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

    private static func certificate() -> OfflineCompactKeyCertificate {
        OfflineCompactKeyCertificate(
            platform: "ios-appattest",
            keyId: "attest-key",
            deviceId: "device-1",
            accountId: "alice@hbl.sbp",
            publicKey: Data(repeating: 1, count: 32).base64EncodedString(),
            issuerSignatureBase64: Data(repeating: 2, count: 64).base64EncodedString()
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
