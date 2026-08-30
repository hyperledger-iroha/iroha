import Foundation
import XCTest
@testable import IrohaSwift

private final class AtomicPrivateSettlementStubURLProtocolV1: URLProtocol {
    static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data))?
    static var lastRequest: URLRequest?
    static var dispatchCount = 0

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        Self.lastRequest = request
        Self.dispatchCount += 1
        do {
            guard let handler = Self.handler else { throw URLError(.badServerResponse) }
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

@available(iOS 15.0, macOS 12.0, *)
final class AtomicPrivateSettlementToriiClientV1Tests: XCTestCase {
    private let sponsorSeed = Data(repeating: 0x61, count: 32)

    override func tearDown() {
        AtomicPrivateSettlementStubURLProtocolV1.handler = nil
        AtomicPrivateSettlementStubURLProtocolV1.lastRequest = nil
        AtomicPrivateSettlementStubURLProtocolV1.dispatchCount = 0
        super.tearDown()
    }

    func testSharedNoritoJSONFixturePinsEveryPreparedRouteAndShape() throws {
        let root = try fixture()
        let routes = try XCTUnwrap(root["request_routes"] as? [[String: Any]])
        XCTAssertEqual(routes.count, AtomicPrivateSettlementOperationV1.allCases.count)
        for route in routes {
            let operation = try XCTUnwrap(
                AtomicPrivateSettlementOperationV1(
                    rawValue: try XCTUnwrap(route["operation"] as? String)
                )
            )
            XCTAssertEqual(operation.path, route["path"] as? String)
            XCTAssertEqual(operation.auth.rawValue, route["auth"] as? String)
            let fields = try XCTUnwrap(route["top_level_fields"] as? [String])
            let object = Dictionary(uniqueKeysWithValues: fields.map { ($0, [:] as [String: Any]) })
            let request = try AtomicPrivateSettlementPreparedRequestV1(
                operation: operation,
                nativePreparedJSON: try JSONSerialization.data(
                    withJSONObject: object,
                    options: [.sortedKeys]
                )
            )
            XCTAssertEqual(request.operation, operation)
            XCTAssertTrue(request.description.contains("[REDACTED]"))
            request.close()
            XCTAssertThrowsError(try request.bytes())
        }
    }

    func testPreparedRequestRejectsDuplicateKeysAndOperationSubstitution() async throws {
        XCTAssertThrowsError(
            try AtomicPrivateSettlementPreparedRequestV1(
                operation: .auditApproval,
                nativePreparedJSON: Data(#"{"approval":{},"approval":{}}"#.utf8)
            )
        )
        let request = try AtomicPrivateSettlementPreparedRequestV1(
            operation: .bundleSubmit,
            nativePreparedJSON: Data(#"{"transaction":{}}"#.utf8)
        )
        let client = try makeClient()
        let auth = try sponsorAuth()
        do {
            _ = try await client.uploadLeg(request, sponsorAuth: auth)
            XCTFail("operation substitution must fail before dispatch")
        } catch AtomicPrivateSettlementClientErrorV1.operationSubstitution {
            XCTAssertEqual(AtomicPrivateSettlementStubURLProtocolV1.dispatchCount, 0)
        }
    }

    func testSponsorLegStatusIsNetworkBoundAndResponseStaysRedacted() async throws {
        let root = try fixture()
        let ids = try XCTUnwrap(root["identifiers"] as? [String: Any])
        let responses = try XCTUnwrap(root["responses"] as? [String: Any])
        let response = try XCTUnwrap(responses["leg_status"] as? [String: Any])
        let payload = try AtomicPrivateSettlementIdentifierV1(
            try XCTUnwrap(ids["payload_hex"] as? String)
        )
        install(response)

        let received = try await makeClient().getLegStatus(
            payloadDigest: payload,
            sponsorAuth: try sponsorAuth()
        )

        let request = try XCTUnwrap(AtomicPrivateSettlementStubURLProtocolV1.lastRequest)
        XCTAssertEqual(
            request.url?.path,
            "/api/v1/nexus/private-settlements/legs/\(payload.pathComponent)/status"
        )
        XCTAssertEqual(request.httpMethod, "GET")
        XCTAssertNotNil(request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerSignature))
        XCTAssertNil(request.value(forHTTPHeaderField: "X-Iroha-Operator-Signature"))
        XCTAssertTrue(received.description.contains("[REDACTED]"))
        XCTAssertFalse(received.description.contains(try XCTUnwrap(ids["payload_json"] as? String)))
    }

    func testSponsorPhaseCertificateRecoveryIsBoundAndStrictlyAllowlisted() async throws {
        let root = try fixture()
        let ids = try XCTUnwrap(root["identifiers"] as? [String: Any])
        let responses = try XCTUnwrap(root["responses"] as? [String: Any])
        let phaseCertificates = try XCTUnwrap(responses["phase_certificates"] as? [String: Any])
        let payload = try AtomicPrivateSettlementIdentifierV1(
            try XCTUnwrap(ids["payload_hex"] as? String)
        )
        install(phaseCertificates)

        let received = try await makeClient().getPhaseCertificates(
            payloadDigest: payload,
            sponsorAuth: try sponsorAuth()
        )

        let request = try XCTUnwrap(AtomicPrivateSettlementStubURLProtocolV1.lastRequest)
        XCTAssertEqual(
            request.url?.path,
            "/api/v1/nexus/private-settlements/legs/\(payload.pathComponent)/phase-certificates"
        )
        XCTAssertEqual(request.httpMethod, "GET")
        XCTAssertNil(request.httpBody)
        XCTAssertNotNil(request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerSignature))
        XCTAssertNil(request.value(forHTTPHeaderField: "X-Iroha-Operator-Signature"))
        XCTAssertTrue(received.description.contains("[REDACTED]"))

        var missingCertificate = phaseCertificates
        missingCertificate.removeValue(forKey: "commit_certificate")
        install(missingCertificate)
        do {
            _ = try await makeClient().getPhaseCertificates(
                payloadDigest: payload,
                sponsorAuth: try sponsorAuth()
            )
            XCTFail("both certificate fields must be explicit")
        } catch AtomicPrivateSettlementClientErrorV1.invalidResponse {}

        var nonObjectCertificate = phaseCertificates
        nonObjectCertificate["prepare_certificate"] = []
        install(nonObjectCertificate)
        do {
            _ = try await makeClient().getPhaseCertificates(
                payloadDigest: payload,
                sponsorAuth: try sponsorAuth()
            )
            XCTFail("certificate values must be null or opaque objects")
        } catch AtomicPrivateSettlementClientErrorV1.invalidResponse {}

        var leakedField = phaseCertificates
        leakedField["plaintext"] = "LEAK_CANARY"
        install(leakedField)
        do {
            _ = try await makeClient().getPhaseCertificates(
                payloadDigest: payload,
                sponsorAuth: try sponsorAuth()
            )
            XCTFail("route-specific allowlist must reject extra fields")
        } catch {
            XCTAssertFalse(String(describing: error).contains("LEAK_CANARY"))
        }
    }

    func testAuditorApprovalUsesSeparateRoleIdentityHeaders() async throws {
        let root = try fixture()
        let ids = try XCTUnwrap(root["identifiers"] as? [String: Any])
        let responses = try XCTUnwrap(root["responses"] as? [String: Any])
        install(try XCTUnwrap(responses["audit_approval"] as? [String: Any]))
        let payload = try AtomicPrivateSettlementIdentifierV1(
            try XCTUnwrap(ids["payload_hex"] as? String)
        )
        let request = try AtomicPrivateSettlementPreparedRequestV1(
            operation: .auditApproval,
            nativePreparedJSON: Data(#"{"approval":{}}"#.utf8)
        )
        let role = try ToriiOperatorSigningContext(
            networkId: TestNetworkIds.canonical,
            signingKey: .ed25519(privateKey: Data(repeating: 0x72, count: 32))
        )

        _ = try await makeClient().submitAuditApproval(
            payloadDigest: payload,
            request: request,
            auditorSigningContext: role
        )

        let captured = try XCTUnwrap(AtomicPrivateSettlementStubURLProtocolV1.lastRequest)
        XCTAssertEqual(
            captured.url?.path,
            "/api/v1/nexus/private-settlements/legs/\(payload.pathComponent)/audit-approvals"
        )
        XCTAssertNotNil(captured.value(forHTTPHeaderField: "X-Iroha-Operator-Signature"))
        XCTAssertNil(captured.value(forHTTPHeaderField: ToriiCanonicalRequest.headerSignature))
    }

    func testPublicReceiptIsUnsignedAndSubstitutionFailsClosed() async throws {
        let root = try fixture()
        let ids = try XCTUnwrap(root["identifiers"] as? [String: Any])
        let responses = try XCTUnwrap(root["responses"] as? [String: Any])
        let bundle = try AtomicPrivateSettlementIdentifierV1(
            try XCTUnwrap(ids["bundle_hex"] as? String)
        )
        install(try XCTUnwrap(responses["receipt_pending"] as? [String: Any]))

        _ = try await makeClient().getBundleReceipt(bundleId: bundle)
        let request = try XCTUnwrap(AtomicPrivateSettlementStubURLProtocolV1.lastRequest)
        XCTAssertNil(request.value(forHTTPHeaderField: ToriiCanonicalRequest.headerSignature))
        XCTAssertNil(request.value(forHTTPHeaderField: "X-Iroha-Operator-Signature"))

        var substituted = try XCTUnwrap(responses["receipt_pending"] as? [String: Any])
        var value = try XCTUnwrap(substituted["value"] as? [String: Any])
        value["bundle_id"] = ids["payload_json"]
        substituted["value"] = value
        install(substituted)
        do {
            _ = try await makeClient().getBundleReceipt(bundleId: bundle)
            XCTFail("substituted receipt must fail")
        } catch AtomicPrivateSettlementClientErrorV1.responseSubstitution {}
    }

    func testHTTPErrorDoesNotRenderCanaryBody() async throws {
        AtomicPrivateSettlementStubURLProtocolV1.handler = { request in
            let response = try XCTUnwrap(
                HTTPURLResponse(
                    url: try XCTUnwrap(request.url),
                    statusCode: 400,
                    httpVersion: "HTTP/1.1",
                    headerFields: ["Content-Type": "text/plain"]
                )
            )
            return (response, Data("memo=LEAK_CANARY amount=987654".utf8))
        }
        let root = try fixture()
        let ids = try XCTUnwrap(root["identifiers"] as? [String: Any])
        let bundle = try AtomicPrivateSettlementIdentifierV1(
            try XCTUnwrap(ids["bundle_hex"] as? String)
        )
        do {
            _ = try await makeClient().getBundleStatus(bundleId: bundle)
            XCTFail("HTTP 400 must fail")
        } catch {
            let rendered = String(describing: error)
            XCTAssertFalse(rendered.contains("LEAK_CANARY"))
            XCTAssertFalse(rendered.contains("987654"))
        }
    }

    func testResponseURLSubstitutionFailsClosed() async throws {
        let root = try fixture()
        let ids = try XCTUnwrap(root["identifiers"] as? [String: Any])
        let responses = try XCTUnwrap(root["responses"] as? [String: Any])
        let status = try XCTUnwrap(responses["bundle_status_aborted"] as? [String: Any])
        let bundle = try AtomicPrivateSettlementIdentifierV1(
            try XCTUnwrap(ids["bundle_hex"] as? String)
        )
        AtomicPrivateSettlementStubURLProtocolV1.handler = { _ in
            let substitutedURL = try XCTUnwrap(URL(string: "https://collector.invalid/status"))
            let response = try XCTUnwrap(
                HTTPURLResponse(
                    url: substitutedURL,
                    statusCode: 200,
                    httpVersion: "HTTP/1.1",
                    headerFields: ["Content-Type": "application/json"]
                )
            )
            return (
                response,
                try JSONSerialization.data(withJSONObject: status, options: [.sortedKeys])
            )
        }

        do {
            _ = try await makeClient().getBundleStatus(bundleId: bundle)
            XCTFail("response URL substitution must fail")
        } catch AtomicPrivateSettlementClientErrorV1.responseSubstitution {}
    }

    private func makeClient() throws -> AtomicPrivateSettlementToriiClientV1 {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [AtomicPrivateSettlementStubURLProtocolV1.self]
        return try AtomicPrivateSettlementToriiClientV1(
            baseURL: try XCTUnwrap(URL(string: "https://torii.example/api/")),
            localSigningContext: ToriiLocalSigningContext(networkId: TestNetworkIds.canonical),
            session: URLSession(configuration: configuration)
        )
    }

    private func sponsorAuth() throws -> ToriiCanonicalRequestAuth {
        ToriiCanonicalRequestAuth(
            accountId: try Keypair(privateKeyBytes: sponsorSeed)
                .accountId(networkPrefix: AccountId.defaultNetworkPrefix),
            privateKey: sponsorSeed,
            timestampMs: 1_700_000_000_000,
            nonce: "atomic-private-settlement-test"
        )
    }

    private func install(_ object: [String: Any]) {
        AtomicPrivateSettlementStubURLProtocolV1.handler = { request in
            let response = try XCTUnwrap(
                HTTPURLResponse(
                    url: try XCTUnwrap(request.url),
                    statusCode: 200,
                    httpVersion: "HTTP/1.1",
                    headerFields: ["Content-Type": "application/json"]
                )
            )
            return (
                response,
                try JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
            )
        }
    }

    private func fixture() throws -> [String: Any] {
        var current = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
        while current.path != "/" {
            let candidate = current.appendingPathComponent(
                "fixtures/norito_rpc/atomic_private_settlement_sdk_v1.json"
            )
            if FileManager.default.fileExists(atPath: candidate.path) {
                return try XCTUnwrap(
                    JSONSerialization.jsonObject(with: Data(contentsOf: candidate))
                        as? [String: Any]
                )
            }
            current.deleteLastPathComponent()
        }
        throw NSError(
            domain: "AtomicPrivateSettlementToriiClientV1Tests",
            code: 1,
            userInfo: [
                NSLocalizedDescriptionKey:
                    "shared atomic-private-settlement fixture was not found"
            ]
        )
    }
}
