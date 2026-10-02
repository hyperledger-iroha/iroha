import Foundation
import XCTest
@testable import IrohaSwift

final class RetailFeeV1Tests: XCTestCase {
    private struct Fixture: Decodable {
        let request: RetailFeeQuoteRequestV1
        let assessment: RetailFeeAssessmentV1
        let intentHashHex: String
        let marker: String

        private enum CodingKeys: String, CodingKey {
            case request
            case assessment
            case intentHashHex = "intent_hash_hex"
            case marker
        }
    }

    private func fixture() throws -> Fixture {
        let url = try XCTUnwrap(Bundle.module.url(
            forResource: "retail_fee_codec_v1",
            withExtension: "json"
        ))
        return try JSONDecoder().decode(Fixture.self, from: Data(contentsOf: url))
    }

    func testTypedJSONUsesTheExactFirstReleaseFields() throws {
        let fixture = try fixture()
        let request = try JSONSerialization.jsonObject(
            with: JSONEncoder().encode(fixture.request)
        ) as? [String: Any]
        XCTAssertEqual(
            Set(try XCTUnwrap(request).keys),
            Set(["account_id", "asset_definition_id", "transfers"])
        )
        let assessment = try JSONSerialization.jsonObject(
            with: JSONEncoder().encode(fixture.assessment)
        ) as? [String: Any]
        XCTAssertEqual(
            Set(try XCTUnwrap(assessment).keys),
            Set([
                "account_id", "retail_enrolled", "billing_month_start_ms",
                "policy_revision", "payments_used_before", "qualifying_payments",
                "fee_minor", "state_commitment", "intent_hash", "expires_at_ms"
            ])
        )
    }

    func testTypedJSONRejectsUnrecognizedAndIncompleteRetailFields() throws {
        let fixture = try fixture()
        let decoder = JSONDecoder()
        var assessment = try XCTUnwrap(
            JSONSerialization.jsonObject(
                with: JSONEncoder().encode(fixture.assessment)
            ) as? [String: Any]
        )
        assessment["retired_fee_metadata"] = "ignored"
        XCTAssertThrowsError(try decoder.decode(
            RetailFeeAssessmentV1.self,
            from: JSONSerialization.data(withJSONObject: assessment)
        ))
        assessment.removeValue(forKey: "retired_fee_metadata")
        assessment.removeValue(forKey: "intent_hash")
        XCTAssertThrowsError(try decoder.decode(
            RetailFeeAssessmentV1.self,
            from: JSONSerialization.data(withJSONObject: assessment)
        ))

        var request = try XCTUnwrap(
            JSONSerialization.jsonObject(
                with: JSONEncoder().encode(fixture.request)
            ) as? [String: Any]
        )
        request["retired_exemption"] = true
        XCTAssertThrowsError(try decoder.decode(
            RetailFeeQuoteRequestV1.self,
            from: JSONSerialization.data(withJSONObject: request)
        ))
        request.removeValue(forKey: "retired_exemption")
        var transfers = try XCTUnwrap(request["transfers"] as? [[String: Any]])
        transfers[0]["retired_fee"] = 1
        request["transfers"] = transfers
        XCTAssertThrowsError(try decoder.decode(
            RetailFeeQuoteRequestV1.self,
            from: JSONSerialization.data(withJSONObject: request)
        ))
    }

    func testNativeRetailIntentAndAssessmentMatchSharedNoritoFixture() throws {
        let fixture = try fixture()
        let hash = try fixture.request.intentHash()
        XCTAssertEqual(
            try RetailFeeAssessmentNative.intentHashV1(requestJSON: JSONEncoder().encode(fixture.request)),
            hash
        )
        XCTAssertEqual(
            hash.map { String(format: "%02x", $0) }.joined(),
            fixture.intentHashHex
        )
        XCTAssertEqual(try fixture.assessment.marker(), fixture.marker)
        XCTAssertEqual(
            try RetailFeeAssessmentV1.decodeMarker(fixture.marker),
            fixture.assessment
        )
    }

    func testOperationBoundsRejectBeforeInvokingNative() throws {
        XCTAssertThrowsError(try RetailFeeAssessmentNative.intentHashV1(
            requestJSON: Data(repeating: 0, count: 262_145)
        )) {
            XCTAssertEqual($0 as? RetailFeeNativeError, .invalidNativeOutput)
        }
        XCTAssertThrowsError(try RetailFeeAssessmentV1.decodeMarker(
            String(repeating: "0", count: 4_097)
        )) {
            XCTAssertEqual($0 as? RetailFeeNativeError, .invalidNativeOutput)
        }
        let fixture = try fixture()
        let large = RetailFeeAssessmentV1(
            accountId: String(repeating: "a", count: 4_097),
            retailEnrolled: fixture.assessment.retailEnrolled,
            billingMonthStartMs: fixture.assessment.billingMonthStartMs,
            policyRevision: fixture.assessment.policyRevision,
            paymentsUsedBefore: fixture.assessment.paymentsUsedBefore,
            qualifyingPayments: fixture.assessment.qualifyingPayments,
            feeMinor: fixture.assessment.feeMinor,
            stateCommitment: fixture.assessment.stateCommitment,
            intentHash: fixture.assessment.intentHash,
            expiresAtMs: fixture.assessment.expiresAtMs
        )
        XCTAssertThrowsError(try large.marker()) {
            XCTAssertEqual($0 as? RetailFeeNativeError, .invalidNativeOutput)
        }
    }

    func testNativeRetailBoundaryRejectsInvalidShapes() throws {
        let fixture = try fixture()
        let emptyIntent = RetailFeeQuoteRequestV1(
            accountId: fixture.request.accountId,
            assetDefinitionId: fixture.request.assetDefinitionId,
            transfers: []
        )
        XCTAssertThrowsError(try emptyIntent.intentHash()) {
            XCTAssertEqual($0 as? RetailFeeNativeError, .nativeRejected(-506))
        }
        XCTAssertThrowsError(
            try RetailFeeAssessmentV1.decodeMarker(fixture.marker.uppercased())
        ) {
            XCTAssertEqual($0 as? RetailFeeNativeError, .nativeRejected(-506))
        }
        XCTAssertThrowsError(
            try RetailFeeAssessmentV1.decodeMarker(fixture.marker + "00")
        ) {
            XCTAssertEqual($0 as? RetailFeeNativeError, .nativeRejected(-506))
        }
    }
}
