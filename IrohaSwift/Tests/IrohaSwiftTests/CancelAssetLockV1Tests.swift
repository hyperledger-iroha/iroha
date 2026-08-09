import Foundation
@testable import IrohaSwift
import XCTest

final class CancelAssetLockV1Tests: XCTestCase {
    private static let merchantLockId = "merchant-lock-001"
    private static let merchantEscrowId =
        "hash:996264C84790C64086AAB0EF693A1D33EC18FC0B1C1229774C461A00939A6687#F2BD"

    func testBuilderDerivesNativeEscrowIdAndExactTwoFieldJSON() throws {
        let value = try CancelAssetLockInstructionV1(
            lockId: Self.merchantLockId,
            expectedRemainingAmount: "1500"
        )
        XCTAssertEqual(value.escrowId, Self.merchantEscrowId)
        XCTAssertEqual(value.expectedRemainingAmount.canonicalString, "1500")

        let instruction = try value.noritoJSON()
        let outer = try XCTUnwrap(
            JSONSerialization.jsonObject(with: instruction.data) as? [String: Any]
        )
        XCTAssertEqual(Set(outer.keys), ["CancelAssetLock"])
        let body = try XCTUnwrap(outer["CancelAssetLock"] as? [String: Any])
        XCTAssertEqual(Set(body.keys), ["escrow_id", "expected_remaining_amount"])
        XCTAssertEqual(body["escrow_id"] as? String, Self.merchantEscrowId)
        XCTAssertEqual(body["expected_remaining_amount"] as? String, "1500")
        XCTAssertEqual(
            try NativeEscrowInstructionBuilders.decodeCancelAssetLock(instruction),
            value
        )
        XCTAssertEqual(
            try NativeEscrowInstructionBuilders.cancelAssetLock(
                lockId: Self.merchantLockId,
                expectedRemainingAmount: "1500"
            ),
            instruction
        )
    }

    func testTypedQuantityAndFinalizedEscrowIdPathsAreExact() throws {
        let quantity = try KotodamaQuantity("1.25")
        let derived = try CancelAssetLockInstructionV1(
            lockId: Self.merchantLockId,
            expectedRemainingAmount: quantity
        )
        let finalized = try CancelAssetLockInstructionV1(
            escrowId: Self.merchantEscrowId,
            expectedRemainingAmount: quantity
        )
        XCTAssertEqual(derived, finalized)
        XCTAssertEqual(
            try NativeEscrowInstructionBuilders.cancelAssetLock(
                escrowId: Self.merchantEscrowId,
                expectedRemainingAmount: quantity
            ),
            try derived.noritoJSON()
        )
    }

    func testBareNoritoRoundTripProducesTransactionInstructionFrame() throws {
        let value = try CancelAssetLockInstructionV1(
            lockId: Self.merchantLockId,
            expectedRemainingAmount: "1.25"
        )
        let archive = try value.noritoArchive()
        XCTAssertEqual(
            try CancelAssetLockInstructionV1.decodeNoritoArchive(archive),
            value
        )

        let instruction = try value.transactionInstructionFrame()
        XCTAssertEqual(
            instruction.wireName,
            "iroha_data_model::isi::escrow::CancelAssetLock"
        )
        XCTAssertEqual(instruction.framedPayload, archive)
    }

    func testRejectsUncleanLockIdsAndMalformedEscrowIds() throws {
        for lockId in [
            "",
            " ",
            " merchant-lock-001",
            "merchant-lock-001 ",
            "\u{FEFF}merchant-lock-001",
            "merchant-lock-001\u{FEFF}",
        ] {
            XCTAssertThrowsError(
                try CancelAssetLockInstructionV1(
                    lockId: lockId,
                    expectedRemainingAmount: "1"
                ),
                "accepted invalid lock id \(String(reflecting: lockId))"
            )
        }
        XCTAssertNotEqual(
            try CancelAssetLockInstructionV1.escrowId(forLockId: "merchant lock"),
            Self.merchantEscrowId,
            "internal lock-id bytes must be hashed exactly rather than normalized"
        )

        let malformed = [
            Self.merchantEscrowId.lowercased(),
            String(Self.merchantEscrowId.dropFirst(5)),
            "0x" + String(repeating: "A", count: 64),
            "hash:" + String(repeating: "A", count: 64) + "#0000",
            "hash:" + String(repeating: "0", count: 64) + "#59D7",
        ]
        for escrowId in malformed {
            XCTAssertThrowsError(
                try CancelAssetLockInstructionV1(
                    escrowId: escrowId,
                    expectedRemainingAmount: "1"
                ),
                "accepted malformed escrow id \(escrowId)"
            )
        }
    }

    func testLockIdPreimageUsesExactUTF8ByteBound() throws {
        let exactBound = String(repeating: "🔒", count: 1024)
        XCTAssertEqual(exactBound.utf8.count, 4096)
        XCTAssertEqual(CancelAssetLockInstructionV1.maxLockIdUTF8BytesV1, 4096)
        XCTAssertNoThrow(
            try CancelAssetLockInstructionV1(
                lockId: exactBound,
                expectedRemainingAmount: "1"
            )
        )

        let overBound = exactBound + "a"
        XCTAssertEqual(overBound.utf8.count, 4097)
        XCTAssertThrowsError(
            try CancelAssetLockInstructionV1(
                lockId: overBound,
                expectedRemainingAmount: "1"
            )
        ) { error in
            XCTAssertEqual(error as? CancelAssetLockV1Error, .invalidLockId)
        }
    }

    func testRejectsMissingZeroAndNoncanonicalQuantities() throws {
        for amount in [
            "",
            " ",
            "0",
            "-0",
            "-1",
            "+1",
            "01",
            "1.",
            ".5",
            "1.0",
            "1e0",
            "NaN",
        ] {
            XCTAssertThrowsError(
                try CancelAssetLockInstructionV1(
                    lockId: Self.merchantLockId,
                    expectedRemainingAmount: amount
                ),
                "accepted invalid expected remaining amount \(amount)"
            )
        }

        let legacy = try NoritoJSON.fromJSONObject([
            "CancelAssetLock": [
                "escrow_id": Self.merchantEscrowId,
            ],
        ])
        XCTAssertThrowsError(
            try CancelAssetLockInstructionV1.decodeInstructionJSON(legacy)
        )
    }

    func testJSONDecoderRejectsAliasesExtrasDuplicatesAndMalformedIds() throws {
        let rawEscrowHash = String(Self.merchantEscrowId.dropFirst(5).prefix(64))
        let invalidJSON = [
            """
            {"CancelAssetLock":{"escrow_id":"\(Self.merchantEscrowId)","expectedRemainingAmount":"1"}}
            """,
            """
            {"CancelAssetLock":{"escrow_id":"\(Self.merchantEscrowId)","expected_remaining_amount":"1","relayer":"alice"}}
            """,
            """
            {"CancelAssetLock":{"escrowId":"\(Self.merchantEscrowId)","expected_remaining_amount":"1"}}
            """,
            """
            {"CancelAssetLock":{"escrow_id":"\(Self.merchantEscrowId)","expected_remaining_amount":"1"},"CancelAssetEscrow":{}}
            """,
            """
            {"CancelAssetLock":{"escrow_id":"\(Self.merchantEscrowId)","expected_remaining_amount":"1","expected_remaining_amount":"2"}}
            """,
            """
            {"CancelAssetLock":{"escrow_id":["\(Self.merchantEscrowId)"],"expected_remaining_amount":"1"}}
            """,
            """
            {"CancelAssetLock":{"escrow_id":{"hash":"\(rawEscrowHash)"},"expected_remaining_amount":"1"}}
            """,
            """
            {"CancelAssetLock":{"escrow_id":"\(rawEscrowHash)","expected_remaining_amount":"1"}}
            """,
        ]
        for json in invalidJSON {
            let payload = try NoritoJSON(data: Data(json.utf8))
            XCTAssertThrowsError(
                try CancelAssetLockInstructionV1.decodeInstructionJSON(payload),
                "accepted noncanonical JSON: \(json)"
            )
        }

        let retiredBareArray = Data(
            """
            {"escrow_id":["\(Self.merchantEscrowId)"],"expected_remaining_amount":"1"}
            """.utf8
        )
        XCTAssertThrowsError(
            try CancelAssetLockInstructionV1.decodeBareJSON(retiredBareArray),
            "accepted the retired one-element EscrowId array"
        )
    }

    func testBareJSONRejectsInvalidUTF8AndUnpairedSurrogates() throws {
        let invalidUTF8 = Data(
            Array("{\"escrow_id\":\"".utf8)
                + [0xFF]
                + Array("\",\"expected_remaining_amount\":\"1\"}".utf8)
        )
        let unpairedSurrogates = [
            """
            {"escrow_id":"\\uD800","expected_remaining_amount":"1"}
            """,
            """
            {"escrow_id":"\\uDC00","expected_remaining_amount":"1"}
            """,
        ]

        XCTAssertThrowsError(
            try CancelAssetLockInstructionV1.decodeBareJSON(invalidUTF8),
            "accepted invalid UTF-8"
        )
        for json in unpairedSurrogates {
            XCTAssertThrowsError(
                try CancelAssetLockInstructionV1.decodeBareJSON(Data(json.utf8)),
                "accepted an unpaired UTF-16 surrogate escape: \(json)"
            )
        }
    }

    func testNoritoDecoderRejectsLegacyZeroNoncanonicalAndTrailingForms() throws {
        let value = try CancelAssetLockInstructionV1(
            escrowId: Self.merchantEscrowId,
            expectedRemainingAmount: "20"
        )
        let escrowBytes = try XCTUnwrap(
            Data(hexString: String(Self.merchantEscrowId.dropFirst(5).prefix(64)))
        )

        var legacyPayload = CompactNoritoWriter()
        legacyPayload.writeField(escrowBytes)
        XCTAssertThrowsError(
            try CancelAssetLockInstructionV1.decodeNoritoArchive(
                noritoEncode(
                    typeName: CancelAssetLockInstructionV1.wireId,
                    payload: legacyPayload.data,
                    flags: NoritoHeader.compactLen
                )
            )
        )

        let invalidQuantities = try [
            "0": CanonicalNorito.encodeCompactQuantity("0"),
            "20.0": noncanonicalCompactQuantity(mantissa: Data([0xC8, 0]), scale: 1),
        ]
        for (amount, encodedQuantity) in invalidQuantities {
            var invalidPayload = CompactNoritoWriter()
            invalidPayload.writeField(escrowBytes)
            invalidPayload.writeField(encodedQuantity)
            XCTAssertThrowsError(
                try CancelAssetLockInstructionV1.decodeNoritoArchive(
                    noritoEncode(
                        typeName: CancelAssetLockInstructionV1.wireId,
                        payload: invalidPayload.data,
                        flags: NoritoHeader.compactLen
                    )
                ),
                "accepted invalid encoded quantity \(amount)"
            )
        }

        var malformedEscrowPayload = CompactNoritoWriter()
        malformedEscrowPayload.writeField(Data(repeating: 0, count: 32))
        try malformedEscrowPayload.writeField(
            CanonicalNorito.encodeCompactQuantity("20")
        )
        XCTAssertThrowsError(
            try CancelAssetLockInstructionV1.decodeNoritoArchive(
                noritoEncode(
                    typeName: CancelAssetLockInstructionV1.wireId,
                    payload: malformedEscrowPayload.data,
                    flags: NoritoHeader.compactLen
                )
            )
        )

        var trailing = try value.noritoArchive()
        trailing.append(0)
        XCTAssertThrowsError(
            try CancelAssetLockInstructionV1.decodeNoritoArchive(trailing)
        )
    }

    func testAppealFinanceReferenceFixturesAreByteExactAndFailClosed() throws {
        let root = fixtureRoot()
        let canonicalJSON = root.appendingPathComponent("cancel_asset_lock_v1.json")
        guard FileManager.default.fileExists(atPath: canonicalJSON.path) else {
            XCTFail(
                "Missing required canonical CancelAssetLock fixture at \(canonicalJSON.path)"
            )
            return
        }

        let decodedJSON = try CancelAssetLockInstructionV1.decodeBareJSON(
            Data(contentsOf: canonicalJSON)
        )
        let canonicalNorito = try Data(
            contentsOf: root.appendingPathComponent("cancel_asset_lock_v1.to")
        )
        let exactCanonicalNorito = try XCTUnwrap(
            Data(
                hexString:
                    "4e5254300000b5c8a665a7de80e2eef75ccb287078fa002d00000000000000"
                        + "d5f0a9bf0af707a1022073ccd4e0dd69ad434db75056b600aa4f74c8fc5556b11bdc"
                        + "799dfdb7ea29851f0b0501000000140400000000"
            )
        )
        XCTAssertEqual(canonicalNorito.count, 85)
        XCTAssertEqual(canonicalNorito, exactCanonicalNorito)
        XCTAssertEqual(try decodedJSON.noritoArchive(), canonicalNorito)
        XCTAssertEqual(
            try CancelAssetLockInstructionV1.decodeNoritoArchive(canonicalNorito),
            decodedJSON
        )

        for name in [
            "cancel_asset_lock_legacy_missing_expected_v1.json",
            "cancel_asset_lock_noncanonical_quantity_v1.json",
            "cancel_asset_lock_zero_expected_v1.json",
        ] {
            XCTAssertThrowsError(
                try CancelAssetLockInstructionV1.decodeBareJSON(
                    Data(contentsOf: root
                        .appendingPathComponent("negative")
                        .appendingPathComponent(name))
                ),
                "accepted negative JSON fixture \(name)"
            )
        }
        let retiredNestedNorito = try Data(
            contentsOf: root
                .appendingPathComponent("negative")
                .appendingPathComponent("cancel_asset_lock_nested_escrow_id_v1.to")
        )
        let exactRetiredNestedNorito = try XCTUnwrap(
            Data(
                hexString:
                    "4e5254300000b5c8a665a7de80e2eef75ccb287078fa002e00000000000000"
                        + "0e55fb7ed463b87302212073ccd4e0dd69ad434db75056b600aa4f74c8fc5556b11b"
                        + "dc799dfdb7ea29851f0b0501000000140400000000"
            )
        )
        XCTAssertEqual(retiredNestedNorito.count, 86)
        XCTAssertEqual(retiredNestedNorito, exactRetiredNestedNorito)
        XCTAssertEqual(Array(retiredNestedNorito[40 ..< 42]), [0x21, 0x20])
        for name in [
            "cancel_asset_lock_legacy_missing_expected_v1.to",
            "cancel_asset_lock_nested_escrow_id_v1.to",
            "cancel_asset_lock_zero_expected_v1.to",
        ] {
            XCTAssertThrowsError(
                try CancelAssetLockInstructionV1.decodeNoritoArchive(
                    Data(contentsOf: root
                        .appendingPathComponent("negative")
                        .appendingPathComponent(name))
                ),
                "accepted negative Norito fixture \(name)"
            )
        }
    }

    private func fixtureRoot() -> URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent(
                "fixtures/sorafs_manifest/appeal_finance",
                isDirectory: true
            )
    }

    private func noncanonicalCompactQuantity(
        mantissa: Data,
        scale: UInt32
    ) -> Data {
        var encodedMantissa = CompactNoritoWriter()
        encodedMantissa.writeUInt32LE(UInt32(mantissa.count))
        encodedMantissa.writeBytes(mantissa)

        var quantity = CompactNoritoWriter()
        quantity.writeField(encodedMantissa.data)
        quantity.writeField(CompactNorito.encodeUInt32(scale))
        return quantity.data
    }
}
