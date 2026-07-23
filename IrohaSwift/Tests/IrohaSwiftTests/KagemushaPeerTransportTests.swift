import CryptoKit
import XCTest
@testable import IrohaSwift

final class KagemushaPeerTransportTests: XCTestCase {
    func testIPM1AdapterReservesOnlyNativeArchiveSchema0102() throws {
        let payload = KagemushaPeerPayload.receiveRequest(
            try KagemushaPeerTransportTestFixtures.receiveRequest()
        )
        let message = try IrohaPeerKagemushaAdapterV1.wrap(payload)
        XCTAssertEqual(message.profile, .kagemusha)
        XCTAssertEqual(message.schemaVersion, 0x0102)
        XCTAssertEqual(try IrohaPeerKagemushaAdapterV1.decode(message), payload)

        XCTAssertThrowsError(try IrohaPeerWireMessageV1(
            profile: .kagemusha,
            kind: .receiveRequest,
            schemaVersion: 1,
            canonicalPayload: payload.archive
        )) {
            XCTAssertEqual(
                $0 as? IrohaPeerWireMessageErrorV1,
                .schemaVersionMismatch(
                    profile: .kagemusha,
                    expected: 0x0102,
                    actual: 1
                )
            )
        }

        let tight = IrohaPeerWireLimitsV1(
            maximumCanonicalBytes: 32 * 1_024,
            maximumOfflineNoteEncodedBytes: 24_576,
            maximumKagemushaEncodedBytes: payload.archive.count - 1
        )
        XCTAssertThrowsError(
            try IrohaPeerKagemushaAdapterV1.wrap(payload, limits: tight)
        )

        let offline = try IrohaPeerWireMessageV1(
            profile: .offlineNote,
            kind: .receiveRequest,
            schemaVersion: 1,
            canonicalPayload: payload.archive
        )
        XCTAssertThrowsError(try IrohaPeerKagemushaAdapterV1.decode(offline)) {
            XCTAssertEqual(
                $0 as? IrohaPeerWireMessageErrorV1,
                .unexpectedProfile(expected: .kagemusha, actual: .offlineNote)
            )
        }
    }

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
            "F049524F48415045455201"
        )
        XCTAssertEqual(KagemushaPeerTransportContract.nearbyServiceName, "pk-kagemusha")
        XCTAssertEqual(
            Set(KagemushaPeerPayloadKind.allCases.map(\.contentType)).count,
            KagemushaPeerPayloadKind.allCases.count
        )
        XCTAssertEqual(
            KagemushaRecursiveSpend.recipientReceiveOfferWireName,
            "iroha_torii_shared::offline_api::OfflineRecipientReceiveOfferV2"
        )
    }

    func testReceiveRequestIPM1RequiresWholeOfferSchema() throws {
        let offer = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let message = try IrohaPeerWireMessageV1(
            profile: .kagemusha,
            kind: .receiveRequest,
            schemaVersion: IrohaPeerWireProfileV1.kagemusha.requiredSchemaVersion,
            canonicalPayload: offer.noritoArchive
        )
        XCTAssertEqual(
            noritoDecodeFrame(message.canonicalPayload)?.header.schema,
            noritoSchemaHash(
                forTypeName: KagemushaRecursiveSpend.recipientReceiveOfferWireName
            )
        )

        let nestedRequest = try offer.project().request.archive
        XCTAssertThrowsError(try IrohaPeerWireMessageV1(
            profile: .kagemusha,
            kind: .receiveRequest,
            schemaVersion: IrohaPeerWireProfileV1.kagemusha.requiredSchemaVersion,
            canonicalPayload: nestedRequest
        )) { error in
            XCTAssertEqual(
                error as? IrohaPeerWireMessageErrorV1,
                .invalidCanonicalPayload(profile: .kagemusha, kind: .receiveRequest)
            )
        }
    }

    func testRustCanonicalReceiveOfferFixtureIsByteExactAcrossSDKProjectionAndIPM1() throws {
        let offer = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let projection = try offer.project()
        let message = try IrohaPeerKagemushaAdapterV1.wrap(.receiveRequest(offer))

        XCTAssertEqual(offer.noritoArchive.count, 14_005)
        XCTAssertEqual(
            sha256Hex(offer.noritoArchive),
            "06360875dc6f6f21f020105ddc995735e94a388dac107b2624beefdb1526f95a"
        )
        XCTAssertEqual(projection.request.archive.count, 759)
        XCTAssertEqual(
            sha256Hex(projection.request.archive),
            "862bfeaf377917c8f32700bcd37f1140ba3a8cf465ccb83749026cb0aeaa2577"
        )
        XCTAssertEqual(projection.lineageArchive.count, 11_218)
        XCTAssertEqual(
            sha256Hex(projection.lineageArchive),
            "62960c5ce0217ae6372ca6e173db7d4b913f0c913e002fda073bb3a086a2932a"
        )
        XCTAssertEqual(projection.publisherCheckpointEnvelope.count, 2_048)
        XCTAssertEqual(
            sha256Hex(projection.publisherCheckpointEnvelope),
            "d155d8352105884fe0bbb10b9fac2ad7573ab851a51dd62f91e36d4c23fe57bd"
        )
        XCTAssertEqual(message.encoded.count, 14_089)
        XCTAssertEqual(
            try IrohaPeerKagemushaAdapterV1.decode(
                IrohaPeerWireMessageV1.decode(message.encoded)
            ),
            .receiveRequest(offer)
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

    func testCanonicalPaymentFixtureUsesFirstReleaseABI21Envelope() throws {
        let request = try KagemushaPeerTransportTestFixtures.paymentRequest()
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

    private func sha256Hex(_ data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }
}
