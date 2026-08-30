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

    func testBundleAdmissionUsesSharedFixtureAndRejectsMalformedDTO() async throws {
        let root = try fixture()
        let ids = try XCTUnwrap(root["identifiers"] as? [String: Any])
        let responses = try XCTUnwrap(root["responses"] as? [String: Any])
        let valid = try XCTUnwrap(responses["bundle_submit"] as? [String: Any])
        let request = try AtomicPrivateSettlementPreparedRequestV1(
            operation: .bundleSubmit,
            nativePreparedJSON: Data(#"{"transaction":{}}"#.utf8)
        )
        install(valid, statusCode: 202)

        let admitted = try await makeClient().submitBundle(
            request,
            sponsorAuth: try sponsorAuth()
        )

        let captured = try XCTUnwrap(AtomicPrivateSettlementStubURLProtocolV1.lastRequest)
        XCTAssertEqual(captured.httpMethod, "POST")
        XCTAssertEqual(captured.url?.path, "/api/v1/nexus/private-settlements/bundles")
        XCTAssertEqual(
            try XCTUnwrap(
                JSONSerialization.jsonObject(with: admitted.bytes()) as? NSDictionary
            ),
            valid as NSDictionary
        )
        XCTAssertNil(valid["lifecycle"])

        let canonical = try JSONSerialization.data(withJSONObject: valid, options: [.sortedKeys])
        let canonicalText = try XCTUnwrap(String(data: canonical, encoding: .utf8))
        let maximumHeight = canonicalText.replacingOccurrences(
            of: #""accepted_at_height":105"#,
            with: #""accepted_at_height":18446744073709551615"#
        )
        installRaw(Data(maximumHeight.utf8), statusCode: 202)
        _ = try await makeClient().submitBundle(
            request,
            sponsorAuth: try sponsorAuth()
        )

        install(valid, statusCode: 200)
        do {
            _ = try await makeClient().submitBundle(
                request,
                sponsorAuth: try sponsorAuth()
            )
            XCTFail("carrier admission must require HTTP 202")
        } catch AtomicPrivateSettlementClientErrorV1.invalidResponse {}

        var malformed: [[String: Any]] = []
        let carrierLiteral = try XCTUnwrap(valid["carrier_id"] as? String)
        for (field, value) in [
            ("bundle_id", ids["bundle_hex"] as Any),
            ("carrier_id", carrierLiteral.lowercased() as Any),
            ("bundle_id", 1 as Any),
            ("carrier_id", NSNull() as Any),
            ("accepted_at_height", true as Any),
            ("accepted_at_height", -1 as Any),
            ("accepted_at_height", "105" as Any),
            ("accepted_at_height", 105.0 as Any),
        ] {
            var candidate = valid
            candidate[field] = value
            malformed.append(candidate)
        }
        var invalidChecksum = valid
        let payloadLiteral = try XCTUnwrap(ids["payload_json"] as? String)
        invalidChecksum["carrier_id"] = String(payloadLiteral.dropLast()) + "0"
        malformed.append(invalidChecksum)
        var missing = valid
        missing.removeValue(forKey: "carrier_id")
        malformed.append(missing)
        var leaked = valid
        leaked["lifecycle"] = ["status": "finalized", "value": NSNull()]
        malformed.append(leaked)

        for candidate in malformed {
            install(candidate, statusCode: 202)
            do {
                _ = try await makeClient().submitBundle(
                    request,
                    sponsorAuth: try sponsorAuth()
                )
                XCTFail("malformed carrier admission DTO must fail closed")
            } catch AtomicPrivateSettlementClientErrorV1.invalidResponse {}
        }

        let negativeZero = canonicalText
            .replacingOccurrences(of: #""accepted_at_height":105"#, with: #""accepted_at_height":-0"#)
        installRaw(Data(negativeZero.utf8), statusCode: 202)
        do {
            _ = try await makeClient().submitBundle(
                request,
                sponsorAuth: try sponsorAuth()
            )
            XCTFail("negative-zero height must fail closed")
        } catch AtomicPrivateSettlementClientErrorV1.invalidResponse {}

        let overflowHeight = maximumHeight.replacingOccurrences(
            of: "18446744073709551615",
            with: "18446744073709551616"
        )
        installRaw(Data(overflowHeight.utf8), statusCode: 202)
        do {
            _ = try await makeClient().submitBundle(
                request,
                sponsorAuth: try sponsorAuth()
            )
            XCTFail("overflowed height must fail closed")
        } catch AtomicPrivateSettlementClientErrorV1.invalidResponse {}
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

        install(
            try XCTUnwrap(responses["receipt_pending"] as? [String: Any]),
            statusCode: 201
        )
        do {
            _ = try await makeClient().getBundleReceipt(bundleId: bundle)
            XCTFail("public receipt must require HTTP 200")
        } catch AtomicPrivateSettlementClientErrorV1.invalidResponse {}

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
        let root = try fixture()
        let ids = try XCTUnwrap(root["identifiers"] as? [String: Any])
        let bundle = try AtomicPrivateSettlementIdentifierV1(
            try XCTUnwrap(ids["bundle_hex"] as? String)
        )
        installHTTPError(rejectCode: "memo=LEAK_CANARY_987654")
        do {
            _ = try await makeClient().getBundleStatus(bundleId: bundle)
            XCTFail("HTTP 400 must fail")
        } catch {
            let rendered = String(describing: error)
            XCTAssertFalse(rendered.contains("LEAK_CANARY"))
            XCTAssertFalse(rendered.contains("987654"))
        }

        installHTTPError(rejectCode: "APS_POLICY_DENIED")
        do {
            _ = try await makeClient().getBundleStatus(bundleId: bundle)
            XCTFail("HTTP 400 must fail")
        } catch let error as AtomicPrivateSettlementClientErrorV1 {
            guard case let .httpStatus(code, rejectCode) = error else {
                XCTFail("HTTP 400 must remain a typed status error")
                return
            }
            XCTAssertEqual(code, 400)
            XCTAssertEqual(rejectCode, "APS_POLICY_DENIED")
        }

        installHTTPError(rejectCode: String(repeating: "A", count: 129))
        do {
            _ = try await makeClient().getBundleStatus(bundleId: bundle)
            XCTFail("HTTP 400 must fail")
        } catch let error as AtomicPrivateSettlementClientErrorV1 {
            guard case let .httpStatus(code, rejectCode) = error else {
                XCTFail("HTTP 400 must remain a typed status error")
                return
            }
            XCTAssertEqual(code, 400)
            XCTAssertNil(rejectCode)
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

    private func install(_ object: [String: Any], statusCode: Int = 200) {
        AtomicPrivateSettlementStubURLProtocolV1.handler = { request in
            let response = try XCTUnwrap(
                HTTPURLResponse(
                    url: try XCTUnwrap(request.url),
                    statusCode: statusCode,
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

    private func installRaw(_ data: Data, statusCode: Int) {
        AtomicPrivateSettlementStubURLProtocolV1.handler = { request in
            let response = try XCTUnwrap(
                HTTPURLResponse(
                    url: try XCTUnwrap(request.url),
                    statusCode: statusCode,
                    httpVersion: "HTTP/1.1",
                    headerFields: ["Content-Type": "application/json"]
                )
            )
            return (response, data)
        }
    }

    private func installHTTPError(rejectCode: String) {
        AtomicPrivateSettlementStubURLProtocolV1.handler = { request in
            let response = try XCTUnwrap(
                HTTPURLResponse(
                    url: try XCTUnwrap(request.url),
                    statusCode: 400,
                    httpVersion: "HTTP/1.1",
                    headerFields: [
                        "Content-Type": "text/plain",
                        "X-Iroha-Reject-Code": rejectCode,
                    ]
                )
            )
            return (response, Data("memo=LEAK_CANARY amount=987654".utf8))
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
