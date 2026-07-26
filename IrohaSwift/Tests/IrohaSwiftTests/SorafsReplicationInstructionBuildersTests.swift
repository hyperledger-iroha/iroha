import Foundation
import XCTest
@testable import IrohaSwift

final class SorafsReplicationInstructionBuildersTests: XCTestCase {
    private let orderId = String(repeating: "ab", count: 32)
    private let providerId = String(repeating: "10", count: 32)

    func testIssueUsesExactRustFieldsAndValidatesFixture() throws {
        let fixture = try replicationFixture()
        let instruction = try SorafsReplicationInstructionBuilders.issueReplicationOrder(
            orderId: orderId,
            orderPayload: fixture,
            issuedEpoch: 20,
            deadlineEpoch: 28
        )
        let outer = try XCTUnwrap(
            JSONSerialization.jsonObject(with: instruction.data) as? [String: Any]
        )
        XCTAssertEqual(Set(outer.keys), ["IssueReplicationOrder"])
        let body = try XCTUnwrap(outer["IssueReplicationOrder"] as? [String: Any])
        XCTAssertEqual(
            Set(body.keys),
            ["order_id", "order_payload", "issued_epoch", "deadline_epoch"]
        )
        XCTAssertEqual(body["order_id"] as? String, orderId)
        XCTAssertEqual(body["order_payload"] as? String, fixture.base64EncodedString())
        XCTAssertEqual((body["issued_epoch"] as? NSNumber)?.uint64Value, 20)
        XCTAssertEqual((body["deadline_epoch"] as? NSNumber)?.uint64Value, 28)

        let summary = try SorafsReplicationInstructionBuilders.validateOrderPayloadV1(
            fixture,
            expectedOrderId: orderId
        )
        XCTAssertEqual(summary.orderId, orderId)
        XCTAssertEqual(summary.targetReplicas, 2)
        XCTAssertEqual(summary.providerIds, [
            String(repeating: "10", count: 32),
            String(repeating: "11", count: 32),
        ])
        XCTAssertEqual(
            try SorafsReplicationInstructionBuilders.decode(instruction),
            .issue(try SorafsIssueReplicationOrderInstruction(
                orderId: orderId,
                orderPayload: fixture,
                issuedEpoch: 20,
                deadlineEpoch: 28
            ))
        )
    }

    func testCompleteIsProviderSpecificAndDecodingIsSchemaClosed() throws {
        let instruction = try SorafsReplicationInstructionBuilders.completeReplicationOrder(
            orderId: orderId,
            providerId: providerId,
            completionEpoch: 27
        )
        let decoded = try SorafsReplicationInstructionBuilders.decode(instruction)
        XCTAssertEqual(
            decoded,
            .complete(try SorafsCompleteReplicationOrderInstruction(
                orderId: orderId,
                providerId: providerId,
                completionEpoch: 27
            ))
        )

        let legacy = try NoritoJSON.fromJSONObject([
            "CompleteReplicationOrder": [
                "order_id": orderId,
                "completion_epoch": 27,
            ],
        ])
        XCTAssertThrowsError(try SorafsReplicationInstructionBuilders.decode(legacy))

        let confused = try NoritoJSON.fromJSONObject([
            "CompleteReplicationOrder": [
                "order_id": orderId,
                "provider_id": providerId,
                "completion_epoch": 27,
                "relayer": "not-an-authority",
            ],
        ])
        XCTAssertThrowsError(try SorafsReplicationInstructionBuilders.decode(confused))
        XCTAssertThrowsError(
            try SorafsCompleteReplicationOrderInstruction(
                orderId: orderId,
                providerId: String(repeating: "00", count: 32),
                completionEpoch: 27
            )
        )
        XCTAssertThrowsError(
            try SorafsCompleteReplicationOrderInstruction(
                orderId: orderId.uppercased(),
                providerId: providerId,
                completionEpoch: 27
            )
        )
    }

    func testIssueRejectsInvalidTargetAssignmentsAndDeadline() throws {
        let fixture = try replicationFixture()
        XCTAssertThrowsError(
            try SorafsIssueReplicationOrderInstruction(
                orderId: orderId,
                orderPayloadBase64: fixture.base64EncodedString() + "\n",
                issuedEpoch: 1,
                deadlineEpoch: 2
            )
        )
        XCTAssertThrowsError(
            try SorafsIssueReplicationOrderInstruction(
                orderId: String(repeating: "ac", count: 32),
                orderPayload: fixture,
                issuedEpoch: 1,
                deadlineEpoch: 2
            )
        )
        XCTAssertThrowsError(
            try SorafsIssueReplicationOrderInstruction(
                orderId: orderId,
                orderPayload: Data(
                    repeating: 0,
                    count: sorafsReplicationOrderMaxPayloadBytesV1 + 1
                ),
                issuedEpoch: 1,
                deadlineEpoch: 2
            )
        )
        XCTAssertThrowsError(
            try SorafsIssueReplicationOrderInstruction(
                orderId: orderId,
                orderPayload: fixture,
                issuedEpoch: 2,
                deadlineEpoch: 2
            )
        )

        let duplicateProvider = try mutate(
            fixture,
            replacing: Data(repeating: 0x11, count: 32),
            with: Data(repeating: 0x10, count: 32)
        )
        XCTAssertThrowsError(
            try SorafsIssueReplicationOrderInstruction(
                orderId: orderId,
                orderPayload: duplicateProvider,
                issuedEpoch: 1,
                deadlineEpoch: 2
            )
        )

        let zeroTarget = try mutate(
            fixture,
            replacing: Data([0x02, 0x02, 0x00]),
            with: Data([0x02, 0x00, 0x00])
        )
        XCTAssertThrowsError(
            try SorafsIssueReplicationOrderInstruction(
                orderId: orderId,
                orderPayload: zeroTarget,
                issuedEpoch: 1,
                deadlineEpoch: 2
            )
        )

        let invalidDeadline = try mutate(
            fixture,
            replacing: littleEndian(1_700_086_400),
            with: littleEndian(1_700_000_000)
        )
        XCTAssertThrowsError(
            try SorafsIssueReplicationOrderInstruction(
                orderId: orderId,
                orderPayload: invalidDeadline,
                issuedEpoch: 1,
                deadlineEpoch: 2
            )
        )
    }

    func testExpireRejectsUnknownFieldsAndNegativeDecodedEpoch() throws {
        let expire = try SorafsReplicationInstructionBuilders.expireReplicationOrder(
            orderId: orderId,
            expirationEpoch: 29
        )
        XCTAssertEqual(
            try SorafsReplicationInstructionBuilders.decode(expire),
            .expire(try SorafsExpireReplicationOrderInstruction(
                orderId: orderId,
                expirationEpoch: 29
            ))
        )

        let unknown = try NoritoJSON.fromJSONObject([
            "ExpireReplicationOrder": [
                "order_id": orderId,
                "expiration_epoch": 29,
                "legacy": true,
            ],
        ])
        XCTAssertThrowsError(try SorafsReplicationInstructionBuilders.decode(unknown))

        let negative = try NoritoJSON.fromJSONObject([
            "ExpireReplicationOrder": [
                "order_id": orderId,
                "expiration_epoch": -1,
            ],
        ])
        XCTAssertThrowsError(try SorafsReplicationInstructionBuilders.decode(negative))
    }

    private func replicationFixture() throws -> Data {
        let testFile = URL(fileURLWithPath: #filePath)
        let url = testFile
            .deletingLastPathComponent()
            .appendingPathComponent(
                "../../../fixtures/sorafs_manifest/replication_order/order_v1.to"
            )
            .standardizedFileURL
        return try Data(contentsOf: url)
    }

    private func mutate(
        _ fixture: Data,
        replacing needle: Data,
        with replacement: Data
    ) throws -> Data {
        XCTAssertEqual(needle.count, replacement.count)
        var mutated = fixture
        let searchRange = NoritoHeader.encodedLength..<mutated.count
        let range = try XCTUnwrap(mutated.range(of: needle, in: searchRange))
        XCTAssertNil(mutated.range(of: needle, in: range.upperBound..<mutated.count))
        mutated.replaceSubrange(range, with: replacement)
        let body = mutated.subdata(in: NoritoHeader.encodedLength..<mutated.count)
        mutated.replaceSubrange(31..<39, with: littleEndian(crc64ECMA(body)))
        return mutated
    }

    private func littleEndian(_ value: UInt64) -> Data {
        var encoded = value.littleEndian
        return withUnsafeBytes(of: &encoded) { Data($0) }
    }
}
