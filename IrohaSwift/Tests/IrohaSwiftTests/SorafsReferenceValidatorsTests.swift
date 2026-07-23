import XCTest
@testable import IrohaSwift

final class SorafsReferenceValidatorsTests: XCTestCase {
    private let maxScaledXor =
        "6703903964971298549787012499102923063739682910296196688861780721860882015" +
        "036773488400937149083451713845015929093243025426876941405973284973216824" +
        ".503042047"

    func testBridgeSelectors() {
        XCTAssertEqual(SorafsOrderbookPayloadKind.orderRequest.rawValue, 1)
        XCTAssertEqual(SorafsOrderbookPayloadKind.runtimeSnapshot.rawValue, 6)
        XCTAssertTrue(SorafsOrderbookPayloadKind.orderRequest.isUserSignedPayload)
        XCTAssertFalse(SorafsOrderbookPayloadKind.runtimeSnapshot.isUserSignedPayload)
        XCTAssertEqual(SorafsPdpPayloadKind.commitment.rawValue, 1)
        XCTAssertEqual(SorafsPdpPayloadKind.proof.rawValue, 3)
        XCTAssertEqual(SorafsPopPayloadKind.credential.rawValue, 1)
        XCTAssertEqual(SorafsPopPayloadKind.membershipProof.rawValue, 6)
        XCTAssertEqual(SorafsPopPayloadKind.issuedCredentialBundle.rawValue, 7)
        XCTAssertEqual(SorafsHedgingPayloadKind.priceFeed.rawValue, 1)
        XCTAssertEqual(SorafsHedgingPayloadKind.billingStatement.rawValue, 4)
        XCTAssertEqual(SorafsOrderbookSide.bid.rawValue, 1)
        XCTAssertEqual(SorafsOrderbookTier.archive.rawValue, 3)
        XCTAssertEqual(SorafsOrderbookCancelReason.replaced.rawValue, 4)
        XCTAssertEqual(SorafsReferenceValidators.orderbookOwnerAccountMaxBytesV1, 256)
        XCTAssertEqual(SorafsReferenceValidators.governanceDagMaxBlocksV1, 64)
        XCTAssertEqual(SorafsReferenceValidators.governanceDagCidBytesV1, 32)
        XCTAssertEqual(SorafsReferenceValidators.referenceMaxInputBytesV1, 67_108_864)
        XCTAssertEqual(SorafsReferenceValidators.referenceMaxLabelBytesV1, 1_024)
    }

    func testRejectsBlankLabelBeforeNativeDispatch() {
        XCTAssertThrowsError(
            try SorafsReferenceValidators.validateHedgingPayloadJSON(
                kind: .priceFeed,
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
        assertInvalidLabel {
            try SorafsReferenceValidators.validateGovernanceDagBlockJSON(
                payload: Data(),
                label: " "
            )
        }
        assertInvalidLabel {
            try SorafsReferenceValidators.validateGovernanceDagHeadChainJSON(
                head: Data(),
                blocks: [SorafsGovernanceDagBlockInput(payload: Data())],
                headLabel: " "
            )
        }
    }

    func testBoundsGovernanceDagInputsBeforeNativeDispatch() {
        XCTAssertThrowsError(
            try SorafsReferenceValidators.validateGovernanceDagHeadChainJSON(
                head: Data(),
                blocks: [],
                generatedAtUnix: 1
            )
        ) { error in
            XCTAssertEqual(
                error as? SorafsReferenceValidationError,
                .invalidGovernanceDagInput("blocks must contain 1...64 entries")
            )
        }

        XCTAssertThrowsError(
            try SorafsReferenceValidators.validateGovernanceDagHeadChainJSON(
                head: Data(),
                blocks: Array(
                    repeating: SorafsGovernanceDagBlockInput(payload: Data()),
                    count: 65
                ),
                generatedAtUnix: 1
            )
        ) { error in
            XCTAssertEqual(
                error as? SorafsReferenceValidationError,
                .invalidGovernanceDagInput("blocks must contain 1...64 entries")
            )
        }

        XCTAssertThrowsError(
            try SorafsReferenceValidators.validateGovernanceDagBlockJSON(
                payload: Data(),
                label: String(repeating: "x", count: 1_025),
                generatedAtUnix: 1
            )
        ) { error in
            XCTAssertEqual(
                error as? SorafsReferenceValidationError,
                .invalidLabel("label must be at most 1024 UTF-8 bytes")
            )
        }

        for invalidLength in [0, 31, 33] {
            XCTAssertThrowsError(
                try SorafsReferenceValidators.validateGovernanceDagBlockJSON(
                    payload: Data(),
                    expectedBlockCid: Data(repeating: 0, count: invalidLength),
                    generatedAtUnix: 1
                )
            ) { error in
                XCTAssertEqual(
                    error as? SorafsReferenceValidationError,
                    .invalidGovernanceDagInput(
                        "expectedBlockCid must contain exactly 32 bytes"
                    )
                )
            }
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

    func testRejectsInvalidOrderIdDerivationInputsBeforeNativeDispatch() {
        XCTAssertThrowsError(
            try SorafsReferenceValidators.deriveOrderbookOrderId(
                ownerAccount: Data(),
                nonce: 7
            )
        ) { error in
            XCTAssertEqual(
                error as? SorafsReferenceValidationError,
                .invalidOrderbookField("ownerAccount must not be empty")
            )
        }
        XCTAssertThrowsError(
            try SorafsReferenceValidators.deriveOrderbookOrderId(
                ownerAccount: Data([0x01]),
                nonce: 0
            )
        ) { error in
            XCTAssertEqual(
                error as? SorafsReferenceValidationError,
                .invalidOrderbookField("nonce must be greater than zero")
            )
        }
    }

    func testRejectsOversizedOrderbookOwnerAccountsBeforeNativeDispatch() {
        let oversized = Data(
            repeating: 0x45,
            count: SorafsReferenceValidators.orderbookOwnerAccountMaxBytesV1 + 1
        )
        let expected = SorafsReferenceValidationError.invalidOrderbookField(
            "ownerAccount must be at most 256 bytes"
        )

        XCTAssertThrowsError(
            try SorafsReferenceValidators.deriveOrderbookOrderId(
                ownerAccount: oversized,
                nonce: 7
            )
        ) { error in
            XCTAssertEqual(error as? SorafsReferenceValidationError, expected)
        }

        let request = SorafsSignedOrderbookOrderRequestFields(
            side: .bid,
            tier: .hot,
            pricePerGib: "1",
            quantityGib: 1,
            ownerAccount: oversized,
            expiryUnix: 1,
            nonce: 7,
            makerFeeBps: 0,
            takerFeeBps: 0
        )
        XCTAssertThrowsError(
            try SorafsReferenceValidators.buildSignedOrderbookOrderRequest(
                request,
                privateKey: Data(repeating: 0xB7, count: 32)
            )
        ) { error in
            XCTAssertEqual(error as? SorafsReferenceValidationError, expected)
        }

        let cancel = SorafsSignedOrderbookOrderCancelFields(
            orderId: Data(repeating: 0x11, count: 32),
            ownerAccount: oversized,
            reason: .ownerRequested,
            nonce: 8
        )
        XCTAssertThrowsError(
            try SorafsReferenceValidators.buildSignedOrderbookOrderCancel(
                cancel,
                privateKey: Data(repeating: 0xB7, count: 32)
            )
        ) { error in
            XCTAssertEqual(error as? SorafsReferenceValidationError, expected)
        }
    }

    func testRejectsOrderbookOrderRequestFieldsBeforeNativeDispatch() {
        let fields = SorafsSignedOrderbookOrderRequestFields(
            orderId: Data(repeating: 0x11, count: 31),
            side: .bid,
            tier: .hot,
            pricePerGib: "42",
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
            xorDebited: "not-a-decimal",
            providerCredit: "10",
            feeAmount: "1",
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
                .invalidOrderbookField("xorDebited must be a canonical non-negative XOR quantity")
            )
        }
    }

    func testRejectsNoncanonicalXorQuantitiesBeforeNativeDispatch() {
        XCTAssertEqual(maxScaledXor.utf8.count, 155)
        for value in ["1.0", "0.0000000001", String(repeating: "1", count: 156)] {
            let fields = SorafsSignedOrderbookSettlementReceiptFields(
                receiptId: Data(repeating: 0x21, count: 32),
                channelId: Data(repeating: 0x22, count: 32),
                tradeId: Data(repeating: 0x23, count: 32),
                rangeStart: 0,
                rangeEnd: 64,
                chunkHash: Data(repeating: 0x24, count: 32),
                bytesDelivered: 64,
                xorDebited: value,
                providerCredit: "0",
                feeAmount: "0",
                issuedAtUnix: 123
            )
            XCTAssertThrowsError(
                try SorafsReferenceValidators.buildSignedOrderbookSettlementReceipt(
                    fields,
                    privateKey: Data(repeating: 0xB7, count: 32)
                )
            ) { error in
                guard let validation = error as? SorafsReferenceValidationError,
                      case let .invalidOrderbookField(message) = validation else {
                    return XCTFail("unexpected error: \(error)")
                }
                XCTAssertTrue(message.contains("xorDebited"), message)
            }
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

    func testValidatesGovernanceDagFixturesAndNegativeVectorsWhenNativeBridgeIsAvailable() throws {
        try XCTSkipIf(
            !SorafsReferenceValidators.isGovernanceDagNativeAvailable,
            "SoraFS governance DAG reference bridge unavailable"
        )
        let first = try fixture("sorafs_manifest/governance/dag_block_0_v1.to")
        let second = try fixture("sorafs_manifest/governance/dag_block_1_v1.to")
        let head = try fixture("sorafs_manifest/governance/dag_head_v1.to")

        let blockOutcome = try SorafsReferenceValidators.validateGovernanceDagBlockJSON(
            payload: first,
            generatedAtUnix: 123
        )
        XCTAssertTrue(blockOutcome.contains("\"status\": \"Ok\""), blockOutcome)

        let cidMismatch = try SorafsReferenceValidators.validateGovernanceDagBlockJSON(
            payload: first,
            expectedBlockCid: Data(repeating: 0x7F, count: 32),
            generatedAtUnix: 123
        )
        XCTAssertTrue(cidMismatch.contains("\"status\": \"Error\""), cidMismatch)
        XCTAssertTrue(cidMismatch.contains("\"code\": \"SFS-GOV-004\""), cidMismatch)

        let headOutcome = try SorafsReferenceValidators.validateGovernanceDagHeadChainJSON(
            head: head,
            blocks: [
                SorafsGovernanceDagBlockInput(
                    payload: first,
                    label: "dag_block_0_v1.to"
                ),
                SorafsGovernanceDagBlockInput(
                    payload: second,
                    label: "dag_block_1_v1.to"
                )
            ],
            headLabel: "dag_head_v1.to",
            generatedAtUnix: 123
        )
        XCTAssertTrue(headOutcome.contains("\"status\": \"Ok\""), headOutcome)
        let goldenOutcome = String(
            decoding: try fixture(
                "sorafs_manifest/governance/dag_head_validation_outcome_v1.json"
            ),
            as: UTF8.self
        )
        XCTAssertEqual(headOutcome, goldenOutcome)

        let reordered = try SorafsReferenceValidators.validateGovernanceDagHeadChainJSON(
            head: head,
            blocks: [
                SorafsGovernanceDagBlockInput(payload: second),
                SorafsGovernanceDagBlockInput(payload: first)
            ],
            generatedAtUnix: 123
        )
        XCTAssertTrue(reordered.contains("\"status\": \"Error\""), reordered)
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

    func testDerivesCanonicalOrderIdAndRejectsExplicitMismatchWhenNativeBridgeIsAvailable() throws {
        try XCTSkipIf(
            !SorafsReferenceValidators.isOrderbookFieldBuilderAvailable,
            "SoraFS orderbook field-builder bridge unavailable"
        )
        let owner = Data("buyer@sora".utf8)
        let orderId = try SorafsReferenceValidators.deriveOrderbookOrderId(
            ownerAccount: owner,
            nonce: 7
        )
        XCTAssertEqual(
            orderId,
            Data(hexString: "9d91ad7700ca0c4762e031f9231aa38dd4502c6048c6ffa31d365e3c4e080b69")
        )
        XCTAssertNotEqual(
            orderId,
            try SorafsReferenceValidators.deriveOrderbookOrderId(ownerAccount: owner, nonce: 8)
        )
        XCTAssertNotEqual(
            orderId,
            try SorafsReferenceValidators.deriveOrderbookOrderId(
                ownerAccount: Data("provider@sora".utf8),
                nonce: 7
            )
        )

        let maximumOwner = Data(
            repeating: 0x45,
            count: SorafsReferenceValidators.orderbookOwnerAccountMaxBytesV1
        )
        let maximumOwnerOrderId = try SorafsReferenceValidators.deriveOrderbookOrderId(
            ownerAccount: maximumOwner,
            nonce: 9
        )
        let maximumOwnerOrder = try SorafsReferenceValidators.buildSignedOrderbookOrderRequest(
            SorafsSignedOrderbookOrderRequestFields(
                side: .bid,
                tier: .hot,
                pricePerGib: "1",
                quantityGib: 1,
                ownerAccount: maximumOwner,
                expiryUnix: 1_800_000_000,
                nonce: 9,
                makerFeeBps: 0,
                takerFeeBps: 0
            ),
            privateKey: Data(repeating: 0xB7, count: 32)
        )
        let maximumOwnerOrderOutcome = try SorafsReferenceValidators.validateOrderbookPayloadJSON(
            kind: .orderRequest,
            payload: maximumOwnerOrder,
            generatedAtUnix: 123
        )
        XCTAssertTrue(maximumOwnerOrderOutcome.contains("\"status\": \"Ok\""))
        let maximumOwnerCancel = try SorafsReferenceValidators.buildSignedOrderbookOrderCancel(
            SorafsSignedOrderbookOrderCancelFields(
                orderId: maximumOwnerOrderId,
                ownerAccount: maximumOwner,
                reason: .ownerRequested,
                nonce: 10
            ),
            privateKey: Data(repeating: 0xB7, count: 32)
        )
        let maximumOwnerCancelOutcome = try SorafsReferenceValidators.validateOrderbookPayloadJSON(
            kind: .orderCancel,
            payload: maximumOwnerCancel,
            generatedAtUnix: 123
        )
        XCTAssertTrue(maximumOwnerCancelOutcome.contains("\"status\": \"Ok\""))

        let canonicalFields = SorafsSignedOrderbookOrderRequestFields(
            side: .bid,
            tier: .hot,
            pricePerGib: maxScaledXor,
            quantityGib: 64,
            ownerAccount: owner,
            expiryUnix: 1_800_000_000,
            nonce: 7,
            makerFeeBps: 10,
            takerFeeBps: 15
        )
        let signed = try SorafsReferenceValidators.buildSignedOrderbookOrderRequest(
            canonicalFields,
            privateKey: Data(repeating: 0xB7, count: 32)
        )
        let outcome = try SorafsReferenceValidators.validateOrderbookPayloadJSON(
            kind: .orderRequest,
            payload: signed,
            generatedAtUnix: 123
        )
        XCTAssertTrue(outcome.contains("\"status\": \"Ok\""), outcome)

        let mismatchedFields = SorafsSignedOrderbookOrderRequestFields(
            orderId: Data(repeating: 0x11, count: 32),
            side: .bid,
            tier: .hot,
            pricePerGib: "0.000000001",
            quantityGib: 64,
            ownerAccount: owner,
            expiryUnix: 1_800_000_000,
            nonce: 7,
            makerFeeBps: 10,
            takerFeeBps: 15
        )
        XCTAssertThrowsError(
            try SorafsReferenceValidators.buildSignedOrderbookOrderRequest(
                mismatchedFields,
                privateKey: Data(repeating: 0xB7, count: 32)
            )
        ) { error in
            XCTAssertEqual(
                error as? SorafsReferenceValidationError,
                .invalidOrderbookField(
                    "orderId must equal the canonical owner-and-nonce derivation"
                )
            )
        }
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
