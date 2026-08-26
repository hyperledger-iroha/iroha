import CryptoKit
@testable import IrohaSwift
import XCTest

private final class ReputationStubURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data?))?

    override class func canInit(with _: URLRequest) -> Bool {
        true
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        guard let handler = Self.handler else {
            client?.urlProtocol(
                self,
                didFailWithError: NSError(domain: "ReputationStub", code: -1)
            )
            return
        }
        do {
            let (response, data) = try handler(request)
            client?.urlProtocol(
                self,
                didReceive: response,
                cacheStoragePolicy: .notAllowed
            )
            if let data {
                client?.urlProtocol(self, didLoad: data)
            }
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}
}

private final class ReputationRequestRecorder: @unchecked Sendable {
    private let lock = NSLock()
    private var requests: [URLRequest] = []

    func append(_ request: URLRequest) {
        lock.lock()
        requests.append(request)
        lock.unlock()
    }

    var snapshot: [URLRequest] {
        lock.lock()
        defer { lock.unlock() }
        return requests
    }
}

@available(iOS 15.0, macOS 12.0, *)
final class SorafsReputationClientTests: XCTestCase {
    private let seed = Data(repeating: 0x47, count: 32)
    private let snapshotA = String(repeating: "a", count: 32)
    private let snapshotB = String(repeating: "b", count: 32)
    private let digestC = String(repeating: "c", count: 64)
    private let digestD = String(repeating: "d", count: 64)

    override func tearDown() {
        ReputationStubURLProtocol.handler = nil
        super.tearDown()
    }

    func testLatestSignsExactBoundRequestAndDecodesClosedSnapshot() async throws {
        let recorder = ReputationRequestRecorder()
        ReputationStubURLProtocol.handler = { [self] request in
            recorder.append(request)
            return try response(
                request,
                body: snapshotJSON(limit: 7, generatedAt: UInt64.max)
            )
        }
        let client = try makeClient()

        let snapshot = try await client.latest(limit: 7)

        XCTAssertEqual(snapshot.generatedAtUnix, UInt64.max)
        XCTAssertEqual(snapshot.limit, 7)
        XCTAssertEqual(snapshot.providers.map(\.providerId), ["provider-1"])
        let request = try XCTUnwrap(recorder.snapshot.first)
        XCTAssertEqual(request.httpMethod, "GET")
        XCTAssertNil(request.httpBody)
        XCTAssertEqual(
            request.url?.absoluteString,
            "https://reputation.example/v1/sorafs/reputation/latest?limit=7"
        )
        XCTAssertNil(request.value(forHTTPHeaderField: "Last-Event-ID"))
        XCTAssertEqual(
            request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerTimestampMs),
            "4102444800000"
        )
        XCTAssertEqual(
            request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerNonce),
            "swift-reputation-test-1"
        )
        try assertCanonicalSignature(request)
    }

    func testInitializationRejectsMismatchedAccountAndPrivateKey() throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [ReputationStubURLProtocol.self]
        let signingKey = try Curve25519.Signing.PrivateKey(rawRepresentation: seed)
        let account = try AccountAddress.fromAccount(
            publicKey: signingKey.publicKey.rawRepresentation
        ).toI105(networkPrefix: AccountId.defaultNetworkPrefix)
        let wrongPrivateKey = Data(repeating: 0x48, count: 32)
        let baseURL = try XCTUnwrap(URL(string: "https://reputation.example"))

        XCTAssertThrowsError(
            try SorafsReputationClient(
                baseURL: baseURL,
                session: URLSession(configuration: configuration),
                networkId: TestNetworkIds.canonical,
                accountId: account,
                privateKey: wrongPrivateKey
            )
        ) { error in
            guard case let SorafsReputationClientError.invalidConfiguration(message) = error else {
                return XCTFail("unexpected error \(error)")
            }
            XCTAssertTrue(message.contains("matching exact canonical"))
        }
    }

    func testFiniteReadsUseFreshProofsAndNeverRetryFailure() async throws {
        let recorder = ReputationRequestRecorder()
        ReputationStubURLProtocol.handler = { [self] request in
            recorder.append(request)
            if recorder.snapshot.count == 3 {
                return try response(request, status: 503, body: #"{"error":"down"}"#)
            }
            return try response(request, body: weightsJSON())
        }
        let client = try makeClient()

        _ = try await client.weights()
        _ = try await client.weights()
        do {
            _ = try await client.weights()
            XCTFail("expected terminal HTTP failure")
        } catch let error as SorafsReputationClientError {
            XCTAssertEqual(error, .httpStatus(503))
        }

        let requests = recorder.snapshot
        XCTAssertEqual(requests.count, 3)
        let nonces = requests.compactMap {
            $0.value(forHTTPHeaderField: ToriiCanonicalRequest.headerNonce)
        }
        let signatures = requests.compactMap {
            $0.value(forHTTPHeaderField: ToriiCanonicalRequest.headerSignature)
        }
        XCTAssertEqual(Set(nonces).count, 3)
        XCTAssertEqual(Set(signatures).count, 3)
        try requests.forEach(assertCanonicalSignature)
    }

    func testRedirectIsTerminalAndDoesNotReachRedirectTarget() async throws {
        for status in [307, 308] {
            let recorder = ReputationRequestRecorder()
            ReputationStubURLProtocol.handler = { [self] request in
                recorder.append(request)
                return try response(
                    request,
                    status: status,
                    body: "",
                    additionalHeaders: ["Location": "https://attacker.example/stolen"]
                )
            }
            let client = try makeClient()

            do {
                _ = try await client.weights()
                XCTFail("expected redirect rejection")
            } catch let error as SorafsReputationClientError {
                XCTAssertEqual(error, .httpStatus(status))
            }

            XCTAssertEqual(recorder.snapshot.count, 1)
            XCTAssertEqual(recorder.snapshot.first?.url?.host, "reputation.example")
        }
    }

    func testNetworkFailureIsTerminalAndNotRetried() async throws {
        let recorder = ReputationRequestRecorder()
        ReputationStubURLProtocol.handler = { request in
            recorder.append(request)
            throw URLError(.networkConnectionLost)
        }
        let client = try makeClient()

        await XCTAssertThrowsErrorAsync(try await client.weights()) { _ in }
        XCTAssertEqual(recorder.snapshot.count, 1)
    }

    func testEventsPreserveMaximumUInt64AndBindCursor() async throws {
        ReputationStubURLProtocol.handler = { [self] request in
            try response(
                request,
                body: eventsJSON(
                    since: UInt64.max - 1,
                    sequence: UInt64.max,
                    limit: 1
                )
            )
        }
        let client = try makeClient()

        let page = try await client.events(since: UInt64.max - 1, limit: 1)

        XCTAssertEqual(page.since, UInt64.max - 1)
        XCTAssertEqual(page.nextSince, UInt64.max)
        XCTAssertEqual(page.events.first?.sequence, UInt64.max)
        XCTAssertEqual(page.events.first?.generatedAtUnix, UInt64.max)
    }

    func testProviderRouteUsesLiteralAllowedIdentifierAndValidatesProofDepth() async throws {
        let recorder = ReputationRequestRecorder()
        ReputationStubURLProtocol.handler = { [self] request in
            recorder.append(request)
            return try response(
                request,
                body: providerResponseJSON(providerId: "provider:eu-1")
            )
        }
        let client = try makeClient()

        let result = try await client.provider(providerId: "provider:eu-1")

        XCTAssertEqual(result.provider.providerId, "provider:eu-1")
        XCTAssertEqual(result.proof.providerId, "provider:eu-1")
        XCTAssertEqual(
            recorder.snapshot.first?.url?.path,
            "/v1/sorafs/reputation/providers/provider:eu-1"
        )

        for invalidProviderId in ["provider/escaped", ".", ".."] {
            do {
                _ = try await client.provider(providerId: invalidProviderId)
                XCTFail("expected provider-id rejection")
            } catch let error as SorafsReputationClientError {
                guard case .invalidRequest = error else {
                    return XCTFail("unexpected error \(error)")
                }
            }
        }
        XCTAssertEqual(recorder.snapshot.count, 1)

        ReputationStubURLProtocol.handler = { [self] request in
            recorder.append(request)
            return try response(
                request,
                body: providerResponseJSON(providerId: "provider:eu-1")
                    .replacingOccurrences(
                        of: #""leaf_count":1,"siblings_hex":[]"#,
                        with: #""leaf_count":2,"siblings_hex":[]"#
                    )
            )
        }
        do {
            _ = try await client.provider(providerId: "provider:eu-1")
            XCTFail("expected exact proof-depth rejection")
        } catch let error as SorafsReputationClientError {
            guard case .invalidResponse = error else {
                return XCTFail("unexpected error \(error)")
            }
        }
        XCTAssertEqual(recorder.snapshot.count, 2)
    }

    func testStrictJSONRejectsDuplicateKeysBoolIntegersAndTrailingMaterial() async throws {
        let bodies = [
            weightsJSON().replacingOccurrences(
                of: #""alpha_bps":8500"#,
                with: #""alpha_bps":8500,"alpha_bps":8500"#
            ),
            weightsJSON().replacingOccurrences(
                of: #""generated_at_unix":12"#,
                with: #""generated_at_unix":true"#
            ),
            weightsJSON() + #"{"second":true}"#,
            weightsJSON().replacingOccurrences(
                of: #""version":1"#,
                with: #""version":NaN"#
            ),
            "\u{FEFF}" + weightsJSON(),
        ]
        var index = 0
        ReputationStubURLProtocol.handler = { [self] request in
            defer { index += 1 }
            return try response(request, body: bodies[index])
        }
        let client = try makeClient()

        for _ in bodies {
            do {
                _ = try await client.weights()
                XCTFail("expected strict JSON rejection")
            } catch let error as SorafsReputationClientError {
                guard case .invalidResponse = error else {
                    return XCTFail("unexpected error \(error)")
                }
            }
        }
    }

    func testFiniteResponseIsBoundedBeforeDecoding() async throws {
        ReputationStubURLProtocol.handler = { [self] request in
            try response(request, body: weightsJSON())
        }
        let client = try makeClient(maximumResponseBytes: 32)

        do {
            _ = try await client.weights()
            XCTFail("expected bounded response rejection")
        } catch let error as SorafsReputationClientError {
            XCTAssertEqual(error, .responseTooLarge(maximumBytes: 32))
        }
    }

    func testSnapshotRejectsWrongBindingAndNonCanonicalFlags() async throws {
        let badSnapshot = snapshotJSON(limit: 5, generatedAt: 12)
            .replacingOccurrences(
                of: #""flag":"reserve_warning","value":null},{"flag":"low_score""#,
                with: #""flag":"low_score","value":null},{"flag":"reserve_warning""#
            )
        ReputationStubURLProtocol.handler = { [self] request in
            try response(request, body: badSnapshot)
        }
        let client = try makeClient()

        do {
            _ = try await client.snapshot(snapshotIdHex: snapshotA, limit: 5)
            XCTFail("expected canonical flag-order rejection")
        } catch let error as SorafsReputationClientError {
            guard case .invalidResponse = error else {
                return XCTFail("unexpected error \(error)")
            }
        }

        ReputationStubURLProtocol.handler = { [self] request in
            try response(
                request,
                body: snapshotJSON(limit: 5, generatedAt: 12)
                    .replacingOccurrences(of: snapshotA, with: snapshotB)
            )
        }
        do {
            _ = try await client.snapshot(snapshotIdHex: snapshotA, limit: 5)
            XCTFail("expected snapshot binding rejection")
        } catch let error as SorafsReputationClientError {
            guard case .invalidResponse = error else {
                return XCTFail("unexpected error \(error)")
            }
        }
    }

    func testSSEIsTrueOneShotStreamWithNoResumeAlias() async throws {
        let recorder = ReputationRequestRecorder()
        let event = eventJSON(
            sequence: UInt64.max,
            generatedAt: UInt64.max,
            snapshotId: snapshotA,
            previousSnapshotId: nil
        )
        let body = "event: reputation_snapshot\n"
            + "id: \(UInt64.max)\n"
            + "data: \(event)\n\n"
        ReputationStubURLProtocol.handler = { [self] request in
            recorder.append(request)
            return try response(
                request,
                contentType: "text/event-stream",
                body: body
            )
        }
        let client = try makeClient()
        let stream = try client.streamEvents(since: UInt64.max - 1, limit: 1)
        var iterator = stream.makeAsyncIterator()

        let frame = try await iterator.next()
        XCTAssertEqual(
            frame,
            .snapshot(
                id: UInt64.max,
                event: SorafsReputationSnapshotEventV1(
                    version: 1,
                    sequence: UInt64.max,
                    snapshotIdHex: snapshotA,
                    generatedAtUnix: UInt64.max,
                    merkleRootHex: digestC,
                    providerCount: 1,
                    previousSnapshotIdHex: nil
                )
            )
        )
        let terminalFrame = try await iterator.next()
        XCTAssertNil(terminalFrame)

        XCTAssertEqual(recorder.snapshot.count, 1)
        let request = try XCTUnwrap(recorder.snapshot.first)
        XCTAssertNil(request.value(forHTTPHeaderField: "Last-Event-ID"))
        XCTAssertEqual(
            request.url?.query,
            "since=\(UInt64.max - 1)&limit=1"
        )
        try assertCanonicalSignature(request)
    }

    func testSSERejectsMalformedFrameWithoutReconnect() async throws {
        let recorder = ReputationRequestRecorder()
        ReputationStubURLProtocol.handler = { [self] request in
            recorder.append(request)
            return try response(
                request,
                contentType: "text/event-stream",
                body: """
                event: reputation_snapshot
                id: 2
                retry: 100
                data: \(eventJSON(
                    sequence: 2,
                    generatedAt: 2,
                    snapshotId: snapshotA,
                    previousSnapshotId: nil
                ))
                """
            )
        }
        let client = try makeClient()
        let stream = try client.streamEvents(since: 1, limit: 1)

        do {
            for try await _ in stream {}
            XCTFail("expected malformed SSE rejection")
        } catch let error as SorafsReputationClientError {
            guard case .invalidResponse = error else {
                return XCTFail("unexpected error \(error)")
            }
        }
        XCTAssertEqual(recorder.snapshot.count, 1)
    }

    private func makeClient(
        maximumResponseBytes: Int = SorafsReputationClient.defaultMaximumResponseBytes
    ) throws -> SorafsReputationClient {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [ReputationStubURLProtocol.self]
        let signingKey = try Curve25519.Signing.PrivateKey(rawRepresentation: seed)
        let account = try AccountAddress.fromAccount(
            publicKey: signingKey.publicKey.rawRepresentation
        ).toI105(networkPrefix: AccountId.defaultNetworkPrefix)
        return try SorafsReputationClient(
            baseURL: URL(string: "https://reputation.example")!,
            session: URLSession(configuration: configuration),
            networkId: TestNetworkIds.canonical,
            accountId: account,
            privateKey: seed,
            maximumResponseBytes: maximumResponseBytes,
            currentTimeMilliseconds: { 4_102_444_800_000 },
            nonceSeed: { "swift-reputation-test" }
        )
    }

    private func response(
        _ request: URLRequest,
        status: Int = 200,
        contentType: String = "application/json",
        body: String,
        additionalHeaders: [String: String] = [:]
    ) throws -> (HTTPURLResponse, Data?) {
        var headers = additionalHeaders
        headers["Content-Type"] = contentType
        let response = try XCTUnwrap(
            try HTTPURLResponse(
                url: XCTUnwrap(request.url),
                statusCode: status,
                httpVersion: nil,
                headerFields: headers
            )
        )
        return (response, Data(body.utf8))
    }

    private func assertCanonicalSignature(_ request: URLRequest) throws {
        let timestamp = try XCTUnwrap(
            try UInt64(
                XCTUnwrap(
                    request.value(
                        forHTTPHeaderField: ToriiCanonicalRequest.headerTimestampMs
                    )
                )
            )
        )
        let nonce = try XCTUnwrap(
            request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerNonce)
        )
        let signature = try XCTUnwrap(
            try Data(
                base64Encoded: XCTUnwrap(
                    request.value(
                        forHTTPHeaderField: ToriiCanonicalRequest.headerSignature
                    )
                )
            )
        )
        let message = try ToriiCanonicalRequest.signatureMessage(
            networkId: TestNetworkIds.canonical,
            method: "GET",
            url: XCTUnwrap(request.url),
            body: Data(),
            timestampMs: timestamp,
            nonce: nonce
        )
        let publicKey = try Curve25519.Signing.PrivateKey(
            rawRepresentation: seed
        ).publicKey
        XCTAssertTrue(publicKey.isValidSignature(signature, for: message))
    }

    private func weightsJSON() -> String {
        """
        {"snapshot_id_hex":"\(snapshotA)","generated_at_unix":12,"alpha_bps":8500,"current_score_weight_bps":7000,"weights":\(weightsObjectJSON())}
        """
    }

    private func weightsObjectJSON() -> String {
        """
        {"version":1,"por_success_bps":2000,"pdp_success_bps":2000,"potr_success_bps":2000,"latency_bps":1000,"dispute_bps":1000,"token_violation_bps":1000,"repair_breach_bps":1000}
        """
    }

    private func metricsJSON() -> String {
        """
        {"version":1,"por_success_bps":9000,"pdp_success_bps":9000,"potr_success_bps":9000,"latency_health_bps":9000,"dispute_rate_bps":100,"token_violation_rate_bps":100,"repair_breach_rate_bps":100}
        """
    }

    private func providerJSON(providerId: String = "provider-1") -> String {
        """
        {"provider_id":"\(providerId)","score_bps":8000,"degradation_flags":[{"flag":"reserve_warning","value":null},{"flag":"low_score","value":null}],"raw_metrics":\(metricsJSON()),"raw_metrics_hash_hex":"\(digestD)"}
        """
    }

    private func snapshotJSON(limit: UInt64, generatedAt: UInt64) -> String {
        """
        {"snapshot_id_hex":"\(snapshotA)","generated_at_unix":\(generatedAt),"previous_snapshot_id_hex":null,"merkle_root_hex":"\(digestC)","provider_count":1,"returned_provider_count":1,"limit":\(limit),"truncated_providers":false,"alpha_bps":8500,"current_score_weight_bps":7000,"weights":\(weightsObjectJSON()),"providers":[\(providerJSON())]}
        """
    }

    private func providerResponseJSON(providerId: String) -> String {
        """
        {"snapshot_id_hex":"\(snapshotA)","generated_at_unix":12,"merkle_root_hex":"\(digestC)","provider":\(providerJSON(providerId: providerId)),"proof":{"provider_id":"\(providerId)","leaf_index":0,"leaf_count":1,"siblings_hex":[]}}
        """
    }

    private func eventJSON(
        sequence: UInt64,
        generatedAt: UInt64,
        snapshotId: String,
        previousSnapshotId: String?
    ) -> String {
        let previous = previousSnapshotId.map { "\"\($0)\"" } ?? "null"
        return """
        {"version":1,"sequence":\(sequence),"snapshot_id_hex":"\(snapshotId)","generated_at_unix":\(generatedAt),"merkle_root_hex":"\(digestC)","provider_count":1,"previous_snapshot_id_hex":\(previous)}
        """
    }

    private func eventsJSON(
        since: UInt64,
        sequence: UInt64,
        limit: UInt64
    ) -> String {
        """
        {"since":\(since),"limit":\(limit),"count":1,"next_since":\(sequence),"events":[\(eventJSON(sequence: sequence, generatedAt: sequence, snapshotId: snapshotA, previousSnapshotId: nil))]}
        """
    }
}
