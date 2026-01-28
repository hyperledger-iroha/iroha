import XCTest
import IrohaSwift

final class ToriiOfflineListParamsTests: XCTestCase {

    func testQueryItemsIncludeConvenienceFilters() throws {
        let params = ToriiOfflineListParams(
            limit: 5,
            offset: 10,
            certificateExpiresBeforeMs: 1_000,
            certificateExpiresAfterMs: 100,
            policyExpiresBeforeMs: 2_500,
            policyExpiresAfterMs: 200,
            verdictIdHex: "DEADBEEF",
            platformPolicy: .playIntegrity,
            requireVerdict: true,
            onlyMissingVerdict: false
        )
        let items = try XCTUnwrap(params.queryItems())
        let map = Self.map(from: items)
        XCTAssertEqual(map["limit"], "5")
        XCTAssertEqual(map["offset"], "10")
        XCTAssertEqual(map["certificate_expires_before_ms"], "1000")
        XCTAssertEqual(map["certificate_expires_after_ms"], "100")
        XCTAssertEqual(map["policy_expires_before_ms"], "2500")
        XCTAssertEqual(map["policy_expires_after_ms"], "200")
        XCTAssertEqual(map["verdict_id_hex"], "deadbeef")
        XCTAssertEqual(map["platform_policy"], ToriiPlatformPolicy.playIntegrity.rawValue)
        XCTAssertEqual(map["require_verdict"], "true")
        XCTAssertNil(map["only_missing_verdict"])
    }

    func testAccountFiltersAreIncluded() throws {
        let params = ToriiOfflineListParams(
            controllerId: " alice@wonderland ",
            receiverId: "bob@wonderland",
            depositAccountId: "  carol@wonderland "
        )
        let items = try XCTUnwrap(params.queryItems())
        let map = Self.map(from: items)
        XCTAssertEqual(map["controller_id"], "alice@wonderland")
        XCTAssertEqual(map["receiver_id"], "bob@wonderland")
        XCTAssertEqual(map["deposit_account_id"], "carol@wonderland")
    }

    func testOnlyMissingVerdictFlag() throws {
        let params = ToriiOfflineListParams(
            onlyMissingVerdict: true
        )
        let items = try XCTUnwrap(params.queryItems())
        let map = Self.map(from: items)
        XCTAssertEqual(map["only_missing_verdict"], "true")
    }

    func testAddressFormatAliasRejection() {
        let params = ToriiOfflineListParams(addressFormat: "  SoRa ")
        XCTAssertThrowsError(try params.queryItems()) { error in
            guard case ToriiClientError.invalidPayload = error else {
                return XCTFail("Expected invalidPayload, got \(error)")
            }
        }
    }

    func testOfflineRevocationQueryItemsIncludeAddressFormat() throws {
        let params = ToriiOfflineRevocationListParams(
            filter: "{\"op\":\"eq\",\"args\":[\"reason\",\"device_compromised\"]}",
            limit: 25,
            offset: 5,
            sort: "revoked_at_ms:desc",
            addressFormat: "ih58"
        )
        let items = try XCTUnwrap(params.queryItems())
        let map = Self.map(from: items)
        XCTAssertEqual(map["filter"], "{\"op\":\"eq\",\"args\":[\"reason\",\"device_compromised\"]}")
        XCTAssertEqual(map["limit"], "25")
        XCTAssertEqual(map["offset"], "5")
        XCTAssertEqual(map["sort"], "revoked_at_ms:desc")
        XCTAssertEqual(map["address_format"], "ih58")
    }

    func testOfflineRevocationListDecodesMetadata() throws {
        let payload = """
        {
          "items": [{
            "verdict_id_hex": "aa",
            "issuer_id": "operator@test",
            "issuer_display": "operator@test",
            "revoked_at_ms": 123,
            "reason": "device_compromised",
            "note": "lost device",
            "metadata": { "ticket": "INC-1" },
            "record": {
              "verdict_id_hex": "aa",
              "issuer_id": "operator@test",
              "revoked_at_ms": 123,
              "reason": "device_compromised",
              "note": "lost device",
              "metadata": { "ticket": "INC-1" }
            }
          }],
          "total": 1
        }
        """.data(using: .utf8)!
        let list = try JSONDecoder().decode(ToriiOfflineRevocationList.self, from: payload)
        XCTAssertEqual(list.total, 1)
        guard let entry = list.items.first else {
            return XCTFail("expected revocation entry")
        }
        XCTAssertEqual(entry.verdictIdHex, "aa")
        XCTAssertEqual(entry.issuerId, "operator@test")
        XCTAssertEqual(entry.reason, "device_compromised")
        XCTAssertEqual(entry.note, "lost device")
        if case let .object(meta)? = entry.metadata {
            XCTAssertEqual(meta["ticket"], .string("INC-1"))
        } else {
            XCTFail("expected metadata object")
        }
    }

    func testOfflineBundleProofStatusParamsIncludeCanonicalFormat() throws {
        let params = ToriiOfflineBundleProofStatusParams(bundleIdHex: "0xDEADBEEF", addressFormat: "compressed")
        let items = try params.queryItems()
        let map = Self.map(from: items)
        XCTAssertEqual(map["bundle_id_hex"], "deadbeef")
        XCTAssertEqual(map["address_format"], "compressed")
    }

    func testOfflineBundleProofStatusParamsRejectsInvalidHex() {
        let params = ToriiOfflineBundleProofStatusParams(bundleIdHex: "not-hex")
        XCTAssertThrowsError(try params.queryItems())
    }

    func testOfflineBundleProofStatusDecodesTypedFields() throws {
        let payload = """
        {
          "proof_status": "fresh",
          "receipts_root_hex": "abcd",
          "aggregate_proof_root_hex": "dcba",
          "receipts_root_matches": true,
          "proof_summary": { "version": 1 }
        }
        """.data(using: .utf8)!
        let status = try JSONDecoder().decode(ToriiOfflineBundleProofStatus.self, from: payload)
        XCTAssertEqual(status.proofStatus, "fresh")
        XCTAssertEqual(status.receiptsRootHex, "abcd")
        XCTAssertEqual(status.aggregateProofRootHex, "dcba")
        XCTAssertEqual(status.receiptsRootMatches, true)
        XCTAssertEqual(status.proofSummary?["version"], .number(1))
        let typed = try status.decodeProofSummary()
        XCTAssertEqual(typed?.version, 1)
        XCTAssertEqual(typed?.proofSumBytes, nil)
        XCTAssertEqual(typed?.metadataKeys, nil)
    }

    func testAddressFormatValidationFailure() {
        let params = ToriiOfflineListParams(addressFormat: "canonical-hex")
        XCTAssertThrowsError(try params.queryItems()) { error in
            guard case ToriiClientError.invalidPayload(let message) = error else {
                return XCTFail("Expected invalidPayload, got \(error)")
            }
            XCTAssertTrue(message.contains("addressFormat"), "message: \(message)")
        }
    }

    func testOfflineTransferDecodesPlatformSnapshot() throws {
        let payload = """
        {
          "items": [{
            "bundle_id_hex": "aa",
            "controller_id": "alice@wonderland",
            "controller_display": "alice@wonderland",
            "receiver_id": "bob@wonderland",
            "receiver_display": "bob@wonderland",
            "deposit_account_id": "carol@wonderland",
            "deposit_account_display": "carol@wonderland",
            "asset_id": "rose#wonderland",
            "receipt_count": 1,
            "total_amount": "10",
            "claimed_delta": "10",
            "status": "Settled",
            "recorded_at_ms": 123,
            "recorded_at_height": 456,
            "transfer": {"receipts": []},
            "platform_policy": "play_integrity",
            "platform_token_snapshot": {
              "policy": "play_integrity",
              "attestation_jws_b64": "dG9rZW4="
            }
          }],
          "total": 1
        }
        """.data(using: .utf8)!
        let decoder = JSONDecoder()
        let list = try decoder.decode(ToriiOfflineTransferList.self, from: payload)
        XCTAssertEqual(list.total, 1)
        guard let item = list.items.first else {
            XCTFail("expected offline transfer item")
            return
        }
        XCTAssertEqual(item.platformPolicy, .playIntegrity)
        XCTAssertEqual(item.platformTokenSnapshot?.policy, .playIntegrity)
        XCTAssertEqual(item.platformTokenSnapshot?.attestationJwsB64, "dG9rZW4=")
    }

    func testOfflineReceiptListParamsQueryItems() throws {
        let params = ToriiOfflineReceiptListParams(
            filter: "{\"op\":\"eq\",\"args\":[\"asset_id\",\"rose#wonderland\"]}",
            limit: 20,
            offset: 5,
            sort: "recorded_at_ms:desc",
            addressFormat: "compressed",
            controllerId: " alice@wonderland ",
            receiverId: "bob@wonderland",
            bundleIdHex: "0xDEADBEEF",
            certificateIdHex: "AAFF",
            invoiceId: "inv-1",
            assetId: "rose#wonderland"
        )
        let items = try XCTUnwrap(params.queryItems())
        let map = Self.map(from: items)
        XCTAssertEqual(map["filter"], "{\"op\":\"eq\",\"args\":[\"asset_id\",\"rose#wonderland\"]}")
        XCTAssertEqual(map["limit"], "20")
        XCTAssertEqual(map["offset"], "5")
        XCTAssertEqual(map["sort"], "recorded_at_ms:desc")
        XCTAssertEqual(map["address_format"], "compressed")
        XCTAssertEqual(map["controller_id"], "alice@wonderland")
        XCTAssertEqual(map["receiver_id"], "bob@wonderland")
        XCTAssertEqual(map["bundle_id_hex"], "deadbeef")
        XCTAssertEqual(map["certificate_id_hex"], "aaff")
        XCTAssertEqual(map["invoice_id"], "inv-1")
        XCTAssertEqual(map["asset_id"], "rose#wonderland")
    }

    func testQueryEnvelopeEncodesPaginationAndSort() throws {
        let filter = ToriiJSONValue.object([
            "op": .string("eq"),
            "args": .array([.string("bundle_id_hex"), .string("abc")])
        ])
        let envelope = ToriiQueryEnvelope(
            query: "offline_receipts",
            filter: filter,
            select: ["bundle_id_hex", "tx_id_hex"],
            sort: [ToriiQuerySortKey(key: "recorded_at_ms", order: .desc)],
            pagination: ToriiQueryPagination(limit: 5, offset: 10),
            fetchSize: 50,
            addressFormat: "ih58"
        )
        let data = try JSONEncoder().encode(envelope)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        XCTAssertEqual(json["query"] as? String, "offline_receipts")
        XCTAssertNotNil(json["filter"] as? [String: Any])
        XCTAssertEqual(json["select"] as? [String], ["bundle_id_hex", "tx_id_hex"])
        XCTAssertEqual((json["sort"] as? [[String: Any]])?.first?["key"] as? String, "recorded_at_ms")
        XCTAssertEqual((json["sort"] as? [[String: Any]])?.first?["order"] as? String, "desc")
        let pagination = json["pagination"] as? [String: Any]
        XCTAssertEqual((pagination?["limit"] as? NSNumber)?.intValue, 5)
        XCTAssertEqual((pagination?["offset"] as? NSNumber)?.intValue, 10)
        XCTAssertEqual((json["fetch_size"] as? NSNumber)?.intValue, 50)
        XCTAssertEqual(json["address_format"] as? String, "ih58")
    }

    private static func map(from items: [URLQueryItem]) -> [String: String] {
        var result: [String: String] = [:]
        for item in items {
            result[item.name] = item.value
        }
        return result
    }
}
