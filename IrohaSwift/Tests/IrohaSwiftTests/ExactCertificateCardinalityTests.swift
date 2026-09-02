import Foundation
import XCTest
@testable import IrohaSwift

final class ExactCertificateCardinalityTests: XCTestCase {
    private let hashLiteral =
        "hash:8F3BF00C1044E5A52CC20C0B914403FA7A8DCA29636504BA565FE3C1B646EACB#DD4A"

    private func payload(signerCount: Int, laneManifest: Any? = NSNull()) throws -> Data {
        let round: [String: Any] = [
            "context_id": [hashLiteral], "height": 1, "view": 0,
        ]
        let subject: [String: Any] = [
            "parent_block_hash": NSNull(), "block_hash": hashLiteral, "payload_hash": hashLiteral,
        ]
        var commitment: [String: Any] = [
            "parent_state_root": hashLiteral,
            "post_state_root": hashLiteral,
            "ordinary_writes_root": hashLiteral,
            "offline_cash_top_up_count": 0,
            "native_amx_application_manifest_version": 1,
            "native_amx_application_manifest_root":
                ToriiSumeragiV2ExecutionCommitment.nativeAmxApplicationManifestEmptyRoot,
            "native_amx_application_manifest_count": 0,
            "merge_carrier": NSNull(),
            "executed_block_wire_len": 1,
            "executed_block_wire_hash": hashLiteral,
        ]
        if let laneManifest { commitment["lane_finality_manifest"] = laneManifest }
        return try JSONSerialization.data(withJSONObject: [
            "certificate": [
                "round": round,
                "proposal_round": round,
                "phase": ["phase": "commit", "details": NSNull()],
                "subject": subject,
                "execution_commitment": commitment,
            ],
            "validator_count": 4,
            "signer_count": signerCount,
            "min_signers": 3,
            "signed_power": signerCount,
            "total_power": 4,
        ])
    }

    func testCommitQCRejectsSignerSuperset() throws {
        func decode(_ laneManifest: Any? = NSNull()) throws -> ToriiSumeragiV2CommitQcStatus {
            try JSONDecoder().decode(
                ToriiSumeragiV2CommitQcStatus.self,
                from: payload(signerCount: 3, laneManifest: laneManifest)
            )
        }
        XCTAssertNil(try decode().certificate.executionCommitment.laneFinalityManifest)
        let lane = ["root": hashLiteral, "leaf_count": 1] as [String: Any]
        let parsedLane = try decode(lane).certificate.executionCommitment.laneFinalityManifest
        XCTAssertEqual(parsedLane?.root, hashLiteral)
        XCTAssertEqual(parsedLane?.leafCount, 1)
        for invalid: Any? in [nil, ["leaf_count": 1], ["root": hashLiteral, "leaf_count": 0],
                              ["root": hashLiteral, "leaf_count": 1025]] {
            XCTAssertThrowsError(try decode(invalid))
        }
        _ = try JSONDecoder().decode(
            ToriiSumeragiV2CommitQcStatus.self,
            from: payload(signerCount: 3)
        )
        XCTAssertThrowsError(
            try JSONDecoder().decode(
                ToriiSumeragiV2CommitQcStatus.self,
                from: payload(signerCount: 4)
            )
        )
    }
}
