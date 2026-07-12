import Foundation
import XCTest
@testable import IrohaSwift

final class ToriiAssetTransferTests: XCTestCase {
    private static let authorityKey = try! Keypair(privateKeyBytes: Data(repeating: 0x41, count: 32))
    private static let destinationKey = try! Keypair(privateKeyBytes: Data(repeating: 0x42, count: 32))
    private static let sponsorKey = try! Keypair(privateKeyBytes: Data(repeating: 0x43, count: 32))

    private static let authority = try! authorityKey.accountId()
    private static let destination = try! destinationKey.accountId()
    private static let sponsor = try! sponsorKey.accountId()
    private static let assetDefinitionId: String = {
        var bytes = Data(repeating: 0, count: 16)
        bytes[0] = 0x10
        bytes[6] = 0x40
        bytes[8] = 0x80
        bytes[15] = 0x7F
        return AssetDefinitionAddressCodec.definitionLiteral(uuidBytes: bytes)!
    }()

    private func request(
        scope: String = "dataspace:10",
        amount: String = "1.25",
        memo: String? = "invoice 42",
        creationTimeMs: UInt64 = 1_700_000_000_000,
        transactionTtlMs: UInt64 = 120_000,
        publicKeyHex: String? = nil,
        signatureBase64: String? = nil
    ) -> ToriiAssetTransferRequest {
        ToriiAssetTransferRequest(
            authority: Self.authority,
            assetDefinitionId: Self.assetDefinitionId,
            assetBalanceScope: scope,
            amount: amount,
            destination: Self.destination,
            memo: memo,
            feeSponsor: Self.sponsor,
            creationTimeMs: creationTimeMs,
            transactionTtlMs: transactionTtlMs,
            publicKeyHex: publicKeyHex,
            signatureBase64: signatureBase64
        )
    }

    func testRequestUsesOnlyExactSharpWireFields() throws {
        let encoded = try JSONEncoder().encode(request())
        let object = try XCTUnwrap(
            JSONSerialization.jsonObject(with: encoded) as? [String: Any]
        )
        XCTAssertEqual(
            Set(object.keys),
            Set([
                "authority", "asset_definition_id", "asset_balance_scope", "amount",
                "destination", "memo", "fee_sponsor", "creation_time_ms",
                "transaction_ttl_ms",
            ])
        )
        XCTAssertEqual(object["asset_balance_scope"] as? String, "dataspace:10")
        XCTAssertNil(object["private_key"])
        XCTAssertNil(object["nonce"])
        XCTAssertNil(object["metadata"])
        XCTAssertNil(object["signature_b64"])

        let publicKeyHex = Self.authorityKey.publicKey.map { String(format: "%02x", $0) }.joined()
        let signatureBase64 = Data(repeating: 0x55, count: 64).base64EncodedString()
        let submitObject = try XCTUnwrap(
            JSONSerialization.jsonObject(
                with: try JSONEncoder().encode(
                    request(
                        publicKeyHex: publicKeyHex,
                        signatureBase64: signatureBase64
                    )
                )
            ) as? [String: Any]
        )
        XCTAssertEqual(submitObject["public_key_hex"] as? String, publicKeyHex)
        XCTAssertEqual(submitObject["signature_base64"] as? String, signatureBase64)
        XCTAssertNil(submitObject["signature_b64"])
    }

    func testRequestRejectsNoncanonicalScopesAmountsAndMemos() {
        for scope in [
            "Global", " global", "global ", "dataspace:", "dataspace:01",
            "dataspace:+1", "dataspace:18446744073709551616",
        ] {
            XCTAssertThrowsError(try JSONEncoder().encode(request(scope: scope)), scope)
        }
        for amount in [
            "", "0", "-1", "+1", "01", "1.0", "1.230", "1e0", " 1", "1 ",
            "0.00000000000000000000000000001", String(repeating: "9", count: 200),
        ] {
            XCTAssertThrowsError(try JSONEncoder().encode(request(amount: amount)), amount)
        }
        for memo in [String(), "line\nbreak", String(repeating: "x", count: 257)] {
            XCTAssertThrowsError(try JSONEncoder().encode(request(memo: memo)))
        }
    }

    func testRequestRejectsInvalidTimesAndHalfSigningStates() {
        XCTAssertThrowsError(
            try JSONEncoder().encode(request(creationTimeMs: 0))
        )
        XCTAssertThrowsError(
            try JSONEncoder().encode(request(transactionTtlMs: 0))
        )
        XCTAssertThrowsError(
            try JSONEncoder().encode(
                request(
                    transactionTtlMs: ToriiAssetTransferRequest.maximumTransactionTtlMs + 1
                )
            )
        )
        let publicKeyHex = Self.authorityKey.publicKey.map { String(format: "%02x", $0) }.joined()
        XCTAssertThrowsError(
            try JSONEncoder().encode(request(publicKeyHex: publicKeyHex))
        )
        XCTAssertThrowsError(
            try JSONEncoder().encode(
                request(signatureBase64: Data(repeating: 0x55, count: 64).base64EncodedString())
            )
        )
        XCTAssertThrowsError(
            try JSONEncoder().encode(
                request(
                    publicKeyHex: publicKeyHex.uppercased(),
                    signatureBase64: Data(repeating: 0x55, count: 64).base64EncodedString()
                )
            )
        )
        XCTAssertThrowsError(
            try JSONEncoder().encode(
                request(publicKeyHex: publicKeyHex, signatureBase64: "not-base64")
            )
        )
    }

    func testResponseModelsRejectUnknownAndNoncanonicalFields() throws {
        let valid = responseObject()
        let decoded = try JSONDecoder().decode(
            ToriiAssetTransferResponse.self,
            from: try JSONSerialization.data(withJSONObject: valid)
        )
        XCTAssertFalse(decoded.submitted)
        XCTAssertEqual(decoded.intent.assetBalanceScope, "dataspace:10")
        XCTAssertEqual(decoded.signingPayload?.algorithm, "ed25519")
        XCTAssertEqual(decoded.signingPayload?.payloadBase64, hashBytes().base64EncodedString())
        XCTAssertEqual(decoded.placeholderTransactionHashHex, hashHex(0x22))
        XCTAssertEqual(decoded.placeholderEntrypointHashHex, hashHex(0x22))
        XCTAssertNil(decoded.transactionHashHex)

        var topLevelUnknown = valid
        topLevelUnknown["signed_transaction_base64"] = "AQ=="
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiAssetTransferResponse.self,
                from: try JSONSerialization.data(withJSONObject: topLevelUnknown)
            )
        )

        var intentUnknown = valid
        var intent = intentUnknown["intent"] as! [String: Any]
        intent["asset_id"] = Self.assetDefinitionId
        intentUnknown["intent"] = intent
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiAssetTransferResponse.self,
                from: try JSONSerialization.data(withJSONObject: intentUnknown)
            )
        )

        var legacySignature = valid
        var signing = legacySignature["signing_payload"] as! [String: Any]
        signing["signature_b64"] = "AQ=="
        legacySignature["signing_payload"] = signing
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiAssetTransferResponse.self,
                from: try JSONSerialization.data(withJSONObject: legacySignature)
            )
        )

        var uppercaseHash = valid
        uppercaseHash["placeholder_transaction_hash_hex"] = hashHex(0xAB).uppercased()
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiAssetTransferResponse.self,
                from: try JSONSerialization.data(withJSONObject: uppercaseHash)
            )
        )

        var wrongAlgorithm = valid
        signing = wrongAlgorithm["signing_payload"] as! [String: Any]
        signing["algorithm"] = "secp256k1"
        wrongAlgorithm["signing_payload"] = signing
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiAssetTransferResponse.self,
                from: try JSONSerialization.data(withJSONObject: wrongAlgorithm)
            )
        )
    }

    private func responseObject() -> [String: Any] {
        let intent: [String: Any] = [
            "chain_id": "asset-transfer-test",
            "authority": Self.authority,
            "asset_definition_id": Self.assetDefinitionId,
            "asset_balance_scope": "dataspace:10",
            "amount": "1.25",
            "destination": Self.destination,
            "memo": "invoice 42",
            "fee_sponsor": Self.sponsor,
            "creation_time_ms": 1_700_000_000_000 as UInt64,
            "transaction_ttl_ms": 120_000 as UInt64,
        ]
        let receipt: [String: Any] = [
            "operation_kind": "asset_transfer",
            "status": "pending_signature",
            "transport": "torii",
            "intent": intent,
            "payload_signing_hash_hex": hashHex(0x11),
            "placeholder_transaction_hash_hex": hashHex(0x22),
            "placeholder_entrypoint_hash_hex": hashHex(0x22),
        ]
        return [
            "ok": true,
            "submitted": false,
            "intent": intent,
            "signing_payload": [
                "payload_base64": hashBytes().base64EncodedString(),
                "algorithm": "ed25519",
            ],
            "transaction_scaffold_base64": Data([1]).base64EncodedString(),
            "placeholder_transaction_hash_hex": hashHex(0x22),
            "placeholder_entrypoint_hash_hex": hashHex(0x22),
            "receipt": receipt,
        ]
    }

    private func hashBytes() -> Data {
        Data(repeating: 0x11, count: 32)
    }

    private func hashHex(_ byte: UInt8) -> String {
        Data(repeating: byte, count: 32).map { String(format: "%02x", $0) }.joined()
    }
}
