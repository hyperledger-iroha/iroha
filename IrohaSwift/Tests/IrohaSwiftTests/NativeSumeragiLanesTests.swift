import Foundation
import XCTest
@testable import IrohaSwift

/// Shared exact Rust-produced `GET /v1/sumeragi/lanes` rows; never hand-written goldens.
enum NativeLaneFixtures {
    static func rows() throws -> [String: Data] {
        var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        while directory.path != "/" {
            let path = directory.appendingPathComponent("fixtures/sumeragi/native_lanes_v1.tsv")
            if FileManager.default.fileExists(atPath: path.path) {
                var rows: [String: Data] = [:]
                for line in try String(contentsOf: path, encoding: .utf8).split(separator: "\n") where !line.hasPrefix("#") {
                    let fields = line.split(separator: "\t", omittingEmptySubsequences: false)
                    guard fields.count == 3, !fields[2].isEmpty, fields[2].count % 2 == 0,
                          fields[2].allSatisfy({ "0123456789abcdef".contains($0) }),
                          rows[String(fields[0])] == nil else {
                        throw CocoaError(.fileReadCorruptFile)
                    }
                    rows[String(fields[0])] = Data(fields[1].utf8)
                }
                guard Set(rows.keys) == ["empty", "running_lane", "mixed_lanes"] else {
                    throw CocoaError(.fileReadCorruptFile)
                }
                return rows
            }
            directory.deleteLastPathComponent()
        }
        throw CocoaError(.fileNoSuchFile)
    }
    static func json(_ name: String) throws -> Data {
        guard let row = try rows()[name] else { throw CocoaError(.fileNoSuchFile) }
        return row
    }
}

final class NativeSumeragiLanesTests: XCTestCase {
    private func lanes() throws -> [[String: Any]] {
        try XCTUnwrap(JSONSerialization.jsonObject(with: NativeLaneFixtures.json("mixed_lanes")) as? [[String: Any]])
    }
    private func changed(_ change: (inout [String: Any]) -> Void) throws -> Data {
        var all = try lanes()
        change(&all[0])
        return try JSONSerialization.data(withJSONObject: all)
    }
    private func changedRecord(_ change: (inout [String: Any]) -> Void) throws -> Data {
        try changed { lane in
            var record = lane["record"] as? [String: Any] ?? [:]
            change(&record)
            lane["record"] = record
        }
    }

    func testSharedRustRowsPreserveEveryLaneStateAndUnsignedRange() throws {
        XCTAssertEqual(try ToriiSumeragiLaneStatus.parseJSONList(NativeLaneFixtures.json("empty")), [])
        let running = try XCTUnwrap(ToriiSumeragiLaneStatus.parseJSONList(NativeLaneFixtures.json("running_lane")).first)
        XCTAssertEqual(running.record.lane, 1)
        XCTAssertEqual(running.record.dataspace, 0)
        XCTAssertEqual(running.record.incarnation, String(repeating: "11", count: 32))
        XCTAssertEqual(running.record.committee.count, 4)
        XCTAssertTrue(running.record.committee.allSatisfy { $0.peer.hasPrefix("ea0130") && $0.proofOfPossession.count == 96 })
        XCTAssertNil(running.record.closing)
        XCTAssertFalse(running.record.isClosing)
        XCTAssertEqual(running.record.createdAt, 40)
        XCTAssertEqual(running.record.activeFrom, 42)
        XCTAssertEqual(running.record.merged.height, 7)
        XCTAssertEqual(running.record.merged.blockHash, String(repeating: "22", count: 32))
        XCTAssertEqual(running.record.merged.result, String(repeating: "33", count: 32))
        XCTAssertEqual(running.record.params.keyAllowedAlgorithms, ["bls_normal"])
        let instance = try XCTUnwrap(running.instance)
        XCTAssertEqual(instance.protocolVersion, 1)
        XCTAssertEqual(instance.leader, running.record.committee[0].peer)
        XCTAssertEqual(instance.signer, running.record.committee[1].peer)
        XCTAssertEqual(instance.instance, String(repeating: "5a", count: 32))
        XCTAssertEqual(instance.footprint.probe, UInt64.max)

        let mixed = try ToriiSumeragiLaneStatus.parseJSONList(NativeLaneFixtures.json("mixed_lanes"))
        XCTAssertEqual(mixed.count, 3)
        XCTAssertEqual(mixed[0], running)
        XCTAssertNil(mixed[1].instance)
        XCTAssertEqual(mixed[1].record.lane, 16)
        XCTAssertEqual(mixed[1].record.dataspace, UInt64.max)
        XCTAssertEqual(mixed[1].record.rescued, UInt64.max)
        XCTAssertEqual(mixed[1].record.merged.height, 0)
        XCTAssertEqual(mixed[1].record.merged.blockHash, String(repeating: "00", count: 32))
        XCTAssertEqual(mixed[2].record.lane, UInt32.max)
        XCTAssertEqual(mixed[2].record.closing, UInt64.max)
        XCTAssertTrue(mixed[2].record.isClosing)
        XCTAssertEqual(mixed[2].record.anchorFreshness, UInt64.max)
        XCTAssertEqual(mixed[2].record.merged.height, UInt64.max)
        XCTAssertEqual(mixed[2].instance?.halted, .publicationRecoveryRequired(UInt64.max))
    }

    func testMissingAndUnknownFieldsAtEveryLevelFailClosed() throws {
        // The unmodified Foundation round trip must stay valid, so each failure below is caused
        // by its own mutation rather than by lossy re-encoding.
        XCTAssertEqual(try ToriiSumeragiLaneStatus.parseJSONList(changed { _ in }).count, 3)
        let lane = try lanes()[0]
        for field in lane.keys {
            XCTAssertThrowsError(try ToriiSumeragiLaneStatus.parseJSONList(changed { $0.removeValue(forKey: field) }), field)
        }
        XCTAssertThrowsError(try ToriiSumeragiLaneStatus.parseJSONList(changed { $0["retired"] = NSNull() }))
        let record = try XCTUnwrap(lane["record"] as? [String: Any])
        for field in record.keys {
            XCTAssertThrowsError(try ToriiSumeragiLaneStatus.parseJSONList(changedRecord { $0.removeValue(forKey: field) }), field)
        }
        for retired in ["lane_finality_manifest", "merge_carrier", "queue_plan", "relay_envelope"] {
            XCTAssertThrowsError(try ToriiSumeragiLaneStatus.parseJSONList(changedRecord { $0[retired] = NSNull() }), retired)
        }
        for owner in ["params", "merged"] {
            let nested = try XCTUnwrap(record[owner] as? [String: Any])
            for field in nested.keys {
                XCTAssertThrowsError(try ToriiSumeragiLaneStatus.parseJSONList(changedRecord {
                    var copy = nested; copy.removeValue(forKey: field); $0[owner] = copy
                }), "\(owner).\(field)")
            }
            XCTAssertThrowsError(try ToriiSumeragiLaneStatus.parseJSONList(changedRecord {
                var copy = nested; copy["legacy"] = 0; $0[owner] = copy
            }))
        }
        let member = try XCTUnwrap((record["committee"] as? [[String: Any]])?.first)
        for field in member.keys {
            XCTAssertThrowsError(try ToriiSumeragiLaneStatus.parseJSONList(changedRecord {
                var copy = member; copy.removeValue(forKey: field); $0["committee"] = [copy]
            }), "committee.\(field)")
        }
    }

    func testMalformedLaneScalarsKeysAndProofsAreRejected() throws {
        let record = try XCTUnwrap(try lanes()[0]["record"] as? [String: Any])
        for bad: Any in [-1, "1", 1.5, NSNull(), true, 4_294_967_296 as UInt64] {
            XCTAssertThrowsError(try ToriiSumeragiLaneStatus.parseJSONList(changedRecord { $0["lane"] = bad }), "lane \(bad)")
        }
        for bad in [String(repeating: "11", count: 31), String(repeating: "aa", count: 32), String(repeating: "11", count: 33)] {
            XCTAssertThrowsError(try ToriiSumeragiLaneStatus.parseJSONList(changedRecord { $0["incarnation"] = bad }), bad)
        }
        let params = try XCTUnwrap(record["params"] as? [String: Any])
        for field in ["block_cadence_ms", "payload_retry_interval_ms", "exec_budget_ms", "apply_budget_ms",
                      "max_block_bytes", "epoch_length_blocks", "demotion_window"] {
            XCTAssertThrowsError(try ToriiSumeragiLaneStatus.parseJSONList(changedRecord {
                var copy = params; copy[field] = 0; $0["params"] = copy
            }), "zero \(field)")
        }
        for algorithms: Any in [["bls"], ["BLS_NORMAL"], "bls_normal", [1]] {
            XCTAssertThrowsError(try ToriiSumeragiLaneStatus.parseJSONList(changedRecord {
                var copy = params; copy["key_allowed_algorithms"] = algorithms; $0["params"] = copy
            }))
        }
        let member = try XCTUnwrap((record["committee"] as? [[String: Any]])?.first)
        let peer = try XCTUnwrap(member["peer"] as? String)
        let pop = try XCTUnwrap(member["pop"] as? String)
        for bad in [peer.lowercased(), "bls_normal:\(peer)", "ed0120" + String(repeating: "AB", count: 32), " \(peer)"] {
            XCTAssertThrowsError(try ToriiSumeragiLaneStatus.parseJSONList(changedRecord {
                var copy = member; copy["peer"] = bad; $0["committee"] = [copy]
            }), bad)
        }
        for bad in [String(pop.dropLast(4)), pop + "AAAA", "-" + String(pop.dropFirst()), pop + "=", ""] {
            XCTAssertThrowsError(try ToriiSumeragiLaneStatus.parseJSONList(changedRecord {
                var copy = member; copy["pop"] = bad; $0["committee"] = [copy]
            }), bad)
        }
        let running = try NativeLaneFixtures.json("running_lane")
        let text = try XCTUnwrap(String(data: running, encoding: .utf8))
        for wire in [text.replacingOccurrences(of: "\"rescued\":0", with: "\"rescued\":-0"),
                     text.replacingOccurrences(of: "\"rescued\":0", with: "\"rescued\":0,\"rescued\":0"),
                     "{}", ""] {
            XCTAssertThrowsError(try ToriiSumeragiLaneStatus.parseJSONList(Data(wire.utf8)), wire)
        }
        XCTAssertThrowsError(try ToriiSumeragiLaneStatus.parseJSONList(Data(repeating: 0x20, count: ToriiSumeragiLaneStatus.maximumJSONBytes + 1)))
    }
}
