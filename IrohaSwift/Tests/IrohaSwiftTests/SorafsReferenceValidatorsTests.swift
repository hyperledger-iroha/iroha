import XCTest
@testable import IrohaSwift

final class SorafsReferenceValidatorsTests: XCTestCase {
    func testBridgeSelectors() {
        XCTAssertEqual(SorafsOrderbookPayloadKind.orderRequest.rawValue, 1)
        XCTAssertEqual(SorafsOrderbookPayloadKind.runtimeSnapshot.rawValue, 6)
        XCTAssertTrue(SorafsOrderbookPayloadKind.orderRequest.isUserSignedPayload)
        XCTAssertFalse(SorafsOrderbookPayloadKind.runtimeSnapshot.isUserSignedPayload)
        XCTAssertEqual(SorafsPdpPayloadKind.commitment.rawValue, 1)
        XCTAssertEqual(SorafsPdpPayloadKind.proof.rawValue, 3)
        XCTAssertEqual(SorafsOrderbookSide.bid.rawValue, 1)
        XCTAssertEqual(SorafsOrderbookTier.archive.rawValue, 3)
        XCTAssertEqual(SorafsOrderbookCancelReason.replaced.rawValue, 4)
    }

    func testRejectsBlankLabelBeforeNativeDispatch() {
        XCTAssertThrowsError(
            try SorafsReferenceValidators.validatePdpPayloadJSON(
                kind: .proof,
                payload: Data(),
                label: " ",
                generatedAtUnix: 1
            )
        ) { error in
            XCTAssertEqual(
                error as? SorafsReferenceValidationError,
                .invalidLabel("label must not be blank")
            )
        }
    }

    func testDefaultTimestampOverloadsValidateLabelsBeforeNativeDispatch() {
        func assertInvalidLabel(_ operation: () throws -> String, line: UInt = #line) {
            XCTAssertThrowsError(try operation(), line: line) { error in
                XCTAssertEqual(
                    error as? SorafsReferenceValidationError,
                    .invalidLabel("label must not be blank"),
                    line: line
                )
            }
        }

        assertInvalidLabel {
            try SorafsReferenceValidators.validateOrderbookPayloadJSON(
                kind: .orderRequest,
                payload: Data(),
                label: " "
            )
        }
        assertInvalidLabel {
            try SorafsReferenceValidators.validatePdpPayloadJSON(
                kind: .proof,
                payload: Data(),
                label: " "
            )
        }
        assertInvalidLabel {
            try SorafsReferenceValidators.validatePdpCommitmentChallengeJSON(
                commitment: Data(),
                challenge: Data(),
                commitmentLabel: " "
            )
        }
        assertInvalidLabel {
            try SorafsReferenceValidators.validatePdpChallengeProofJSON(
                challenge: Data(),
                proof: Data(),
                challengeLabel: " "
            )
        }
        assertInvalidLabel {
            try SorafsReferenceValidators.validatePdpBundleJSON(
                commitment: Data(),
                challenge: Data(),
                proof: Data(),
                commitmentLabel: " "
            )
        }
    }

    func testRejectsRuntimeSnapshotSigningBeforeNativeDispatch() {
        XCTAssertThrowsError(
            try SorafsReferenceValidators.signOrderbookPayload(
                kind: .runtimeSnapshot,
                payload: Data(),
                privateKey: Data(repeating: 0xB7, count: 32)
            )
        ) { error in
            XCTAssertEqual(
                error as? SorafsReferenceValidationError,
                .unsupportedOrderbookPayloadKind(.runtimeSnapshot)
            )
        }
    }

    func testRejectsBadSigningKeyBeforeNativeDispatch() {
        XCTAssertThrowsError(
            try SorafsReferenceValidators.signOrderbookPayload(
                kind: .orderRequest,
                payload: Data(),
                privateKey: Data(repeating: 0x00, count: 32)
            )
        ) { error in
            XCTAssertEqual(
                error as? SorafsReferenceValidationError,
                .invalidPrivateKey("privateKey must not be all zero")
            )
        }
    }

    func testRejectsOrderbookOrderRequestFieldsBeforeNativeDispatch() {
        let fields = SorafsSignedOrderbookOrderRequestFields(
            orderId: Data(repeating: 0x11, count: 31),
            side: .bid,
            tier: .hot,
            pricePerGibMicroXor: "42",
            quantityGib: 7,
            ownerAccount: Data([0x01]),
            expiryUnix: 123,
            nonce: 1,
            makerFeeBps: 0,
            takerFeeBps: 25
        )
        XCTAssertThrowsError(
            try SorafsReferenceValidators.buildSignedOrderbookOrderRequest(
                fields,
                privateKey: Data(repeating: 0xB7, count: 32)
            )
        ) { error in
            XCTAssertEqual(
                error as? SorafsReferenceValidationError,
                .invalidOrderbookField("orderId must be 32 bytes")
            )
        }
    }

    func testRejectsOrderbookSettlementReceiptFieldsBeforeNativeDispatch() {
        let fields = SorafsSignedOrderbookSettlementReceiptFields(
            receiptId: Data(repeating: 0x21, count: 32),
            channelId: Data(repeating: 0x22, count: 32),
            tradeId: Data(repeating: 0x23, count: 32),
            rangeStart: 0,
            rangeEnd: 64,
            chunkHash: Data(repeating: 0x24, count: 32),
            bytesDelivered: 64,
            xorDebitedMicroXor: "not-a-decimal",
            providerCreditMicroXor: "10",
            feeAmountMicroXor: "1",
            issuedAtUnix: 123
        )
        XCTAssertThrowsError(
            try SorafsReferenceValidators.buildSignedOrderbookSettlementReceipt(
                fields,
                privateKey: Data(repeating: 0xB7, count: 32)
            )
        ) { error in
            XCTAssertEqual(
                error as? SorafsReferenceValidationError,
                .invalidOrderbookField("xorDebitedMicroXor must be an unsigned decimal integer")
            )
        }
    }

    func testValidatesOrderbookFixtureWhenNativeBridgeIsAvailable() throws {
        try XCTSkipIf(!SorafsReferenceValidators.isNativeAvailable, "SoraFS reference bridge unavailable")
        let payload = try fixture("sorafs_manifest/orderbook/order_request_v1.to")
        let json = try SorafsReferenceValidators.validateOrderbookPayloadJSON(
            kind: .orderRequest,
            payload: payload,
            generatedAtUnix: 123
        )
        XCTAssertTrue(json.contains("\"status\": \"Ok\""), json)
        XCTAssertTrue(json.contains("\"code\": \"SFS-OK-000\""), json)
    }

    func testSignsOrderbookFixtureWhenNativeBridgeIsAvailable() throws {
        try XCTSkipIf(!SorafsReferenceValidators.isOrderbookSigningAvailable, "SoraFS orderbook signing bridge unavailable")
        let payload = try fixture("sorafs_manifest/orderbook/order_request_v1.to")
        let signed = try SorafsReferenceValidators.signOrderbookPayload(
            kind: .orderRequest,
            payload: payload,
            privateKey: Data(repeating: 0xB7, count: 32)
        )
        XCTAssertFalse(signed.isEmpty)
        XCTAssertNotEqual(signed, payload)
    }

    private func fixture(_ relativePath: String) throws -> Data {
        let testFile = URL(fileURLWithPath: #filePath)
        let url = testFile
            .deletingLastPathComponent()
            .appendingPathComponent("../../../fixtures/\(relativePath)")
            .standardizedFileURL
        return try Data(contentsOf: url)
    }
}
