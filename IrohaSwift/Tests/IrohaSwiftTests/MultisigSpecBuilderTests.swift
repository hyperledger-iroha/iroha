import XCTest
@testable import IrohaSwift

final class MultisigSpecBuilderTests: XCTestCase {
    func testBuilderProducesPayloadAndJson() throws {
        let builder = MultisigSpecBuilder()
            .setQuorum(3)
            .setTransactionTtl(milliseconds: 60_000)
            .addSignatory(accountId: "alice@wonderland", weight: 2)
            .addSignatory(accountId: "bob@wonderland", weight: 1)

        let payload = try builder.build()

        XCTAssertEqual(payload.quorum, 3)
        XCTAssertEqual(payload.transactionTtlMs, 60_000)
        XCTAssertEqual(payload.signatories["alice@wonderland"], 2)
        XCTAssertEqual(payload.signatories["bob@wonderland"], 1)

        let encoded = try builder.encodeJSON()
        let decoded = try JSONDecoder().decode(MultisigSpecPayload.self, from: encoded)
        XCTAssertEqual(decoded.signatories.count, 2)
        XCTAssertEqual(decoded.signatories["bob@wonderland"], 1)
    }

    func testMissingFieldsThrow() {
        let builder = MultisigSpecBuilder()
            .addSignatory(accountId: "alice@wonderland", weight: 1)

        XCTAssertThrowsError(try builder.build()) { error in
            XCTAssertTrue(error is MultisigSpecBuilderError)
        }
    }

    func testInvalidTtlAndQuorum() {
        let builder = MultisigSpecBuilder()
            .setQuorum(5)
            .setTransactionTtl(milliseconds: 0)
            .addSignatory(accountId: "alice@wonderland", weight: 1)
            .addSignatory(accountId: "bob@wonderland", weight: 2)

        XCTAssertThrowsError(try builder.build()) { error in
            XCTAssertEqual(error as? MultisigSpecBuilderError, .transactionTtlZero)
        }

        let quorumBuilder = MultisigSpecBuilder()
            .setQuorum(10)
            .setTransactionTtl(milliseconds: 1)
            .addSignatory(accountId: "alice@wonderland", weight: 1)

        XCTAssertThrowsError(try quorumBuilder.build()) { error in
            XCTAssertEqual(error as? MultisigSpecBuilderError,
                           .quorumExceedsTotalWeight(quorum: 10, totalWeight: UInt32(1)))
        }
    }

    func testProposalTtlPreviewClampsToPolicyCap() throws {
        let payload = MultisigSpecPayload(
            signatories: ["alice@wonderland": 1],
            quorum: 1,
            transactionTtlMs: 60_000
        )
        let preview = payload.previewProposalExpiry(
            requestedTtlMs: 120_000,
            now: Date(timeIntervalSince1970: 0)
        )

        XCTAssertTrue(preview.wasCapped)
        XCTAssertEqual(preview.policyCapMs, 60_000)
        XCTAssertEqual(preview.effectiveTtlMs, 60_000)
        XCTAssertEqual(preview.expiresAtMs, 60_000)
    }

    func testProposalTtlPreviewKeepsShorterOverride() throws {
        let payload = MultisigSpecPayload(
            signatories: ["alice@wonderland": 1],
            quorum: 1,
            transactionTtlMs: 60_000
        )
        let preview = payload.previewProposalExpiry(
            requestedTtlMs: 30_000,
            now: Date(timeIntervalSince1970: 0)
        )

        XCTAssertFalse(preview.wasCapped)
        XCTAssertEqual(preview.policyCapMs, 60_000)
        XCTAssertEqual(preview.effectiveTtlMs, 30_000)
        XCTAssertEqual(preview.expiresAtMs, 30_000)
    }

    func testProposalTtlEnforcementRejectsAboveCap() throws {
        let payload = MultisigSpecPayload(
            signatories: ["alice@wonderland": 1],
            quorum: 1,
            transactionTtlMs: 60_000
        )
        XCTAssertThrowsError(try payload.enforceProposalExpiry(requestedTtlMs: 120_000)) { error in
            XCTAssertEqual(error as? MultisigProposalTtlError,
                           .overrideExceedsPolicy(requested: 120_000, cap: 60_000))
        }
    }

    func testProposalTtlEnforcementAcceptsShorterOverride() throws {
        let payload = MultisigSpecPayload(
            signatories: ["alice@wonderland": 1],
            quorum: 1,
            transactionTtlMs: 60_000
        )
        let preview = try payload.enforceProposalExpiry(requestedTtlMs: 10_000,
                                                        now: Date(timeIntervalSince1970: 0))
        XCTAssertFalse(preview.wasCapped)
        XCTAssertEqual(preview.effectiveTtlMs, 10_000)
        XCTAssertEqual(preview.expiresAtMs, 10_000)
    }
}
