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

private struct AtomicPrivateSettlementAcceptingVerifierV1:
    AtomicPrivateSettlementResponseVerifyingV1
{
    func requireAvailable() throws {}

    func verifyCommitteeProof(
        responseJSON: Data,
        expectedNetworkID: Data,
        requestedPayloadDigest: Data
    ) throws {}

    func verifyAuditorCapsule(
        responseJSON: Data,
        expectedNetworkID: Data,
        requestedPayloadDigest: Data,
        auditorSigningKey: String
    ) throws {}

    func verifyAuditApproval(
        responseJSON: Data,
        requestJSON: Data,
        expectedNetworkID: Data,
        requestedPayloadDigest: Data,
        auditorSigningKey: String
    ) throws {}
}

private struct AtomicPrivateSettlementRejectingVerifierV1:
    AtomicPrivateSettlementResponseVerifyingV1
{
    func requireAvailable() throws {}

    func verifyCommitteeProof(
        responseJSON: Data,
        expectedNetworkID: Data,
        requestedPayloadDigest: Data
    ) throws {
        throw AtomicPrivateSettlementNativeVerifierErrorV1.nativeRejected(-507)
    }

    func verifyAuditorCapsule(
        responseJSON: Data,
        expectedNetworkID: Data,
        requestedPayloadDigest: Data,
        auditorSigningKey: String
    ) throws {
        throw AtomicPrivateSettlementNativeVerifierErrorV1.nativeRejected(-507)
    }

    func verifyAuditApproval(
        responseJSON: Data,
        requestJSON: Data,
        expectedNetworkID: Data,
        requestedPayloadDigest: Data,
        auditorSigningKey: String
    ) throws {
        throw AtomicPrivateSettlementNativeVerifierErrorV1.nativeRejected(-507)
    }
}

private struct AtomicPrivateSettlementUnavailableVerifierV1:
    AtomicPrivateSettlementResponseVerifyingV1
{
    func requireAvailable() throws {
        throw AtomicPrivateSettlementNativeVerifierErrorV1.bridgeUnavailable
    }

    func verifyCommitteeProof(
        responseJSON: Data,
        expectedNetworkID: Data,
        requestedPayloadDigest: Data
    ) throws {
        XCTFail("unavailable verifier must fail before committee-proof verification")
    }

    func verifyAuditorCapsule(
        responseJSON: Data,
        expectedNetworkID: Data,
        requestedPayloadDigest: Data,
        auditorSigningKey: String
    ) throws {
        XCTFail("unavailable verifier must fail before capsule verification")
    }

    func verifyAuditApproval(
        responseJSON: Data,
        requestJSON: Data,
        expectedNetworkID: Data,
        requestedPayloadDigest: Data,
        auditorSigningKey: String
    ) throws {
        XCTFail("unavailable verifier must fail before approval verification")
    }
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

    func testNativeResponseVerifierLinkageProbeResolvesAllRestrictedSymbols() throws {
        #if canImport(Darwin)
        try AtomicPrivateSettlementNativeResponseVerifierV1().requireAvailable()
        #else
        throw XCTSkip("the native Apple bridge is only available on Darwin")
        #endif
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

        let floatingHeight = canonicalText.replacingOccurrences(
            of: #""accepted_at_height":105"#,
            with: #""accepted_at_height":105.0"#
        )
        installRaw(Data(floatingHeight.utf8), statusCode: 202)
        do {
            _ = try await makeClient().submitBundle(
                request,
                sponsorAuth: try sponsorAuth()
            )
            XCTFail("floating-point height must fail closed")
        } catch AtomicPrivateSettlementClientErrorV1.invalidResponse {}

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
        let request = try auditApprovalRequest(root)
        let role = try ToriiOperatorSigningContext(
            networkId: TestNetworkIds.canonical,
            signingKey: .ed25519(privateKey: Data(repeating: 0x72, count: 32))
        )

        let response = try await makeClient().submitAuditApproval(
            payloadDigest: payload,
            request: request,
            auditorSigningContext: role
        )

        XCTAssertFalse(try response.bytes().isEmpty)
        XCTAssertTrue(response.description.contains("[REDACTED]"))
        response.close()
        XCTAssertThrowsError(try response.bytes())

        let captured = try XCTUnwrap(AtomicPrivateSettlementStubURLProtocolV1.lastRequest)
        XCTAssertEqual(
            captured.url?.path,
            "/api/v1/nexus/private-settlements/legs/\(payload.pathComponent)/audit-approvals"
        )
        XCTAssertNotNil(captured.value(forHTTPHeaderField: "X-Iroha-Operator-Signature"))
        XCTAssertNil(captured.value(forHTTPHeaderField: ToriiCanonicalRequest.headerSignature))
    }

    func testRestrictedResponsesFailClosedWhenNativeVerifierRejects() async throws {
        let root = try fixture()
        let ids = try XCTUnwrap(root["identifiers"] as? [String: Any])
        let responses = try XCTUnwrap(root["responses"] as? [String: Any])
        let payload = try AtomicPrivateSettlementIdentifierV1(
            try XCTUnwrap(ids["payload_hex"] as? String)
        )
        let role = try ToriiOperatorSigningContext(
            networkId: TestNetworkIds.canonical,
            signingKey: .ed25519(privateKey: Data(repeating: 0x7A, count: 32))
        )
        let verifier = AtomicPrivateSettlementRejectingVerifierV1()

        install(try XCTUnwrap(responses["auditor_capsule"] as? [String: Any]))
        do {
            _ = try await makeClient(responseVerifier: verifier).getAuditorCapsule(
                payloadDigest: payload,
                auditorSigningContext: role
            )
            XCTFail("native capsule rejection must fail closed")
        } catch AtomicPrivateSettlementClientErrorV1.invalidResponse {}

        install(try XCTUnwrap(responses["audit_approval"] as? [String: Any]))
        do {
            _ = try await makeClient(responseVerifier: verifier).submitAuditApproval(
                payloadDigest: payload,
                request: try auditApprovalRequest(root),
                auditorSigningContext: role
            )
            XCTFail("native approval rejection must fail closed")
        } catch AtomicPrivateSettlementClientErrorV1.invalidResponse {}
    }

    func testRestrictedRoutesRequireNativeVerifierBeforeHTTPDispatch() async throws {
        let root = try fixture()
        let ids = try XCTUnwrap(root["identifiers"] as? [String: Any])
        let responses = try XCTUnwrap(root["responses"] as? [String: Any])
        let payload = try AtomicPrivateSettlementIdentifierV1(
            try XCTUnwrap(ids["payload_hex"] as? String)
        )
        let role = try ToriiOperatorSigningContext(
            networkId: TestNetworkIds.canonical,
            signingKey: .ed25519(privateKey: Data(repeating: 0x7B, count: 32))
        )
        let client = try makeClient(
            responseVerifier: AtomicPrivateSettlementUnavailableVerifierV1()
        )

        let restrictedCalls: [() async throws -> AtomicPrivateSettlementJSONResponseV1] = [
            { try await client.getCommitteeProof(
                payloadDigest: payload,
                validatorSigningContext: role
            ) },
            { try await client.getAuditorCapsule(
                payloadDigest: payload,
                auditorSigningContext: role
            ) },
            { try await client.submitAuditApproval(
                payloadDigest: payload,
                request: try self.auditApprovalRequest(root),
                auditorSigningContext: role
            ) },
        ]
        for restrictedCall in restrictedCalls {
            do {
                _ = try await restrictedCall()
                XCTFail("restricted route must fail when the native verifier is unavailable")
            } catch AtomicPrivateSettlementClientErrorV1.invalidResponse {}
            XCTAssertEqual(AtomicPrivateSettlementStubURLProtocolV1.dispatchCount, 0)
        }

        install(try XCTUnwrap(responses["bundle_status_aborted"] as? [String: Any]))
        let bundle = try AtomicPrivateSettlementIdentifierV1(
            try XCTUnwrap(ids["bundle_hex"] as? String)
        )
        _ = try await client.getBundleStatus(bundleId: bundle)
        XCTAssertEqual(AtomicPrivateSettlementStubURLProtocolV1.dispatchCount, 1)
    }

    func testAuditorCapsuleRequiresExactNonzeroAuthoritativeHeight() async throws {
        let root = try fixture()
        let ids = try XCTUnwrap(root["identifiers"] as? [String: Any])
        let responses = try XCTUnwrap(root["responses"] as? [String: Any])
        let valid = try XCTUnwrap(responses["auditor_capsule"] as? [String: Any])
        let payload = try AtomicPrivateSettlementIdentifierV1(
            try XCTUnwrap(ids["payload_hex"] as? String)
        )
        let role = try ToriiOperatorSigningContext(
            networkId: TestNetworkIds.canonical,
            signingKey: .ed25519(privateKey: Data(repeating: 0x73, count: 32))
        )

        install(valid)
        _ = try await makeClient().getAuditorCapsule(
            payloadDigest: payload,
            auditorSigningContext: role
        )
        let captured = try XCTUnwrap(AtomicPrivateSettlementStubURLProtocolV1.lastRequest)
        XCTAssertEqual(
            captured.url?.path,
            "/api/v1/nexus/private-settlements/legs/\(payload.pathComponent)/audit-capsule"
        )
        XCTAssertNotNil(captured.value(forHTTPHeaderField: "X-Iroha-Operator-Signature"))

        for invalidHeight: Any in [0, -1, 105.5, "105"] {
            var invalid = valid
            invalid["authoritative_height"] = invalidHeight
            install(invalid)
            do {
                _ = try await makeClient().getAuditorCapsule(
                    payloadDigest: payload,
                    auditorSigningContext: role
                )
                XCTFail("invalid authoritative height must fail closed")
            } catch {
                XCTAssertEqual(
                    error as? AtomicPrivateSettlementClientErrorV1,
                    .invalidResponse
                )
            }
        }

        let validData = try JSONSerialization.data(withJSONObject: valid, options: [.sortedKeys])
        let validText = try XCTUnwrap(String(data: validData, encoding: .utf8))
        let overflow = validText.replacingOccurrences(
            of: "\"authoritative_height\":105",
            with: "\"authoritative_height\":18446744073709551616"
        )
        installRaw(Data(overflow.utf8), statusCode: 200)
        do {
            _ = try await makeClient().getAuditorCapsule(
                payloadDigest: payload,
                auditorSigningContext: role
            )
            XCTFail("overflow authoritative height must fail closed")
        } catch {
            XCTAssertEqual(error as? AtomicPrivateSettlementClientErrorV1, .invalidResponse)
        }
    }

    func testRestrictedAttestationsRejectSubstitutionMalformedShapeAndEncoding() async throws {
        let root = try fixture()
        let ids = try XCTUnwrap(root["identifiers"] as? [String: Any])
        let responses = try XCTUnwrap(root["responses"] as? [String: Any])
        let payload = try AtomicPrivateSettlementIdentifierV1(
            try XCTUnwrap(ids["payload_hex"] as? String)
        )
        let role = try ToriiOperatorSigningContext(
            networkId: TestNetworkIds.canonical,
            signingKey: .ed25519(privateKey: Data(repeating: 0x74, count: 32))
        )
        let capsule = try XCTUnwrap(responses["auditor_capsule"] as? [String: Any])

        var invalidCapsules: [[String: Any]] = []
        invalidCapsules.append(
            changingAttestationBody(capsule) {
                $0["network_id"] = ids["payload_json"]
            }
        )
        invalidCapsules.append(
            changingAttestationBody(capsule) {
                $0["payload_digest"] = ids["bundle_json"]
            }
        )
        invalidCapsules.append(
            changingAttestationBody(capsule) {
                $0["responder"] = ""
            }
        )
        invalidCapsules.append(changingAttestationSignature(capsule, to: "AQ=="))
        for invalid in invalidCapsules {
            install(invalid)
            do {
                _ = try await makeClient().getAuditorCapsule(
                    payloadDigest: payload,
                    auditorSigningContext: role
                )
                XCTFail("substituted or malformed capsule attestation must fail closed")
            } catch {
                XCTAssertEqual(error as? AtomicPrivateSettlementClientErrorV1, .invalidResponse)
            }
        }

        let approval = try XCTUnwrap(responses["audit_approval"] as? [String: Any])
        var invalidApprovals: [[String: Any]] = []
        invalidApprovals.append(
            changingAttestationBody(approval) {
                $0["network_id"] = ids["payload_json"]
            }
        )
        invalidApprovals.append(
            changingAttestationBody(approval) {
                $0["payload_digest"] = ids["bundle_json"]
            }
        )
        var wrongBundle = approval
        wrongBundle["bundle_id"] = ids["payload_json"]
        invalidApprovals.append(wrongBundle)
        var wrongOrdinal = approval
        wrongOrdinal["leg_ordinal"] = 1
        invalidApprovals.append(wrongOrdinal)
        var expired = approval
        expired["authoritative_height"] = 201
        if var attestation = expired["responder_attestation"] as? [String: Any],
           var body = attestation["body"] as? [String: Any] {
            body["authoritative_height"] = 201
            attestation["body"] = body
            expired["responder_attestation"] = attestation
        }
        invalidApprovals.append(expired)
        var wrongDataspace = approval
        if var authority = wrongDataspace["committee_authority"] as? [String: Any],
           var route = authority["route"] as? [String: Any] {
            route["dataspace_id"] = 8
            authority["route"] = route
            wrongDataspace["committee_authority"] = authority
        }
        invalidApprovals.append(wrongDataspace)
        var invalidCounts = approval
        invalidCounts["collected"] = 2
        invalidApprovals.append(invalidCounts)
        var invalidRequired = approval
        invalidRequired["required"] = 0
        invalidApprovals.append(invalidRequired)
        var invalidBoolean = approval
        invalidBoolean["newly_recorded"] = 1
        invalidApprovals.append(invalidBoolean)
        var invalidLifecycle = approval
        invalidLifecycle["lifecycle"] = ["status": "collecting", "value": NSNull()]
        invalidApprovals.append(invalidLifecycle)
        invalidApprovals.append(
            changingAttestationBody(approval) {
                $0["responder"] = ""
            }
        )
        invalidApprovals.append(changingAttestationSignature(approval, to: "AQ=="))

        for invalid in invalidApprovals {
            install(invalid)
            do {
                _ = try await makeClient().submitAuditApproval(
                    payloadDigest: payload,
                    request: try auditApprovalRequest(root),
                    auditorSigningContext: role
                )
                XCTFail("substituted or malformed approval acknowledgement must fail closed")
            } catch {
                XCTAssertEqual(error as? AtomicPrivateSettlementClientErrorV1, .invalidResponse)
            }
        }

        let wrongNetworkRole = try ToriiOperatorSigningContext(
            networkId: TestNetworkIds.other,
            signingKey: .ed25519(privateKey: Data(repeating: 0x75, count: 32))
        )
        XCTAssertEqual(AtomicPrivateSettlementStubURLProtocolV1.dispatchCount, invalidCapsules.count + invalidApprovals.count)
        do {
            _ = try await makeClient().getAuditorCapsule(
                payloadDigest: payload,
                auditorSigningContext: wrongNetworkRole
            )
            XCTFail("a role identity from another network must fail before dispatch")
        } catch {
            XCTAssertEqual(error as? AtomicPrivateSettlementClientErrorV1, .invalidPreparedRequest)
        }
        XCTAssertEqual(AtomicPrivateSettlementStubURLProtocolV1.dispatchCount, invalidCapsules.count + invalidApprovals.count)

        do {
            _ = try await makeClient().submitAuditApproval(
                payloadDigest: payload,
                request: try auditApprovalRequest(root, networkId: TestNetworkIds.other),
                auditorSigningContext: role
            )
            XCTFail("an approval prepared for another network must fail before dispatch")
        } catch {
            XCTAssertEqual(error as? AtomicPrivateSettlementClientErrorV1, .invalidPreparedRequest)
        }
        XCTAssertEqual(AtomicPrivateSettlementStubURLProtocolV1.dispatchCount, invalidCapsules.count + invalidApprovals.count)

        install(
            try XCTUnwrap(responses["bundle_status_aborted"] as? [String: Any]),
            headers: ["Content-Type": "application/json", "Content-Encoding": "gzip"]
        )
        let bundle = try AtomicPrivateSettlementIdentifierV1(
            try XCTUnwrap(ids["bundle_hex"] as? String)
        )
        do {
            _ = try await makeClient().getBundleStatus(bundleId: bundle)
            XCTFail("non-identity response encoding must fail closed")
        } catch {
            XCTAssertEqual(error as? AtomicPrivateSettlementClientErrorV1, .invalidResponse)
        }

        XCTAssertThrowsError(
            try AtomicPrivateSettlementPreparedRequestV1(
                operation: .auditApproval,
                nativePreparedJSON: Data(#"{"approval":{"fractional":1e0}}"#.utf8)
            )
        )
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

    private func makeClient(
        responseVerifier: any AtomicPrivateSettlementResponseVerifyingV1 =
            AtomicPrivateSettlementAcceptingVerifierV1()
    ) throws -> AtomicPrivateSettlementToriiClientV1 {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [AtomicPrivateSettlementStubURLProtocolV1.self]
        return try AtomicPrivateSettlementToriiClientV1(
            baseURL: try XCTUnwrap(URL(string: "https://torii.example/api/")),
            localSigningContext: ToriiLocalSigningContext(networkId: TestNetworkIds.canonical),
            session: URLSession(configuration: configuration),
            responseVerifier: responseVerifier
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

    private func install(
        _ object: [String: Any],
        statusCode: Int = 200,
        headers: [String: String] = ["Content-Type": "application/json"]
    ) {
        AtomicPrivateSettlementStubURLProtocolV1.handler = { request in
            let response = try XCTUnwrap(
                HTTPURLResponse(
                    url: try XCTUnwrap(request.url),
                    statusCode: statusCode,
                    httpVersion: "HTTP/1.1",
                    headerFields: headers
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

    private func auditApprovalRequest(
        _ root: [String: Any],
        networkId: NetworkId = TestNetworkIds.canonical
    ) throws -> AtomicPrivateSettlementPreparedRequestV1 {
        let ids = try XCTUnwrap(root["identifiers"] as? [String: Any])
        let body: [String: Any] = [
            "version": 1,
            "network_id": networkId.literal,
            "bundle_id": try XCTUnwrap(ids["bundle_json"] as? String),
            "leg_ordinal": 0,
            "dataspace_id": 7,
            "auditor_id": "auditor-test",
            "audit_policy_digest": try XCTUnwrap(ids["payload_json"] as? String),
            "audit_key_epoch": 1,
            "proof_digest": try XCTUnwrap(ids["payload_json"] as? String),
            "capsule_digest": try XCTUnwrap(ids["payload_json"] as? String),
            "delta_digest": try XCTUnwrap(ids["payload_json"] as? String),
            "old_root": String(repeating: "11", count: 32),
            "new_root": String(repeating: "22", count: 32),
            "expiry_height": 200,
        ]
        let object: [String: Any] = [
            "approval": ["body": body, "signature": "opaque-native-signature"],
        ]
        return try AtomicPrivateSettlementPreparedRequestV1(
            operation: .auditApproval,
            nativePreparedJSON: try JSONSerialization.data(
                withJSONObject: object,
                options: [.sortedKeys]
            )
        )
    }

    private func changingAttestationBody(
        _ object: [String: Any],
        _ change: (inout [String: Any]) -> Void
    ) -> [String: Any] {
        var candidate = object
        guard var attestation = candidate["responder_attestation"] as? [String: Any],
              var body = attestation["body"] as? [String: Any] else {
            return candidate
        }
        change(&body)
        attestation["body"] = body
        candidate["responder_attestation"] = attestation
        return candidate
    }

    private func changingAttestationSignature(
        _ object: [String: Any],
        to signature: String
    ) -> [String: Any] {
        var candidate = object
        guard var attestation = candidate["responder_attestation"] as? [String: Any] else {
            return candidate
        }
        attestation["signature"] = signature
        candidate["responder_attestation"] = attestation
        return candidate
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
