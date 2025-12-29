import XCTest
import IrohaSwift

final class ToriiOfflineResponseTests: XCTestCase {
    func testOfflineSpendReceiptsSubmitResponseDecodes() throws {
        let payload = """
        {
          "receipts_root_hex": "deadbeef",
          "receipt_count": 2,
          "total_amount": 42,
          "asset_id": "rose#wonderland"
        }
        """.data(using: .utf8)!
        let decoded = try JSONDecoder().decode(ToriiOfflineSpendReceiptsSubmitResponse.self, from: payload)
        XCTAssertEqual(decoded.receiptsRootHex, "deadbeef")
        XCTAssertEqual(decoded.receiptCount, 2)
        XCTAssertEqual(decoded.totalAmount, "42")
        XCTAssertEqual(decoded.assetId, "rose#wonderland")
    }

    func testOfflineStateResponseDecodesArrays() throws {
        let payload = """
        {
          "allowances": [],
          "transfers": [],
          "summaries": [],
          "revocations": [],
          "now_ms": 123
        }
        """.data(using: .utf8)!
        let decoded = try JSONDecoder().decode(ToriiOfflineStateResponse.self, from: payload)
        XCTAssertEqual(decoded.allowances.count, 0)
        XCTAssertEqual(decoded.transfers.count, 0)
        XCTAssertEqual(decoded.summaries.count, 0)
        XCTAssertEqual(decoded.revocations.count, 0)
        XCTAssertEqual(decoded.nowMs, 123)
    }
}
