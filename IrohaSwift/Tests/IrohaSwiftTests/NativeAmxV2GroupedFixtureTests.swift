import Foundation
#if canImport(NoritoBridge)
import NoritoBridge
#endif
@testable import IrohaSwift
import XCTest

private enum NativeAmxGroupedFixtureError: Error {
    case malformed(String)
}

private func requireNativeAmxABI21Bridge() throws {
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
        "Native AMX V2 parity requires the ABI-21 NoritoBridge module"
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
        parsedExecution.mergeCarrier != nil,
        "merge carrier"
    )
    try require(
        try fixtureUInt(execution, "native_amx_application_manifest_count")
            == UInt64(artifacts.count) && artifacts.count == 1,
        "manifest count"
    )
    let artifact = artifacts[0]
    let leaf = try XCTUnwrap(artifact["leaf"] as? [String: Any])
    let proof = try XCTUnwrap(artifact["proof"] as? [String: Any])
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
        try fixtureUInt(artifact, "manifest_leaf_count") == 1
            && fixtureScalarEqual(
                artifact["manifest_root"],
                execution["native_amx_application_manifest_root"]
            )
            && fixtureScalarEqual(artifact["manifest_root"], artifact["leaf_hash"]),
        "manifest root"
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
    func testRustOwnedGroupedNativeAmxV2GoldenFixture() throws {
        try requireNativeAmxABI21Bridge()
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
        XCTAssertEqual(
            firstLeg.participantSettlementHash,
            "hash:C6B18DBE6BEC468DB021B79604233F3CB9E2D6CDF3384C491CE7A6DA89747825#9D72"
        )
        let remoteLeg = try XCTUnwrap(
            group.nativeAmxReceipts.first?.legs.dropFirst().first
        )
        XCTAssertEqual(
            remoteLeg.participantSettlementHash,
            "hash:40C7FCA7AA143B323B473A9958B96F49896C03C3547B83DD340FAE2FC1A85D29#B452"
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
                    leg.participantSettlement.receipts.map(\.sourceId),
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
        try validateApplicationEvidenceFixture(document)
    }

    func testRustOwnedGroupedNativeAmxV2EndpointSeparation() async throws {
        try requireNativeAmxABI21Bridge()
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
                "validator_count": 1,
                "quorum": ["min_signers": 1, "total_power": 1],
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
        let client = ToriiClient(
            baseURL: URL(string: "https://native-amx-grouped.test")!,
            session: session
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
    }

    func testRustOwnedGroupedNativeAmxV2NegativeCorpus() throws {
        try requireNativeAmxABI21Bridge()
        let canonical = try loadNativeAmxGroupedFixture()
        let controls = try XCTUnwrap(canonical["negative_controls"] as? [[String: Any]])
        let identifiers = Set(controls.compactMap { $0["id"] as? String })
        XCTAssertTrue(
            Set([
                "coherent_forged_validator_set_hash",
                "coherent_stale_descriptor_hash",
                "coherent_stale_proposal_hash",
                "coherent_stale_settlement_hash",
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
