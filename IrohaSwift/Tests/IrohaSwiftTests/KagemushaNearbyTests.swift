import XCTest
@testable import IrohaSwift

final class KagemushaNearbyTests: XCTestCase {
    private final class InvocationRecorder: @unchecked Sendable {
        private let lock = NSLock()
        private var value = 0

        func record() {
            lock.lock()
            value += 1
            lock.unlock()
        }

        var count: Int {
            lock.lock()
            defer { lock.unlock() }
            return value
        }
    }

    func testPairingSymbolsPreserveFirstReleaseWireValues() throws {
        XCTAssertEqual(
            KagemushaNearbyPairingSymbol.allCases.map(\.rawValue),
            [
                "nearby_pairing_stars",
                "nearby_pairing_bird",
                "nearby_pairing_mask",
            ]
        )
        for symbol in KagemushaNearbyPairingSymbol.allCases {
            let challenge = KagemushaNearbyPairingChallenge(symbol: symbol)
            XCTAssertEqual(
                try JSONDecoder().decode(
                    KagemushaNearbyPairingChallenge.self,
                    from: JSONEncoder().encode(challenge)
                ),
                challenge
            )
        }
        XCTAssertThrowsError(try JSONDecoder().decode(
            KagemushaNearbyPairingChallenge.self,
            from: Data("\"nearby_pairing_unknown\"".utf8)
        ))
    }

    func testTimeoutNanosecondsSaturatesOnOverflow() {
        XCTAssertEqual(
            KagemushaNearbyTransportPolicy.timeoutNanoseconds(seconds: 1),
            1_000_000_000
        )
        XCTAssertEqual(
            KagemushaNearbyTransportPolicy.timeoutNanoseconds(seconds: UInt64.max),
            UInt64.max
        )
    }

    func testNearbyEnvelopeRoundTripsEveryTypedMessage() throws {
        let offer = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let request = try offer.project(
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ).request
        let payment = try KagemushaPeerTransportTestFixtures.payment(request: request)
        let acknowledgement = try KagemushaPeerTransportTestFixtures.acknowledgement(
            request: request,
            payment: payment
        )
        let challenge = KagemushaNearbyPairingChallenge(symbol: .bird)

        let encodedRequest = try KagemushaNearbyEnvelopeCodec.encode(
            .receiveRequest(offer),
            pairingChallenge: challenge
        )
        XCTAssertEqual(encodedRequest.prefix(5), Data("PKNB1".utf8))
        XCTAssertEqual(encodedRequest.count, 12 + 84 + 12_306)
        let decodedRequest = try KagemushaNearbyEnvelopeCodec.decode(
            encodedRequest,
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        )
        XCTAssertEqual(decodedRequest.messageKind, .receiveRequest)
        XCTAssertEqual(decodedRequest.payload, .receiveRequest(offer))
        XCTAssertEqual(decodedRequest.pairingChallenge, challenge)

        let encodedPayment = try KagemushaNearbyEnvelopeCodec.encode(.payment(payment))
        if KagemushaRecursiveSpend.hasRequiredNativeSymbols {
            let decodedPayment = try KagemushaNearbyEnvelopeCodec.decode(
                encodedPayment,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            )
            XCTAssertEqual(decodedPayment.payload, .payment(payment))
            XCTAssertNil(decodedPayment.pairingChallenge)
        } else {
            XCTAssertThrowsError(try KagemushaNearbyEnvelopeCodec.decode(
                encodedPayment,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            )) { error in
                XCTAssertEqual(error as? KagemushaNearbyError, .invalidMessage)
            }
        }

        let decodedAcknowledgement = try KagemushaNearbyEnvelopeCodec.decode(
            KagemushaNearbyEnvelopeCodec.encode(.acknowledgement(acknowledgement)),
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        )
        XCTAssertEqual(decodedAcknowledgement.payload, .acknowledgement(acknowledgement))
        XCTAssertNil(decodedAcknowledgement.pairingChallenge)
    }

    func testRequestRequiresPairingAndOtherMessagesForbidIt() throws {
        let offer = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let payment = try KagemushaPeerTransportTestFixtures.payment(
            request: KagemushaPeerTransportTestFixtures.paymentRequest()
        )
        let challenge = KagemushaNearbyPairingChallenge(symbol: .stars)

        XCTAssertThrowsError(try KagemushaNearbyEnvelopeCodec.encode(
            .receiveRequest(offer)
        ))
        XCTAssertThrowsError(try KagemushaNearbyEnvelopeCodec.encode(
            .payment(payment),
            pairingChallenge: challenge
        ))
    }

    func testCanonicalRejectionEnvelopeIsExactAndTypedPayloadFree() throws {
        let data = try KagemushaNearbyEnvelopeCodec.encodeRejection()
        let decoded = try KagemushaNearbyEnvelopeCodec.decode(
            data,
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        )
        XCTAssertEqual(decoded.messageKind, .rejected)
        XCTAssertNil(decoded.payload)
        XCTAssertNil(decoded.pairingChallenge)
        XCTAssertEqual(
            data,
            Data([0x50, 0x4b, 0x4e, 0x42, 0x31, 0x04, 0x00, 0x00,
                  0x00, 0x00, 0x00, 0x00])
        )
    }

    func testBinaryEnvelopeRejectsHeaderAndLengthSubstitution() throws {
        let offer = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let canonical = try KagemushaNearbyEnvelopeCodec.encode(
            .receiveRequest(offer),
            pairingChallenge: .init(symbol: .mask)
        )
        for mutation in [
            "magic", "kind", "pairing", "reserved", "length", "ipm", "trailing",
        ] {
            var bytes = canonical
            switch mutation {
            case "magic":
                bytes[0] ^= 0xff
            case "kind":
                bytes[5] = 2
            case "pairing":
                bytes[6] = 4
            case "reserved":
                bytes[7] = 1
            case "length":
                bytes[11] &-= 1
            case "ipm":
                bytes[12] ^= 0xff
            default:
                bytes.append(0)
            }
            XCTAssertThrowsError(
                try KagemushaNearbyEnvelopeCodec.decode(
                    bytes,
                    chainDiscriminant: SccpV1.tairaI105DiscriminantV1
                ),
                mutation
            )
        }
    }

    func testEnvelopeSizeLimitAndTruncatedHeaderFailBeforeModelUse() {
        XCTAssertEqual(KagemushaNearbyEnvelopeCodec.maximumEnvelopeBytes, 32_704)
        XCTAssertThrowsError(try KagemushaNearbyEnvelopeCodec.decode(
            Data(repeating: 0x20, count: KagemushaNearbyEnvelopeCodec.maximumEnvelopeBytes + 1),
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ))
        XCTAssertThrowsError(try KagemushaNearbyEnvelopeCodec.decode(
            Data("PKNB1".utf8),
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        ))
    }

    func testNearbyPolicyKeepsStableServiceAndKagemushaDiscoveryMetadata() {
        XCTAssertEqual(KagemushaNearbyTransportPolicy.serviceName, "pk-kagemusha")
        XCTAssertEqual(KagemushaNearbyTransportPolicy.bonjourService, "_pk-kagemusha._tcp")
        XCTAssertEqual(
            KagemushaNearbyTransportPolicy.discoveryInfo,
            ["protocol": "kagemusha-v2"]
        )
        XCTAssertLessThanOrEqual(
            KagemushaNearbyTransportPolicy.peerDisplayName().utf8.count,
            63
        )
        XCTAssertEqual(
            KagemushaNearbyTransportPolicy.acknowledgementDisconnectGraceNanoseconds,
            1_500_000_000
        )
    }

    func testLocalNetworkDenialNormalizationDoesNotExposeNSError() {
        let error = NSError(
            domain: NetService.errorDomain,
            code: -72008,
            userInfo: [NSLocalizedDescriptionKey: "Local network policy denied"]
        )
        XCTAssertEqual(
            KagemushaNearbyError.normalized(error),
            .localNetworkPermissionDenied
        )
        XCTAssertEqual(
            KagemushaNearbyError.normalized(CancellationError()),
            .cancelled
        )
    }

    func testLiveTransportRequiresAuditedAuthenticatedTranscriptBackend() {
        XCTAssertTrue(
            KagemushaNearbyAuthenticationPolicy
                .requiresCertificateAuthenticatedECDHTranscript
        )
        XCTAssertFalse(
            KagemushaNearbyAuthenticationPolicy
                .hasAuditedAuthenticatedTranscriptBackend
        )
        XCTAssertFalse(KagemushaNearbyExchange.isAvailable)
    }

    func testClosedAuthenticationGateThrowsBeforeCallbacksOrPermissionWork() async throws {
        let exchange = KagemushaNearbyExchange(chainDiscriminant: SccpV1.tairaI105DiscriminantV1)
        let recorder = InvocationRecorder()
        do {
            try await exchange.requestLocalNetworkAccess()
            XCTFail("unauthenticated Nearby permission preflight must not run")
        } catch {
            XCTAssertEqual(error as? KagemushaNearbyError, .unavailable)
        }

        do {
            _ = try await exchange.sendPayment(
                onEvent: { _ in recorder.record() },
                confirmPairing: { _ in
                    recorder.record()
                    return .accepted
                },
                createPayment: { _ in
                    recorder.record()
                    throw KagemushaNearbyError.invalidMessage
                }
            )
            XCTFail("unauthenticated Nearby send must not start")
        } catch {
            XCTAssertEqual(error as? KagemushaNearbyError, .unavailable)
        }

        let request = try KagemushaPeerTransportTestFixtures.receiveRequest()
        do {
            _ = try await exchange.receivePayment(
                receiveRequest: request,
                pairingChallenge: .init(symbol: .stars),
                onEvent: { _ in recorder.record() },
                acceptPayment: { _ in
                    recorder.record()
                    throw KagemushaNearbyError.invalidMessage
                }
            )
            XCTFail("unauthenticated Nearby receive must not start")
        } catch {
            XCTAssertEqual(error as? KagemushaNearbyError, .unavailable)
        }
        XCTAssertEqual(recorder.count, 0)
    }
}
