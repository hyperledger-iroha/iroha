import Foundation
#if canImport(NoritoBridge)
import NoritoBridge
#endif
@testable import IrohaSwift
import XCTest

private enum NativeAmxGroupedFixtureError: Error {
    case malformed(String)
}

private func requireNativeAmxABI23Bridge() throws {
    #if canImport(NoritoBridge)
    let actualABI = connect_norito_bridge_abi_version()
    try requireNativeTestCapability(
        actualABI == NoritoBridgeLoader.expectedBridgeAbiVersion,
        "Native AMX V2 parity requires ABI-\(NoritoBridgeLoader.expectedBridgeAbiVersion) "
            + "NoritoBridge; linked artifact reports ABI-\(actualABI)"
    )
    try requireNativeTestCapability(
        NoritoNativeBridge.shared.isAvailable,
        "Native AMX V2 parity requires the complete ABI-\(actualABI) NoritoBridge symbol set"
    )
    #else
    try failRequiredNativeTestCapability(
        "Native AMX V2 parity requires the ABI-23 NoritoBridge module"
    )
    #endif
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

func loadNativeAmxGroupedFixture() throws -> [String: Any] {
    let data = try Data(contentsOf: nativeAmxGroupedFixtureURL())
    guard let document = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
        throw NativeAmxGroupedFixtureError.malformed("fixture root must be an object")
    }
    return document
}

private final class NativeAmxGroupedEndpointURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data?))?

    override class func canInit(with request: URLRequest) -> Bool { true }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        guard let handler = Self.handler else {
            client?.urlProtocol(
                self,
                didFailWithError: NSError(domain: "NativeAmxGroupedEndpoint", code: -1)
            )
            return
        }
        do {
            let (response, data) = try handler(request)
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            if let data {
                client?.urlProtocol(self, didLoad: data)
            }
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}
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

private func fixtureScalarEqual(_ lhs: Any?, _ rhs: Any?) -> Bool {
    if lhs == nil || lhs is NSNull {
        return rhs == nil || rhs is NSNull
    }
    if rhs == nil || rhs is NSNull {
        return false
    }
    if let left = lhs as? String, let right = rhs as? String {
        return left == right
    }
    if let left = lhs as? NSNumber, let right = rhs as? NSNumber {
        return left == right
    }
    return false
}

private func fixtureCanonicalHashBytes(_ value: Any?, field: String) throws -> Data {
    guard let literal = value as? String,
          ToriiNativeAmxWire.isCanonicalHash(literal)
    else {
        throw NativeAmxGroupedFixtureError.malformed("\(field) must be a canonical hash")
    }
    let body = literal.dropFirst(5).prefix(64)
    guard let bytes = Data(hexString: String(body)), bytes.count == 32 else {
        throw NativeAmxGroupedFixtureError.malformed("\(field) must contain 32 hash bytes")
    }
    return bytes
}

private func nativeAmxApplicationManifestSingletonRoot(_ leafHash: Any?) throws -> Data {
    var preimage = Data("iroha:merkle:leaf:v1\u{0}".utf8)
    preimage.append(try fixtureCanonicalHashBytes(leafHash, field: "manifest leaf hash"))
    return IrohaHash.hash(preimage)
}

private func fixtureUInt(_ object: [String: Any], _ field: String) throws -> UInt64 {
    guard let number = object[field] as? NSNumber else {
        throw NativeAmxGroupedFixtureError.malformed("\(field) must be an integer")
    }
    return number.uint64Value
}

private func validateApplicationEvidenceFixture(_ document: [String: Any]) throws {
    func require(
        _ condition: @autoclosure () throws -> Bool,
        _ message: String
    ) throws {
        guard try condition() else {
            throw NativeAmxGroupedFixtureError.malformed(message)
        }
    }

    let golden = try fixtureValue(at: ["golden"][...], in: document) as? [String: Any]
    let group = try XCTUnwrap(golden?["receipt_group"] as? [String: Any])
    let evidence = try XCTUnwrap(golden?["application_evidence"] as? [String: Any])
    let execution = try XCTUnwrap(evidence["execution_commitment"] as? [String: Any])
    let artifacts = try XCTUnwrap(evidence["manifest_artifacts"] as? [[String: Any]])
    try require(
        try fixtureUInt(execution, "native_amx_application_manifest_version") == 1,
        "manifest version"
    )
    let parsedExecution = try JSONDecoder().decode(
        ToriiSumeragiV2ExecutionCommitment.self,
        from: JSONSerialization.data(withJSONObject: execution)
    )
    try require(
        parsedExecution.laneFinalityManifest == nil && parsedExecution.mergeCarrier != nil,
        "lane-finality manifest and merge carrier"
    )
    try require(
        try fixtureUInt(execution, "native_amx_application_manifest_count")
            == UInt64(artifacts.count) && artifacts.count == 1,
        "manifest count"
    )
    let artifact = artifacts[0]
    let leaf = try XCTUnwrap(artifact["leaf"] as? [String: Any])
    try require(leaf.keys.contains("previous_native_settlement_hash"), "required previous Native hash")
    let previousNativeHash = leaf["previous_native_settlement_hash"]
    if !(previousNativeHash is NSNull) {
        let previousBytes = try fixtureCanonicalHashBytes(previousNativeHash, field: "previous Native hash")
        try require(previousBytes != Data(repeating: 0, count: 31) + Data([1]), "nonzero previous Native hash")
        try require(try fixtureUInt(leaf, "participant_height") > 1, "first lane block has no previous Native hash")
    }
    let proof = try XCTUnwrap(artifact["proof"] as? [String: Any])
    let manifestCount = try fixtureUInt(
        execution,
        "native_amx_application_manifest_count"
    )
    let manifestRoot = try fixtureCanonicalHashBytes(
        artifact["manifest_root"],
        field: "manifest root"
    )
    let expectedManifestRoot = try nativeAmxApplicationManifestSingletonRoot(
        artifact["leaf_hash"]
    )
    try require(
        try fixtureUInt(artifact, "version") == 1
            && fixtureUInt(leaf, "version") == 1,
        "artifact version"
    )
    try require(
        try fixtureUInt(artifact, "leaf_index") == 0
            && fixtureUInt(proof, "leaf_index") == 0,
        "proof position"
    )
    try require(
        (proof["audit_path"] as? [Any])?.isEmpty == true,
        "singleton proof path"
    )
    try require(
        try fixtureUInt(artifact, "manifest_leaf_count") == manifestCount
            && fixtureScalarEqual(
                artifact["manifest_root"],
                execution["native_amx_application_manifest_root"]
            )
            && manifestRoot == expectedManifestRoot,
        "manifest root must match the execution commitment and authenticate the leaf hash"
    )
    try require(
        fixtureScalarEqual(
            leaf["executed_block_wire_hash"],
            execution["executed_block_wire_hash"]
        ),
        "executed wire"
    )
    try require(
        try fixtureUInt(execution, "executed_block_wire_len") == 49,
        "executed wire length"
    )
    try require(
        try fixtureUInt(leaf, "predecessor_height") + 1
            == fixtureUInt(leaf, "participant_height"),
        "participant predecessor"
    )
    let activeRows = try XCTUnwrap(evidence["active_lane_incarnations"] as? [[String: Any]])
    try require(activeRows.count == 1, "active incarnation count")
    let active = activeRows[0]
    for field in ["lane_id", "dataspace_id", "lane_incarnation"] {
        try require(
            fixtureScalarEqual(active[field], leaf[field]),
            "active incarnation \(field)"
        )
    }
    try require(
        !fixtureScalarEqual(leaf["lane_id"], group["lane_id"])
            || !fixtureScalarEqual(leaf["dataspace_id"], group["dataspace_id"]),
        "same-route coordinator must not have separate evidence"
    )

    let members = try XCTUnwrap(leaf["members"] as? [[String: Any]])
    let receipts = try XCTUnwrap(group["native_amx_receipts"] as? [[String: Any]])
    try require(
        !members.isEmpty && members.count <= 4_096 && members.count == receipts.count,
        "manifest members"
    )
    try require(
        zip(members, receipts).allSatisfy { pair in
            fixtureScalarEqual(pair.0["source_id"], pair.1["source_id"])
        },
        "manifest source membership"
    )
    let carrierEntrypoints = Set(
        try XCTUnwrap(evidence["carrier_entrypoint_hashes"] as? [String])
    )
    for (member, receipt) in zip(members, receipts) {
        let legs = try XCTUnwrap(receipt["legs"] as? [[String: Any]])
        let matching = legs.filter {
            fixtureScalarEqual($0["lane_id"], leaf["lane_id"])
                && fixtureScalarEqual($0["dataspace_id"], leaf["dataspace_id"])
        }
        try require(matching.count == 1, "manifest route")
        let leg = matching[0]
        let proposal = try XCTUnwrap(leg["participant_proposal"] as? [String: Any])
        let descriptor = try XCTUnwrap(proposal["descriptor"] as? [String: Any])
        let identityFields = [
            ("lane_incarnation", "lane_incarnation"),
            ("lane_block_height", "participant_height"),
            ("lane_block_view", "participant_view"),
            ("previous_lane_block_height", "predecessor_height"),
            ("previous_lane_block_descriptor_hash", "predecessor_descriptor_hash"),
            ("descriptor_hash", "descriptor_hash"),
        ]
        for (descriptorField, leafField) in identityFields {
            try require(
                fixtureScalarEqual(descriptor[descriptorField], leaf[leafField]),
                "manifest participant \(leafField)"
            )
        }
        try require(
            fixtureScalarEqual(proposal["proposal_hash"], leaf["proposal_hash"])
                && fixtureScalarEqual(
                    leg["participant_settlement_hash"],
                    leaf["settlement_hash"]
                ),
            "manifest proposal or settlement"
        )
        let prepare = try XCTUnwrap(leg["prepare_qc"] as? [String: Any])
        let body = try XCTUnwrap(prepare["body"] as? [String: Any])
        let settlement = try XCTUnwrap(leg["participant_settlement"] as? [String: Any])
        try require(settlement.keys.contains("previous_native_settlement_hash")
            && fixtureScalarEqual(settlement["previous_native_settlement_hash"], previousNativeHash),
            "manifest previous Native hash")
        try require(
            fixtureScalarEqual(body["source_id"], member["source_id"])
                && fixtureScalarEqual(
                    body["tx_entrypoint_hash"],
                    member["entrypoint_hash"]
                ),
            "manifest member identity"
        )
        let accepted = try XCTUnwrap(
            descriptor["accepted_transaction_hashes"] as? [String]
        )
        try require(
            accepted.allSatisfy { carrierEntrypoints.contains($0) },
            "mixed-role carrier anchor"
        )
    }

    let diagnostics = try XCTUnwrap(golden?["expected_diagnostics"] as? [String: Any])
    let rows = try XCTUnwrap(
        diagnostics["native_amx_participant_applications"] as? [[String: Any]]
    )
    try require(rows.count == 1, "diagnostic application count")
    let row = rows[0]
    for field in [
        "lane_id", "dataspace_id", "lane_incarnation", "participant_height",
        "participant_view", "predecessor_height", "predecessor_descriptor_hash",
        "descriptor_hash", "proposal_hash", "settlement_hash",
        "application_block_height", "application_block_hash",
    ] {
        try require(
            fixtureScalarEqual(row[field], leaf[field]),
            "diagnostic application \(field)"
        )
    }
    try require(
        try fixtureUInt(row, "source_count") == UInt64(members.count),
        "diagnostic source count"
    )
}

final class NativeAmxV2GroupedFixtureTests: XCTestCase {
    func testRustOwnedGroupedNativeAmxV2ApplicationEvidenceNegativeCorpus() throws {
        let canonical = try loadNativeAmxGroupedFixture()
        try validateApplicationEvidenceFixture(canonical)
        let controls = try XCTUnwrap(canonical["negative_controls"] as? [[String: Any]])
        XCTAssertEqual(controls.count, 58)
        let applicationControls = controls.filter {
            $0["validator"] as? String == "application_evidence"
        }
        XCTAssertTrue(
            applicationControls.contains {
                $0["id"] as? String == "manifest_leaf_hash_tampering"
            }
        )
        for control in applicationControls {
            let identifier = try XCTUnwrap(control["id"] as? String)
            try XCTContext.runActivity(named: identifier) { _ in
                var mutated: Any = canonical
                for mutation in try XCTUnwrap(control["mutations"] as? [[String: Any]]) {
                    mutated = try applyFixtureMutation(mutation, to: mutated)
                }
                let root = try XCTUnwrap(mutated as? [String: Any])
                XCTAssertThrowsError(try validateApplicationEvidenceFixture(root))
            }
        }
    }

    private func flatParticipantSettlement() -> [String: Any] {
        ["lane_id": 0, "dataspace_id": 0,
         "lane_incarnation": "hash:0101010101010101010101010101010101010101010101010101010101010101#B86C",
         "participant_lane_block_height": 1, "authority_context_height": 2,
         "previous_native_settlement_hash": NSNull(),
         "source_ids": [String(repeating: "F0", count: 32), String(repeating: "10", count: 32)]]
    }

    private func decodeFlatParticipant(_ value: [String: Any]) throws -> ToriiNativeAmxParticipantSettlement {
        try JSONDecoder().decode(ToriiNativeAmxParticipantSettlement.self,
                                 from: JSONSerialization.data(withJSONObject: value))
    }

    func testParticipantSettlementUsesExactSevenFieldFifoFrame() throws {
        var wire = flatParticipantSettlement()
        let parsed = try decodeFlatParticipant(wire)
        XCTAssertEqual(parsed.laneId, 0)
        XCTAssertEqual(parsed.dataspaceId, 0)
        XCTAssertEqual(parsed.participantLaneBlockHeight, 1)
        XCTAssertEqual(parsed.authorityContextHeight, 2)
        XCTAssertEqual(parsed.sourceIds.map(\.rawValue),
                       [String(repeating: "F0", count: 32), String(repeating: "10", count: 32)])
        XCTAssertEqual(ToriiNativeAmxWire.settlementHash(parsed),
                       "hash:350CB3C0D8728E39820775AC522B345C84631FA81BA164F72FB70043657012CF#EB51")
        XCTAssertNil(parsed.previousNativeSettlementHash)
        var linked = wire
        linked["previous_native_settlement_hash"] = wire["lane_incarnation"]
        XCTAssertThrowsError(try decodeFlatParticipant(linked))
        linked["participant_lane_block_height"] = 2
        let laterLinked = try decodeFlatParticipant(linked)
        XCTAssertEqual(laterLinked.previousNativeSettlementHash, parsed.laneIncarnation)
        linked["previous_native_settlement_hash"] = NSNull()
        XCTAssertNil(try decodeFlatParticipant(linked).previousNativeSettlementHash)
        XCTAssertNotEqual(ToriiNativeAmxWire.settlementHash(laterLinked),
                          ToriiNativeAmxWire.settlementHash(try decodeFlatParticipant(linked)))
        wire["source_ids"] = parsed.sourceIds.reversed().map(\.rawValue)
        XCTAssertNotEqual(ToriiNativeAmxWire.settlementHash(parsed),
                          ToriiNativeAmxWire.settlementHash(try decodeFlatParticipant(wire)))
    }

    func testParticipantSettlementRejectsRetiredFieldsAndInvalidMembership() throws {
        let wire = flatParticipantSettlement()
        for missing in wire.keys {
            var invalid = wire
            invalid.removeValue(forKey: missing)
            XCTAssertThrowsError(try decodeFlatParticipant(invalid))
        }
        for retired in ["block_height", "tx_count", "total_local_amount", "total_xor_due",
                        "total_xor_after_haircut", "total_xor_variance", "swap_metadata",
                        "receipts", "nexus_fee_receipts", "native_amx_receipts"] {
            var invalid = wire
            invalid[retired] = NSNull()
            XCTAssertThrowsError(try decodeFlatParticipant(invalid))
        }
        for height in ["participant_lane_block_height", "authority_context_height"] {
            for value in [0, -1] {
                var invalid = wire
                invalid[height] = value
                XCTAssertThrowsError(try decodeFlatParticipant(invalid))
            }
        }
        var invalid = wire
        let markedZeroBody = "hash:" + String(repeating: "0", count: 63) + "1"
        invalid["lane_incarnation"] = markedZeroBody + String(
            format: "#%04X", Int(ToriiNativeAmxWire.crc16(Array(markedZeroBody.utf8))))
        XCTAssertThrowsError(try decodeFlatParticipant(invalid))
        invalid["previous_native_settlement_hash"] = invalid["lane_incarnation"]
        invalid["lane_incarnation"] = wire["lane_incarnation"]
        invalid["participant_lane_block_height"] = 2
        XCTAssertThrowsError(try decodeFlatParticipant(invalid))
        for sources in [[], [String(repeating: "00", count: 32)],
                        [String(repeating: "F0", count: 32), String(repeating: "F0", count: 32)],
                        [String(repeating: "f0", count: 32)]] {
            invalid = wire
            invalid["source_ids"] = sources
            XCTAssertThrowsError(try decodeFlatParticipant(invalid))
        }
        let maximum = (1...4096).map { String(format: "%064X", $0) }
        invalid = wire
        invalid["source_ids"] = maximum
        XCTAssertEqual(try decodeFlatParticipant(invalid).sourceIds.count, 4096)
        invalid["source_ids"] = maximum + [String(format: "%064X", 4097)]
        XCTAssertThrowsError(try decodeFlatParticipant(invalid))
    }

    func testRustOwnedGroupedNativeAmxV2GoldenFixture() throws {
        try requireNativeAmxABI23Bridge()
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
        XCTAssertEqual(group.nativeAmxReceipts.map(\.sourceId.rawValue), sourceOrder)
        XCTAssertEqual(group.nativeAmxReceipts.count, 2)
        let firstLeg = try XCTUnwrap(group.nativeAmxReceipts.first?.legs.first)
        XCTAssertEqual(
            firstLeg.participantProposal.descriptor.validatorSetHash,
            "hash:33F884E54077B6570826E5DB30B64CEA24B8B559C057F152848E4D1DE7FE8041#6EF8"
        )
        XCTAssertEqual(
            firstLeg.participantProposal.descriptor.descriptorHash,
            "hash:568077DEBB5ECE0F6655571DBD81F8B8935CA5FB064F6B74864B4F58F3CB1A33#E6A5"
        )
        XCTAssertEqual(
            firstLeg.participantProposal.proposalHash,
            "hash:AAC0F352914C21699F3F8D571196C9A5DFCAA9EF1272A7DEFA7FFD35A93C21AD#8B3F"
        )
        XCTAssertNil(firstLeg.participantProposal.payloadBlockHint)
        XCTAssertEqual(
            firstLeg.participantSettlementHash,
            "hash:32950D237EC6ACA2B345D3EFFBD0FE7E30C6E9AF9BD90EE18F8FBFDBDE2A8699#E813"
        )
        let remoteLeg = try XCTUnwrap(
            group.nativeAmxReceipts.first?.legs.dropFirst().first
        )
        XCTAssertEqual(
            remoteLeg.participantSettlementHash,
            "hash:954C813DA9EC5BE63036F21582293E718CF706A2275B25DA96E061FED76492CB#3240"
        )
        let firstValidator = try XCTUnwrap(
            firstLeg.participantProposal.descriptor.validatorSet.first
        )
        XCTAssertTrue(ToriiNativeAmxWire.isCanonicalBlsNormalPeerId(firstValidator))
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
                    leg.participantSettlement.sourceIds.map(\.rawValue),
                    sourceOrder
                )
            }
        }
        XCTAssertEqual(
            diagnostics.nativeAmxParticipantApplications.first?.sourceCount,
            2
        )

        var diagnosticsWithUnknownApplicationField = try XCTUnwrap(
            expected as? [String: Any]
        )
        var applicationRows = try XCTUnwrap(
            diagnosticsWithUnknownApplicationField[
                "native_amx_participant_applications"
            ] as? [[String: Any]]
        )
        var applicationRow = try XCTUnwrap(applicationRows.first)
        applicationRow["unexpected_application_field"] = true
        applicationRows[0] = applicationRow
        diagnosticsWithUnknownApplicationField[
            "native_amx_participant_applications"
        ] = applicationRows
        let diagnosticsWithUnknownApplicationFieldData = try JSONSerialization.data(
            withJSONObject: diagnosticsWithUnknownApplicationField
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiSumeragiDiagnosticsSnapshot.self,
                from: diagnosticsWithUnknownApplicationFieldData
            )
        )
        let metadata: [String: Any] = [
            "epsilon_bps": 50,
            "twap_window_seconds": 60,
            "liquidity_profile": ["profile": "Tier1", "state": NSNull()],
            "twap_local_per_xor": "0.5",
            "volatility_class": ["bucket": "Stable", "state": NSNull()],
        ]
        func diagnosticsWithMetadata(_ value: Any) throws -> ToriiSumeragiDiagnosticsSnapshot {
            let changed = try assigningFixtureValue(
                value,
                at: pointerTokens("/lane_settlement_commitments/0/swap_metadata")[...],
                in: expected
            )
            return try JSONDecoder().decode(
                ToriiSumeragiDiagnosticsSnapshot.self,
                from: JSONSerialization.data(withJSONObject: changed)
            )
        }
        XCTAssertEqual(
            try diagnosticsWithMetadata(metadata).laneSettlementCommitments.first?
                .swapMetadata?.twapLocalPerXor,
            "0.5"
        )
        for canonical in ["0", "-1", "-0.5", "0." + String(repeating: "0", count: 27) + "1"] {
            var exact = metadata
            exact["twap_local_per_xor"] = canonical
            XCTAssertEqual(
                try diagnosticsWithMetadata(exact).laneSettlementCommitments.first?
                    .swapMetadata?.twapLocalPerXor,
                canonical
            )
        }
        for invalid: Any in [
            "", " ", "-0", "01", "1.0", "1e0", "NaN", 1, true,
            "0." + String(repeating: "0", count: 28) + "1",
            String(repeating: "9", count: 155),
        ] {
            var malformed = metadata
            malformed["twap_local_per_xor"] = invalid
            XCTAssertThrowsError(try diagnosticsWithMetadata(malformed))
        }
        for (field, value): (String, Any) in [
            ("unexpected", true),
            ("epsilon_bps", 65_536),
            ("twap_window_seconds", 4_294_967_296),
            ("liquidity_profile", ["profile": "Tier1", "state": NSNull(), "extra": true]),
            ("liquidity_profile", ["profile": "Tier1", "state": true]),
            ("volatility_class", ["bucket": "Stable", "state": NSNull(), "extra": true]),
            ("volatility_class", ["bucket": "Stable", "state": true]),
        ] {
            var malformed = metadata
            malformed[field] = value
            XCTAssertThrowsError(try diagnosticsWithMetadata(malformed))
        }
        for field in metadata.keys {
            var malformed = metadata
            malformed.removeValue(forKey: field)
            XCTAssertThrowsError(try diagnosticsWithMetadata(malformed))
        }
        try validateApplicationEvidenceFixture(document)
    }

    func testParticipantSettlementUsesExactNonrecursiveFields() throws {
        let canonical = try loadNativeAmxGroupedFixture()
        let settlementPath = [
            "golden", "receipt_group", "native_amx_receipts", "0", "legs", "0",
            "participant_settlement",
        ]
        let settlement = try XCTUnwrap(
            try fixtureValue(at: settlementPath[...], in: canonical) as? [String: Any]
        )
        XCTAssertEqual(settlement.count, 7)
        XCTAssertNil(settlement["native_amx_receipts"])
        let data = try JSONSerialization.data(withJSONObject: settlement)
        let decoded = try JSONDecoder().decode(ToriiNativeAmxParticipantSettlement.self, from: data)
        XCTAssertEqual(decoded.sourceIds.count, 2)
        XCTAssertNil(decoded.previousNativeSettlementHash)

        var recursive = settlement
        recursive["native_amx_receipts"] = [Any]()
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiNativeAmxParticipantSettlement.self,
                from: JSONSerialization.data(withJSONObject: recursive)
            )
        )
        for requiredField in settlement.keys {
            var missing = settlement
            missing.removeValue(forKey: requiredField)
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiNativeAmxParticipantSettlement.self,
                    from: JSONSerialization.data(withJSONObject: missing)
                ),
                "missing required participant settlement field: \(requiredField)"
            )
        }
    }

    func testParticipantProposalRequiresExplicitNullPayloadHint() throws {
        try requireNativeAmxABI23Bridge()
        let canonical = try loadNativeAmxGroupedFixture()
        let proposalPath = [
            "golden", "expected_diagnostics", "lane_settlement_commitments", "0",
            "native_amx_receipts", "0", "legs", "0", "participant_proposal",
        ]
        let proposal = try XCTUnwrap(
            try fixtureValue(at: proposalPath[...], in: canonical) as? [String: Any]
        )
        XCTAssertTrue(proposal.keys.contains("payload_block_hint"))
        XCTAssertTrue(proposal["payload_block_hint"] is NSNull)

        var missing = proposal
        missing.removeValue(forKey: "payload_block_hint")
        var nonnull = proposal
        nonnull["payload_block_hint"] = ["proposal_height": 1]
        var unknown = proposal
        unknown["future_proposal_field"] = NSNull()

        for invalidProposal in [missing, nonnull, unknown] {
            let mutated = try assigningFixtureValue(
                invalidProposal,
                at: proposalPath[...],
                in: canonical
            )
            let document = try XCTUnwrap(mutated as? [String: Any])
            let golden = try XCTUnwrap(document["golden"] as? [String: Any])
            let diagnostics = try XCTUnwrap(golden["expected_diagnostics"])
            let data = try JSONSerialization.data(withJSONObject: diagnostics)
            XCTAssertThrowsError(
                try JSONDecoder().decode(
                    ToriiSumeragiDiagnosticsSnapshot.self,
                    from: data
                )
            )
        }
    }

    func testParticipantSettlementRejectsNestedNativeAmxReceipts() throws {
        try requireNativeAmxABI23Bridge()
        let canonical = try loadNativeAmxGroupedFixture()
        let settlementPath = [
            "golden", "expected_diagnostics", "lane_settlement_commitments", "0",
            "native_amx_receipts", "0", "legs", "0", "participant_settlement",
        ]
        var settlement = try XCTUnwrap(
            try fixtureValue(at: settlementPath[...], in: canonical) as? [String: Any]
        )
        XCTAssertNil(settlement["native_amx_receipts"])
        settlement["native_amx_receipts"] = []
        let mutated = try assigningFixtureValue(
            settlement,
            at: settlementPath[...],
            in: canonical
        )
        let document = try XCTUnwrap(mutated as? [String: Any])
        let golden = try XCTUnwrap(document["golden"] as? [String: Any])
        let diagnostics = try XCTUnwrap(golden["expected_diagnostics"])
        let data = try JSONSerialization.data(withJSONObject: diagnostics)
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiSumeragiDiagnosticsSnapshot.self, from: data)
        )
    }

    func testRustOwnedGroupedNativeAmxV2EndpointSeparation() async throws {
        try requireNativeAmxABI23Bridge()
        let document = try loadNativeAmxGroupedFixture()
        let golden = try XCTUnwrap(document["golden"] as? [String: Any])
        let diagnosticsObject = try XCTUnwrap(
            golden["expected_diagnostics"] as? [String: Any]
        )
        let diagnosticsData = try JSONSerialization.data(
            withJSONObject: diagnosticsObject
        )
        let applicationRows = try XCTUnwrap(
            diagnosticsObject[
                "native_amx_participant_applications"
            ] as? [[String: Any]]
        )
        let canonicalHash = try XCTUnwrap(
            applicationRows.first?["lane_incarnation"] as? String
        )
        let idle: [String: Any] = ["stage": "idle", "details": NSNull()]
        let statusObject: [String: Any] = [
            "protocol_version": 4,
            "node_fingerprint": canonicalHash,
            "build_fingerprint": canonicalHash,
            "config_fingerprint": canonicalHash,
            "restart_required": false,
            "height_context_id": [canonicalHash],
            "height": 1,
            "view": 0,
            "phase": ["phase": "awaiting_proposal", "details": NSNull()],
            "leader": 0,
            "body_state": ["state": "missing", "details": NSNull()],
            "last_committed_height": 0,
            "height_context": [
                "epoch": 0,
                "epoch_end_height": 1,
                "mode": ["mode": "permissioned", "details": NSNull()],
                "epoch_seed": [UInt8](repeating: 1, count: 32),
                "validator_count": 4,
                "quorum": ["min_signers": 3, "total_power": 4],
            ],
            "liveness": [
                "generation": 0,
                "prepare_quorums": [],
                "commit_quorums": [],
                "timeout_quorums": [],
                "outbound_intents": [],
                "work": [
                    "candidate": idle,
                    "body_recovery": idle,
                    "body_store": idle,
                    "validation": idle,
                    "application": idle,
                    "successor_height": idle,
                ],
                "queues": [],
                "no_progress_age_ms": 0,
                "ignore_counts": [],
            ],
        ]
        let statusData = try JSONSerialization.data(withJSONObject: statusObject)
        _ = try JSONDecoder().decode(
            ToriiSumeragiStatusSnapshot.self,
            from: statusData
        )

        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [NativeAmxGroupedEndpointURLProtocol.self]
        let session = URLSession(configuration: configuration)
        let operatorSigningKey = try SigningKey.ed25519(
            privateKey: Data(repeating: 0x6B, count: 32)
        )
        let operatorSigningContext = try ToriiOperatorSigningContext(
            networkId: TestNetworkIds.canonical,
            signingKey: operatorSigningKey
        )
        let client = ToriiClient(
            baseURL: URL(string: "https://native-amx-grouped.test")!,
            session: session,
            operatorSigningContext: operatorSigningContext
        )
        defer {
            NativeAmxGroupedEndpointURLProtocol.handler = nil
            client.invalidateAndCancel()
        }

        func response(
            for request: URLRequest,
            body: Data
        ) -> (HTTPURLResponse, Data?) {
            (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )!,
                body
            )
        }

        var requestedPaths: [String] = []
        NativeAmxGroupedEndpointURLProtocol.handler = { request in
            requestedPaths.append(request.url?.path ?? "<missing>")
            return response(for: request, body: diagnosticsData)
        }
        let diagnostics = try await client.getSumeragiDiagnostics()
        XCTAssertEqual(
            diagnostics.nativeAmxParticipantApplications.first?.sourceCount,
            2
        )
        XCTAssertEqual(requestedPaths, ["/v1/sumeragi/diagnostics"])

        requestedPaths = []
        NativeAmxGroupedEndpointURLProtocol.handler = { request in
            requestedPaths.append(request.url?.path ?? "<missing>")
            return response(for: request, body: diagnosticsData)
        }
        do {
            _ = try await client.getSumeragiStatus()
            XCTFail("status endpoint must reject a diagnostics-shaped payload")
        } catch let error as ToriiClientError {
            guard case .decoding = error else {
                XCTFail("expected status decoding failure, got \(error)")
                return
            }
        }
        XCTAssertEqual(requestedPaths, ["/v1/sumeragi/status"])

        requestedPaths = []
        NativeAmxGroupedEndpointURLProtocol.handler = { request in
            requestedPaths.append(request.url?.path ?? "<missing>")
            return response(for: request, body: statusData)
        }
        do {
            _ = try await client.getSumeragiDiagnostics()
            XCTFail("diagnostics endpoint must reject a status-shaped payload")
        } catch let error as ToriiClientError {
            guard case .decoding = error else {
                XCTFail("expected diagnostics decoding failure, got \(error)")
                return
            }
        }
        XCTAssertEqual(requestedPaths, ["/v1/sumeragi/diagnostics"])

        // Insert the number only after serialization so the hostile decimal or
        // exponent lexeme reaches the endpoint unchanged from the Rust fixture.
        func rawNumberPayload(_ lexeme: String, at path: String, in object: Any) throws -> Data {
            let marker = "__native_amx_raw_integer_token__"
            let marked = try assigningFixtureValue(
                marker,
                at: try pointerTokens(path)[...],
                in: object
            )
            let text = String(
                decoding: try JSONSerialization.data(withJSONObject: marked, options: [.sortedKeys]),
                as: UTF8.self
            )
            let quotedMarker = "\"\(marker)\""
            guard text.components(separatedBy: quotedMarker).count == 2 else {
                throw NativeAmxGroupedFixtureError.malformed("raw integer marker must occur once")
            }
            return Data(text.replacingOccurrences(of: quotedMarker, with: lexeme).utf8)
        }

        let roundHeightPath = "/lane_settlement_commitments/0/native_amx_receipts/0"
            + "/legs/0/prepare_qc/body/round/height"
        let nativeHeight = try XCTUnwrap(
            try fixtureValue(at: pointerTokens(roundHeightPath)[...], in: diagnosticsObject) as? NSNumber
        ).uint64Value
        for (path, lexeme) in [
            (roundHeightPath, "\(nativeHeight).0"),
            (roundHeightPath, "\(nativeHeight)e0"),
            (roundHeightPath, "\(nativeHeight)E+0"),
            ("/tx_queue_capacity", "1.0"),
        ] {
            let malformed = try rawNumberPayload(lexeme, at: path, in: diagnosticsObject)
            requestedPaths = []
            NativeAmxGroupedEndpointURLProtocol.handler = { request in
                requestedPaths.append(request.url?.path ?? "<missing>")
                return response(for: request, body: malformed)
            }
            do {
                _ = try await client.getSumeragiDiagnostics()
                XCTFail("diagnostics must reject the raw non-integer token \(lexeme)")
            } catch let error as ToriiClientError {
                guard case let .invalidPayload(reason) = error else {
                    XCTFail("expected raw integer-token rejection, got \(error)")
                    continue
                }
                XCTAssertTrue(reason.contains("integer numeric tokens"))
            }
            XCTAssertEqual(requestedPaths, ["/v1/sumeragi/diagnostics"])
        }

        for (path, lexeme) in [
            (roundHeightPath, "true"),
            ("/tx_queue_saturated", "0"),
            ("/tx_queue_capacity", "18446744073709551616"),
        ] {
            let malformed = try rawNumberPayload(lexeme, at: path, in: diagnosticsObject)
            NativeAmxGroupedEndpointURLProtocol.handler = { request in
                response(for: request, body: malformed)
            }
            do {
                _ = try await client.getSumeragiDiagnostics()
                XCTFail("diagnostics must reject Boolean/integer confusion and u64 overflow")
            } catch let error as ToriiClientError {
                guard case .decoding = error else {
                    XCTFail("expected typed integer/Boolean rejection, got \(error)")
                    continue
                }
            }
        }

        let exactFraction = try assigningFixtureValue(
            "0.5",
            at: pointerTokens("/lane_settlement_commitments/0/total_xor_due")[...],
            in: diagnosticsObject
        )
        let maximumInteger = try rawNumberPayload(
            "18446744073709551615",
            at: "/tx_queue_capacity",
            in: exactFraction
        )
        NativeAmxGroupedEndpointURLProtocol.handler = { request in
            response(for: request, body: maximumInteger)
        }
        let maximumSnapshot = try await client.getSumeragiDiagnostics()
        XCTAssertEqual(maximumSnapshot.txQueueCapacity, UInt64.max)
        XCTAssertEqual(maximumSnapshot.fields["tx_queue_capacity"], .integer("18446744073709551615"))
        XCTAssertEqual(maximumSnapshot.laneSettlementCommitments.first?.totalXorDue, "0.5")
    }

    func testRustOwnedGroupedNativeAmxV2NegativeCorpus() throws {
        try requireNativeAmxABI23Bridge()
        let canonical = try loadNativeAmxGroupedFixture()
        let controls = try XCTUnwrap(canonical["negative_controls"] as? [[String: Any]])
        XCTAssertEqual(controls.count, 58)
        let identifiers = Set(controls.compactMap { $0["id"] as? String })
        XCTAssertTrue(
            Set([
                "coherent_forged_validator_set_hash",
                "coherent_stale_descriptor_hash",
                "coherent_stale_proposal_hash",
                "coherent_stale_settlement_hash",
                "coherent_duplicate_validator_set",
                "coherent_over_quorum_requirement",
                "manifest_leaf_hash_tampering",
                "missing_previous_native_settlement_hash",
                "manifest_missing_previous_native_settlement_hash",
                "non_canonical_validator_peer_id",
                "execution_commitment_merge_carrier_wrong_version",
                "execution_commitment_missing_merge_carrier_field",
            ]).isSubset(of: identifiers)
        )

        // This value has the exact multihash tag and byte length expected for
        // BLS-Normal, but its all-zero compressed point is invalid. Exercise
        // key admission directly so rejection does not depend on stale hashes
        // in the corpus mutation.
        let invalidCompressedPoint = "ea0130" + String(repeating: "00", count: 48)
        XCTAssertFalse(
            ToriiNativeAmxWire.isCanonicalBlsNormalPeerId(invalidCompressedPoint)
        )
        for control in controls {
            let identifier = try XCTUnwrap(control["id"] as? String)
            try XCTContext.runActivity(named: identifier) { _ in
                XCTAssertEqual(control["expectation"] as? String, "reject")
                var mutated: Any = canonical
                for mutation in try XCTUnwrap(control["mutations"] as? [[String: Any]]) {
                    mutated = try applyFixtureMutation(mutation, to: mutated)
                }
                let root = try XCTUnwrap(mutated as? [String: Any])
                if control["validator"] as? String == "application_evidence" {
                    XCTAssertThrowsError(
                        try validateApplicationEvidenceFixture(root)
                    )
                } else {
                    XCTAssertEqual(control["validator"] as? String, "receipt_group")
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
}
