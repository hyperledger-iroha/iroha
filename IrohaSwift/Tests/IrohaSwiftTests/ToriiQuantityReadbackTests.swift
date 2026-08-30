import XCTest
@testable import IrohaSwift

/// Adversarial coverage for nominal quantity values embedded in explorer instruction readbacks.
final class ToriiQuantityReadbackTests: XCTestCase {
    private let source =
        "62Fk4FPcMuLvW5QjDGNF2a4jAmjM#sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
    private let destination =
        "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"

    func testAssetTransferReadbackAcceptsOnlyCanonicalQuantityStrings() throws {
        let valid = try item(
            kind: "Transfer",
            payload: assetPayload(amountJSON: #""1.25""#)
        )
        guard case let .asset(asset)? = valid.transferDetails() else {
            return XCTFail("canonical transfer quantity was rejected")
        }
        XCTAssertEqual(asset.amount, "1.25")

        for invalid in invalidQuantityJSONValues() {
            let decoded = try item(
                kind: "Transfer",
                payload: assetPayload(amountJSON: invalid)
            )
            XCTAssertNil(decoded.transferDetails(), "accepted invalid transfer quantity \(invalid)")
        }
    }

    func testMintAndBurnReadbacksAcceptOnlyCanonicalQuantityStrings() throws {
        for kind in ["Mint", "Burn"] {
            let valid = try item(
                kind: kind,
                payload: mintBurnPayload(amountJSON: #""2.5""#)
            )
            guard case let .asset(asset)? = valid.transferDetails() else {
                return XCTFail("canonical \(kind) quantity was rejected")
            }
            XCTAssertEqual(asset.amount, "2.5")

            for invalid in invalidQuantityJSONValues() {
                let decoded = try item(
                    kind: kind,
                    payload: mintBurnPayload(amountJSON: invalid)
                )
                XCTAssertNil(decoded.transferDetails(), "accepted invalid \(kind) quantity \(invalid)")
            }
        }
    }

    func testAssetBatchReadbackRejectsOneInvalidQuantityEntry() throws {
        let valid = try item(
            kind: "Transfer",
            payload: batchPayload(amountJSON: #""3""#)
        )
        guard case let .assetBatch(entries)? = valid.transferDetails() else {
            return XCTFail("canonical batch quantity was rejected")
        }
        XCTAssertEqual(entries.map(\.amount), ["3"])

        for invalid in invalidQuantityJSONValues() {
            let decoded = try item(
                kind: "Transfer",
                payload: batchPayload(amountJSON: invalid)
            )
            XCTAssertNil(decoded.transferDetails(), "accepted invalid batch quantity \(invalid)")
        }
    }

    private func item(kind: String, payload: String) throws -> ToriiExplorerInstructionItem {
        let json = """
        {
            "authority":"sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
            "created_at":"2025-01-01T00:00:00Z",
            "kind":"\(kind)",
            "box":{
                "encoded":"0x00",
                "framed_sha256":"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "json":{
                    "kind":"\(kind)",
                    "payload":\(payload),
                    "wire_id":"iroha.numeric.quantity.readback",
                    "encoded":"00"
                }
            },
            "transaction_hash":"hash",
            "transaction_status":"Committed",
            "block":1,
            "index":0
        }
        """
        return try JSONDecoder().decode(
            ToriiExplorerInstructionItem.self,
            from: Data(json.utf8)
        )
    }

    private func assetPayload(amountJSON: String) -> String {
        """
        {
            "variant":"Asset",
            "value":{
                "source":"\(source)",
                "object":\(amountJSON),
                "destination":"\(destination)"
            }
        }
        """
    }

    private func mintBurnPayload(amountJSON: String) -> String {
        """
        {
            "variant":"Asset",
            "value":{
                "destination":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                "object":\(amountJSON)
            }
        }
        """
    }

    private func batchPayload(amountJSON: String) -> String {
        """
        {
            "variant":"AssetBatch",
            "value":{
                "entries":[{
                    "from":"\(source.split(separator: "#", maxSplits: 1)[1])",
                    "to":"\(destination)",
                    "asset_definition":"62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                    "amount":\(amountJSON)
                }]
            }
        }
        """
    }

    private func invalidQuantityJSONValues() -> [String] {
        let scaleTwentyNine = "0." + String(repeating: "0", count: 28) + "1"
        let overflow = String(repeating: "9", count: 155)
        return [
            "1",
            "true",
            #""+1""#,
            #""01""#,
            #""1.""#,
            #""1.0""#,
            #""0.0""#,
            #""1e0""#,
            #"" 1""#,
            #""1 ""#,
            #""-0""#,
            #""-1""#,
            #""NaN""#,
            "\"\(scaleTwentyNine)\"",
            "\"\(overflow)\"",
        ]
    }
}
