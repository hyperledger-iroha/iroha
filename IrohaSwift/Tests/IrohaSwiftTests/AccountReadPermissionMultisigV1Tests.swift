import XCTest
@testable import IrohaSwift

final class AccountReadPermissionMultisigV1Tests: XCTestCase {
    private let networkLiteral =
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
    private let source = "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53"
    private let reporter = "testuﾛ1PｵEmｷjMZZﾑﾙeｱﾁﾎﾅﾂﾊmECepdbﾎｳ2uWﾃｸﾊﾘvｵi2ｦP1Y18A"

    func testDraftBindsExactApprovedFeeJSONAndRejectsAlteration() throws {
        let fee = FeePaymentIntent.authority(chargeLimits: [
            try FeeChargeLimit(kind: .nexus,
                               assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
                               maxAmount: "1")
        ], gasLimit: 100)
        let feeJSON = try fee.canonicalJSONData()
        let network = try NetworkId(literal: networkLiteral)
        XCTAssertNoThrow(try AccountReadPermissionMultisigV1(
            networkId: network, authority: source, reportingAccount: reporter,
            change: .grant, creationTimeMs: 1, feePayment: fee, feePaymentJSON: feeJSON
        ))
        var changed = String(decoding: feeJSON, as: UTF8.self)
        changed = changed.replacingOccurrences(of: #""gas_limit":100"#, with: #""gas_limit":101"#)
        XCTAssertThrowsError(try AccountReadPermissionMultisigV1(
            networkId: network, authority: source, reportingAccount: reporter,
            change: .grant, creationTimeMs: 1, feePayment: fee, feePaymentJSON: Data(changed.utf8)
        ))
        XCTAssertThrowsError(try AccountReadPermissionMultisigV1(
            networkId: network, authority: source, reportingAccount: reporter,
            change: .grant, creationTimeMs: 1, feePayment: fee, feePaymentJSON: feeJSON + Data([0x0a])
        ))
        let emptyFee = FeePaymentIntent.authority(chargeLimits: [], gasLimit: nil)
        XCTAssertThrowsError(try AccountReadPermissionMultisigV1(
            networkId: network, authority: source, reportingAccount: reporter,
            change: .grant, creationTimeMs: 1,
            feePayment: emptyFee, feePaymentJSON: emptyFee.canonicalJSONData()
        ))
    }
}
