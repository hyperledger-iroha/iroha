import Foundation
import XCTest
@testable import IrohaSwift

final class ToriiTimeSnapshotTests: XCTestCase {
    private func object() -> [String: Any] {
        ["now": 1_700_000_000_000, "offset_ms": -12, "confidence_ms": 2_000,
         "sample_count": 6, "peer_count": 3, "enforcement_mode": "reject",
         "fallback": false, "health": ["healthy": true, "min_samples_ok": true,
         "offset_ok": true, "confidence_ok": true]]
    }

    private func decode(_ object: [String: Any]) throws -> ToriiTimeSnapshot {
        try JSONDecoder().decode(ToriiTimeSnapshot.self, from: JSONSerialization.data(withJSONObject: object))
    }

    func testHealthySnapshotCarriesEvidenceAndConservativeBound() throws {
        let snapshot = try decode(object())
        XCTAssertEqual(snapshot.healthyLowerBoundMs, 1_699_999_998_000)
        XCTAssertEqual(snapshot.enforcement_mode, .reject)
        XCTAssertEqual(snapshot.peer_count, 3)
    }

    func testFallbackUnhealthyAndEmptySamplesCannotAuthorizeExpiry() throws {
        for field in ["fallback", "sample_count", "peer_count", "now", "confidence_ms"] {
            var invalid = object()
            switch field {
            case "fallback": invalid[field] = true
            case "confidence_ms": invalid[field] = UInt64.max
            default: invalid[field] = 0
            }
            XCTAssertNil(try decode(invalid).healthyLowerBoundMs, field)
        }
        for field in ["healthy", "min_samples_ok", "offset_ok", "confidence_ok"] {
            var invalid = object()
            var health = try XCTUnwrap(invalid["health"] as? [String: Bool])
            health[field] = false
            invalid["health"] = health
            XCTAssertNil(try decode(invalid).healthyLowerBoundMs, field)
        }
    }

    func testMissingEvidenceAndUnknownEnforcementModeAreRejected() throws {
        for field in ["sample_count", "peer_count", "enforcement_mode", "fallback", "health"] {
            var invalid = object(); invalid.removeValue(forKey: field)
            XCTAssertThrowsError(try decode(invalid), field)
        }
        var invalid = object(); invalid["enforcement_mode"] = "disabled"
        XCTAssertThrowsError(try decode(invalid))
    }

    func testLowerBoundNeverUsesPhoneTimeAndCannotUnderflow() throws {
        var input = object(); input["now"] = 2_000
        XCTAssertEqual(try decode(input).healthyLowerBoundMs, 0)
        input["now"] = 1_999
        XCTAssertNil(try decode(input).healthyLowerBoundMs)
        input["now"] = 5_000; input["enforcement_mode"] = "warn"
        XCTAssertEqual(try decode(input).healthyLowerBoundMs, 3_000)
    }
}
