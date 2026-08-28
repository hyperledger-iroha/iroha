import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import XCTest
@testable import IrohaSwift

private final class HijiriQuoteStubURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data))?

    override class func canInit(with request: URLRequest) -> Bool { true }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let handler = Self.handler else {
            client?.urlProtocol(
                self,
                didFailWithError: NSError(domain: "HijiriQuoteStub", code: -1)
            )
            return
        }
        do {
            let (response, body) = try handler(request)
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: body)
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}
}

private final class HijiriQuoteTestCodec: ValidationFeeHijiriQuoteCoding {
    let encodedRequest: Data
    let quote: ValidationFeeHijiriQuoteV1
    private(set) var encodeCallCount = 0
    private(set) var verifiedResponse: Data?
    private(set) var verifiedRequest: Data?

    init(encodedRequest: Data, quote: ValidationFeeHijiriQuoteV1) {
        self.encodedRequest = encodedRequest
        self.quote = quote
    }

    func encode(_ request: ValidationFeeHijiriQuoteRequestV1) throws -> Data {
        encodeCallCount += 1
        return encodedRequest
    }

    func verify(
        _ responseNorito: Data,
        requestNorito: Data
    ) throws -> ValidationFeeHijiriQuoteV1 {
        verifiedResponse = responseNorito
        verifiedRequest = requestNorito
        return quote
    }
}

@available(iOS 15.0, macOS 12.0, *)
final class ValidationFeeHijiriQuoteTests: XCTestCase {
    private let seed = Data(repeating: 0x41, count: 32)
    private let responseBody = Data([0x4E, 0x52, 0x54, 0x31, 0x01])

    override func tearDown() {
        HijiriQuoteStubURLProtocol.handler = nil
        super.tearDown()
    }

    func testRequestRequiresCanonicalAccountAndBoundedPositiveCount() throws {
        let accountId = try canonicalAccountId()
        let request = try ValidationFeeHijiriQuoteRequestV1(
            accountId: accountId,
            qualifyingTransferCount:
                ValidationFeeHijiriQuoteRequestV1.maximumQualifyingTransferCount
        )
        XCTAssertEqual(request.version, 1)
        XCTAssertEqual(request.accountId, accountId)

        for invalidAccount in ["", " \(accountId)", "\(accountId) ", "alice@wonderland"] {
            XCTAssertThrowsError(
                try ValidationFeeHijiriQuoteRequestV1(
                    accountId: invalidAccount,
                    qualifyingTransferCount: 1
                )
            )
        }
        for invalidCount: UInt32 in [
            0,
            ValidationFeeHijiriQuoteRequestV1.maximumQualifyingTransferCount + 1,
        ] {
            XCTAssertThrowsError(
                try ValidationFeeHijiriQuoteRequestV1(
                    accountId: accountId,
                    qualifyingTransferCount: invalidCount
                )
            )
        }
    }

    func testNativeProjectionRequiresFrozenFieldSetAndRiskPair() throws {
        let accountId = try canonicalAccountId()
        let valid = quoteProjection(accountId: accountId, count: 3)
        let quote = try parseProjection(valid)
        XCTAssertEqual(quote.accountId, accountId)
        XCTAssertEqual(quote.qualifyingTransferCount, 3)

        var unknown = valid
        unknown["futureField"] = true
        XCTAssertThrowsError(try parseProjection(unknown))

        var partialRisk = valid
        partialRisk["accountRiskRevision"] = "2"
        XCTAssertThrowsError(try parseProjection(partialRisk))

        var wrongAssurance = valid
        wrongAssurance["assurance"] = "WITNESS_VERIFIED"
        XCTAssertThrowsError(try parseProjection(wrongAssurance))
    }

    func testAbi23NativeBridgeEncodesAndRejectsMalformedResponse() throws {
        let request = try ValidationFeeHijiriQuoteRequestV1(
            accountId: canonicalAccountId(),
            qualifyingTransferCount: 2
        )
        let requestNorito = try request.noritoBytes()
        XCTAssertFalse(requestNorito.isEmpty)
        XCTAssertLessThanOrEqual(
            requestNorito.count,
            ValidationFeeHijiriQuoteRequestV1.maximumRequestBytes
        )

        XCTAssertThrowsError(
            try ValidationFeeHijiriQuoteNative.verifyResponseV1(
                Data([0x00]),
                requestNorito: requestNorito
            )
        ) { error in
            XCTAssertEqual(
                error as? ValidationFeeHijiriQuoteError,
                .nativeRejected(-506)
            )
        }
    }

    func testSignedNativeNoritoPostBindsHeadersBodyAndResponse() async throws {
        let accountId = try canonicalAccountId()
        let requestBody = Data([0x10, 0x20, 0x30])
        let responseBody = self.responseBody
        let quote = try parseProjection(quoteProjection(accountId: accountId, count: 7))
        let codec = HijiriQuoteTestCodec(encodedRequest: requestBody, quote: quote)
        let expectedAccountHeader = try AccountAddress.parseEncoded(accountId).canonicalHex()

        HijiriQuoteStubURLProtocol.handler = { [responseBody] request in
            XCTAssertEqual(request.url?.absoluteString, "https://torii.example/v1/validation-fee/hijiri/quote")
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(toriiClientTestBodyData(from: request), requestBody)
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/x-norito")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept"), "application/x-norito")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Accept-Encoding"), "identity")
            XCTAssertEqual(request.value(forHTTPHeaderField: "Cache-Control"), "no-store")
            XCTAssertEqual(request.value(forHTTPHeaderField: "X-Iroha-Account"), expectedAccountHeader)
            XCTAssertEqual(request.value(forHTTPHeaderField: "X-Iroha-Timestamp-Ms"), "1700000000000")
            XCTAssertEqual(
                request.value(forHTTPHeaderField: "X-Iroha-Nonce"),
                "0123456789abcdef0123456789abcdef"
            )
            XCTAssertNotNil(request.value(forHTTPHeaderField: "X-Iroha-Signature"))
            return (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: [
                        "Content-Type": "application/x-norito",
                        "Content-Encoding": "identity",
                        "Cache-Control": "private, no-store",
                        "Content-Length": String(responseBody.count),
                    ]
                )!,
                responseBody
            )
        }

        let request = try ValidationFeeHijiriQuoteRequestV1(
            accountId: accountId,
            qualifyingTransferCount: 7
        )
        let received = try await makeClient().postValidationFeeHijiriQuote(
            request,
            canonicalAuth: canonicalAuth(accountId: accountId),
            codec: codec
        )
        XCTAssertEqual(received, quote)
        XCTAssertEqual(codec.verifiedResponse, responseBody)
        XCTAssertEqual(codec.verifiedRequest, requestBody)
    }

    func testRequestAndResponseBoundsFailBeforeNativeVerification() async throws {
        let accountId = try canonicalAccountId()
        let responseBody = self.responseBody
        let request = try ValidationFeeHijiriQuoteRequestV1(
            accountId: accountId,
            qualifyingTransferCount: 2
        )
        let quote = try parseProjection(quoteProjection(accountId: accountId, count: 2))

        let oversizedRequestCodec = HijiriQuoteTestCodec(
            encodedRequest: Data(
                repeating: 0,
                count: ValidationFeeHijiriQuoteRequestV1.maximumRequestBytes + 1
            ),
            quote: quote
        )
        HijiriQuoteStubURLProtocol.handler = { _ in
            XCTFail("oversized request must fail before transport")
            throw URLError(.badServerResponse)
        }
        await assertRejected {
            _ = try await makeClient().postValidationFeeHijiriQuote(
                request,
                canonicalAuth: canonicalAuth(accountId: accountId),
                codec: oversizedRequestCodec
            )
        }
        XCTAssertNil(oversizedRequestCodec.verifiedResponse)

        let oversizedResponseCodec = HijiriQuoteTestCodec(encodedRequest: Data([1]), quote: quote)
        HijiriQuoteStubURLProtocol.handler = { [responseBody] request in
            (
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: [
                        "Content-Type": "application/x-norito",
                        "Cache-Control": "private, no-store",
                        "Content-Length": String(
                            ValidationFeeHijiriQuoteV1.maximumResponseBytes + 1
                        ),
                    ]
                )!,
                responseBody
            )
        }
        await assertRejected {
            _ = try await makeClient().postValidationFeeHijiriQuote(
                request,
                canonicalAuth: canonicalAuth(accountId: accountId),
                codec: oversizedResponseCodec
            )
        }
        XCTAssertNil(oversizedResponseCodec.verifiedResponse)
    }

    func testHttpsIsRequiredBeforeNativeEncodingOrTransport() async throws {
        let accountId = try canonicalAccountId()
        let request = try ValidationFeeHijiriQuoteRequestV1(
            accountId: accountId,
            qualifyingTransferCount: 1
        )
        let quote = try parseProjection(quoteProjection(accountId: accountId, count: 1))
        let codec = HijiriQuoteTestCodec(encodedRequest: Data([1]), quote: quote)
        HijiriQuoteStubURLProtocol.handler = { _ in
            XCTFail("insecure Hijiri quote must fail before transport")
            throw URLError(.badURL)
        }

        await assertRejected {
            _ = try await makeClient(
                baseURL: URL(string: "http://torii.example")!
            ).postValidationFeeHijiriQuote(
                request,
                canonicalAuth: canonicalAuth(accountId: accountId),
                codec: codec
            )
        }
        XCTAssertEqual(codec.encodeCallCount, 0)
        XCTAssertNil(codec.verifiedResponse)
    }

    func testResponsePolicyAndEchoSubstitutionFailClosed() async throws {
        let accountId = try canonicalAccountId()
        let responseBody = self.responseBody
        let request = try ValidationFeeHijiriQuoteRequestV1(
            accountId: accountId,
            qualifyingTransferCount: 4
        )
        let exactQuote = try parseProjection(quoteProjection(accountId: accountId, count: 4))
        let invalidResponses: [(Int, [String: String])] = [
            (201, responseHeaders()),
            (403, responseHeaders(cacheControl: "no-store")),
            (200, responseHeaders(contentType: "application/x-norito; charset=binary")),
            (200, responseHeaders(cacheControl: "no-store")),
            (200, responseHeaders(contentEncoding: "gzip")),
            (200, responseHeaders(rejectCode: "")),
            (200, responseHeaders(rejectCode: "   ")),
            (200, responseHeaders(rejectCode: "STALE_QUOTE")),
            (200, responseHeaders(cacheControl: "No-Store, PRIVATE, PuBlIc")),
            (200, responseHeaders(cacheControl: "no-store, Private=\"Set-Cookie\"")),
            (200, responseHeaders(cacheControl: "private, no-store=enabled")),
            (200, responseHeaders(cacheControl: "extension=\"private, no-store\"")),
        ]
        for (status, headers) in invalidResponses {
            let codec = HijiriQuoteTestCodec(encodedRequest: Data([2]), quote: exactQuote)
            HijiriQuoteStubURLProtocol.handler = { [responseBody] urlRequest in
                (
                    HTTPURLResponse(
                        url: urlRequest.url!,
                        statusCode: status,
                        httpVersion: nil,
                        headerFields: headers
                    )!,
                    responseBody
                )
            }
            await assertRejected {
                _ = try await makeClient().postValidationFeeHijiriQuote(
                    request,
                    canonicalAuth: canonicalAuth(accountId: accountId),
                    codec: codec
                )
            }
            XCTAssertNil(codec.verifiedResponse)
        }

        let substituted = try parseProjection(quoteProjection(accountId: accountId, count: 5))
        let codec = HijiriQuoteTestCodec(encodedRequest: Data([3]), quote: substituted)
        let exactHeaders = responseHeaders(contentLength: responseBody.count)
        HijiriQuoteStubURLProtocol.handler = { [responseBody] urlRequest in
            (
                HTTPURLResponse(
                    url: urlRequest.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: exactHeaders
                )!,
                responseBody
            )
        }
        await assertRejected {
            _ = try await makeClient().postValidationFeeHijiriQuote(
                request,
                canonicalAuth: canonicalAuth(accountId: accountId),
                codec: codec
            )
        }
        XCTAssertEqual(codec.verifiedResponse, responseBody)
    }

    func testResponseCacheControlDirectivesAreCaseInsensitiveAndOrderIndependent() async throws {
        let accountId = try canonicalAccountId()
        let responseBody = self.responseBody
        let request = try ValidationFeeHijiriQuoteRequestV1(
            accountId: accountId,
            qualifyingTransferCount: 6
        )
        let quote = try parseProjection(quoteProjection(accountId: accountId, count: 6))
        let codec = HijiriQuoteTestCodec(encodedRequest: Data([4]), quote: quote)
        let headers = responseHeaders(
            cacheControl: "max-age=0, NO-STORE, PrIvAtE",
            contentLength: responseBody.count
        )
        HijiriQuoteStubURLProtocol.handler = { [responseBody] urlRequest in
            (
                HTTPURLResponse(
                    url: urlRequest.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: headers
                )!,
                responseBody
            )
        }

        let received = try await makeClient().postValidationFeeHijiriQuote(
            request,
            canonicalAuth: canonicalAuth(accountId: accountId),
            codec: codec
        )
        XCTAssertEqual(received, quote)
        XCTAssertEqual(codec.verifiedResponse, responseBody)
    }

    private func canonicalAccountId() throws -> String {
        try Keypair(privateKeyBytes: seed)
            .accountId(networkPrefix: AccountId.defaultNetworkPrefix)
    }

    private func canonicalAuth(accountId: String) -> ToriiCanonicalRequestAuth {
        ToriiCanonicalRequestAuth(
            accountId: accountId,
            privateKey: seed,
            timestampMs: 1_700_000_000_000,
            nonce: "0123456789abcdef0123456789abcdef"
        )
    }

    private func makeClient(
        baseURL: URL = URL(string: "https://torii.example")!
    ) -> ToriiClient {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [HijiriQuoteStubURLProtocol.self]
        return ToriiClient(
            baseURL: baseURL,
            session: URLSession(configuration: configuration),
            localSigningContext: ToriiLocalSigningContext(networkId: TestNetworkIds.canonical)
        )
    }

    private func quoteProjection(accountId: String, count: UInt32) -> [String: Any] {
        [
            "schema": ValidationFeeHijiriQuoteV1.schemaV1,
            "version": 1,
            "assurance": ValidationFeeHijiriQuoteV1.evaluatedAssuranceV1,
            "evaluatedStateHeight": "40",
            "quotedExecutionHeight": "41",
            "accountId": accountId,
            "activePolicyVersion": "8",
            "activePolicyHash": String(repeating: "1", count: 64),
            "feeAssetDefinitionId": "xor#sora",
            "treasuryAccountId": accountId,
            "feeScale": 0,
            "hijiriParametersVersion": 1,
            "hijiriParametersRevision": "2",
            "hijiriParametersDigest": String(repeating: "2", count: 64),
            "defaultAccountRiskQ16": 0,
            "effectiveAccountRiskQ16": 65_536,
            "accountRiskRevision": NSNull(),
            "accountRiskDigest": NSNull(),
            "feeMultiplierQ16": 65_536,
            "hijiriFeeQuoteHash": String(repeating: "3", count: 64),
            "basePerTransferFeeMinorUnits": "10",
            "adjustedPerTransferFeeMinorUnits": "10",
            "qualifyingTransferCount": count,
            "aggregateBaseFeeMinorUnits": String(UInt64(count) * 10),
            "aggregateAdjustedFeeMinorUnits": String(UInt64(count) * 10),
        ]
    }

    private func parseProjection(_ object: [String: Any]) throws -> ValidationFeeHijiriQuoteV1 {
        try ValidationFeeHijiriQuoteV1.parseNativeVerifiedProjection(
            JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
        )
    }

    private func responseHeaders(
        contentType: String = "application/x-norito",
        cacheControl: String = "private, no-store",
        contentEncoding: String? = nil,
        rejectCode: String? = nil,
        contentLength: Int? = nil
    ) -> [String: String] {
        var headers = [
            "Content-Type": contentType,
            "Cache-Control": cacheControl,
        ]
        if let contentEncoding { headers["Content-Encoding"] = contentEncoding }
        if let rejectCode { headers["x-iroha-reject-code"] = rejectCode }
        if let contentLength { headers["Content-Length"] = String(contentLength) }
        return headers
    }

    private func assertRejected(
        _ operation: () async throws -> Void,
        file: StaticString = #filePath,
        line: UInt = #line
    ) async {
        do {
            try await operation()
            XCTFail("expected Hijiri quote request to fail closed", file: file, line: line)
        } catch {
            // Expected.
        }
    }
}
