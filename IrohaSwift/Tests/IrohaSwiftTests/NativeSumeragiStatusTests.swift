import Foundation
import XCTest
@testable import IrohaSwift

final class NativeSumeragiStatusTests: XCTestCase {
    private func changed(_ change: (inout [String: Any]) -> Void) throws -> Data {
        var root = try XCTUnwrap(JSONSerialization.jsonObject(with: NativeStatusFixtures.json()) as? [String: Any])
        change(&root)
        return try JSONSerialization.data(withJSONObject: root)
    }
    func testSharedRustRowsPreserveUnsignedRangesAndHaltVariants() throws {
        let rows = try NativeStatusFixtures.rows()
        XCTAssertEqual(rows.count, 8)
        for (name, json, _) in rows {
            let status = try ToriiSumeragiStatusSnapshot.parseJSON(json)
            XCTAssertEqual(status.protocolVersion, 8)
            XCTAssertEqual(status.view, UInt64.max)
            XCTAssertEqual(status.level, UInt32.max)
            XCTAssertEqual(status.footprint.votes, UInt64.max)
            XCTAssertEqual(status.applyLag, 1)
            XCTAssertEqual(status, try ToriiSumeragiStatusSnapshot.parseJSON(json))
            if name == "validator" {
                XCTAssertTrue(status.isSigning)
                XCTAssertFalse(status.isHalted)
                XCTAssertEqual(status.beaconHorizon?.activeSessionId, String(repeating: "AB", count: 32))
            } else {
                XCTAssertFalse(status.isSigning)
                XCTAssertNil(status.beaconHorizon)
                XCTAssertEqual(status.isHalted, name != "observer")
            }
        }
    }
    func testMissingAndUnknownFieldsAtEveryLevelFailClosed() throws {
        let root = try XCTUnwrap(JSONSerialization.jsonObject(with: NativeStatusFixtures.json()) as? [String: Any])
        for field in root.keys {
            XCTAssertThrowsError(try ToriiSumeragiStatusSnapshot.parseJSON(changed { $0.removeValue(forKey: field) }), field)
        }
        for parent in ["footprint", "beacon_horizon"] {
            let nested = try XCTUnwrap(root[parent] as? [String: Any])
            for field in nested.keys {
                XCTAssertThrowsError(try ToriiSumeragiStatusSnapshot.parseJSON(changed {
                    var copy = nested; copy.removeValue(forKey: field); $0[parent] = copy
                }), "\(parent).\(field)")
            }
            XCTAssertThrowsError(try ToriiSumeragiStatusSnapshot.parseJSON(changed {
                var copy = nested; copy["legacy"] = 0; $0[parent] = copy
            }))
        }
        for field in ["height_context_id", "phase", "liveness", "node_fingerprint", "build_fingerprint",
                      "restart_required", "body_state", "rbc_status", "last_commit_qc"] {
            XCTAssertThrowsError(try ToriiSumeragiStatusSnapshot.parseJSON(changed { $0[field] = NSNull() }), field)
        }
    }
    func testScalarAliasesAndMalformedBodiesAreRejected() throws {
        for version in [0, 4, 6, 7, 9] {
            XCTAssertThrowsError(try ToriiSumeragiStatusSnapshot.parseJSON(changed { $0["protocol_version"] = version }))
        }
        for bad: Any in [-1, "15", 1.5, true, NSNull()] {
            XCTAssertThrowsError(try ToriiSumeragiStatusSnapshot.parseJSON(changed { $0["height"] = bad }))
        }
        XCTAssertThrowsError(try ToriiSumeragiStatusSnapshot.parseJSON(changed { $0["stage"] = 3 }))
        XCTAssertThrowsError(try ToriiSumeragiStatusSnapshot.parseJSON(changed { $0["level"] = UInt64(UInt32.max) + 1 }))
        XCTAssertThrowsError(try ToriiSumeragiStatusSnapshot.parseJSON(changed { $0["instance"] = String(repeating: "CD", count: 32) }))
        XCTAssertThrowsError(try ToriiSumeragiStatusSnapshot.parseJSON(changed { $0["config_fingerprint"] = String(repeating: "00", count: 32) }))
        let json = try XCTUnwrap(String(data: NativeStatusFixtures.json("observer"), encoding: .utf8))
        for bad in ["{\"view\":0," + json.dropFirst(), json.replacingOccurrences(of: "\"height\":15", with: "\"height\":-0"),
                    json.replacingOccurrences(of: "\"height\":15", with: "\"height\":18446744073709551616")] {
            XCTAssertThrowsError(try ToriiSumeragiStatusSnapshot.parseJSON(Data(bad.utf8)))
        }
        for bytes in [Data(), Data([0xff]), Data(repeating: 0x20, count: 1_048_577)] {
            XCTAssertThrowsError(try ToriiSumeragiStatusSnapshot.parseJSON(bytes))
        }
    }
    func testPublicKeysHorizonAndHaltOptionsAreStrict() throws {
        let root = try XCTUnwrap(JSONSerialization.jsonObject(with: NativeStatusFixtures.json()) as? [String: Any])
        let key = try XCTUnwrap(root["leader"] as? String)
        for invalid in ["bls_normal:" + key, key.lowercased(), " " + key, "ea0130" + String(repeating: "00", count: 48)] {
            XCTAssertThrowsError(try ToriiSumeragiStatusSnapshot.parseJSON(changed { $0["leader"] = invalid }))
        }
        let horizon = try XCTUnwrap(root["beacon_horizon"] as? [String: Any])
        for (field, value): (String, Any) in [("active_session_id", NSNull()), ("next_required_pulse_height", NSNull()),
                                            ("active_session_id", String(repeating: "ab", count: 32)), ("local_provider_ready", 1)] {
            XCTAssertThrowsError(try ToriiSumeragiStatusSnapshot.parseJSON(changed {
                var copy = horizon; copy[field] = value; $0["beacon_horizon"] = copy
            }))
        }
        for value: [String: Any] in [["reason": "unknown", "details": NSNull()],
                                     ["reason": "driver_anomaly", "details": 1],
                                     ["reason": "apply_diverged", "details": NSNull()],
                                     ["reason": "safety_record_corrupt"]] {
            XCTAssertThrowsError(try ToriiSumeragiStatusSnapshot.parseJSON(changed { $0["halted"] = value }))
        }
    }
    func testAvailableHorizonWithoutDemandRetainsExplicitNulls() throws {
        let json = try changed {
            $0["beacon_horizon"] = ["epoch_length_blocks": 0,
                "next_required_pulse_height": NSNull(), "active_session_id": NSNull(),
                "session_covers_next_pulse": false, "local_provider_ready": false]
        }
        let status = try ToriiSumeragiStatusSnapshot.parseJSON(json)
        let horizon = try XCTUnwrap(status.beaconHorizon)
        XCTAssertEqual(horizon.epochLengthBlocks, 0)
        XCTAssertNil(horizon.activeSessionId)
        XCTAssertNil(horizon.nextRequiredPulseHeight)
        XCTAssertFalse(horizon.localProviderReady)
        XCTAssertEqual(try SumeragiStatusWire.decodeCanonical(SumeragiStatusWire.encode(status)), status)
    }

}
