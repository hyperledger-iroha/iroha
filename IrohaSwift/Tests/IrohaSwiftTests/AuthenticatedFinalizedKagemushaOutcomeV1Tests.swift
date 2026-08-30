import Foundation
import XCTest
@testable import IrohaSwift

final class AuthenticatedFinalizedKagemushaOutcomeV1Tests: XCTestCase {
    private let markedHash = String(repeating: "00", count: 31) + "01"
    private let otherMarkedHash = String(repeating: "00", count: 31) + "03"
    private let operationIdHex = String(repeating: "02", count: 32)
    private let queryAuthority = "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE"

    override func tearDown() {
        StubURLProtocol.handler = nil
        super.tearDown()
    }

    func testCheckpointProjectionRejectsZeroAndU63PlusOne() throws {
        let context = Data(repeating: 0, count: 31) + Data([1])
        let checkpoint = try AuthenticatedFinalityCheckpointV1(
            height: Int64.max,
            heightContextId: context
        )
        XCTAssertEqual(
            try AuthenticatedFinalityCheckpointV1(projectionBytes: checkpoint.projection),
            checkpoint
        )

        var zero = Data(repeating: 0, count: 8)
        zero.append(context)
        XCTAssertThrowsError(try AuthenticatedFinalityCheckpointV1(projectionBytes: zero))

        var tooLarge = UInt64(Int64.max).addingReportingOverflow(1).partialValue.bigEndian
        var projection = withUnsafeBytes(of: &tooLarge) { Data($0) }
        projection.append(context)
        XCTAssertThrowsError(
            try AuthenticatedFinalityCheckpointV1(projectionBytes: projection),
            "u63+1 checkpoint height was accepted"
        )
    }

    func testPageBinderRejects65ProofsAndClosedByteBoundsBeforeNative() {
        XCTAssertThrowsError(
            try PrivacyNativeBridge.bindFinalityProofPageV1(
                Array(repeating: Data([1]), count: 65)
            )
        )
        XCTAssertThrowsError(
            try PrivacyNativeBridge.bindFinalityProofPageV1([
                Data(repeating: 1, count: AuthenticatedFinalityProofPageV1.maximumProofBytes + 1),
            ])
        )
        let maximumProof = Data(
            repeating: 1,
            count: AuthenticatedFinalityProofPageV1.maximumProofBytes
        )
        XCTAssertThrowsError(
            try PrivacyNativeBridge.bindFinalityProofPageV1(
                Array(repeating: maximumProof, count: 8)
            ),
            "aggregate page larger than 64 MiB was accepted"
        )
    }

    func testPersistedPageRequiresItsExactMarkedContentHash() throws {
        let archive = Data([0x01, 0x02, 0x03])
        var digest = Blake2b.hash256(archive)
        digest[digest.count - 1] |= 1
        let exactHash = digest.hexEncodedString()
        XCTAssertNoThrow(
            try AuthenticatedFinalityProofPageV1(
                evidenceArchive: archive,
                hashHex: exactHash
            )
        )
        XCTAssertThrowsError(
            try AuthenticatedFinalityProofPageV1(
                evidenceArchive: archive,
                hashHex: markedHash == exactHash ? otherMarkedHash : markedHash
            ),
            "content-addressed finality page accepted a substituted marked hash"
        )
    }

    func testAppliedAndRejectedOutcomeJSONAreClosedAndStatusOnlyIsNotAuthority() throws {
        let applied = try decodeOutcome(outcomeObject())
        XCTAssertEqual(applied.terminalState, .applied)
        XCTAssertNil(applied.rejectionCode)
        XCTAssertNil(applied.rejectionMessage)

        var rejected = outcomeObject()
        rejected["terminal_state"] = "rejected"
        rejected["rejection_code"] = "instruction_execution"
        rejected["rejection_message"] = "issuer policy rejected the request"
        XCTAssertEqual(try decodeOutcome(rejected).terminalState, .rejected)

        var inventedCode = rejected
        inventedCode["rejection_code"] = "server_error"
        XCTAssertThrowsError(try decodeOutcome(inventedCode))

        var oversizedUTF8 = rejected
        oversizedUTF8["rejection_message"] = String(repeating: "é", count: 600)
        XCTAssertThrowsError(try decodeOutcome(oversizedUTF8))

        let statusOnly: [String: Any] = [
            "operation_id": operationIdHex,
            "kind": "top_up",
            "state": "applied",
            "transaction_hash": markedHash,
            "finalized_block_height": 7,
        ]
        XCTAssertThrowsError(try decodeOutcome(statusOnly))

        var extra = outcomeObject()
        extra["status"] = "applied"
        XCTAssertThrowsError(try decodeOutcome(extra))
    }

    func testTopUpAgreementRejectsKindOperationTransactionHeightBlockAndContextSubstitutions() throws {
        let outcome = try decodeOutcome(outcomeObject())
        let specialized = try decodeSpecialized(specializedObject())
        XCTAssertNoThrow(
            try PrivacyNativeBridge.requireKagemushaTopUpFinalityAgreementV1(
                outcome: outcome,
                specialized: specialized
            )
        )

        var substitutions: [[String: Any]] = []
        var operation = specializedObject()
        operation["operation_id_hex"] = String(repeating: "04", count: 32)
        substitutions.append(operation)
        var transaction = specializedObject()
        transaction["transaction_hash_hex"] = otherMarkedHash
        substitutions.append(transaction)
        var height = specializedObject()
        height["committed_block_height"] = "8"
        substitutions.append(height)
        var block = specializedObject()
        block["block_hash_hex"] = otherMarkedHash
        substitutions.append(block)
        var context = specializedObject()
        context["height_context_id_hex"] = otherMarkedHash
        substitutions.append(context)

        for substituted in substitutions {
            XCTAssertThrowsError(
                try PrivacyNativeBridge.requireKagemushaTopUpFinalityAgreementV1(
                    outcome: outcome,
                    specialized: try decodeSpecialized(substituted)
                )
            )
        }

        var wrongKind = outcomeObject()
        wrongKind["operation_kind"] = "redeem"
        XCTAssertThrowsError(
            try PrivacyNativeBridge.requireKagemushaTopUpFinalityAgreementV1(
                outcome: try decodeOutcome(wrongKind),
                specialized: specialized
            )
        )
    }

    func testOutcomeRejectsCheckpointHeightAndContextShapeSubstitution() {
        var wrongHeight = outcomeObject()
        wrongHeight["committed_block_height"] = "8"
        XCTAssertThrowsError(try decodeOutcome(wrongHeight))

        var unmarkedContext = outcomeObject()
        unmarkedContext["finalized_checkpoint_hex"] = "0000000000000007"
            + String(repeating: "00", count: 32)
        XCTAssertThrowsError(try decodeOutcome(unmarkedContext))

        var tooLarge = outcomeObject()
        tooLarge["committed_block_height"] = "9223372036854775808"
        tooLarge["finalized_checkpoint_hex"] = "8000000000000000" + markedHash
        XCTAssertThrowsError(try decodeOutcome(tooLarge))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func testFinalityAndBlockWireTransportRequireExactStatusURLAndMediaType() async throws {
        let client = tcMakeClient()
        let proof = Data([0x01, 0x02, 0x03])
        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "GET")
            XCTAssertEqual(request.url?.path, "/v1/bridge/finality/7")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/x-norito")
            return (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: [
                        "Content-Type": "application/x-norito",
                        "Content-Length": "3",
                    ]
                )!,
                proof
            )
        }
        XCTAssertEqual(try await client.getBridgeFinalityProofV1(height: 7), proof)

        StubURLProtocol.handler = { request in
            XCTAssertEqual(request.url?.path, "/v1/ledger/block/7")
            return (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/x-norito; charset=binary"]
                )!,
                proof
            )
        }
        do {
            _ = try await client.getLedgerExecutedBlockWire(height: 7)
            XCTFail("parameterized content type was accepted")
        } catch {}
    }

    func testOnlyAuthenticatedDetails404CanBeTypedAbsence() throws {
        let client = tcMakeClient()
        let url = URL(string: "https://example.test/v1/pipeline/transactions/details")!
        let response = HTTPURLResponse(
            url: url,
            statusCode: 404,
            httpVersion: nil,
            headerFields: nil
        )!
        XCTAssertNil(
            try client.validatedExactNoritoResponseV1(
                data: Data(),
                response: response,
                requestURL: url,
                context: "authenticated transaction-details carrier V2",
                maximumBytes: 64 * 1_024 * 1_024,
                allowNotFound: true
            )
        )
        XCTAssertThrowsError(
            try client.validatedExactNoritoResponseV1(
                data: Data(),
                response: response,
                requestURL: url,
                context: "bridge finality proof",
                maximumBytes: AuthenticatedFinalityProofPageV1.maximumProofBytes,
                allowNotFound: false
            )
        )
        let changedURL = URL(string: "https://other.example/v1/pipeline/transactions/details")!
        let changedResponse = HTTPURLResponse(
            url: changedURL,
            statusCode: 404,
            httpVersion: nil,
            headerFields: nil
        )!
        XCTAssertThrowsError(
            try client.validatedExactNoritoResponseV1(
                data: Data(),
                response: changedResponse,
                requestURL: url,
                context: "authenticated transaction-details carrier V2",
                maximumBytes: 64 * 1_024 * 1_024,
                allowNotFound: true
            )
        )
    }

    private func outcomeObject() -> [String: Any] {
        [
            "version": 1,
            "terminal_state": "applied",
            "operation_id_hex": operationIdHex,
            "operation_kind": "top_up",
            "transaction_hash_hex": markedHash,
            "query_authority": queryAuthority,
            "transaction_authority": queryAuthority,
            "block_hash_hex": markedHash,
            "result_hash_hex": markedHash,
            "committed_block_height": "7",
            "finalized_checkpoint_hex": "0000000000000007" + markedHash,
            "executed_block_wire_hash_hex": markedHash,
            "rejection_code": NSNull(),
            "rejection_message": NSNull(),
            "evidence_id_hex": markedHash,
            "transaction_details_hash_hex": markedHash,
            "finality_page_hash_hex": markedHash,
        ]
    }

    private func specializedObject() -> [String: Any] {
        [
            "version": 4,
            "operation_id_hex": operationIdHex,
            "transaction_hash_hex": markedHash,
            "committed_block_height": "7",
            "block_hash_hex": markedHash,
            "height_context_id_hex": markedHash,
        ]
    }

    private func decodeOutcome(
        _ object: [String: Any]
    ) throws -> AuthenticatedFinalizedKagemushaOutcomeV1 {
        try JSONDecoder().decode(
            AuthenticatedFinalizedKagemushaOutcomeV1.self,
            from: JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
        )
    }

    private func decodeSpecialized(_ object: [String: Any]) throws -> VerifiedTopUpFinalityV4 {
        try JSONDecoder().decode(
            VerifiedTopUpFinalityV4.self,
            from: JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
        )
    }
}
