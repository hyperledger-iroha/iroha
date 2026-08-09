import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

private final class BootleLanternIssuanceStubURLProtocolV1: URLProtocol {
    struct Stub {
        let status: Int
        let headers: [String: String]
        let body: Data
        let responseURL: URL?
    }

    static let lock = NSLock()
    static var handler: ((URLRequest) throws -> Stub)?
    static var requests: [URLRequest] = []

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        Self.lock.lock()
        let handler = Self.handler
        Self.requests.append(request)
        Self.lock.unlock()
        do {
            guard let handler else { throw URLError(.badServerResponse) }
            let stub = try handler(request)
            let response = try XCTUnwrap(
                HTTPURLResponse(
                    url: stub.responseURL ?? XCTUnwrap(request.url),
                    statusCode: stub.status,
                    httpVersion: "HTTP/1.1",
                    headerFields: stub.headers
                )
            )
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            if !stub.body.isEmpty {
                client?.urlProtocol(self, didLoad: stub.body)
            }
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}

    static func reset() {
        lock.lock()
        handler = nil
        requests = []
        lock.unlock()
    }

    static var requestCount: Int {
        lock.lock()
        defer { lock.unlock() }
        return requests.count
    }

    static var lastRequest: URLRequest? {
        lock.lock()
        defer { lock.unlock() }
        return requests.last
    }
}

final class BootleLanternIssuanceClientV1Tests: XCTestCase {
    override func tearDown() {
        BootleLanternIssuanceStubURLProtocolV1.reset()
        super.tearDown()
    }

    func testSharedClientContractFixtureBindsExactWireBytes() async throws {
        let fixture = try contractObject(
            JSONSerialization.jsonObject(with: Data(contentsOf: try clientContractFixtureURL()))
        )
        XCTAssertEqual(
            fixture["schema"] as? String,
            "iroha.bootle_lantern.issuance_client_contract"
        )
        XCTAssertEqual((fixture["version"] as? NSNumber)?.intValue, 1)
        XCTAssertEqual(fixture["classification"] as? String, "public-synthetic-test-data")

        let transport = try contractObject(fixture["transport"])
        XCTAssertEqual(transport["method"] as? String, "POST")
        XCTAssertEqual(
            transport["authorize_path"] as? String,
            BootleLanternIssuanceClientV1.authorizePath
        )
        XCTAssertEqual(
            transport["issue_path"] as? String,
            BootleLanternIssuanceClientV1.issuePath
        )
        XCTAssertEqual(
            transport["norito_media_type"] as? String,
            BootleLanternIssuanceClientV1.mediaType
        )
        XCTAssertEqual(
            transport["unauthorized_www_authenticate"] as? String,
            "Bearer realm=\"iroha-bootle-lantern-issuance\""
        )

        let credentialContract = try contractObject(fixture["credential"])
        XCTAssertEqual(
            credentialContract["encoding"] as? String,
            "base64url-unpadded-canonical"
        )
        XCTAssertEqual(
            (credentialContract["minimum_decoded_bytes"] as? NSNumber)?.intValue,
            1
        )
        XCTAssertEqual(
            (credentialContract["maximum_decoded_bytes"] as? NSNumber)?.intValue,
            BootleLanternIssuanceCredentialV1.maximumBytes
        )
        let examples = try contractArray(credentialContract["examples"])
        XCTAssertEqual(examples.count, 3)
        for value in examples {
            let example = try contractObject(value)
            let decoded = try XCTUnwrap(
                Data(hexString: try XCTUnwrap(example["decoded_hex"] as? String))
            )
            let encoded = try XCTUnwrap(example["encoded"] as? String)
            XCTAssertEqual(canonicalBase64URL(decoded), encoded)
            let admitted = try BootleLanternIssuanceCredentialV1(
                canonicalBase64URL: encoded
            )
            setSuccess(body: patterned(BootleLanternIssuanceClientV1.authorizationBytes))
            _ = try await client().authorize(credential: admitted)
            XCTAssertEqual(
                BootleLanternIssuanceStubURLProtocolV1.lastRequest?
                    .value(forHTTPHeaderField: "Authorization"),
                "Bearer \(encoded)"
            )
            admitted.destroy()
            BootleLanternIssuanceStubURLProtocolV1.reset()
        }

        let bodies = try contractObject(fixture["bodies"])
        XCTAssertEqual(
            bodies["pattern"] as? String,
            "byte-at-index-equals-index-modulo-256-with-canonical-wire-magics"
        )
        for (name, wire, length) in [
            (
                "authorization_response",
                "ILA1",
                BootleLanternIssuanceClientV1.authorizationBytes
            ),
            ("issue_request", "ILA1+ILQ1", BootleLanternIssuanceClientV1.issueRequestBytes),
            ("issue_response", "ILR1", BootleLanternIssuanceClientV1.issueResponseBytes),
        ] {
            let body = try contractObject(bodies[name])
            XCTAssertEqual(body["wire"] as? String, wire)
            XCTAssertEqual((body["length_bytes"] as? NSNumber)?.intValue, length)
            XCTAssertEqual(
                SHA256.hash(data: patterned(length))
                    .map { String(format: "%02x", $0) }
                    .joined(),
                body["pattern_sha256_hex"] as? String
            )
        }
        XCTAssertEqual(String(data: patterned(320).prefix(4), encoding: .utf8), "ILA1")
        XCTAssertEqual(String(data: patterned(71_896).prefix(4), encoding: .utf8), "ILA1")
        XCTAssertEqual(
            String(data: patterned(71_896).subdata(in: 320..<324), encoding: .utf8),
            "ILQ1"
        )
        XCTAssertEqual(String(data: patterned(3_176).prefix(4), encoding: .utf8), "ILR1")
        let issueRequest = try contractObject(bodies["issue_request"])
        let componentLengths = try contractArray(issueRequest["component_lengths_bytes"])
            .compactMap { ($0 as? NSNumber)?.intValue }
        XCTAssertEqual(componentLengths, [320, 71_576])
        XCTAssertEqual(
            componentLengths.reduce(0, +),
            BootleLanternIssuanceClientV1.issueRequestBytes
        )

        let errors = try contractObject(fixture["errors"])
        XCTAssertEqual(
            (errors["maximum_body_bytes"] as? NSNumber)?.intValue,
            BootleLanternIssuanceClientV1.errorResponseMaximumBytes
        )
        let envelope = try contractObject(errors["norito_envelope"])
        XCTAssertEqual(
            envelope["schema_type_name"] as? String,
            "iroha_torii_shared::ErrorEnvelope"
        )
        XCTAssertEqual(
            envelope["schema_hash_hex"] as? String,
            "793f11768076bfe270a17aeb86752cd9"
        )
        XCTAssertEqual(envelope["flags_hex"] as? String, "02")
        let errorResponses = try contractArray(errors["responses"])
        XCTAssertEqual(errorResponses.count, 8)
        for value in errorResponses {
            let contract = try contractObject(value)
            XCTAssertEqual(
                contract["www_authenticate"] as? String,
                ((contract["status"] as? NSNumber)?.intValue == 401)
                    ? transport["unauthorized_www_authenticate"] as? String
                    : nil
            )
            try setError(contract)
            do {
                _ = try await client().authorize(credential: try credential())
                XCTFail("fixture error response must fail")
            } catch let error as BootleLanternIssuanceClientErrorV1 {
                XCTAssertEqual(
                    error,
                    .httpError(
                        status: try XCTUnwrap((contract["status"] as? NSNumber)?.intValue),
                        code: try XCTUnwrap(contract["code"] as? String),
                        retryAfterSeconds: (contract["retry_after_seconds"] as? NSNumber)?.intValue
                    )
                )
            }
            BootleLanternIssuanceStubURLProtocolV1.reset()
        }
    }

    func testAuthorizeUsesCanonicalEmptySingleAttemptRequest() async throws {
        let expected = patterned(BootleLanternIssuanceClientV1.authorizationBytes)
        setSuccess(body: expected)

        let result = try await client().authorize(
            credential: try BootleLanternIssuanceCredentialV1(opaqueBytes: Data([0x61]))
        )

        XCTAssertEqual(result, expected)
        XCTAssertEqual(BootleLanternIssuanceStubURLProtocolV1.requestCount, 1)
        let request = try XCTUnwrap(BootleLanternIssuanceStubURLProtocolV1.lastRequest)
        XCTAssertEqual(request.httpMethod, "POST")
        XCTAssertEqual(request.url?.path, BootleLanternIssuanceClientV1.authorizePath)
        XCTAssertEqual(try requestBody(request), Data())
        XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer YQ")
        XCTAssertEqual(
            request.value(forHTTPHeaderField: "Content-Type"),
            BootleLanternIssuanceClientV1.mediaType
        )
        XCTAssertEqual(
            request.value(forHTTPHeaderField: "Accept"),
            BootleLanternIssuanceClientV1.mediaType
        )
        XCTAssertEqual(request.value(forHTTPHeaderField: "Accept-Encoding"), "identity")
        XCTAssertEqual(request.value(forHTTPHeaderField: "Cache-Control"), "no-store")
        XCTAssertEqual(request.value(forHTTPHeaderField: "Pragma"), "no-cache")
        XCTAssertNil(request.value(forHTTPHeaderField: "Content-Encoding"))
        XCTAssertFalse(request.httpShouldHandleCookies)
    }

    func testIssueUsesExactDefensiveRequestAndResponse() async throws {
        let requestBytes = patterned(BootleLanternIssuanceClientV1.issueRequestBytes)
        let responseBytes = patterned(BootleLanternIssuanceClientV1.issueResponseBytes)
        setSuccess(body: responseBytes)

        let result = try await client().issue(
            credential: try BootleLanternIssuanceCredentialV1(
                canonicalBase64URL: "AQID"
            ),
            canonicalRequest: requestBytes
        )

        XCTAssertEqual(result, responseBytes)
        XCTAssertEqual(BootleLanternIssuanceStubURLProtocolV1.requestCount, 1)
        let request = try XCTUnwrap(BootleLanternIssuanceStubURLProtocolV1.lastRequest)
        XCTAssertEqual(request.url?.path, BootleLanternIssuanceClientV1.issuePath)
        XCTAssertEqual(try requestBody(request), requestBytes)
        XCTAssertEqual(request.value(forHTTPHeaderField: "Authorization"), "Bearer AQID")
    }

    func testIssueRejectsEmptyTruncatedExtendedAndOversizedBodiesBeforeTransport() async throws {
        setSuccess(body: patterned(BootleLanternIssuanceClientV1.issueResponseBytes))
        for size in [
            0,
            1,
            BootleLanternIssuanceClientV1.issueRequestBytes - 1,
            BootleLanternIssuanceClientV1.issueRequestBytes + 1,
            BootleLanternIssuanceClientV1.issueRequestBytes * 2,
        ] {
            do {
                _ = try await client().issue(
                    credential: try credential(),
                    canonicalRequest: Data(repeating: 0, count: size)
                )
                XCTFail("size \(size) must fail")
            } catch let error as BootleLanternIssuanceClientErrorV1 {
                XCTAssertEqual(
                    error,
                    .invalidIssueRequestLength(
                        expected: BootleLanternIssuanceClientV1.issueRequestBytes,
                        actual: size
                    )
                )
            }
        }
        XCTAssertEqual(BootleLanternIssuanceStubURLProtocolV1.requestCount, 0)
    }

    func testIssueRejectsSameLengthWrongTruncatedShiftedAndSubstitutedILA1Magic() async throws {
        setSuccess(body: patterned(BootleLanternIssuanceClientV1.issueResponseBytes))
        for prefix in [Data([0, 0, 0, 0]), Data("ILA0".utf8), Data([0x49, 0x4C, 0x41, 0]), Data("XLA1".utf8)] {
            var request = patterned(BootleLanternIssuanceClientV1.issueRequestBytes)
            request.replaceSubrange(0..<4, with: prefix)
            do {
                _ = try await client().issue(
                    credential: try credential(),
                    canonicalRequest: request
                )
                XCTFail("noncanonical ILA1 magic must fail")
            } catch let error as BootleLanternIssuanceClientErrorV1 {
                XCTAssertEqual(error, .invalidIssueRequestMagic)
            }
        }
        for prefix in [Data([0, 0, 0, 0]), Data("ILQ0".utf8), Data([0x49, 0x4C, 0x51, 0]), Data("XLQ1".utf8)] {
            var request = patterned(BootleLanternIssuanceClientV1.issueRequestBytes)
            request.replaceSubrange(320..<324, with: prefix)
            do {
                _ = try await client().issue(
                    credential: try credential(),
                    canonicalRequest: request
                )
                XCTFail("noncanonical ILQ1 magic must fail")
            } catch let error as BootleLanternIssuanceClientErrorV1 {
                XCTAssertEqual(error, .invalidIssueRequestMagic)
            }
        }
        XCTAssertEqual(BootleLanternIssuanceStubURLProtocolV1.requestCount, 0)
    }

    func testCredentialAdmissionIsCanonicalBoundedDefensiveDestroyableAndRedacted() async throws {
        XCTAssertThrowsError(try BootleLanternIssuanceCredentialV1(opaqueBytes: Data()))
        XCTAssertThrowsError(
            try BootleLanternIssuanceCredentialV1(
                opaqueBytes: Data(
                    repeating: 0,
                    count: BootleLanternIssuanceCredentialV1.maximumBytes + 1
                )
            )
        )
        let malformed = [
            "",
            "A",
            "YQ==",
            "YR",
            "Y Q",
            "YQ\n",
            "Bearer YQ",
            "+w",
            String(
                repeating: "A",
                count: ((BootleLanternIssuanceCredentialV1.maximumBytes + 2) / 3) * 4 + 1
            ),
            canonicalBase64URL(
                Data(
                    repeating: 0,
                    count: BootleLanternIssuanceCredentialV1.maximumBytes + 1
                )
            ),
        ]
        for encoded in malformed {
            XCTAssertThrowsError(
                try BootleLanternIssuanceCredentialV1(canonicalBase64URL: encoded),
                "credential \(encoded.prefix(16)) must fail"
            )
        }

        var source = Data([0x61])
        let secret = try BootleLanternIssuanceCredentialV1(opaqueBytes: source)
        source[0] = 0x62
        XCTAssertEqual(secret.description, "BootleLanternIssuanceCredentialV1([REDACTED])")
        XCTAssertEqual(secret.debugDescription, "BootleLanternIssuanceCredentialV1([REDACTED])")
        XCTAssertTrue(Mirror(reflecting: secret).children.isEmpty)
        setSuccess(body: patterned(BootleLanternIssuanceClientV1.authorizationBytes))
        _ = try await client().authorize(credential: secret)
        XCTAssertEqual(
            BootleLanternIssuanceStubURLProtocolV1.lastRequest?
                .value(forHTTPHeaderField: "Authorization"),
            "Bearer YQ"
        )
        secret.destroy()
        secret.destroy()
        do {
            _ = try await client().authorize(credential: secret)
            XCTFail("destroyed credential must fail")
        } catch let error as BootleLanternIssuanceClientErrorV1 {
            XCTAssertEqual(error, .credentialDestroyed)
        }
        XCTAssertEqual(BootleLanternIssuanceStubURLProtocolV1.requestCount, 1)

        let maximum = Data(
            repeating: 0xFF,
            count: BootleLanternIssuanceCredentialV1.maximumBytes
        )
        let maximumEncoded = canonicalBase64URL(maximum)
        let maximumCredential = try BootleLanternIssuanceCredentialV1(
            canonicalBase64URL: maximumEncoded
        )
        _ = try await client().authorize(credential: maximumCredential)
        XCTAssertEqual(
            BootleLanternIssuanceStubURLProtocolV1.lastRequest?
                .value(forHTTPHeaderField: "Authorization"),
            "Bearer \(maximumEncoded)"
        )
        maximumCredential.destroy()
    }

    func testAuthorizationAndIssueResponsesRequireExactLengthsAndBoundOversize() async throws {
        for size in [
            0,
            BootleLanternIssuanceClientV1.authorizationBytes - 1,
            BootleLanternIssuanceClientV1.authorizationBytes + 1,
            BootleLanternIssuanceClientV1.authorizationBytes * 32,
        ] {
            setSuccess(body: patterned(size))
            await assertClientFailure {
                try await self.client().authorize(credential: try self.credential())
            }
            XCTAssertEqual(BootleLanternIssuanceStubURLProtocolV1.requestCount, 1)
            BootleLanternIssuanceStubURLProtocolV1.reset()
        }

        for size in [
            0,
            BootleLanternIssuanceClientV1.issueResponseBytes - 1,
            BootleLanternIssuanceClientV1.issueResponseBytes + 1,
        ] {
            setSuccess(body: patterned(size))
            await assertClientFailure {
                try await self.client().issue(
                    credential: try self.credential(),
                    canonicalRequest: self.patterned(
                        BootleLanternIssuanceClientV1.issueRequestBytes
                    )
                )
            }
            XCTAssertEqual(BootleLanternIssuanceStubURLProtocolV1.requestCount, 1)
            BootleLanternIssuanceStubURLProtocolV1.reset()
        }
    }

    func testSuccessfulResponsesRequireExactILA1AndILR1Magic() async throws {
        for prefix in [Data([0, 0, 0, 0]), Data("ILA0".utf8), Data([0x49, 0x4C, 0x41, 0]), Data("XLA1".utf8)] {
            var body = patterned(BootleLanternIssuanceClientV1.authorizationBytes)
            body.replaceSubrange(0..<4, with: prefix)
            setSuccess(body: body)
            await assertClientFailure {
                try await self.client().authorize(credential: try self.credential())
            }
            BootleLanternIssuanceStubURLProtocolV1.reset()
        }
        for prefix in [Data([0, 0, 0, 0]), Data("ILR0".utf8), Data([0x49, 0x4C, 0x52, 0]), Data("XLR1".utf8)] {
            var body = patterned(BootleLanternIssuanceClientV1.issueResponseBytes)
            body.replaceSubrange(0..<4, with: prefix)
            setSuccess(body: body)
            await assertClientFailure {
                try await self.client().issue(
                    credential: try self.credential(),
                    canonicalRequest: self.patterned(BootleLanternIssuanceClientV1.issueRequestBytes)
                )
            }
            BootleLanternIssuanceStubURLProtocolV1.reset()
        }
    }

    func testResponsesRequireExact200AndRejectRedirectEvidence() async throws {
        for status in [201, 204, 301, 307, 308, 418, 500] {
            setSuccess(
                body: patterned(BootleLanternIssuanceClientV1.authorizationBytes),
                status: status,
                headers: [
                    "Content-Type": BootleLanternIssuanceClientV1.mediaType,
                    "Location": "https://attacker.example/result",
                ]
            )
            await assertClientFailure {
                try await self.client().authorize(credential: try self.credential())
            }
            XCTAssertEqual(BootleLanternIssuanceStubURLProtocolV1.requestCount, 1)
            BootleLanternIssuanceStubURLProtocolV1.reset()
        }

        setSuccess(
            body: patterned(BootleLanternIssuanceClientV1.authorizationBytes),
            responseURL: URL(string: "https://attacker.example/result")
        )
        await assertClientFailure {
            try await self.client().authorize(credential: try self.credential())
        }
        XCTAssertEqual(BootleLanternIssuanceStubURLProtocolV1.requestCount, 1)
    }

    func testStructuredErrorsBindStatusMediaCodeAndRetryHint() async throws {
        for value in try errorContracts() {
            let contract = try contractObject(value)
            try setError(contract)
            do {
                _ = try await client().authorize(credential: try credential())
                XCTFail("structured error must fail")
            } catch let error as BootleLanternIssuanceClientErrorV1 {
                XCTAssertEqual(
                    error,
                    .httpError(
                        status: try XCTUnwrap((contract["status"] as? NSNumber)?.intValue),
                        code: try XCTUnwrap(contract["code"] as? String),
                        retryAfterSeconds: (contract["retry_after_seconds"] as? NSNumber)?.intValue
                    )
                )
            }
            XCTAssertEqual(BootleLanternIssuanceStubURLProtocolV1.requestCount, 1)
            BootleLanternIssuanceStubURLProtocolV1.reset()
        }
    }

    func testAllSevenNoritoErrorsRejectLegacyMalformedTruncatedAndTrailingFrames() async throws {
        let contracts = try errorContracts().map { try contractObject($0) }.filter {
            ($0["media_type"] as? String) == BootleLanternIssuanceClientV1.mediaType
        }
        XCTAssertEqual(contracts.count, 7)
        for contract in contracts {
            let canonical = try errorFixtureBody(contract)
            var trailing = canonical
            trailing.append(0)
            let variants = [
                try rejectedLegacyNoritoErrorFrame(
                    template: canonical,
                    code: XCTUnwrap(contract["code"] as? String)
                ),
                try malformedNoritoFieldFrame(canonical),
                Data(canonical.dropLast()),
                trailing,
            ]
            for body in variants {
                try setError(contract, body: body)
                do {
                    _ = try await client().authorize(credential: try credential())
                    XCTFail("non-canonical Norito error envelope must fail")
                } catch let error as BootleLanternIssuanceClientErrorV1 {
                    guard case .invalidResponse = error else {
                        XCTFail("non-canonical envelope exposed structured status: \(error)")
                        BootleLanternIssuanceStubURLProtocolV1.reset()
                        continue
                    }
                }
                XCTAssertEqual(BootleLanternIssuanceStubURLProtocolV1.requestCount, 1)
                BootleLanternIssuanceStubURLProtocolV1.reset()
            }
        }
    }

    func testStructuredErrorsRejectMalformedSubstitutedAndOversizedEnvelopes() async throws {
        let contracts = try Dictionary(
            uniqueKeysWithValues: errorContracts().map { value -> (Int, [String: Any]) in
                let contract = try contractObject(value)
                return (try XCTUnwrap((contract["status"] as? NSNumber)?.intValue), contract)
            }
        )
        let badRequest = try XCTUnwrap(contracts[400])
        let unauthorized = try XCTUnwrap(contracts[401])
        let notAcceptable = try XCTUnwrap(contracts[406])
        let capacity = try XCTUnwrap(contracts[429])
        let unavailable = try XCTUnwrap(contracts[503])

        var corrupted = try errorFixtureBody(badRequest)
        corrupted[corrupted.startIndex] ^= 1
        let cases: [(contract: [String: Any], body: Data?, headers: [String: String]?)] = [
            (
                badRequest,
                corrupted,
                [
                    "Content-Type": BootleLanternIssuanceClientV1.mediaType,
                    "Content-Length": String(corrupted.count),
                ]
            ),
            (
                badRequest,
                nil,
                [
                    "Content-Type": "application/json",
                    "Content-Length": String(try errorFixtureBody(badRequest).count),
                ]
            ),
            (
                badRequest,
                nil,
                [
                    "Content-Type": BootleLanternIssuanceClientV1.mediaType,
                    "Content-Encoding": "identity",
                ]
            ),
            (
                badRequest,
                nil,
                [
                    "Content-Type": BootleLanternIssuanceClientV1.mediaType,
                    "Content-Length": "0107",
                ]
            ),
            (badRequest, try errorFixtureBody(unauthorized), nil),
            (
                notAcceptable,
                Data("\(try XCTUnwrap(notAcceptable["body_utf8"] as? String)) ".utf8),
                ["Content-Type": "application/json"]
            ),
            (
                capacity,
                nil,
                [
                    "Content-Type": BootleLanternIssuanceClientV1.mediaType,
                    "Retry-After": "2",
                ]
            ),
            (
                unavailable,
                nil,
                [
                    "Content-Type": BootleLanternIssuanceClientV1.mediaType,
                    "Retry-After": "1",
                ]
            ),
            (
                unauthorized,
                nil,
                [
                    "Content-Type": BootleLanternIssuanceClientV1.mediaType,
                    "Content-Length": String(try errorFixtureBody(unauthorized).count),
                ]
            ),
            (
                unauthorized,
                nil,
                [
                    "Content-Type": BootleLanternIssuanceClientV1.mediaType,
                    "Content-Length": String(try errorFixtureBody(unauthorized).count),
                    "WWW-Authenticate": "\(try XCTUnwrap(unauthorized["www_authenticate"] as? String)), \(try XCTUnwrap(unauthorized["www_authenticate"] as? String))",
                ]
            ),
            (
                unauthorized,
                nil,
                [
                    "Content-Type": BootleLanternIssuanceClientV1.mediaType,
                    "Content-Length": String(try errorFixtureBody(unauthorized).count),
                    "WWW-Authenticate": "Bearer realm=\"attacker\"",
                ]
            ),
            (
                badRequest,
                nil,
                [
                    "Content-Type": BootleLanternIssuanceClientV1.mediaType,
                    "Content-Length": String(try errorFixtureBody(badRequest).count),
                    "WWW-Authenticate": try XCTUnwrap(unauthorized["www_authenticate"] as? String),
                ]
            ),
            (
                badRequest,
                Data(repeating: 0, count: BootleLanternIssuanceClientV1.errorResponseMaximumBytes + 1),
                ["Content-Type": BootleLanternIssuanceClientV1.mediaType]
            ),
        ]
        for candidate in cases {
            try setError(candidate.contract, body: candidate.body, headers: candidate.headers)
            await assertClientFailure {
                try await self.client().authorize(credential: try self.credential())
            }
            BootleLanternIssuanceStubURLProtocolV1.reset()
        }
    }

    func testResponsesRejectMissingDuplicateParameterizedOrWrongContentType() async throws {
        for contentType in [
            nil,
            "Application/X-Norito",
            "application/octet-stream",
            "application/x-norito; charset=binary",
            "application/x-norito, application/x-norito",
        ] {
            let headers = contentType.map { ["Content-Type": $0] } ?? [:]
            setSuccess(
                body: patterned(BootleLanternIssuanceClientV1.authorizationBytes),
                headers: headers
            )
            await assertClientFailure {
                try await self.client().authorize(credential: try self.credential())
            }
            XCTAssertEqual(BootleLanternIssuanceStubURLProtocolV1.requestCount, 1)
            BootleLanternIssuanceStubURLProtocolV1.reset()
        }
    }

    func testResponsesRejectCompressionAndNoncanonicalOrConflictingLength() async throws {
        for encoding in ["gzip", "identity", "br", "gzip, br"] {
            setSuccess(
                body: patterned(BootleLanternIssuanceClientV1.authorizationBytes),
                headers: [
                    "Content-Type": BootleLanternIssuanceClientV1.mediaType,
                    "Content-Encoding": encoding,
                ]
            )
            await assertClientFailure {
                try await self.client().authorize(credential: try self.credential())
            }
            BootleLanternIssuanceStubURLProtocolV1.reset()
        }

        for length in ["0", "319", "321", "0320", "+320", "320 ", "320, 320"] {
            setSuccess(
                body: patterned(BootleLanternIssuanceClientV1.authorizationBytes),
                headers: [
                    "Content-Type": BootleLanternIssuanceClientV1.mediaType,
                    "Content-Length": length,
                ]
            )
            await assertClientFailure {
                try await self.client().authorize(credential: try self.credential())
            }
            XCTAssertEqual(BootleLanternIssuanceStubURLProtocolV1.requestCount, 1)
            BootleLanternIssuanceStubURLProtocolV1.reset()
        }

        setSuccess(
            body: patterned(BootleLanternIssuanceClientV1.authorizationBytes),
            headers: [
                "Content-Type": BootleLanternIssuanceClientV1.mediaType,
                "Content-Length": String(BootleLanternIssuanceClientV1.authorizationBytes),
            ]
        )
        _ = try await client().authorize(credential: try credential())

        BootleLanternIssuanceStubURLProtocolV1.reset()
        setSuccess(
            body: patterned(BootleLanternIssuanceClientV1.authorizationBytes),
            headers: [
                "Content-Type": BootleLanternIssuanceClientV1.mediaType,
                "WWW-Authenticate": "Bearer realm=\"iroha-bootle-lantern-issuance\"",
            ]
        )
        await assertClientFailure {
            try await self.client().authorize(credential: try self.credential())
        }
    }

    func testAsynchronousTransportFailureIsSanitizedAndNeverRetried() async throws {
        let leaked = "opaque-secret-must-not-appear"
        BootleLanternIssuanceStubURLProtocolV1.handler = { _ in
            throw NSError(domain: leaked, code: 1)
        }
        do {
            _ = try await client().authorize(credential: try credential())
            XCTFail("transport failure must fail")
        } catch let error as BootleLanternIssuanceClientErrorV1 {
            XCTAssertEqual(error, .transportFailure)
            XCTAssertFalse(error.localizedDescription.contains(leaked))
        }
        XCTAssertEqual(BootleLanternIssuanceStubURLProtocolV1.requestCount, 1)
    }

    func testBaseURLAdmissionIsOriginOnlyHTTPS() throws {
        for value in [
            "http://torii.example",
            "https://user:secret@torii.example",
            "https://torii.example/v1",
            "https://torii.example/?query=1",
            "https://torii.example/#fragment",
        ] {
            XCTAssertThrowsError(
                try BootleLanternIssuanceClientV1(
                    baseURL: XCTUnwrap(URL(string: value)),
                    session: stubSession()
                )
            )
        }
        XCTAssertNoThrow(
            try BootleLanternIssuanceClientV1(
                baseURL: XCTUnwrap(URL(string: "https://torii.example/")),
                session: stubSession()
            )
        )
    }

    private func client() throws -> BootleLanternIssuanceClientV1 {
        try BootleLanternIssuanceClientV1(
            baseURL: XCTUnwrap(URL(string: "https://torii.example")),
            session: stubSession()
        )
    }

    private func stubSession() -> URLSession {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [BootleLanternIssuanceStubURLProtocolV1.self]
        return URLSession(configuration: configuration)
    }

    private func credential() throws -> BootleLanternIssuanceCredentialV1 {
        try BootleLanternIssuanceCredentialV1(opaqueBytes: Data([1, 2, 3]))
    }

    private func setSuccess(
        body: Data,
        status: Int = 200,
        headers: [String: String] = [
            "Content-Type": BootleLanternIssuanceClientV1.mediaType
        ],
        responseURL: URL? = nil
    ) {
        BootleLanternIssuanceStubURLProtocolV1.handler = { _ in
            .init(status: status, headers: headers, body: body, responseURL: responseURL)
        }
    }

    private func setError(
        _ contract: [String: Any],
        body: Data? = nil,
        headers: [String: String]? = nil
    ) throws {
        let canonicalBody: Data
        if let body {
            canonicalBody = body
        } else {
            canonicalBody = try errorFixtureBody(contract)
        }
        var canonicalHeaders = [
            "Content-Type": try XCTUnwrap(contract["media_type"] as? String),
            "Content-Length": String(canonicalBody.count),
        ]
        if let retry = (contract["retry_after_seconds"] as? NSNumber)?.intValue {
            canonicalHeaders["Retry-After"] = String(retry)
        }
        if let challenge = contract["www_authenticate"] as? String {
            canonicalHeaders["WWW-Authenticate"] = challenge
        }
        setSuccess(
            body: canonicalBody,
            status: try XCTUnwrap((contract["status"] as? NSNumber)?.intValue),
            headers: headers ?? canonicalHeaders
        )
    }

    private func assertClientFailure(
        _ operation: () async throws -> Data,
        file: StaticString = #filePath,
        line: UInt = #line
    ) async {
        do {
            _ = try await operation()
            XCTFail("operation must fail", file: file, line: line)
        } catch {
            XCTAssertTrue(
                error is BootleLanternIssuanceClientErrorV1,
                "unexpected error: \(error)",
                file: file,
                line: line
            )
        }
    }

    private func patterned(_ length: Int) -> Data {
        var body = Data((0..<length).map { UInt8($0 & 0xFF) })
        if length == BootleLanternIssuanceClientV1.authorizationBytes {
            body.replaceSubrange(0..<4, with: Data("ILA1".utf8))
        } else if length == BootleLanternIssuanceClientV1.issueRequestBytes {
            body.replaceSubrange(0..<4, with: Data("ILA1".utf8))
            body.replaceSubrange(320..<324, with: Data("ILQ1".utf8))
        } else if length == BootleLanternIssuanceClientV1.issueResponseBytes {
            body.replaceSubrange(0..<4, with: Data("ILR1".utf8))
        }
        return body
    }

    private func canonicalBase64URL(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    private func clientContractFixtureURL() throws -> URL {
        var current = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        for _ in 0..<8 {
            let candidate = current.appendingPathComponent(
                "fixtures/privacy/bootle_lantern_issuance_client_v1.json"
            )
            if FileManager.default.fileExists(atPath: candidate.path) { return candidate }
            current.deleteLastPathComponent()
        }
        throw BootleLanternIssuanceClientErrorV1.invalidResponse(
            "shared Bootle/Lantern issuance client fixture was not found"
        )
    }

    private func errorContracts() throws -> [Any] {
        let fixture = try contractObject(
            JSONSerialization.jsonObject(with: Data(contentsOf: try clientContractFixtureURL()))
        )
        let errors = try contractObject(fixture["errors"])
        return try contractArray(errors["responses"])
    }

    private func errorFixtureBody(_ contract: [String: Any]) throws -> Data {
        if let bodyHex = contract["body_hex"] as? String {
            return try XCTUnwrap(Data(hexString: bodyHex))
        }
        return Data(try XCTUnwrap(contract["body_utf8"] as? String).utf8)
    }

    private func malformedNoritoFieldFrame(_ body: Data) throws -> Data {
        var malformed = body
        XCTAssertEqual(String(data: malformed.prefix(4), encoding: .utf8), "NRT0")
        let payloadLength = try littleEndianUInt64(malformed, at: 23)
        XCTAssertEqual(UInt64(malformed.count), 40 + payloadLength)
        XCTAssertLessThan(malformed[40], 0x7F)
        malformed[40] += 1
        replaceLittleEndianUInt64(
            crc64ECMA(Data(malformed.dropFirst(40))),
            in: &malformed,
            at: 31
        )
        return malformed
    }

    private func rejectedLegacyNoritoErrorFrame(
        template: Data,
        code: String
    ) throws -> Data {
        let encoded = Data(code.utf8)
        guard encoded.count < 0x80 else {
            throw BootleLanternIssuanceClientErrorV1.invalidResponse(
                "test error code does not fit one-byte compact length"
            )
        }
        var payload = Data([UInt8(encoded.count)])
        payload.append(encoded)
        payload.append(UInt8(encoded.count))
        payload.append(encoded)
        payload.append(0)
        return noritoFrameWithPayload(template: template, payload: payload)
    }

    private func noritoFrameWithPayload(template: Data, payload: Data) -> Data {
        var frame = Data(template.prefix(40))
        replaceLittleEndianUInt64(UInt64(payload.count), in: &frame, at: 23)
        replaceLittleEndianUInt64(crc64ECMA(payload), in: &frame, at: 31)
        frame.append(payload)
        return frame
    }

    private func littleEndianUInt64(_ data: Data, at offset: Int) throws -> UInt64 {
        guard offset >= 0, offset <= data.count - 8 else {
            throw BootleLanternIssuanceClientErrorV1.invalidResponse(
                "test frame is truncated"
            )
        }
        return (0..<8).reduce(UInt64(0)) { value, index in
            value | (UInt64(data[offset + index]) << UInt64(index * 8))
        }
    }

    private func replaceLittleEndianUInt64(
        _ value: UInt64,
        in data: inout Data,
        at offset: Int
    ) {
        var littleEndian = value.littleEndian
        withUnsafeBytes(of: &littleEndian) { bytes in
            data.replaceSubrange(offset..<(offset + 8), with: bytes)
        }
    }

    private func contractObject(_ value: Any?) throws -> [String: Any] {
        try XCTUnwrap(value as? [String: Any])
    }

    private func contractArray(_ value: Any?) throws -> [Any] {
        try XCTUnwrap(value as? [Any])
    }

    private func requestBody(_ request: URLRequest) throws -> Data {
        if let body = request.httpBody { return body }
        guard let stream = request.httpBodyStream else { return Data() }
        stream.open()
        defer { stream.close() }
        var body = Data()
        var buffer = [UInt8](repeating: 0, count: 4_096)
        while true {
            let count = stream.read(&buffer, maxLength: buffer.count)
            if count < 0 { throw try XCTUnwrap(stream.streamError) }
            if count == 0 { return body }
            body.append(contentsOf: buffer.prefix(count))
        }
    }
}
