import Foundation
@testable import IrohaSwift
import XCTest

private enum NativeAmxGroupedFixtureError: Error {
    case malformed(String)
}

private func nativeAmxGroupedFixtureURL() throws -> URL {
    var current = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    while current.path != "/" {
        let candidate = current
            .appendingPathComponent("fixtures")
            .appendingPathComponent("sumeragi_v2")
            .appendingPathComponent("native_amx_v2_grouped.json")
        if FileManager.default.fileExists(atPath: candidate.path) {
            return candidate
        }
        current.deleteLastPathComponent()
    }
    throw NativeAmxGroupedFixtureError.malformed(
        "fixtures/sumeragi_v2/native_amx_v2_grouped.json was not found"
    )
}

private func loadNativeAmxGroupedFixture() throws -> [String: Any] {
    let data = try Data(contentsOf: nativeAmxGroupedFixtureURL())
    guard let document = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
        throw NativeAmxGroupedFixtureError.malformed("fixture root must be an object")
    }
    return document
}

private func pointerTokens(_ pointer: String) throws -> [String] {
    guard pointer.first == "/" else {
        throw NativeAmxGroupedFixtureError.malformed(
            "fixture mutation path must be an absolute JSON pointer"
        )
    }
    return pointer.dropFirst().split(separator: "/", omittingEmptySubsequences: false).map {
        $0.replacingOccurrences(of: "~1", with: "/")
            .replacingOccurrences(of: "~0", with: "~")
    }
}

private func fixtureValue(at tokens: ArraySlice<String>, in value: Any) throws -> Any {
    guard let head = tokens.first else { return value }
    if let object = value as? [String: Any], let child = object[head] {
        return try fixtureValue(at: tokens.dropFirst(), in: child)
    }
    if let array = value as? [Any],
       let index = Int(head),
       array.indices.contains(index)
    {
        return try fixtureValue(at: tokens.dropFirst(), in: array[index])
    }
    throw NativeAmxGroupedFixtureError.malformed("JSON pointer does not resolve")
}

private func assigningFixtureValue(
    _ replacement: Any,
    at tokens: ArraySlice<String>,
    in value: Any
) throws -> Any {
    guard let head = tokens.first else { return replacement }
    if var object = value as? [String: Any] {
        guard let child = object[head] else {
            throw NativeAmxGroupedFixtureError.malformed("JSON pointer does not resolve")
        }
        object[head] = try assigningFixtureValue(
            replacement,
            at: tokens.dropFirst(),
            in: child
        )
        return object
    }
    if var array = value as? [Any],
       let index = Int(head),
       array.indices.contains(index)
    {
        array[index] = try assigningFixtureValue(
            replacement,
            at: tokens.dropFirst(),
            in: array[index]
        )
        return array
    }
    throw NativeAmxGroupedFixtureError.malformed("JSON pointer does not resolve")
}

private func removingFixtureValue(
    at tokens: ArraySlice<String>,
    in value: Any
) throws -> Any {
    guard let head = tokens.first else {
        throw NativeAmxGroupedFixtureError.malformed("cannot remove the fixture root")
    }
    if var object = value as? [String: Any] {
        if tokens.count == 1 {
            guard object.removeValue(forKey: head) != nil else {
                throw NativeAmxGroupedFixtureError.malformed("JSON pointer does not resolve")
            }
        } else {
            guard let child = object[head] else {
                throw NativeAmxGroupedFixtureError.malformed("JSON pointer does not resolve")
            }
            object[head] = try removingFixtureValue(at: tokens.dropFirst(), in: child)
        }
        return object
    }
    if var array = value as? [Any],
       let index = Int(head),
       array.indices.contains(index)
    {
        if tokens.count == 1 {
            array.remove(at: index)
        } else {
            array[index] = try removingFixtureValue(
                at: tokens.dropFirst(),
                in: array[index]
            )
        }
        return array
    }
    throw NativeAmxGroupedFixtureError.malformed("JSON pointer does not resolve")
}

private func applyFixtureMutation(_ mutation: [String: Any], to root: Any) throws -> Any {
    guard let operation = mutation["op"] as? String,
          let path = mutation["path"] as? String
    else {
        throw NativeAmxGroupedFixtureError.malformed("mutation must carry op and path")
    }
    let tokens = try pointerTokens(path)[...]
    switch operation {
    case "replace":
        guard let replacement = mutation["value"] else {
            throw NativeAmxGroupedFixtureError.malformed("replace mutation is missing value")
        }
        return try assigningFixtureValue(replacement, at: tokens, in: root)
    case "remove":
        return try removingFixtureValue(at: tokens, in: root)
    case "copy":
        guard let options = mutation["value"] as? [String: Any],
              let source = options["from"] as? String
        else {
            throw NativeAmxGroupedFixtureError.malformed("copy mutation is missing source")
        }
        let copied = try fixtureValue(at: pointerTokens(source)[...], in: root)
        return try assigningFixtureValue(copied, at: tokens, in: root)
    case "swap":
        guard let options = mutation["value"] as? [String: Any],
              let left = options["left"] as? Int,
              let right = options["right"] as? Int,
              var array = try fixtureValue(at: tokens, in: root) as? [Any],
              array.indices.contains(left),
              array.indices.contains(right)
        else {
            throw NativeAmxGroupedFixtureError.malformed("swap mutation is malformed")
        }
        array.swapAt(left, right)
        return try assigningFixtureValue(array, at: tokens, in: root)
    case "repeat":
        guard let options = mutation["value"] as? [String: Any],
              let sourceIndex = options["source_index"] as? Int,
              let count = options["count"] as? Int,
              let array = try fixtureValue(at: tokens, in: root) as? [Any],
              array.indices.contains(sourceIndex)
        else {
            throw NativeAmxGroupedFixtureError.malformed("repeat mutation is malformed")
        }
        return try assigningFixtureValue(
            Array(repeating: array[sourceIndex], count: count),
            at: tokens,
            in: root
        )
    default:
        throw NativeAmxGroupedFixtureError.malformed(
            "unsupported fixture mutation operation \(operation)"
        )
    }
}

final class NativeAmxV2GroupedFixtureTests: XCTestCase {
    func testRustOwnedGroupedNativeAmxV2GoldenFixture() throws {
        let document = try loadNativeAmxGroupedFixture()
        XCTAssertEqual(document["format"] as? String, "iroha-native-amx-v2-grouped")
        XCTAssertEqual(document["fixture_version"] as? Int, 1)
        XCTAssertEqual(
            document["rust_owner"] as? String,
            "iroha_data_model::block::consensus"
        )
        let golden = try XCTUnwrap(document["golden"] as? [String: Any])
        let expected = try XCTUnwrap(golden["expected_diagnostics"])
        let data = try JSONSerialization.data(withJSONObject: expected)
        let diagnostics = try JSONDecoder().decode(
            ToriiSumeragiDiagnosticsSnapshot.self,
            from: data
        )
        let sourceOrder = try XCTUnwrap(golden["ordered_source_ids"] as? [String])
        let group = try XCTUnwrap(diagnostics.laneSettlementCommitments.first)
        XCTAssertEqual(group.nativeAmxReceipts.map(\.sourceId), sourceOrder)
        XCTAssertEqual(group.nativeAmxReceipts.count, 2)
        for receipt in group.nativeAmxReceipts {
            XCTAssertEqual(receipt.legs.count, 2)
            XCTAssertEqual(receipt.laneBlockView, 9)
            for leg in receipt.legs {
                XCTAssertEqual(leg.prepareQc.body.phase, .prepare)
                XCTAssertEqual(leg.commitQc.body.phase, .commit)
                XCTAssertEqual(leg.prepareQc.body.round.view, 6)
                XCTAssertEqual(leg.prepareQc.body.coordinatorLaneBlockView, 9)
                XCTAssertEqual(leg.prepareQc.validatorSet.count, 4)
                XCTAssertTrue(leg.prepareQc.validatorSetPops.allSatisfy { $0.count == 96 })
                XCTAssertEqual(leg.prepareQc.blsAggregateSignature.count, 96)
                XCTAssertEqual(
                    leg.participantSettlement.receipts.map(\.sourceId),
                    sourceOrder
                )
            }
        }
        XCTAssertEqual(
            diagnostics.nativeAmxParticipantApplications.first?.sourceCount,
            2
        )
    }

    func testRustOwnedGroupedNativeAmxV2NegativeCorpus() throws {
        let canonical = try loadNativeAmxGroupedFixture()
        let controls = try XCTUnwrap(canonical["negative_controls"] as? [[String: Any]])
        for control in controls {
            let identifier = try XCTUnwrap(control["id"] as? String)
            try XCTContext.runActivity(named: identifier) { _ in
                XCTAssertEqual(control["expectation"] as? String, "reject")
                var mutated: Any = canonical
                for mutation in try XCTUnwrap(control["mutations"] as? [[String: Any]]) {
                    mutated = try applyFixtureMutation(mutation, to: mutated)
                }
                let root = try XCTUnwrap(mutated as? [String: Any])
                let golden = try XCTUnwrap(root["golden"] as? [String: Any])
                var diagnostics = try XCTUnwrap(
                    golden["expected_diagnostics"] as? [String: Any]
                )
                diagnostics["lane_settlement_commitments"] = try [
                    XCTUnwrap(golden["receipt_group"]),
                ]
                let data = try JSONSerialization.data(withJSONObject: diagnostics)
                XCTAssertThrowsError(
                    try JSONDecoder().decode(
                        ToriiSumeragiDiagnosticsSnapshot.self,
                        from: data
                    )
                )
            }
        }
    }
}
