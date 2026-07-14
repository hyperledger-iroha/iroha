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

    func testNearbyEnvelopeRoundTripsEveryTypedMessage() throws {
        let request = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let payment = try KagemushaPeerTransportTestFixtures.payment(request: request)
        let acknowledgement = try KagemushaPeerTransportTestFixtures.acknowledgement(
            request: request,
            payment: payment
        )
        let challenge = KagemushaNearbyPairingChallenge(symbol: .bird)

        let decodedRequest = try KagemushaNearbyEnvelopeCodec.decode(
            KagemushaNearbyEnvelopeCodec.encode(
                .receiveRequest(request),
                pairingChallenge: challenge
            )
        )
        XCTAssertEqual(decodedRequest.messageKind, .receiveRequest)
        XCTAssertEqual(decodedRequest.payload, .receiveRequest(request))
        XCTAssertEqual(decodedRequest.pairingChallenge, challenge)

        let encodedPayment = try KagemushaNearbyEnvelopeCodec.encode(.payment(payment))
        if KagemushaRecursiveSpend.hasRequiredNativeSymbols {
            let decodedPayment = try KagemushaNearbyEnvelopeCodec.decode(encodedPayment)
            XCTAssertEqual(decodedPayment.payload, .payment(payment))
            XCTAssertNil(decodedPayment.pairingChallenge)
        } else {
            XCTAssertThrowsError(try KagemushaNearbyEnvelopeCodec.decode(encodedPayment)) { error in
                XCTAssertEqual(error as? KagemushaNearbyError, .invalidMessage)
            }
        }

        let decodedAcknowledgement = try KagemushaNearbyEnvelopeCodec.decode(
            KagemushaNearbyEnvelopeCodec.encode(.acknowledgement(acknowledgement))
        )
        XCTAssertEqual(decodedAcknowledgement.payload, .acknowledgement(acknowledgement))
        XCTAssertNil(decodedAcknowledgement.pairingChallenge)
    }

    func testRequestRequiresPairingAndOtherMessagesForbidIt() throws {
        let request = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let payment = try KagemushaPeerTransportTestFixtures.payment(request: request)
        let challenge = KagemushaNearbyPairingChallenge(symbol: .stars)

        XCTAssertThrowsError(try KagemushaNearbyEnvelopeCodec.encode(
            .receiveRequest(request)
        ))
        XCTAssertThrowsError(try KagemushaNearbyEnvelopeCodec.encode(
            .payment(payment),
            pairingChallenge: challenge
        ))
    }

    func testCanonicalRejectionEnvelopeIsExactAndTypedPayloadFree() throws {
        let data = try KagemushaNearbyEnvelopeCodec.encodeRejection()
        let decoded = try KagemushaNearbyEnvelopeCodec.decode(data)
        XCTAssertEqual(decoded.messageKind, .rejected)
        XCTAssertNil(decoded.payload)
        XCTAssertNil(decoded.pairingChallenge)
        XCTAssertEqual(
            try JSONSerialization.jsonObject(with: data) as? [String: String],
            [
                "contentType": "text/plain",
                "kind": "rejected",
                "payload": "cmVqZWN0ZWQ",
            ]
        )
    }

    func testEnvelopeRejectsUnknownDuplicateAndNonCanonicalJSON() throws {
        let request = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let canonical = try KagemushaNearbyEnvelopeCodec.encode(
            .receiveRequest(request),
            pairingChallenge: .init(symbol: .mask)
        )
        var object = try XCTUnwrap(
            try JSONSerialization.jsonObject(with: canonical) as? [String: Any]
        )
        object["unknown"] = true
        XCTAssertThrowsError(try KagemushaNearbyEnvelopeCodec.decode(
            try JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
        ))

        let string = try XCTUnwrap(String(data: canonical, encoding: .utf8))
        let duplicate = string.replacingOccurrences(
            of: "{",
            with: "{\"kind\":\"payment\",",
            options: [],
            range: string.startIndex..<string.index(after: string.startIndex)
        )
        XCTAssertThrowsError(try KagemushaNearbyEnvelopeCodec.decode(Data(duplicate.utf8)))

        let pretty = try JSONSerialization.data(
            withJSONObject: try JSONSerialization.jsonObject(with: canonical),
            options: [.prettyPrinted, .sortedKeys]
        )
        XCTAssertThrowsError(try KagemushaNearbyEnvelopeCodec.decode(pretty))
    }

    func testEnvelopeRejectsPaddedBase64WrongContentTypeAndKindSubstitution() throws {
        let request = try KagemushaPeerTransportTestFixtures.receiveRequest()
        let canonical = try KagemushaNearbyEnvelopeCodec.encode(
            .receiveRequest(request),
            pairingChallenge: .init(symbol: .stars)
        )
        let original = try XCTUnwrap(
            try JSONSerialization.jsonObject(with: canonical) as? [String: Any]
        )
        for mutation in ["padding", "content", "kind", "missing_pairing"] {
            var object = original
            switch mutation {
            case "padding":
                object["payload"] = (object["payload"] as! String) + "="
            case "content":
                object["contentType"] = KagemushaPeerPayloadKind.payment.contentType
            case "kind":
                object["kind"] = "payment"
            default:
                object.removeValue(forKey: "pairingChallenge")
            }
            XCTAssertThrowsError(try KagemushaNearbyEnvelopeCodec.decode(
                try JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
            ), mutation)
        }
    }

    func testEnvelopeSizeLimitAndMalformedUTF8FailBeforeModelUse() {
        XCTAssertThrowsError(try KagemushaNearbyEnvelopeCodec.decode(
            Data(repeating: 0x20, count: KagemushaNearbyEnvelopeCodec.maximumEnvelopeBytes + 1)
        ))
        XCTAssertThrowsError(try KagemushaNearbyEnvelopeCodec.decode(
            Data([0x7B, 0xFF, 0x7D])
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
        let exchange = KagemushaNearbyExchange()
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
