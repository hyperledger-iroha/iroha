import XCTest
@testable import IrohaSwift

final class FeePaymentTests: XCTestCase {
    private let authority =
        "sorauﾛ1NｲﾘｳdPBeｼRoｸQ2ﾔgｼQqeｶﾍｽﾁhRW2ｺｿZ9ﾕｦUﾅRX5NJYH53"
    private let assetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"

    func testAuthorityIntentUsesCanonicalJSONAndNorito() throws {
        let intent = FeePaymentIntent.authority(chargeLimits: [], gasLimit: nil)

        XCTAssertEqual(
            String(decoding: try intent.canonicalJSONData(), as: UTF8.self),
            #"{"payer":"authority","value":{"charge_limits":[]}}"#
        )
        XCTAssertEqual(
            try intent.canonicalNorito().hexEncodedString(),
            "00000000190000000000000008000000000000000000000000000000010000000000000000"
        )
    }

    func testSponsorIntentRoundTripsWithoutRewritingSelection() throws {
        let program = try FeeSponsorProgramId(sponsor: authority, name: "wallet_fx")
        let limit = try FeeChargeLimit(
            kind: .pipelineGas,
            assetDefinitionId: assetDefinitionId,
            maxAmount: "10.5"
        )
        let intent = FeePaymentIntent.sponsor(
            programId: program,
            programRevision: 7,
            chargeLimits: [limit],
            gasLimit: 9000
        )
        let encoded = try intent.canonicalJSONData()

        XCTAssertEqual(try JSONDecoder().decode(FeePaymentIntent.self, from: encoded), intent)
        XCTAssertTrue(String(decoding: encoded, as: UTF8.self).contains(#""program_revision":7"#))
    }

    func testIntentRejectsUnknownFieldsAndNonCanonicalLimits() throws {
        let unknown = Data(
            #"{"payer":"authority","value":{"charge_limits":[]},"fee_sponsor":"legacy"}"#.utf8
        )
        XCTAssertThrowsError(try JSONDecoder().decode(FeePaymentIntent.self, from: unknown))

        let nexus = try FeeChargeLimit(
            kind: .nexus,
            assetDefinitionId: assetDefinitionId,
            maxAmount: "1"
        )
        let gas = try FeeChargeLimit(
            kind: .pipelineGas,
            assetDefinitionId: assetDefinitionId,
            maxAmount: "2"
        )
        let reversed = FeePaymentIntent.authority(chargeLimits: [gas, nexus], gasLimit: nil)
        XCTAssertThrowsError(try reversed.canonicalJSONData()) { error in
            XCTAssertEqual(error as? FeePaymentIntentError, .nonCanonicalChargeLimits)
        }
    }

    func testQuoteCanOnlyReplaceMaxima() throws {
        let draft = FeePaymentIntent.authority(chargeLimits: [], gasLimit: 100)
        let quoted = FeePaymentIntent.authority(
            chargeLimits: [try FeeChargeLimit(
                kind: .nexus,
                assetDefinitionId: assetDefinitionId,
                maxAmount: "3"
            )],
            gasLimit: 100
        )
        let substituted = FeePaymentIntent.authority(chargeLimits: [], gasLimit: 101)

        XCTAssertTrue(draft.hasSamePayerAndGasBound(as: quoted))
        XCTAssertFalse(draft.hasSamePayerAndGasBound(as: substituted))
    }
}
