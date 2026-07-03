import XCTest
@testable import IrohaSwift

final class OfflineNoteTextTransferContractTests: XCTestCase {
    func testTextTransferContractAcceptsOnlyAsciiTransportEnvelope() throws {
        let payload = "wallet-offline-bearer-cash-payment:abc_DEF-123"

        XCTAssertEqual(
            OfflineNoteTextTransferContract.trimmingBoundaryWhitespace(" \t\r\n\(payload)\n\r\t "),
            payload
        )
        XCTAssertEqual(
            try OfflineNoteTextTransferContract.normalizeTextTransportEnvelope(payload),
            payload
        )
        XCTAssertTrue(OfflineNoteTextTransferContract.hasOnlyTextTransportCharacters(payload))

        for candidate in [
            "",
            " \(payload)",
            "\(payload) ",
            "\t\(payload)",
            "\(payload)\n",
            "wallet-offline-bearer-cash-payment:abc DEF",
            "wallet-offline-bearer-cash-payment:abc\tDEF",
            "wallet-offline-bearer-cash-payment:abc\nDEF",
            "\u{000B}\(payload)",
            "\(payload)\u{000C}",
            "\(payload)\u{007F}",
            "\u{0085}\(payload)",
            "\(payload)\u{00A0}",
            "\(payload)\u{200B}",
            "\(payload)\u{202E}",
            "\(payload)\u{FE0F}",
            "\(payload)😀",
        ] {
            XCTAssertFalse(OfflineNoteTextTransferContract.hasOnlyTextTransportCharacters(candidate), candidate)
            XCTAssertThrowsError(try OfflineNoteTextTransferContract.normalizeTextTransportEnvelope(candidate), candidate)
        }
    }

    func testTextTransferContractRequiresBase64URLBodies() {
        let prefix = OfflineNoteTransferTextPayloadCodec.paymentTokenPrefix
        for body in ["A", "abc", "abc-DEF_123", "0"] {
            XCTAssertTrue(
                OfflineNoteTextTransferContract.hasBase64URLTextBody("\(prefix)\(body)", prefixes: [prefix]),
                body
            )
            XCTAssertEqual(
                try OfflineNoteTextTransferContract.requireBase64URLTextBody(
                    "\(prefix)\(body)",
                    prefixes: [prefix]
                ).body,
                body
            )
        }

        for body in [
            "",
            "abc/def",
            "abc+def",
            "abc=def",
            "abc.def",
            "abc:def",
            "abc def",
            "abc\tdef",
            "abc\nDEF",
            "abc\u{00A0}def",
            "abc\u{200B}def",
            "abc😀def",
        ] {
            XCTAssertFalse(
                OfflineNoteTextTransferContract.hasBase64URLTextBody("\(prefix)\(body)", prefixes: [prefix]),
                body
            )
            XCTAssertThrowsError(
                try OfflineNoteTextTransferContract.requireBase64URLTextBody(
                    "\(prefix)\(body)",
                    prefixes: [prefix]
                ),
                body
            ) { error in
                XCTAssertEqual(error as? OfflineNoteTextTransferContractError, .invalidBase64URLBody)
            }
        }
        XCTAssertThrowsError(
            try OfflineNoteTextTransferContract.requireBase64URLTextBody(
                "wallet-offline-bearer-cash-receive:abc",
                prefixes: [prefix]
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteTextTransferContractError, .invalidBase64URLBody)
        }
    }

    func testBase64URLDecodeRejectsPaddedAndSeparatorSmuggling() {
        let data = Data([0x00, 0x01, 0xFE, 0xFF])
        let encoded = OfflineNoteTextTransferContract.base64URLEncodedString(data)
        XCTAssertEqual(OfflineNoteTextTransferContract.base64URLDecodedData(encoded), data)

        for value in ["", "AA==", "AA/BB", "AA+BB", "AA BB", "AA\nBB", "AA\u{200B}BB"] {
            XCTAssertNil(OfflineNoteTextTransferContract.base64URLDecodedData(value), value)
        }
    }

    func testBase64URLDecodeRejectsNonCanonicalPadBits() {
        XCTAssertEqual(OfflineNoteTextTransferContract.base64URLDecodedData("AA"), Data([0x00]))
        XCTAssertEqual(OfflineNoteTextTransferContract.base64URLDecodedData("AQ"), Data([0x01]))
        XCTAssertEqual(OfflineNoteTextTransferContract.base64URLDecodedData("YQ"), Data([0x61]))

        for value in ["AB", "Af", "YR"] {
            XCTAssertNil(OfflineNoteTextTransferContract.base64URLDecodedData(value), value)
        }
    }

    func testDeviceToDevicePayloadBudgetMatchesNfcApduBudget() throws {
        XCTAssertEqual(
            OfflineNoteTextTransferContract.maxDeviceToDevicePayloadBytes,
            OfflineNoteNfcApduProtocol.maxIncomingPayloadBytes
        )
        XCTAssertNoThrow(
            try OfflineNoteTextTransferContract.requireDeviceToDevicePayloadByteCount(
                OfflineNoteNfcApduProtocol.maxIncomingPayloadBytes
            )
        )
        XCTAssertThrowsError(
            try OfflineNoteTextTransferContract.requireDeviceToDevicePayloadByteCount(
                OfflineNoteNfcApduProtocol.maxIncomingPayloadBytes + 1
            )
        ) { error in
            XCTAssertEqual(
                error as? OfflineNoteTextTransferContractError,
                .payloadTooLarge(maxBytes: OfflineNoteNfcApduProtocol.maxIncomingPayloadBytes)
            )
        }
    }
}
