import Foundation
import XCTest
@testable import IrohaSwift

/// `/v1/assets/{definition_id}/movements` rows, decoded exactly like the JS SDK's
/// `decodeAssetMovementRow` and the Kotlin `AssetMovementRow`.
final class ToriiAssetMovementRowTests: XCTestCase {
    private let definition = "5HFTUK41MFWnFdeHo5XySHC7PQun"
    private let txHash = "cc9ce14e2c0a848abb93cd5d2ee03a0701ce5b9930f38434e7e8834898b558b5"

    /// A complete row as `routing/asset_movements.rs` writes it; a nil change removes the field.
    private func row(_ changes: [String: String?] = [:]) -> String {
        var fields: [String: String] = [
            "amount": #""0.5""#,
            "asset_definition_id": "\"\(definition)\"",
            "authority": #""payer""#,
            "block_height": "5503",
            "block_index": "0",
            "from_account_id": #""payer""#,
            "id": "\"\(txHash):0\"",
            "kind": #""OFFLINE_LOAD""#,
            "movement_index": "0",
            "result_ok": "true",
            "timestamp_ms": "1793147618051",
            "to_account_id": #""reserve""#,
            "tx_hash": "\"\(txHash)\"",
        ]
        for (name, value) in changes { fields[name] = value }
        return "{" + fields.keys.sorted().map { "\"\($0)\":\(fields[$0]!)" }.joined(separator: ",") + "}"
    }

    private func decode(_ json: String) throws -> ToriiAssetMovementRow {
        try JSONDecoder().decode(ToriiAssetMovementRow.self, from: Data(json.utf8))
    }

    func testOfflineMovementKeepsItsKindExactAmountAndCoordinates() throws {
        let load = try decode(row())
        XCTAssertEqual(load.kind, .offlineLoad)
        XCTAssertTrue(load.kind.isOffline)
        XCTAssertEqual(load.amount, "0.5")
        XCTAssertEqual(load.fromAccountId, "payer")
        XCTAssertEqual(load.toAccountId, "reserve")
        XCTAssertEqual(load.blockHeight, 5503)
        XCTAssertEqual(load.blockIndex, 0)
        XCTAssertEqual(load.movementIndex, 0)
        XCTAssertEqual(load.timestampMs, 1_793_147_618_051)
        XCTAssertEqual(load.id, "\(txHash):0")
        XCTAssertTrue(load.resultOk)
        let mint = try decode(row(["kind": #""MINT""#, "from_account_id": "null", "amount": #""312""#]))
        XCTAssertNil(mint.fromAccountId)
        XCTAssertFalse(mint.kind.isOffline)
        XCTAssertNil(try decode(row(["kind": #""BURN""#, "to_account_id": "null"])).toAccountId)
        let failed = try decode(row(["kind": #""TRANSFER""#, "result_ok": "false", "id": "\"\(txHash):2\"", "movement_index": "2"]))
        XCTAssertFalse(failed.resultOk)
        // Fields Torii adds later are ignored.
        XCTAssertEqual(try decode(row(["note": #""later""#])), load)
    }

    func testEveryKindTheIndexWritesDecodes() throws {
        let kinds = ["TRANSFER", "MINT", "BURN", "OFFLINE_LOAD", "OFFLINE_UNLOAD", "OFFLINE_FEE", "OFFLINE_CHARGE"]
        XCTAssertEqual(ToriiAssetMovementKind.allCases.map(\.rawValue), kinds)
        for kind in kinds {
            let decoded = try decode(row([
                "kind": "\"\(kind)\"",
                "from_account_id": kind == "MINT" ? "null" : #""payer""#,
                "to_account_id": kind == "BURN" ? "null" : #""reserve""#,
            ]))
            XCTAssertEqual(decoded.kind.rawValue, kind)
            XCTAssertEqual(decoded.kind.isOffline, kind.hasPrefix("OFFLINE_"))
        }
    }

    func testMalformedRowsAreRejected() {
        let cases: [[String: String?]] = [
            ["kind": #""SWAP""#],
            ["kind": nil],
            ["amount": #""0.50""#],
            ["amount": #""01""#],
            ["amount": #""-1""#],
            ["amount": #""1e3""#],
            ["amount": #""1.""#],
            ["amount": #"".5""#],
            ["amount": "0.5"],
            ["from_account_id": "null"],
            ["from_account_id": nil],
            ["from_account_id": #""""#],
            ["to_account_id": nil],
            ["kind": #""MINT""#],
            ["kind": #""BURN""#],
            ["tx_hash": "\"\(txHash.uppercased())\"", "id": "\"\(txHash.uppercased()):0\""],
            ["id": "\"\(txHash):1\""],
            ["id": #""other""#],
            ["block_height": "0"],
            ["movement_index": "-1"],
            ["timestamp_ms": nil],
            ["result_ok": nil],
            ["result_ok": "false"],
            ["authority": #""""#],
            ["asset_definition_id": nil],
        ]
        for changes in cases {
            XCTAssertThrowsError(try decode(row(changes)), "\(changes)")
        }
    }

    func testOneAccountReadsItsMovementsWithAFilteredHistoryQuery() async throws {
        let bodies = NSLock()
        var seen: [(path: String, body: String)] = []
        let page = #"{"items":[\#(row())],"next_cursor":"q1_older"}"#
        let baseURL = CollectionQueryStubProtocol.register { request in
            bodies.lock()
            seen.append((request.url?.path ?? "",
                         String(decoding: toriiClientTestBodyData(from: request) ?? Data(), as: UTF8.self)))
            bodies.unlock()
            return (HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: nil,
                                    headerFields: ["Content-Type": "application/json"])!, Data(page.utf8))
        }
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [CollectionQueryStubProtocol.self]
        let torii = ToriiClient(baseURL: baseURL, session: URLSession(configuration: configuration),
                                localSigningContext: ToriiLocalSigningContext(networkId: TestNetworkIds.canonical))
        let movements = torii.assetMovements(of: definition)
        let account = ToriiAssetMovementRow.Fields.fromAccountId == "payer"
            || ToriiAssetMovementRow.Fields.toAccountId == "payer"
        let result = try await movements.page(ToriiListQuery(filter: account, limit: 50))
        XCTAssertEqual(result.items.map(\.kind), [.offlineLoad])
        XCTAssertEqual(result.nextCursor, "q1_older")
        XCTAssertEqual(seen.map(\.path), ["/v1/assets/\(definition)/movements/query"])
        XCTAssertEqual(seen.first?.body,
                       #"{"filter":{"args":[{"args":["from_account_id","payer"],"op":"eq"},"#
                       + #"{"args":["to_account_id","payer"],"op":"eq"}],"op":"or"},"limit":50}"#)
        // A history collection: Torii's order only, refused before anything is sent.
        await XCTAssertThrowsErrorAsync(try await movements.page(ToriiListQuery(sort: [.descending("block_height")])))
        XCTAssertEqual(seen.count, 1)
    }
}
