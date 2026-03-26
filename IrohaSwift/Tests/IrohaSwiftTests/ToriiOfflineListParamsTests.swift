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
            controllerId: " soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ ",
            receiverId: "soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ",
            depositAccountId: "  soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ ",
            assetId: " 62Fk4FPcMuLvW5QjDGNF2a4jAmjM "
        )
        let items = try XCTUnwrap(params.queryItems())
        let map = Self.map(from: items)
        XCTAssertEqual(map["controller_id"], "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ")
        XCTAssertEqual(map["receiver_id"], "soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ")
        XCTAssertEqual(map["deposit_account_id"], "soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ")
        XCTAssertEqual(map["asset_id"], "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
    }

    func testOnlyMissingVerdictFlag() throws {
        let params = ToriiOfflineListParams(
            onlyMissingVerdict: true
        )
        let items = try XCTUnwrap(params.queryItems())
        let map = Self.map(from: items)
        XCTAssertEqual(map["only_missing_verdict"], "true")
    }

    func testOfflineRevocationQueryItems() throws {
        let params = ToriiOfflineRevocationListParams(
            filter: "{\"op\":\"eq\",\"args\":[\"reason\",\"device_compromised\"]}",
            limit: 25,
            offset: 5,
            sort: "revoked_at_ms:desc"
        )
        let items = try XCTUnwrap(params.queryItems())
        let map = Self.map(from: items)
        XCTAssertEqual(map["filter"], "{\"op\":\"eq\",\"args\":[\"reason\",\"device_compromised\"]}")
        XCTAssertEqual(map["limit"], "25")
        XCTAssertEqual(map["offset"], "5")
        XCTAssertEqual(map["sort"], "revoked_at_ms:desc")
    }

    func testOfflineRevocationListDecodesMetadata() throws {
        let payload = """
        {
          "items": [{
            "verdict_id_hex": "aa",
            "issuer_id": "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ",
            "issuer_display": "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ",
            "revoked_at_ms": 123,
            "reason": "device_compromised",
            "note": "lost device",
            "metadata": { "ticket": "INC-1" },
            "record": {
              "verdict_id_hex": "aa",
              "issuer_id": "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ",
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
        XCTAssertEqual(entry.issuerId, "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ")
        XCTAssertEqual(entry.reason, "device_compromised")
        XCTAssertEqual(entry.note, "lost device")
        if case let .object(meta)? = entry.metadata {
            XCTAssertEqual(meta["ticket"], .string("INC-1"))
        } else {
            XCTFail("expected metadata object")
        }
    }

    func testOfflineTransferDecodesPlatformSnapshot() throws {
        let payload = """
        {
          "items": [{
            "bundle_id_hex": "aa",
            "controller_id": "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ",
            "controller_display": "soraゴヂアヌャェボヰセキュホュヨモチゥカッパダォレジゴシホセギツキゴヒョヲヌタシャッヱロゥテニョヒシホイヌヘ",
            "receiver_id": "soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ",
            "receiver_display": "soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ",
            "deposit_account_id": "soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ",
            "deposit_account_display": "soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ",
            "asset_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
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

    func testQueryEnvelopeEncodesPaginationAndSort() throws {
        let filter = ToriiJSONValue.object([
            "op": .string("eq"),
            "args": .array([.string("bundle_id_hex"), .string("abc")])
        ])
        let envelope = ToriiQueryEnvelope(
            query: "offline_transfers",
            filter: filter,
            select: ["bundle_id_hex", "status"],
            sort: [ToriiQuerySortKey(key: "recorded_at_ms", order: .desc)],
            pagination: ToriiQueryPagination(limit: 5, offset: 10),
            fetchSize: 50
        )
        let data = try JSONEncoder().encode(envelope)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        XCTAssertEqual(json["query"] as? String, "offline_transfers")
        XCTAssertNotNil(json["filter"] as? [String: Any])
        XCTAssertEqual(json["select"] as? [String], ["bundle_id_hex", "status"])
        XCTAssertEqual((json["sort"] as? [[String: Any]])?.first?["key"] as? String, "recorded_at_ms")
        XCTAssertEqual((json["sort"] as? [[String: Any]])?.first?["order"] as? String, "desc")
        let pagination = json["pagination"] as? [String: Any]
        XCTAssertEqual((pagination?["limit"] as? NSNumber)?.intValue, 5)
        XCTAssertEqual((pagination?["offset"] as? NSNumber)?.intValue, 10)
        XCTAssertEqual((json["fetch_size"] as? NSNumber)?.intValue, 50)
    }

    private static func map(from items: [URLQueryItem]) -> [String: String] {
        var result: [String: String] = [:]
        for item in items {
            result[item.name] = item.value
        }
        return result
    }
}
