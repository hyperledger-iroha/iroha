import XCTest
import IrohaSwift

final class OfflineAllowanceCapacityTests: XCTestCase {
    func testCapacitySeparatesAggregateFromMaxSingleTransfer() throws {
        let list = try makeAllowanceList(remainingAmounts: ["1000", "2000"])

        XCTAssertEqual(decimalString(list.aggregateRemainingAmount), "3000")
        XCTAssertEqual(decimalString(list.maxSingleTransferAmount), "2000")
        XCTAssertFalse(list.supportsSingleTransfer(amount: Decimal(string: "3000")!))
        XCTAssertTrue(list.supportsSingleTransfer(amount: Decimal(string: "2000")!))
        XCTAssertEqual(
            list.firstAllowanceSupportingSingleTransfer(amount: Decimal(string: "1500")!)?.certificateIdHex,
            "cert-1"
        )
        XCTAssertNil(list.firstAllowanceSupportingSingleTransfer(amount: Decimal(string: "3000")!))
    }

    func testCapacitySkipsInvalidRemainingValues() throws {
        let list = try makeAllowanceList(remainingAmounts: ["oops", "5"])

        XCTAssertEqual(decimalString(list.aggregateRemainingAmount), "5")
        XCTAssertEqual(decimalString(list.maxSingleTransferAmount), "5")
        XCTAssertTrue(list.supportsSingleTransfer(amount: Decimal(string: "5")!))
        XCTAssertFalse(list.supportsSingleTransfer(amount: Decimal(string: "6")!))
    }

    private func makeAllowanceList(remainingAmounts: [String]) throws -> ToriiOfflineAllowanceList {
        let items: [[String: Any]] = remainingAmounts.enumerated().map { index, remaining in
            [
                "certificate_id_hex": "cert-\(index)",
                "controller_id": "controller-\(index)",
                "controller_display": "Controller \(index)",
                "asset_id": "PKR#sbp",
                "registered_at_ms": 1,
                "expires_at_ms": 2,
                "policy_expires_at_ms": 3,
                "remaining_amount": remaining,
                "record": [
                    "remaining_amount": remaining
                ]
            ]
        }
        let payload: [String: Any] = [
            "items": items,
            "total": items.count
        ]
        let data = try JSONSerialization.data(withJSONObject: payload, options: [])
        return try JSONDecoder().decode(ToriiOfflineAllowanceList.self, from: data)
    }

    private func decimalString(_ value: Decimal) -> String {
        NSDecimalNumber(decimal: value).stringValue
    }
}
