import Foundation
import XCTest
@testable import IrohaSwift

final class ToriiJSONValueTests: XCTestCase {
    func testNormalizedStringRejectsOutOfRangeInteger() {
        let tooLarge = Double(Int.max) * 2
        let value = ToriiJSONValue.number(tooLarge)
        XCTAssertNil(value.normalizedString)
    }

    func testNormalizedUInt64RejectsFractionalNumber() {
        let value = ToriiJSONValue.number(12.5)
        XCTAssertNil(value.normalizedUInt64)
    }

    func testNormalizedInt64RejectsFractionalNumber() {
        let value = ToriiJSONValue.number(-3.75)
        XCTAssertNil(value.normalizedInt64)
    }

    func testOfflineAllowanceRejectsOutOfRangeNumericStrings() throws {
        let tooLarge = Double(Int.max) * 2
        let payload: [String: Any] = [
            "certificate_id_hex": tooLarge,
            "controller_id": "alice@sora",
            "controller_display": "Alice",
            "asset_id": "xor#sora",
            "registered_at_ms": 1,
            "expires_at_ms": 2,
            "policy_expires_at_ms": 2,
            "record": [:],
        ]
        let data = try JSONSerialization.data(withJSONObject: payload, options: [])
        XCTAssertThrowsError(try JSONDecoder().decode(ToriiOfflineAllowanceItem.self, from: data))
    }

    func testOfflineAllowanceRejectsFractionalNumericFields() throws {
        let payload: [String: Any] = [
            "certificate_id_hex": "deadbeef",
            "controller_id": "alice@sora",
            "controller_display": "Alice",
            "asset_id": "xor#sora",
            "registered_at_ms": 1.5,
            "expires_at_ms": 2,
            "policy_expires_at_ms": 2,
            "record": [:],
        ]
        let data = try JSONSerialization.data(withJSONObject: payload, options: [])
        XCTAssertThrowsError(try JSONDecoder().decode(ToriiOfflineAllowanceItem.self, from: data))
    }

    func testBundleProofStatusSkipsOutOfRangeNumericProofStatus() throws {
        let tooLarge = Double(Int.max) * 2
        let payload: [String: Any] = [
            "proof_status": tooLarge,
        ]
        let data = try JSONSerialization.data(withJSONObject: payload, options: [])
        let decoded = try JSONDecoder().decode(ToriiOfflineBundleProofStatus.self, from: data)
        XCTAssertNil(decoded.proofStatus)
    }
}
