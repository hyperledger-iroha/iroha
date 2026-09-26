import Foundation
import XCTest
@testable import IrohaSwift

final class ToriiExplorerParsingTests: XCTestCase {
    func testExplorerHistoryParamsRejectInvalidCursorAndLimit() {
        let invalidCursors = [
            "",
            "padded=",
            "a",
            "contains space",
            String(repeating: "A", count: 1_425),
        ]
        for cursor in invalidCursors {
            XCTAssertThrowsError(
                try ToriiExplorerInstructionsParams(cursor: cursor).queryItems(),
                "instructions cursor \(cursor.prefix(16)) should be rejected"
            )
            XCTAssertThrowsError(
                try ToriiExplorerTransactionsParams(cursor: cursor).queryItems(),
                "transactions cursor \(cursor.prefix(16)) should be rejected"
            )
        }
        for limit: UInt32 in [0, 101] {
            XCTAssertThrowsError(try ToriiExplorerInstructionsParams(limit: limit).queryItems())
            XCTAssertThrowsError(try ToriiExplorerTransactionsParams(limit: limit).queryItems())
        }
    }

    func testExplorerHistoryCursorMetaDecodesSnapshotContract() throws {
        let populated = """
        {
          "limit":25,
          "snapshot_height":5,
          "snapshot_hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
          "next_cursor":"Y3Vyc29y",
          "has_more":true
        }
        """
        let page = try JSONDecoder().decode(
            ToriiExplorerHistoryCursorMeta.self,
            from: Data(populated.utf8)
        )
        XCTAssertEqual(page.limit, 25)
        XCTAssertEqual(page.snapshotHeight, 5)
        XCTAssertEqual(page.snapshotHash, String(repeating: "a", count: 64))
        XCTAssertEqual(page.nextCursor, "Y3Vyc29y")
        XCTAssertTrue(page.hasMore)

        let empty = """
        {"limit":10,"snapshot_height":0,"snapshot_hash":null,"next_cursor":null,"has_more":false}
        """
        let emptyPage = try JSONDecoder().decode(
            ToriiExplorerHistoryCursorMeta.self,
            from: Data(empty.utf8)
        )
        XCTAssertEqual(emptyPage.snapshotHeight, 0)
        XCTAssertNil(emptyPage.snapshotHash)
        XCTAssertNil(emptyPage.nextCursor)
        XCTAssertFalse(emptyPage.hasMore)
    }

    func testExplorerHistoryCursorMetaRejectsRetiredUnknownAndInconsistentFields() {
        let invalidPayloads = [
            #"{"page":1,"per_page":25,"total_pages":1,"total_items":0}"#,
            #"{"limit":25,"snapshot_height":5,"next_cursor":null,"has_more":false}"#,
            #"{"limit":25,"snapshot_height":5,"snapshot_hash":null,"next_cursor":null,"has_more":false}"#,
            #"{"limit":25,"snapshot_height":0,"snapshot_hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","next_cursor":null,"has_more":false}"#,
            #"{"limit":25,"snapshot_height":5,"snapshot_hash":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","next_cursor":null,"has_more":false}"#,
            #"{"limit":25,"snapshot_height":5,"snapshot_hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","has_more":false}"#,
            #"{"limit":25,"snapshot_height":5,"snapshot_hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","next_cursor":"padded=","has_more":true}"#,
            #"{"limit":25,"snapshot_height":5,"snapshot_hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","next_cursor":null,"has_more":true}"#,
            #"{"limit":25,"snapshot_height":5,"snapshot_hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","next_cursor":null,"has_more":false,"total_items":0}"#,
        ]
        for payload in invalidPayloads {
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiExplorerHistoryCursorMeta.self,
                    from: Data(payload.utf8)
                ),
                "history pagination should reject \(payload)"
            )
        }
    }

    func testExplorerHistoryPagesRejectUnknownFieldsAndOversizedItems() {
        let unknownOuter = """
        {
          "pagination":{"limit":1,"snapshot_height":0,"snapshot_hash":null,"next_cursor":null,"has_more":false},
          "items":[],
          "total_items":0
        }
        """
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiExplorerInstructionsPage.self,
                from: Data(unknownOuter.utf8)
            )
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiExplorerTransactionsPage.self,
                from: Data(unknownOuter.utf8)
            )
        )

        let oversized = """
        {
          "pagination":{"limit":1,"snapshot_height":5,"snapshot_hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","next_cursor":null,"has_more":false},
          "items":[
            {"authority":"alice","hash":"one","block":5,"created_at":"2025-01-01T00:00:00Z","executable":"Instructions","status":"Committed"},
            {"authority":"alice","hash":"two","block":5,"created_at":"2025-01-01T00:00:01Z","executable":"Instructions","status":"Committed"}
          ]
        }
        """
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiExplorerTransactionsPage.self,
                from: Data(oversized.utf8)
            )
        )
    }

    func testCanonicalQuerySelectorsRejectSurroundingWhitespace() {
        let accountId = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
        let assetId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
        let cases: [(String, () throws -> Void)] = [
            (
                "explorer instructions account",
                { _ = try ToriiExplorerInstructionsParams(account: " \(accountId)").queryItems() }
            ),
            (
                "explorer instructions authority",
                { _ = try ToriiExplorerInstructionsParams(authority: "\(accountId) ").queryItems() }
            ),
            (
                "explorer instructions asset",
                { _ = try ToriiExplorerInstructionsParams(assetDefinitionId: " \(assetId)").queryItems() }
            ),
            (
                "explorer transactions authority",
                { _ = try ToriiExplorerTransactionsParams(authority: "\(accountId) ").queryItems() }
            ),
            (
                "explorer transactions asset",
                { _ = try ToriiExplorerTransactionsParams(assetDefinitionId: "\(assetId) ").queryItems() }
            ),
            (
                "explorer rwas owner",
                { _ = try ToriiExplorerRwasParams(ownedBy: " \(accountId)").queryItems() }
            ),
            (
                "contract activity authority",
                { _ = try ToriiContractActivityParams(authority: "\(accountId) ").queryItems() }
            ),
            (
                "contract activity address",
                { _ = try ToriiContractActivityParams(contractAddress: " cntr:deadbeef").queryItems() }
            ),
            (
                "contract activity alias",
                { _ = try ToriiContractActivityParams(contractAlias: "benefits::paynet ").queryItems() }
            ),
            (
                "contract event authority",
                { _ = try ToriiContractEventParams(authority: " \(accountId)").queryItems() }
            ),
            (
                "contract event address",
                { _ = try ToriiContractEventParams(contractAddress: "cntr:deadbeef ").queryItems() }
            ),
            (
                "contract event alias",
                { _ = try ToriiContractEventParams(contractAlias: " benefits::paynet").queryItems() }
            ),
            (
                "contract event participant",
                { _ = try ToriiContractEventParams(participant: "merchant@paynet ").queryItems() }
            ),
            (
                "contract event asset",
                { _ = try ToriiContractEventParams(assetId: " \(assetId)").queryItems() }
            ),
            (
                "subscription owner",
                { _ = try ToriiSubscriptionListParams(ownedBy: "\(accountId) ").queryItems() }
            ),
            (
                "uaid portfolio asset",
                { _ = try ToriiUaidPortfolioQuery(assetId: " \(assetId)").queryItems() }
            ),
        ]

        for (label, action) in cases {
            XCTAssertThrowsError(try action(), label) { error in
                guard case let ToriiClientError.invalidPayload(reason) = error else {
                    return XCTFail("Expected invalidPayload for \(label), got \(error)")
                }
                XCTAssertTrue(
                    reason.contains("surrounding whitespace"),
                    "Expected whitespace diagnostic for \(label), got \(reason)"
                )
            }
        }
    }

    func testExplorerTransferDetailsParsesAsset() throws {
        let json = """
        {
            "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            "created_at":"2025-01-01T00:00:00Z",
            "kind":"Transfer",
            "box":{
                "encoded":"0x00","framed_sha256":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "json":{
                    "kind":"Transfer",
                    "payload":{
                        "variant":"Asset",
                        "value":{
                            "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                            "object":"10",
                            "destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                        }
                    },
                    "wire_id":"10",
                    "encoded":"beef"
                }
            },
            "transaction_hash":"hash",
            "transaction_status":"Committed",
            "block":1,
            "index":0
        }
        """
        let item = try JSONDecoder().decode(ToriiExplorerInstructionItem.self, from: Data(json.utf8))
        guard let details = item.transferDetails() else {
            return XCTFail("Expected transfer details.")
        }
        switch details {
        case .asset(let asset):
            XCTAssertEqual(asset.destinationAccountId, "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
            XCTAssertEqual(asset.amount, "10")
            XCTAssertEqual(asset.senderAccountId, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
            XCTAssertEqual(asset.assetDefinitionId, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
            XCTAssertNil(details.role(for: "sorauﾛ1PgﾉﾀXﾖnWｱﾊｷﾕﾈjｷZﾖrﾅxｲWﾔﾀﾘYヰﾍxｺﾀﾃﾛｽfﾖ2Gｲ8P3LSM"))
            XCTAssertEqual(details.role(for: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), .sender)
            XCTAssertEqual(details.role(for: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"), .receiver)
            XCTAssertTrue(details.involvesAccount("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"))
            XCTAssertTrue(details.involvesAssetDefinition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"))
            XCTAssertFalse(details.involvesAssetDefinition("61CtjvNd9T3THAR65GsMVHr82Bjc"))
        case .assetBatch:
            XCTFail("Expected asset transfer details.")
        }
    }

    func testExplorerTransferDetailsParsesAssetBatch() throws {
        let json = """
        {
            "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            "created_at":"2025-01-01T00:00:00Z",
            "kind":"Transfer",
            "box":{
                "encoded":"0x00","framed_sha256":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "json":{
                    "kind":"Transfer",
                    "payload":{
                        "variant":"AssetBatch",
                        "value":{
                            "entries":[
                                {
                                    "from":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                                    "to":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                    "asset_definition":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                    "amount":"5"
                                },
                                {
                                    "from":"sorauﾛ1Pﾀﾚｿ1ﾍｶsFｲAfｾeB3ｽヱヱｳcyﾊyｹ1ﾂﾈヰヰ6ﾛヰEAﾃｱｳﾖLPN4XM",
                                    "to":"sorauﾛ1PﾜKNﾗ7ｼｺa2WｸｼﾒﾐQﾎbｺﾄocﾆﾁヰJaｱbg6sｾgｲﾖPfX7WAWRY",
                                    "asset_definition":"61CtjvNd9T3THAR65GsMVHr82Bjc",
                                    "amount":"2"
                                }
                            ]
                        }
                    },
                    "wire_id":"10",
                    "encoded":"beef"
                }
            },
            "transaction_hash":"hash",
            "transaction_status":"Committed",
            "block":1,
            "index":0
        }
        """
        let item = try JSONDecoder().decode(ToriiExplorerInstructionItem.self, from: Data(json.utf8))
        guard let details = item.transferDetails() else {
            return XCTFail("Expected transfer details.")
        }
        switch details {
        case .asset:
            XCTFail("Expected batch transfer details.")
        case .assetBatch(let entries):
            XCTAssertEqual(entries.count, 2)
            XCTAssertEqual(entries[0].senderAccountId, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
            XCTAssertEqual(entries[0].receiverAccountId, "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
            XCTAssertEqual(entries[0].assetDefinitionId, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
            XCTAssertEqual(entries[0].amount, "5")
            XCTAssertEqual(entries[1].senderAccountId, "sorauﾛ1Pﾀﾚｿ1ﾍｶsFｲAfｾeB3ｽヱヱｳcyﾊyｹ1ﾂﾈヰヰ6ﾛヰEAﾃｱｳﾖLPN4XM")
            XCTAssertEqual(entries[1].receiverAccountId, "sorauﾛ1PﾜKNﾗ7ｼｺa2WｸｼﾒﾐQﾎbｺﾄocﾆﾁヰJaｱbg6sｾgｲﾖPfX7WAWRY")
            XCTAssertEqual(entries[1].assetDefinitionId, "61CtjvNd9T3THAR65GsMVHr82Bjc")
            XCTAssertEqual(entries[1].amount, "2")
            XCTAssertEqual(details.role(for: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), .sender)
            XCTAssertEqual(details.role(for: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"), .receiver)
            XCTAssertTrue(details.involvesAccount("sorauﾛ1PﾜKNﾗ7ｼｺa2WｸｼﾒﾐQﾎbｺﾄocﾆﾁヰJaｱbg6sｾgｲﾖPfX7WAWRY"))
            XCTAssertTrue(details.involvesAssetDefinition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"))
            XCTAssertTrue(details.involvesAssetDefinition("61CtjvNd9T3THAR65GsMVHr82Bjc"))
            XCTAssertFalse(details.involvesAssetDefinition("5ywNgSPQ5KyuQh7SwaZmwMW4GTXu"))
        }
    }

    func testExplorerTransferRecordsFiltersByAccountAndAssetDefinition() throws {
        let json = """
        {
            "pagination": {"limit":10,"snapshot_height":10,"snapshot_hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","next_cursor":null,"has_more":false},
            "items": [
                {
                    "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "box":{
                        "encoded":"0x00","framed_sha256":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                                    "object":"10",
                                    "destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                                }
                            },
                            "wire_id":"10",
                            "encoded":"beef"
                        }
                    },
                    "transaction_hash":"hash1",
                    "transaction_status":"Committed",
                    "block":1,
                    "index":0
                },
                {
                    "authority":"sorauﾛ1Pﾀﾚｿ1ﾍｶsFｲAfｾeB3ｽヱヱｳcyﾊyｹ1ﾂﾈヰヰ6ﾛヰEAﾃｱｳﾖLPN4XM",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "box":{
                        "encoded":"0x00","framed_sha256":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"AssetBatch",
                                "value":{
                                    "entries":[
                                        {
                                            "from":"sorauﾛ1Pﾀﾚｿ1ﾍｶsFｲAfｾeB3ｽヱヱｳcyﾊyｹ1ﾂﾈヰヰ6ﾛヰEAﾃｱｳﾖLPN4XM",
                                            "to":"sorauﾛ1PﾜKNﾗ7ｼｺa2WｸｼﾒﾐQﾎbｺﾄocﾆﾁヰJaｱbg6sｾgｲﾖPfX7WAWRY",
                                            "asset_definition":"61CtjvNd9T3THAR65GsMVHr82Bjc",
                                            "amount":"2"
                                        }
                                    ]
                                }
                            },
                            "wire_id":"10",
                            "encoded":"beef"
                        }
                    },
                    "transaction_hash":"hash2",
                    "transaction_status":"Committed",
                    "block":1,
                    "index":1
                }
            ]
        }
        """
        let page = try JSONDecoder().decode(ToriiExplorerInstructionsPage.self, from: Data(json.utf8))
        XCTAssertEqual(page.transferRecords().count, 2)
        XCTAssertEqual(page.transferRecords(matchingAccount: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D").count, 1)
        XCTAssertEqual(page.transferRecords(matchingAccount: "sorauﾛ1Pﾀﾚｿ1ﾍｶsFｲAfｾeB3ｽヱヱｳcyﾊyｹ1ﾂﾈヰヰ6ﾛヰEAﾃｱｳﾖLPN4XM").count, 1)
        XCTAssertEqual(page.transferRecords(assetDefinitionId: "61CtjvNd9T3THAR65GsMVHr82Bjc").count, 1)
        XCTAssertEqual(page.transferRecords(assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM").count, 1)
        XCTAssertEqual(page.transferRecords(matchingAccount: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                            assetDefinitionId: "61CtjvNd9T3THAR65GsMVHr82Bjc").count, 0)
    }

    func testExplorerTransferSummariesDeriveDirection() throws {
        let json = """
        {
            "pagination": {"limit":10,"snapshot_height":10,"snapshot_hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","next_cursor":null,"has_more":false},
            "items": [
                {
                    "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "box":{
                        "encoded":"0x00","framed_sha256":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                                    "object":"10",
                                    "destination":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
                                }
                            },
                            "wire_id":"10",
                            "encoded":"beef"
                        }
                    },
                    "transaction_hash":"hash1",
                    "transaction_status":"Committed",
                    "block":1,
                    "index":0
                }
            ]
        }
        """
        let page = try JSONDecoder().decode(ToriiExplorerInstructionsPage.self, from: Data(json.utf8))
        let summaries = page.transferSummaries(matchingAccount: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
        XCTAssertEqual(summaries.count, 1)
        let summary = summaries[0]
        XCTAssertEqual(summary.direction, .incoming)
        XCTAssertEqual(summary.senderAccountId, "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
        XCTAssertEqual(summary.receiverAccountId, "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
        XCTAssertEqual(summary.assetDefinitionId, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM")
        XCTAssertEqual(summary.amount, "10")
        XCTAssertTrue(summary.isIncoming)
        XCTAssertFalse(summary.isOutgoing)
        XCTAssertFalse(summary.isSelfTransfer)
        XCTAssertEqual(summary.transferIndex, 0)
        XCTAssertEqual(summary.id, "hash1|0|0")
        XCTAssertEqual(summary.direction(relativeTo: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"), .incoming)
        XCTAssertEqual(summary.counterpartyAccountId(relativeTo: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"), "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
        XCTAssertTrue(summary.isIncoming(relativeTo: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"))
        XCTAssertFalse(summary.isOutgoing(relativeTo: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"))
        XCTAssertFalse(summary.isSelfTransfer(relativeTo: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"))
        XCTAssertEqual(summary.signedAmount(relativeTo: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"), "+10")
        XCTAssertEqual(summary.signedAmount(relativeTo: "sorauﾛ1PgﾉﾀXﾖnWｱﾊｷﾕﾈjｷZﾖrﾅxｲWﾔﾀﾘYヰﾍxｺﾀﾃﾛｽfﾖ2Gｲ8P3LSM"), "10")
    }

    func testExplorerTransferSummariesDeriveSelfTransfer() throws {
        let json = """
        {
            "pagination": {"limit":10,"snapshot_height":10,"snapshot_hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","next_cursor":null,"has_more":false},
            "items": [
                {
                    "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "box":{
                        "encoded":"0x00","framed_sha256":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"Asset",
                                "value":{
                                    "source":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                                    "object":"10",
                                    "destination":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
                                }
                            },
                            "wire_id":"10",
                            "encoded":"beef"
                        }
                    },
                    "transaction_hash":"hash1",
                    "transaction_status":"Committed",
                    "block":1,
                    "index":0
                }
            ]
        }
        """
        let page = try JSONDecoder().decode(ToriiExplorerInstructionsPage.self, from: Data(json.utf8))
        let summaries = page.transferSummaries(matchingAccount: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
        XCTAssertEqual(summaries.count, 1)
        let summary = summaries[0]
        XCTAssertEqual(summary.direction, .selfTransfer)
        XCTAssertTrue(summary.isSelfTransfer)
        XCTAssertFalse(summary.isIncoming)
        XCTAssertFalse(summary.isOutgoing)
        XCTAssertEqual(summary.direction(relativeTo: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), .selfTransfer)
        XCTAssertEqual(summary.counterpartyAccountId(relativeTo: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
        XCTAssertNil(summary.counterpartyAccountId(relativeTo: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"))
        XCTAssertTrue(summary.isSelfTransfer(relativeTo: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"))
        XCTAssertFalse(summary.isIncoming(relativeTo: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"))
        XCTAssertFalse(summary.isOutgoing(relativeTo: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"))
        XCTAssertEqual(summary.signedAmount(relativeTo: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), "10")
    }

    func testTransferSummarySignedAmountPreservesExistingSign() {
        let outgoing = ToriiExplorerTransferSummary(transactionHash: "hash1",
                                                    block: 1,
                                                    createdAt: "2025-01-01T00:00:00Z",
                                                    status: "Committed",
                                                    authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                                                    instructionIndex: 0,
                                                    senderAccountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                                                    receiverAccountId: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                                    assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                                    amount: "-10",
                                                    direction: .outgoing,
                                                    kind: "Transfer",
                                                    transferIndex: 0)
        XCTAssertEqual(outgoing.signedAmount(relativeTo: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"), "-10")

        let incoming = ToriiExplorerTransferSummary(transactionHash: "hash2",
                                                    block: 1,
                                                    createdAt: "2025-01-01T00:00:00Z",
                                                    status: "Committed",
                                                    authority: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                                                    instructionIndex: 0,
                                                    senderAccountId: "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                                                    receiverAccountId: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                                    assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                                    amount: "+10",
                                                    direction: .incoming,
                                                    kind: "Transfer",
                                                    transferIndex: 0)
        XCTAssertEqual(incoming.signedAmount(relativeTo: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"), "+10")
    }

    func testExplorerTransferSummariesAssignBatchIndices() throws {
        let json = """
        {
            "pagination": {"limit":10,"snapshot_height":10,"snapshot_hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","next_cursor":null,"has_more":false},
            "items": [
                {
                    "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                    "created_at":"2025-01-01T00:00:00Z",
                    "kind":"Transfer",
                    "box":{
                        "encoded":"0x00","framed_sha256":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "json":{
                            "kind":"Transfer",
                            "payload":{
                                "variant":"AssetBatch",
                                "value":{
                                    "entries":[
                                        {
                                            "from":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                                            "to":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                                            "asset_definition":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                            "amount":"5"
                                        },
                                        {
                                            "from":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
                                            "to":"sorauﾛ1Pﾀﾚｿ1ﾍｶsFｲAfｾeB3ｽヱヱｳcyﾊyｹ1ﾂﾈヰヰ6ﾛヰEAﾃｱｳﾖLPN4XM",
                                            "asset_definition":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                            "amount":"7"
                                        }
                                    ]
                                }
                            },
                            "wire_id":"10",
                            "encoded":"beef"
                        }
                    },
                    "transaction_hash":"hash1",
                    "transaction_status":"Committed",
                    "block":1,
                    "index":0
                }
            ]
        }
        """
        let page = try JSONDecoder().decode(ToriiExplorerInstructionsPage.self, from: Data(json.utf8))
        let summaries = page.transferSummaries()
        XCTAssertEqual(summaries.count, 2)
        XCTAssertEqual(summaries[0].transferIndex, 0)
        XCTAssertEqual(summaries[1].transferIndex, 1)
        XCTAssertEqual(summaries[0].id, "hash1|0|0")
        XCTAssertEqual(summaries[1].id, "hash1|0|1")
    }

    // MARK: - Mint / Burn instruction parsing

    func testExplorerMintInstructionParsedAsSummary() throws {
        // Real Mint response from Iroha explorer API
        let json = """
        {
            "pagination":{"limit":20,"snapshot_height":10,"snapshot_hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","next_cursor":null,"has_more":false},
            "items":[{
                "authority":"sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
                "created_at":"2026-03-17T14:07:35.576Z",
                "kind":"Mint",
                "box":{
                    "json":{
                        "encoded":"deadbeef",
                        "kind":"Mint",
                        "payload":{
                            "value":{
                                "destination":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                                "object":"500"
                            },
                            "variant":"Asset"
                        },
                        "wire_id":"iroha.instruction.v1::mint_burn::MintBox"
                    }
                },
                "transaction_hash":"9bca4ad18474058cbbad5bbc49e5e11cf58d90fc28b094ac8f8963a5116fdff5",
                "transaction_status":"Committed",
                "block":17,
                "index":0
            }]
        }
        """
        let page = try JSONDecoder().decode(ToriiExplorerInstructionsPage.self, from: Data(json.utf8))
        XCTAssertEqual(page.items.count, 1)

        let item = page.items[0]
        XCTAssertEqual(item.kind, "Mint")

        // transferDetails() should parse Mint payloads
        let details = item.transferDetails()
        XCTAssertNotNil(details, "transferDetails() should parse Mint instructions")

        // Generate summaries relative to the mint recipient
        let accountId = "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"
        let summaries = page.transferSummaries(relativeTo: accountId)
        XCTAssertEqual(summaries.count, 1, "Mint should produce exactly 1 summary")

        let summary = summaries[0]
        XCTAssertEqual(summary.kind, "Mint")
        XCTAssertEqual(summary.amount, "500")
        XCTAssertEqual(summary.direction, .incoming, "Mint should always be incoming")
        XCTAssertEqual(summary.status, "Committed")
        XCTAssertEqual(summary.transactionHash, "9bca4ad18474058cbbad5bbc49e5e11cf58d90fc28b094ac8f8963a5116fdff5")
        // assetDefinitionId should remain in canonical Base58 form
        XCTAssertFalse(summary.assetDefinitionId.isEmpty)
        XCTAssertFalse(summary.assetDefinitionId.contains(":"),
                       "assetDefinitionId should decode to unprefixed Base58 form")
        // receiverAccountId should be extracted from the canonical asset ID
        XCTAssertFalse(summary.receiverAccountId.isEmpty, "receiverAccountId should not be empty")
    }

}
