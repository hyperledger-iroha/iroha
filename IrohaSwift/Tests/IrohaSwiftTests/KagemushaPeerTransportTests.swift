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
        XCTAssertEqual(
            try IrohaPeerKagemushaAdapterV1.decode(
                message,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            ),
            payload
        )

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
            maximumKagemushaEncodedBytes: payload.archive.count - 1
        )
        XCTAssertThrowsError(
            try IrohaPeerKagemushaAdapterV1.wrap(payload, limits: tight)
        )
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
            "F0504B45504B524E464301"
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

        let nestedRequest = try offer.project(
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ).request.archive
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

    func testReceiveRequestIPM1CommitmentBindsEveryOfferField() throws {
        let offerArchive = try KagemushaPeerTransportTestFixtures.receiveOfferArchive()
        let baseline = try receiveRequestMessage(canonicalPayload: offerArchive)
        let requestFrame = try XCTUnwrap(noritoDecodeFrame(
            try KagemushaPeerTransportTestFixtures.recipientRequestArchive()
        ))
        let lineageFrame = try XCTUnwrap(noritoDecodeFrame(
            try KagemushaPeerTransportTestFixtures.recipientRegistrationLineageArchive()
        ))
        let fields = [
            ("request", requestFrame.payload),
            ("lineage", lineageFrame.payload),
            (
                "publisher_checkpoint_envelope",
                try KagemushaPeerTransportTestFixtures.publisherCheckpointEnvelope()
            ),
        ]

        for (name, fieldBytes) in fields {
            let alteredArchive = try mutateUniqueReceiveOfferField(
                offerArchive,
                fieldBytes: fieldBytes,
                name: name
            )
            let altered = try receiveRequestMessage(canonicalPayload: alteredArchive)
            XCTAssertNotEqual(
                altered.canonicalHash,
                baseline.canonicalHash,
                "IPM1 did not commit to receive-offer field \(name)"
            )

            let forged = replacingCanonicalHash(
                in: altered.encoded,
                with: baseline.canonicalHash
            )
            XCTAssertThrowsError(try IrohaPeerWireMessageV1.decode(forged)) { error in
                XCTAssertEqual(
                    error as? IrohaPeerWireMessageErrorV1,
                    .canonicalHashMismatch,
                    "IPM1 admitted substituted receive-offer field \(name)"
                )
            }
        }
    }

    func testRustCanonicalReceiveOfferFixtureIsByteExactAcrossSDKProjectionAndIPM1() throws {
        let offer = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let projection = try offer.project(
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        )
        let message = try IrohaPeerKagemushaAdapterV1.wrap(.receiveRequest(offer))

        XCTAssertEqual(
            projection.request.payload.networkID,
            TestNetworkIds.canonical
        )
        XCTAssertEqual(
            projection.request.payload.assetDefinitionID,
            "7ZepsJTHCVLKsrFFNZGSRGZgvBhv"
        )
        XCTAssertEqual(offer.noritoArchive.count, 12_423)
        XCTAssertEqual(
            sha256Hex(offer.noritoArchive),
            "6b38813ab66f1ecb83861d1641454e2d4de438472189b02ccca52a22bc6431df"
        )
        XCTAssertEqual(projection.request.archive.count, 753)
        XCTAssertEqual(
            sha256Hex(projection.request.archive),
            "d325566b1117fa368703a971367056173f2d8349d2e86101dc06187aaf8fd2b4"
        )
        XCTAssertEqual(projection.lineageArchive.count, 11_297)
        XCTAssertEqual(
            sha256Hex(projection.lineageArchive),
            "b61dd641527bfb9e09479906c008b6c061b54009229e6e9ec5f0717572cfb561"
        )
        XCTAssertEqual(projection.publisherCheckpointEnvelope.count, 393)
        XCTAssertEqual(
            sha256Hex(projection.publisherCheckpointEnvelope),
            "ed6f4796046ee1d35f844cc862586dbe1d7d0f59db51638c33559052f4196bef"
        )
        XCTAssertEqual(message.encoded.count, 12_507)
        XCTAssertEqual(
            try IrohaPeerKagemushaAdapterV1.decode(
                IrohaPeerWireMessageV1.decode(message.encoded),
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            ),
            .receiveRequest(offer)
        )
    }

    func testTypedAcknowledgementTextRoundTripIsCanonical() throws {
        let request = try KagemushaPeerTransportTestFixtures.paymentRequest()
        let payment = try KagemushaPeerTransportTestFixtures.payment(request: request)
        let acknowledgement = try KagemushaPeerTransportTestFixtures.acknowledgement(
            request: request,
            payment: payment
        )
        let payload = KagemushaPeerPayload.acknowledgement(acknowledgement)
        let text = try KagemushaPeerTextCodec.encode(payload)

        XCTAssertTrue(text.hasPrefix("PKK2A."))
        XCTAssertFalse(text.contains("="))
        XCTAssertLessThanOrEqual(
            text.utf8.count,
            KagemushaRecursiveSpend.maximumPeerTextEnvelopeBytes
        )
        XCTAssertEqual(
            try KagemushaPeerTextCodec.decode(
                text,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1,
                expectedKind: .acknowledgement
            ),
            payload
        )
        XCTAssertEqual(try KagemushaPeerTextCodec.encode(
            KagemushaPeerTextCodec.decode(
                text,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            )
        ), text)
    }

    func testCanonicalPaymentFixtureUsesFirstReleaseABI21Envelope() throws {
        let request = try KagemushaPeerTransportTestFixtures.paymentRequest()
        let payment = try KagemushaPeerTransportTestFixtures.payment(request: request)
        let verifiedRequest = try request.verified(atMilliseconds: 1_900_000_001_000)
        let summary = try payment.recipientBundle.projectedSummary()

        let frame = try XCTUnwrap(noritoDecodeFrame(payment.recipientBundle.noritoArchive))
        XCTAssertEqual(
            frame.header.schema,
            noritoSchemaHash(forTypeName: KagemushaRecursiveSpend.bundleWireNameV4)
        )
        XCTAssertEqual(KagemushaRecursiveSpend.wireVersionV4, 4)
        XCTAssertEqual(payment.archive.count, 12_896)
        XCTAssertEqual(
            sha256Hex(payment.archive),
            "37ee56ad5663ab67b8b5b9a72927f1e0811142122bf04fa28a55634f96b7d3af"
        )
        XCTAssertEqual(
            verifiedRequest.digest.hexEncodedString(),
            "cb6508a5aa6b56ada90978d4db638b2176f20f154e9de4ed8a450d95a940c71b"
        )
        XCTAssertEqual(summary.assetDefinitionID, request.payload.assetDefinitionID)
        XCTAssertEqual(summary.amount, request.payload.amount)
        XCTAssertEqual(
            summary.noteCommitment,
            request.payload.recipientOutput.noteCommitment
        )
        XCTAssertEqual(summary.hopCount, 1)
        XCTAssertEqual(summary.proofStepCount, 2)
        XCTAssertEqual(summary.branchClaims.count, 1)
        XCTAssertEqual(
            summary.artifactBinding.generation,
            "swift-kagemusha-abi21-fixture"
        )
        XCTAssertEqual(summary.artifactBinding.manifestSHA256, Data(repeating: 0x51, count: 32))
        XCTAssertEqual(
            summary.verifierKeyID,
            try KagemushaRecursiveSpend.releaseQualifiedStepEqVerifierKeyIDV4(
                manifestSHA256: Data(repeating: 0x51, count: 32)
            )
        )
    }

    func testGenuinePaymentExceedsDirectTextArchiveContract() throws {
        let request = try KagemushaPeerTransportTestFixtures.paymentRequest()
        let payment = try KagemushaPeerTransportTestFixtures.payment(request: request)

        XCTAssertThrowsError(try KagemushaPeerTextCodec.encode(.payment(payment))) { error in
            XCTAssertEqual(
                error as? KagemushaPeerTransportError,
                .archiveTooLarge(
                    actual: 12_896,
                    maximum: KagemushaPeerTransportContract.maximumTextArchiveBytes
                )
            )
        }
    }

    func testUserPresentedBoundaryNormalizationIsNarrowAndExplicit() throws {
        let request = try KagemushaPeerTransportTestFixtures.paymentRequest()
        let payment = try KagemushaPeerTransportTestFixtures.payment(request: request)
        let payload = KagemushaPeerPayload.acknowledgement(
            try KagemushaPeerTransportTestFixtures.acknowledgement(
                request: request,
                payment: payment
            )
        )
        let text = try KagemushaPeerTextCodec.encode(payload)
        let presented = " \t\r\n" + text + "\n\r\t "

        XCTAssertThrowsError(try KagemushaPeerTextCodec.decode(
            presented,
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ))
        XCTAssertEqual(
            KagemushaPeerTextCodec.canonicalizeUserPresented(presented),
            text
        )
        XCTAssertEqual(
            try KagemushaPeerTextCodec.decodeUserPresented(
                presented,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            ),
            payload
        )
        for scalar in ["\u{000B}", "\u{0085}", "\u{00A0}", "\u{200B}", "\u{2028}", "\u{FEFF}"] {
            XCTAssertThrowsError(
                try KagemushaPeerTextCodec.decodeUserPresented(
                    scalar + text,
                    chainDiscriminant: SccpV1.tairaI105DiscriminantV1
                ),
                "unexpectedly normalized U+\(scalar.unicodeScalars.first!.value)"
            )
        }
    }

    func testExpectedKindCannotBeSubstituted() throws {
        let request = try KagemushaPeerTransportTestFixtures.paymentRequest()
        let payment = try KagemushaPeerTransportTestFixtures.payment(request: request)
        let text = try KagemushaPeerTextCodec.encode(.acknowledgement(
            KagemushaPeerTransportTestFixtures.acknowledgement(
                request: request,
                payment: payment
            )
        ))
        XCTAssertThrowsError(
            try KagemushaPeerTextCodec.decode(
                text,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1,
                expectedKind: .receiveRequest
            )
        ) { error in
            XCTAssertEqual(
                error as? KagemushaPeerTransportError,
                .unexpectedKind(expected: .receiveRequest, actual: .acknowledgement)
            )
        }
    }

    func testTextDecoderRejectsAdversarialSyntaxAndArbitraryBytes() throws {
        let request = try KagemushaPeerTransportTestFixtures.paymentRequest()
        let payment = try KagemushaPeerTransportTestFixtures.payment(request: request)
        let text = try KagemushaPeerTextCodec.encode(.acknowledgement(
            KagemushaPeerTransportTestFixtures.acknowledgement(
                request: request,
                payment: payment
            )
        ))
        let body = String(text.dropFirst("PKK2A.".count))
        let cases = [
            "", "PKK2A.", "pkk2a." + body, "PKK2A." + body + "=",
            "PKK2A." + body + "+", "PKK2A." + body + "/",
            "PKK2A." + body + ".", "PKK2A." + body + "~",
            "PKK2A." + body + "?x=1", "PKK2A." + body + "#fragment",
            "https://example.test/?payload=" + text,
            "prefix" + text,
            "PKK2A.PKK2R." + body,
            text + "\nPKK2P.AQ",
            text + "\u{0000}", text + "\u{007F}", text + "\u{200D}",
            text + "😀",
        ]
        for value in cases {
            XCTAssertThrowsError(try KagemushaPeerTextCodec.decode(
                value,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            ), value)
        }

        for archive in [Data([0]), Data("not norito".utf8), Data(repeating: 0, count: 128)] {
            let arbitrary = "PKK2A." + KagemushaPeerTextCodec.base64URLEncode(archive)
            XCTAssertThrowsError(try KagemushaPeerTextCodec.decode(
                arbitrary,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            ))
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
        XCTAssertThrowsError(try KagemushaPeerTextCodec.decode(
            oversized,
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        )) { error in
            guard case .textEnvelopeTooLarge = error as? KagemushaPeerTransportError else {
                return XCTFail("unexpected error: \(error)")
            }
        }
    }

    func testUserPresentedInputIsBoundedBeforeBoundaryNormalization() throws {
        let request = try KagemushaPeerTransportTestFixtures.paymentRequest()
        let payment = try KagemushaPeerTransportTestFixtures.payment(request: request)
        let text = try KagemushaPeerTextCodec.encode(.acknowledgement(
            KagemushaPeerTransportTestFixtures.acknowledgement(
                request: request,
                payment: payment
            )
        ))
        let leadingBytes = KagemushaPeerTransportContract.maximumTextEnvelopeBytes
            - text.utf8.count + 1
        let oversized = String(repeating: " ", count: leadingBytes) + text

        XCTAssertThrowsError(
            try KagemushaPeerTextCodec.decodeUserPresented(
                oversized,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            )
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

    private func receiveRequestMessage(
        canonicalPayload: Data
    ) throws -> IrohaPeerWireMessageV1 {
        try IrohaPeerWireMessageV1(
            profile: .kagemusha,
            kind: .receiveRequest,
            schemaVersion: IrohaPeerWireProfileV1.kagemusha.requiredSchemaVersion,
            canonicalPayload: canonicalPayload
        )
    }

    private func mutateUniqueReceiveOfferField(
        _ archive: Data,
        fieldBytes: Data,
        name: String
    ) throws -> Data {
        let frame = try XCTUnwrap(noritoDecodeFrame(archive))
        var payload = frame.payload
        let range = try XCTUnwrap(
            payload.range(of: fieldBytes),
            "receive-offer field \(name) is absent from the canonical archive"
        )
        XCTAssertNil(
            payload[range.upperBound...].range(of: fieldBytes),
            "receive-offer field \(name) is not uniquely encoded"
        )
        payload[range.lowerBound] ^= 0x01
        return KagemushaRecursiveSpend.frameArchive(
            schema: KagemushaRecursiveSpend.recipientReceiveOfferWireName,
            payload: payload
        )
    }

    private func replacingCanonicalHash(
        in encoded: Data,
        with canonicalHash: Data
    ) -> Data {
        precondition(encoded.count >= IrohaPeerWireMessageV1.headerBytes)
        precondition(canonicalHash.count == 32)
        var forged = encoded
        forged.replaceSubrange(20..<52, with: canonicalHash)
        let prefix = forged.subdata(in: 0..<52)
        let body = forged.subdata(
            in: IrohaPeerWireMessageV1.headerBytes..<forged.count
        )
        let wireHash = Blake2b.hash256(
            Data("IROHA-PEER-MESSAGE-V1\0".utf8) + prefix + body
        )
        forged.replaceSubrange(52..<84, with: wireHash)
        return forged
    }

    private func sha256Hex(_ data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }
}
