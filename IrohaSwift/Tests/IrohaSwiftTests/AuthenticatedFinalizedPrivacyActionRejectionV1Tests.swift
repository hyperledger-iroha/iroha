import Foundation
import XCTest
@testable import IrohaSwift

final class AuthenticatedFinalizedPrivacyActionRejectionV1Tests: XCTestCase {
    func testClosedSixCaseRejectionProjectionDecodesExactFinalityEvidence() throws {
        XCTAssertEqual(
            AuthenticatedPrivacyActionRejectionCodeV1.allCases.map(\.canonicalLabel),
            [
                "account_does_not_exist", "limit_check", "validation",
                "instruction_execution", "ivm_execution", "trigger_execution",
            ]
        )
        for code in AuthenticatedPrivacyActionRejectionCodeV1.allCases {
            var object = rejectionObject()
            object["rejection_code"] = code.canonicalLabel
            let rejection = try decode(object)
            XCTAssertEqual(rejection.rejectionCode, code)
            XCTAssertEqual(rejection.operationSchema, .zkAceAuthorizationActionV1)
            XCTAssertEqual(rejection.protocolId, .zkAcePqAuthorizationV0)
            XCTAssertEqual(rejection.ledgerEffectKind, .zkAceTransparentTransfer)
            XCTAssertEqual(rejection.actionIndex, 0)
            XCTAssertEqual(rejection.committedBlockHeight, 9)
            XCTAssertEqual(rejection.finalizedCheckpoint.height, 9)
        }
    }

    func testProjectionRejectsUnknownCodeExtraFieldAndContradictoryFinality() throws {
        var unknown = rejectionObject()
        unknown["rejection_code"] = "server_error"
        XCTAssertThrowsError(try decode(unknown))

        var extra = rejectionObject()
        extra["untrusted_status"] = "Rejected"
        XCTAssertThrowsError(try decode(extra))

        var wrongCheckpoint = rejectionObject()
        wrongCheckpoint["finalized_checkpoint_hex"] = checkpointHex(height: 8)
        XCTAssertThrowsError(try decode(wrongCheckpoint))

        var wrongEffect = rejectionObject()
        wrongEffect["ledger_effect_kind"] = "verification_only"
        XCTAssertThrowsError(try decode(wrongEffect))
    }

    private func decode(
        _ object: [String: Any]
    ) throws -> AuthenticatedFinalizedPrivacyActionRejectionV1 {
        try JSONDecoder().decode(
            AuthenticatedFinalizedPrivacyActionRejectionV1.self,
            from: JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
        )
    }

    private func rejectionObject() -> [String: Any] {
        [
            "version": 1,
            "network_id_hex": hash(0x11),
            "protocol_id": "zk-ace-pq-authorization-v0",
            "operation_schema": "zk_ace_authorization_action_v1",
            "ledger_effect_kind": "zk_ace_transparent_transfer",
            "transaction_hash_hex": hash(0x21),
            "action_index": 0,
            "transaction_intent_digest_hex": rawDigest(0x22),
            "statement_digest_hex": rawDigest(0x24),
            "proof_envelope_hash_hex": rawDigest(0x26),
            "query_authority": "wallet-query-authority",
            "transaction_authority": "exact12-transaction-authority",
            "block_hash_hex": hash(0x31),
            "result_hash_hex": hash(0x41),
            "rejection_code": "validation",
            "rejection_message": "Exact12 validation rejected the action",
            "committed_block_height": "9",
            "finalized_checkpoint_hex": checkpointHex(height: 9),
            "executed_block_wire_hash_hex": hash(0x51),
            "evidence_id_hex": hash(0x61),
            "transaction_details_hash_hex": hash(0x71),
            "finality_page_hash_hex": hash(0x81),
        ]
    }

    private func checkpointHex(height: UInt64) -> String {
        String(format: "%016llx", height) + hash(0x11)
    }

    private func hash(_ byte: UInt8) -> String {
        let marked = byte | 1
        return String(repeating: String(format: "%02x", marked), count: 32)
    }

    private func rawDigest(_ byte: UInt8) -> String {
        String(repeating: String(format: "%02x", byte), count: 32)
    }
}
