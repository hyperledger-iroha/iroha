import XCTest
@testable import IrohaSwift

final class KagemushaPeerTransportTests: XCTestCase {
    func testFirstReleaseIdentifiersAreExactAndUnique() {
        XCTAssertEqual(KagemushaPeerPayloadKind.receiveRequest.rawValue, 1)
        XCTAssertEqual(KagemushaPeerPayloadKind.payment.rawValue, 2)
        XCTAssertEqual(KagemushaPeerPayloadKind.acknowledgement.rawValue, 3)
        XCTAssertEqual(KagemushaPeerPayloadKind.receiveRequest.textPrefix, "PKK2R.")
        XCTAssertEqual(KagemushaPeerPayloadKind.payment.textPrefix, "PKK2P.")
        XCTAssertEqual(KagemushaPeerPayloadKind.acknowledgement.textPrefix, "PKK2A.")
        XCTAssertEqual(KagemushaPeerTransportContract.qrStreamTextPrefix, "PKKQ1.")
        XCTAssertEqual(
            KagemushaPeerTransportContract.nfcApplicationIdentifierHex,
            "F0504B45504B524E464301"
        )
        XCTAssertEqual(KagemushaPeerTransportContract.nearbyServiceName, "pk-kagemusha")
        XCTAssertEqual(
            Set(KagemushaPeerPayloadKind.allCases.map(\.contentType)).count,
            KagemushaPeerPayloadKind.allCases.count
        )
    }

    func testTypedRequestTextRoundTripIsCanonical() throws {
        let request = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let payload = KagemushaPeerPayload.receiveRequest(request)
        let text = try KagemushaPeerTextCodec.encode(payload)

        XCTAssertTrue(text.hasPrefix("PKK2R."))
        XCTAssertFalse(text.contains("="))
        XCTAssertLessThanOrEqual(
            text.utf8.count,
            KagemushaRecursiveSpend.maximumPeerTextEnvelopeBytes
        )
        XCTAssertEqual(
            try KagemushaPeerTextCodec.decode(text, expectedKind: .receiveRequest),
            payload
        )
        XCTAssertEqual(try KagemushaPeerTextCodec.encode(
            KagemushaPeerTextCodec.decode(text)
        ), text)
    }

    func testCanonicalPaymentFixtureUsesFirstReleaseABI20Envelope() throws {
        let request = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let payment = try KagemushaPeerTransportTestFixtures.payment(request: request)

        let frame = try XCTUnwrap(noritoDecodeFrame(payment.recipientBundle.noritoArchive))
        XCTAssertEqual(
            frame.header.schema,
            noritoSchemaHash(forTypeName: KagemushaRecursiveSpend.bundleWireNameV4)
        )
        XCTAssertEqual(KagemushaRecursiveSpend.wireVersionV4, 4)
    }

    func testUserPresentedBoundaryNormalizationIsNarrowAndExplicit() throws {
        let payload = KagemushaPeerPayload.receiveRequest(
            try KagemushaPeerTransportTestFixtures.receiveRequest()
        )
        let text = try KagemushaPeerTextCodec.encode(payload)
        let presented = " \t\r\n" + text + "\n\r\t "

        XCTAssertThrowsError(try KagemushaPeerTextCodec.decode(presented))
        XCTAssertEqual(
            KagemushaPeerTextCodec.canonicalizeUserPresented(presented),
            text
        )
        XCTAssertEqual(
            try KagemushaPeerTextCodec.decodeUserPresented(presented),
            payload
        )
        for scalar in ["\u{000B}", "\u{0085}", "\u{00A0}", "\u{200B}", "\u{2028}", "\u{FEFF}"] {
            XCTAssertThrowsError(
                try KagemushaPeerTextCodec.decodeUserPresented(scalar + text),
                "unexpectedly normalized U+\(scalar.unicodeScalars.first!.value)"
            )
        }
    }

    func testExpectedKindCannotBeSubstituted() throws {
        let text = try KagemushaPeerTextCodec.encode(.receiveRequest(
            KagemushaPeerTransportTestFixtures.receiveRequest()
        ))
        XCTAssertThrowsError(
            try KagemushaPeerTextCodec.decode(text, expectedKind: .payment)
        ) { error in
            XCTAssertEqual(
                error as? KagemushaPeerTransportError,
                .unexpectedKind(expected: .payment, actual: .receiveRequest)
            )
        }
    }

    func testTextDecoderRejectsAdversarialSyntaxAndArbitraryBytes() throws {
        let text = try KagemushaPeerTextCodec.encode(.receiveRequest(
            KagemushaPeerTransportTestFixtures.receiveRequest()
        ))
        let body = String(text.dropFirst("PKK2R.".count))
        let cases = [
            "", "PKK2R.", "pkk2r." + body, "PKK2R." + body + "=",
            "PKK2R." + body + "+", "PKK2R." + body + "/",
            "PKK2R." + body + ".", "PKK2R." + body + "~",
            "PKK2R." + body + "?x=1", "PKK2R." + body + "#fragment",
            "https://example.test/?payload=" + text,
            "prefix" + text,
            "PKK2R.PKK2P." + body,
            text + "\nPKK2A.AQ",
            text + "\u{0000}", text + "\u{007F}", text + "\u{200D}",
            text + "😀",
        ]
        for value in cases {
            XCTAssertThrowsError(try KagemushaPeerTextCodec.decode(value), value)
        }

        for archive in [Data([0]), Data("not norito".utf8), Data(repeating: 0, count: 128)] {
            let arbitrary = "PKK2R." + KagemushaPeerTextCodec.base64URLEncode(archive)
            XCTAssertThrowsError(try KagemushaPeerTextCodec.decode(arbitrary))
        }
    }

    func testBase64URLCanonicalizerRejectsPaddingInvalidLengthAndAlternateSpellings() {
        XCTAssertNil(KagemushaPeerTextCodec.base64URLDecode(""))
        XCTAssertNil(KagemushaPeerTextCodec.base64URLDecode("A"))
        XCTAssertNil(KagemushaPeerTextCodec.base64URLDecode("AQ=="))
        XCTAssertNil(KagemushaPeerTextCodec.base64URLDecode("AQ+"))
        XCTAssertNil(KagemushaPeerTextCodec.base64URLDecode("AQ/"))
        XCTAssertEqual(KagemushaPeerTextCodec.base64URLDecode("AQ"), Data([1]))
        XCTAssertEqual(
            KagemushaPeerTextCodec.base64URLEncode(Data([0xFB, 0xFF])),
            "-_8"
        )
    }

    func testDirectTextArchiveLimitMapsExactlyToTwelveKiBText() {
        XCTAssertEqual(KagemushaPeerTransportContract.maximumArchiveBytesV2, 32_768)
        XCTAssertEqual(
            KagemushaPeerTransportContract.maximumArchiveBytesV4,
            32 * 1_024 * 1_024
        )
        XCTAssertEqual(
            KagemushaPeerTransportContract.maximumArchiveBytes,
            KagemushaPeerTransportContract.maximumArchiveBytesV4
        )
        XCTAssertEqual(KagemushaPeerTransportContract.maximumTextArchiveBytes, 9_211)
        let archive = Data(
            repeating: 0xA5,
            count: KagemushaPeerTransportContract.maximumTextArchiveBytes
        )
        let text = "PKK2P." + KagemushaPeerTextCodec.base64URLEncode(archive)
        XCTAssertEqual(
            text.utf8.count,
            KagemushaPeerTransportContract.maximumTextEnvelopeBytes
        )
        XCTAssertEqual(
            KagemushaPeerTextCodec.base64URLDecode(String(text.dropFirst(6))),
            archive
        )
    }

    func testOversizedTextFailsBeforeArchiveParsing() {
        let oversized = "PKK2P." + String(
            repeating: "A",
            count: KagemushaPeerTransportContract.maximumTextEnvelopeBytes
        )
        XCTAssertThrowsError(try KagemushaPeerTextCodec.decode(oversized)) { error in
            guard case .textEnvelopeTooLarge = error as? KagemushaPeerTransportError else {
                return XCTFail("unexpected error: \(error)")
            }
        }
    }

    func testUserPresentedInputIsBoundedBeforeBoundaryNormalization() throws {
        let text = try KagemushaPeerTextCodec.encode(.receiveRequest(
            KagemushaPeerTransportTestFixtures.receiveRequest()
        ))
        let leadingBytes = KagemushaPeerTransportContract.maximumTextEnvelopeBytes
            - text.utf8.count + 1
        let oversized = String(repeating: " ", count: leadingBytes) + text

        XCTAssertThrowsError(
            try KagemushaPeerTextCodec.decodeUserPresented(oversized)
        ) { error in
            XCTAssertEqual(
                error as? KagemushaPeerTransportError,
                .textEnvelopeTooLarge(
                    actual: KagemushaPeerTransportContract.maximumTextEnvelopeBytes + 1,
                    maximum: KagemushaPeerTransportContract.maximumTextEnvelopeBytes
                )
            )
        }
    }
}
