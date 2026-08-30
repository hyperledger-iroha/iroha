import Foundation
import XCTest
@testable import IrohaSwift

final class PrivacyFinalizedStateModelsV1Tests: XCTestCase {
    private func fixed(_ byte: UInt8) -> Data {
        Data(repeating: byte, count: 32)
    }

    private func bytes(_ byte: UInt8) -> [Int] {
        Array(repeating: Int(byte), count: 32)
    }

    private func canonicalHash(_ byte: UInt8) throws -> String {
        var value = fixed(byte)
        value[value.count - 1] |= 1
        return try NetworkId(bytes: value).literal
    }

    func testClosedQueryIdsAndSelectorOrder() throws {
        let replay = try PrivacyZkAceReplayNullifierRequestV1(
            policyId: fixed(1),
            replayNullifier: fixed(2)
        )
        XCTAssertEqual(PrivacyZkAceReplayNullifierRequestV1.queryId.rawValue, 97)
        XCTAssertEqual(replay.protocolIndex, 0)
        XCTAssertEqual(replay.requestBinding, fixed(1) + fixed(2))

        let admission = try PrivacyZkAmsAdmissionRequestV1(
            issuerId: fixed(3),
            registryId: fixed(4),
            policyId: fixed(5),
            phcHash: fixed(6)
        )
        XCTAssertEqual(PrivacyZkAmsAdmissionRequestV1.queryId.rawValue, 102)
        XCTAssertEqual(
            admission.requestBinding,
            fixed(3) + fixed(4) + fixed(5) + fixed(6)
        )

        let certificate = try PrivacyZkX509CertificateNullifierRequestV1(
            trustAnchorId: fixed(7),
            policyId: fixed(8),
            nullifier: fixed(9)
        )
        XCTAssertEqual(
            PrivacyZkX509CertificateNullifierRequestV1.queryId.rawValue,
            104
        )
        XCTAssertEqual(certificate.requestBinding, fixed(7) + fixed(8) + fixed(9))
    }

    func testRequestSelectorsRejectWrongWidthAndZero() {
        XCTAssertThrowsError(
            try PrivacyOrchardPoolStateRequestV1(poolId: Data(repeating: 1, count: 31))
        )
        XCTAssertThrowsError(
            try PrivacyOrchardPoolStateRequestV1(poolId: Data(repeating: 0, count: 32))
        )
        XCTAssertThrowsError(
            try PrivacyZkAmsProvisionRequestV1(
                issuerId: fixed(1),
                registryId: fixed(2),
                policyId: fixed(3),
                keyImage: Data(repeating: 0, count: 32)
            )
        )
    }

    func testProofManagedProtocolIndicesAreClosed() throws {
        XCTAssertEqual(
            try PrivacyProofManagedPoolStateRequestV1(
                protocolId: .moneroFcmpPlusPlusV1,
                poolId: fixed(1)
            ).protocolIndex,
            0
        )
        XCTAssertEqual(
            try PrivacyProofManagedPoolStateRequestV1(
                protocolId: .irohaIvmPrivateNoteStarkV1,
                poolId: fixed(1)
            ).protocolIndex,
            1
        )
        XCTAssertEqual(
            try PrivacyProofManagedPoolStateRequestV1(
                protocolId: .pqMaspStarkV0,
                poolId: fixed(1)
            ).protocolIndex,
            2
        )
        XCTAssertThrowsError(
            try PrivacyProofManagedPoolStateRequestV1(
                protocolId: .orchardHalo2ActionsV1,
                poolId: fixed(1)
            )
        )
    }

    func testReplayProjectionUsesCanonicalHashLiteralsAndDecimalStrings() throws {
        let json: [String: Any] = [
            "network_id": try canonicalHash(1),
            "policy_id": bytes(2),
            "replay_nullifier": bytes(3),
            "policy_record_digest": bytes(4),
            "statement_digest": bytes(5),
            "admitted_at_height": "7",
            "action_index": "0",
            "finalized_height": "9",
            "finalized_block_hash": try canonicalHash(7),
        ]
        let decoded = try JSONDecoder().decode(
            PrivacyZkAceReplayNullifierProvenanceV1.self,
            from: JSONSerialization.data(withJSONObject: json)
        )
        XCTAssertEqual(decoded.networkId.bytes, fixed(1))
        XCTAssertEqual(decoded.policyId, fixed(2))
        XCTAssertEqual(decoded.admittedAtHeight, 7)
        XCTAssertEqual(decoded.actionIndex, 0)
        XCTAssertEqual(decoded.finalizedHeight, 9)
        XCTAssertEqual(decoded.finalizedBlockHash, fixed(7))
    }

    func testProjectionRejectsNoncanonicalHashAndNumericLeaves() throws {
        let canonical = try canonicalHash(1)
        var json: [String: Any] = [
            "network_id": canonical.lowercased(),
            "policy_id": bytes(2),
            "replay_nullifier": bytes(3),
            "policy_record_digest": bytes(4),
            "statement_digest": bytes(5),
            "admitted_at_height": "7",
            "action_index": "0",
            "finalized_height": "9",
            "finalized_block_hash": try canonicalHash(7),
        ]
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                PrivacyZkAceReplayNullifierProvenanceV1.self,
                from: JSONSerialization.data(withJSONObject: json)
            )
        )

        json["network_id"] = canonical
        json["finalized_height"] = 9
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                PrivacyZkAceReplayNullifierProvenanceV1.self,
                from: JSONSerialization.data(withJSONObject: json)
            )
        )
    }

    func testProjectionRejectsZeroAndTrailingFixedBytes() throws {
        var json: [String: Any] = [
            "network_id": try canonicalHash(1),
            "policy_id": bytes(2),
            "replay_nullifier": bytes(3),
            "policy_record_digest": bytes(4),
            "statement_digest": bytes(5),
            "admitted_at_height": "7",
            "action_index": "0",
            "finalized_height": "9",
            "finalized_block_hash": try canonicalHash(7),
        ]
        json["policy_id"] = Array(repeating: 0, count: 32)
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                PrivacyZkAceReplayNullifierProvenanceV1.self,
                from: JSONSerialization.data(withJSONObject: json)
            )
        )
        json["policy_id"] = Array(repeating: 2, count: 33)
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                PrivacyZkAceReplayNullifierProvenanceV1.self,
                from: JSONSerialization.data(withJSONObject: json)
            )
        )
    }

    func testTaggedProtocolRootRoleAndBalanceScopeProjection() throws {
        let protocolProjection = Data(
            #"{"protocol":"pq-masp-stark-v0","value":null}"#.utf8
        )
        let protocolId = try JSONDecoder().decode(
            PrivacyFinalizedProtocolIdV1.self,
            from: protocolProjection
        )
        XCTAssertEqual(protocolId.wrappedValue, .pqMaspStarkV0)

        let roleProjection = Data(#"{"role":"OutputSet","value":null}"#.utf8)
        let role = try JSONDecoder().decode(
            PrivacyFinalizedRootRoleProjectionV1.self,
            from: roleProjection
        )
        XCTAssertEqual(role.wrappedValue, .outputSet)

        let global = try JSONDecoder().decode(
            PrivacyFinalizedAssetBalanceScopeV1.self,
            from: Data(#"{"kind":"Global","content":null}"#.utf8)
        )
        XCTAssertEqual(global, .global)
        let dataspace = try JSONDecoder().decode(
            PrivacyFinalizedAssetBalanceScopeV1.self,
            from: Data(#"{"kind":"Dataspace","content":"42"}"#.utf8)
        )
        XCTAssertEqual(dataspace, .dataspace(42))
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                PrivacyFinalizedAssetBalanceScopeV1.self,
                from: Data(#"{"kind":"Dataspace","content":"0"}"#.utf8)
            )
        )
    }
}
