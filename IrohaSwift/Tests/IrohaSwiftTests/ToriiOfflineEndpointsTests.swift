import XCTest
import IrohaSwift

private final class OfflineStubURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (HTTPURLResponse, Data?))?

    override class func canInit(with request: URLRequest) -> Bool { true }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        guard let handler = Self.handler else {
            client?.urlProtocol(self, didFailWithError: URLError(.badServerResponse))
            return
        }
        do {
            let (response, data) = try handler(request)
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            if let data { client?.urlProtocol(self, didLoad: data) }
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}
}

@available(iOS 15.0, macOS 12.0, *)
final class ToriiOfflineEndpointsTests: XCTestCase {
    private struct ExpectedRequest {
        let method: String
        let path: String
        let responseBody: Data
        let assertBody: ((Data?) throws -> Void)?
    }

    override func tearDown() {
        OfflineStubURLProtocol.handler = nil
        super.tearDown()
    }

    private func makeCertificateObject(from cert: OfflineWalletCertificate) -> [String: Any] {
        [
            "controller": cert.controller,
            "operator": cert.operatorId,
            "allowance": [
                "asset": cert.allowance.assetId,
                "amount": cert.allowance.amount,
                "commitment": cert.allowance.commitment.map(Int.init)
            ] as [String: Any],
            "spend_public_key": cert.spendPublicKey,
            "attestation_report": cert.attestationReport.map(Int.init),
            "issued_at_ms": cert.issuedAtMs,
            "expires_at_ms": cert.expiresAtMs,
            "policy": [
                "max_balance": cert.policy.maxBalance,
                "max_tx_value": cert.policy.maxTxValue,
                "expires_at_ms": cert.policy.expiresAtMs
            ] as [String: Any],
            "operator_signature": cert.operatorSignature.map { String(format: "%02X", $0) }.joined(),
            "metadata": [:] as [String: Any]
        ]
    }

    private func makeRecordData(for certificate: OfflineWalletCertificate) throws -> Data {
        try JSONSerialization.data(
            withJSONObject: ["certificate": makeCertificateObject(from: certificate)],
            options: [.sortedKeys])
    }

    private func makeClient() -> ToriiClient {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [OfflineStubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        return ToriiClient(baseURL: URL(string: "https://example.com")!, session: session)
    }

    private func requestBody(from request: URLRequest) throws -> Data? {
        if let body = request.httpBody {
            return body
        }
        guard let stream = request.httpBodyStream else {
            return nil
        }
        stream.open()
        defer { stream.close() }
        var data = Data()
        var buffer = [UInt8](repeating: 0, count: 1024)
        while stream.hasBytesAvailable {
            let count = stream.read(&buffer, maxLength: buffer.count)
            if count < 0 {
                throw stream.streamError ?? URLError(.cannotDecodeRawData)
            }
            if count == 0 {
                break
            }
            data.append(buffer, count: count)
        }
        return data
    }

    func testOfflineEndpointsRequests() async throws {
        let accountId = "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs"
        let assetId = "rose#wonderland#\(accountId)"
        let receiptsPayload = """
        {
          "items": [{
            "bundle_id_hex": "aa",
            "tx_id_hex": "bb",
            "certificate_id_hex": "cc",
            "controller_id": "\(accountId)",
            "controller_display": "\(accountId)",
            "receiver_id": "\(accountId)",
            "receiver_display": "\(accountId)",
            "asset_id": "\(assetId)",
            "amount": "10",
            "invoice_id": "inv-1",
            "counter": 4,
            "recorded_at_ms": 123,
            "recorded_at_height": 456
          }],
          "total": 1
        }
        """.data(using: .utf8)!
        let emptyListPayload = """
        {"items": [], "total": 0}
        """.data(using: .utf8)!
        let spendReceiptsPayload = """
        {
          "receipts_root_hex": "aa",
          "receipt_count": 1,
          "total_amount": "10",
          "asset_id": "\(assetId)"
        }
        """.data(using: .utf8)!
        let settlementPayload = """
        {"bundle_id_hex": "beef"}
        """.data(using: .utf8)!
        let statePayload = """
        {"allowances": [], "transfers": [], "summaries": [], "revocations": [], "now_ms": 123}
        """.data(using: .utf8)!
        let proofPayload = """
        {"header": {"version": 1}}
        """.data(using: .utf8)!
        let issuePayload = """
        {
          "certificate_id_hex": "deadbeef",
          "certificate": {
            "controller": "\(accountId)",
            "operator": "\(accountId)",
            "allowance": {
              "asset": "\(assetId)",
              "amount": "10",
              "commitment": [1, 2, 3]
            },
            "spend_public_key": "ed0120deadbeef",
            "attestation_report": [4, 5, 6],
            "issued_at_ms": 100,
            "expires_at_ms": 200,
            "policy": {
              "max_balance": "10",
              "max_tx_value": "5",
              "expires_at_ms": 200
            },
            "operator_signature": "AA",
            "metadata": {},
            "verdict_id_hex": null,
            "attestation_nonce_hex": null,
            "refresh_at_ms": null
          }
        }
        """.data(using: .utf8)!
        let registerPayload = """
        {"certificate_id_hex": "deadbeef"}
        """.data(using: .utf8)!
        let renewPayload = """
        {
          "certificate_id_hex": "beadfeed",
          "certificate": {
            "controller": "\(accountId)",
            "operator": "\(accountId)",
            "allowance": {
              "asset": "\(assetId)",
              "amount": "11",
              "commitment": [7, 8, 9]
            },
            "spend_public_key": "ed0120cafebabe",
            "attestation_report": [9, 9, 9],
            "issued_at_ms": 300,
            "expires_at_ms": 400,
            "policy": {
              "max_balance": "20",
              "max_tx_value": "6",
              "expires_at_ms": 400
            },
            "operator_signature": "BB",
            "metadata": {},
            "verdict_id_hex": null,
            "attestation_nonce_hex": null,
            "refresh_at_ms": null
          }
        }
        """.data(using: .utf8)!
        let buildClaimIssuePayload = """
        {
          "claim_id_hex": "feedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedface",
          "build_claim": {
            "claim_id": "hash:FEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACE#CD31",
            "nonce": "hash:ABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCD#C9C5",
            "platform": "Apple",
            "app_id": "com.example.ios",
            "build_number": 77,
            "issued_at_ms": 1700000000000,
            "expires_at_ms": 1700000100000,
            "operator_signature": "AA"
          }
        }
        """.data(using: .utf8)!

        let envelope = ToriiQueryEnvelope(
            filter: .object(["op": .string("eq"), "args": .array([.string("bundle_id_hex"), .string("aa")])]),
            sort: [ToriiQuerySortKey(key: "recorded_at_ms", order: .desc)],
            pagination: ToriiQueryPagination(limit: 5, offset: 0)
        )

        let receiptsRequest = ToriiOfflineSpendReceiptsSubmitRequest(receipts: [.object(["tx_id": .string("aa")])])
        let settlementRequest = ToriiOfflineSettlementSubmitRequest(
            authority: accountId,
            privateKey: "ed0120deadbeef",
            transfer: .object(["bundle_id": .string("aa")])
        )
        let proofRequest = ToriiOfflineTransferProofRequest(transfer: .object(["bundle_id": .string("aa")]),
                                                            kind: "counter",
                                                            counterCheckpoint: 1)
        let draft = OfflineWalletCertificateDraft(
            controller: accountId,
            allowance: OfflineAllowanceCommitment(assetId: assetId,
                                                  amount: "10",
                                                  commitment: Data([0x01, 0x02])),
            spendPublicKey: "ed0120deadbeef",
            attestationReport: Data([0xAA, 0xBB]),
            issuedAtMs: 100,
            expiresAtMs: 200,
            policy: OfflineWalletPolicy(maxBalance: "10", maxTxValue: "5", expiresAtMs: 200)
        )
        let issueRequest = try ToriiOfflineCertificateIssueRequest(certificate: draft)
        let buildClaimIssueRequest = ToriiOfflineBuildClaimIssueRequest(
            certificateIdHex: String(repeating: "ab", count: 32),
            txIdHex: String(repeating: "cd", count: 32),
            platform: "apple",
            appId: "com.example.ios",
            buildNumber: 77,
            issuedAtMs: 1_700_000_000_000,
            expiresAtMs: 1_700_000_100_000
        )
        let renewRequest = try ToriiOfflineCertificateIssueRequest(certificate: draft)

        var queue: [ExpectedRequest] = [
            ExpectedRequest(method: "GET", path: "/v1/offline/receipts", responseBody: receiptsPayload, assertBody: nil),
            ExpectedRequest(method: "POST", path: "/v1/offline/receipts/query", responseBody: receiptsPayload, assertBody: { data in
                let body = try XCTUnwrap(data)
                let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
                XCTAssertNotNil(json["filter"] as? [String: Any])
                XCTAssertNotNil(json["pagination"] as? [String: Any])
            }),
            ExpectedRequest(method: "POST", path: "/v1/offline/allowances/query", responseBody: emptyListPayload, assertBody: nil),
            ExpectedRequest(method: "POST", path: "/v1/offline/certificates/query", responseBody: emptyListPayload, assertBody: nil),
            ExpectedRequest(method: "POST", path: "/v1/offline/transfers/query", responseBody: emptyListPayload, assertBody: nil),
            ExpectedRequest(method: "POST", path: "/v1/offline/settlements/query", responseBody: emptyListPayload, assertBody: nil),
            ExpectedRequest(method: "POST", path: "/v1/offline/revocations/query", responseBody: emptyListPayload, assertBody: nil),
            ExpectedRequest(method: "POST", path: "/v1/offline/certificates/issue", responseBody: issuePayload, assertBody: { data in
                let body = try XCTUnwrap(data)
                let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
                let certificate = try XCTUnwrap(json["certificate"] as? [String: Any])
                XCTAssertNil(certificate["operator"])
                XCTAssertNil(certificate["operator_signature"])
                XCTAssertNotNil(certificate["allowance"] as? [String: Any])
                XCTAssertNotNil(certificate["attestation_report"] as? [Any])
            }),
            ExpectedRequest(method: "POST", path: "/v1/offline/build-claims/issue", responseBody: buildClaimIssuePayload, assertBody: { data in
                let body = try XCTUnwrap(data)
                let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
                XCTAssertEqual(json["certificate_id_hex"] as? String, buildClaimIssueRequest.certificateIdHex)
                XCTAssertEqual(json["tx_id_hex"] as? String, buildClaimIssueRequest.txIdHex)
                XCTAssertEqual(json["platform"] as? String, "apple")
                XCTAssertEqual(json["app_id"] as? String, "com.example.ios")
                XCTAssertEqual(json["build_number"] as? NSNumber, 77)
            }),
            ExpectedRequest(method: "POST", path: "/v1/offline/allowances", responseBody: registerPayload, assertBody: { data in
                let body = try XCTUnwrap(data)
                let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
                XCTAssertEqual(json["authority"] as? String, accountId)
                XCTAssertNotNil(json["private_key"] as? String)
                let certificate = try XCTUnwrap(json["certificate"] as? [String: Any])
                XCTAssertNotNil(certificate["operator_signature"] as? String)
                XCTAssertNotNil(certificate["allowance"] as? [String: Any])
            }),
            ExpectedRequest(method: "POST", path: "/v1/offline/certificates/deadbeef/renew/issue", responseBody: renewPayload, assertBody: { data in
                let body = try XCTUnwrap(data)
                let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
                XCTAssertNotNil(json["certificate"] as? [String: Any])
            }),
            ExpectedRequest(method: "POST", path: "/v1/offline/spend-receipts", responseBody: spendReceiptsPayload, assertBody: { data in
                let body = try XCTUnwrap(data)
                let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
                XCTAssertNotNil(json["receipts"] as? [Any])
            }),
            ExpectedRequest(method: "POST", path: "/v1/offline/settlements", responseBody: settlementPayload, assertBody: { data in
                let body = try XCTUnwrap(data)
                let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
                XCTAssertEqual(json["authority"] as? String, accountId)
                XCTAssertNotNil(json["private_key"] as? String)
                XCTAssertNotNil(json["transfer"] as? [String: Any])
                XCTAssertNil(json["build_claim_overrides"])
                XCTAssertNil(json["repair_existing_build_claims"])
            }),
            ExpectedRequest(method: "GET", path: "/v1/offline/state", responseBody: statePayload, assertBody: nil),
            ExpectedRequest(method: "POST", path: "/v1/offline/transfers/proof", responseBody: proofPayload, assertBody: { data in
                let body = try XCTUnwrap(data)
                let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
                XCTAssertNil(json["bundle_id_hex"])
                XCTAssertNotNil(json["transfer"] as? [String: Any])
            })
        ]

        OfflineStubURLProtocol.handler = { request in
            guard !queue.isEmpty else {
                throw URLError(.badServerResponse)
            }
            let expected = queue.removeFirst()
            XCTAssertEqual(request.httpMethod, expected.method)
            XCTAssertEqual(request.url?.path, expected.path)
            if expected.method == "POST" {
                XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            }
            let body = try self.requestBody(from: request)
            try expected.assertBody?(body)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, expected.responseBody)
        }

        let client = makeClient()
        let receipts = try await client.listOfflineReceipts()
        XCTAssertEqual(receipts.items.count, 1)
        XCTAssertEqual(receipts.items.first?.invoiceId, "inv-1")

        let queriedReceipts = try await client.queryOfflineReceipts(envelope)
        XCTAssertEqual(queriedReceipts.total, 1)

        _ = try await client.queryOfflineAllowances(envelope)
        _ = try await client.queryOfflineCertificates(envelope)
        _ = try await client.queryOfflineTransfers(envelope)
        _ = try await client.queryOfflineSettlements(envelope)
        _ = try await client.queryOfflineRevocations(envelope)

        let issueResponse = try await client.issueOfflineCertificate(issueRequest)
        let issuedCertificate = try issueResponse.decodeCertificate()
        XCTAssertEqual(issuedCertificate.controller, accountId)

        let issuedBuildClaim = try await client.issueOfflineBuildClaim(buildClaimIssueRequest)
        XCTAssertEqual(issuedBuildClaim.claimIdHex,
                       "feedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedfacefeedface")

        let registerRequest = try ToriiOfflineAllowanceRegisterRequest(
            authority: accountId,
            privateKey: "ed0120deadbeef",
            certificate: issuedCertificate
        )
        let registerResponse = try await client.registerOfflineAllowance(registerRequest)
        XCTAssertEqual(registerResponse.certificateIdHex, issueResponse.certificateIdHex)

        let renewResponse = try await client.issueOfflineCertificateRenewal(certificateIdHex: "deadbeef",
                                                                            requestBody: renewRequest)
        XCTAssertEqual(renewResponse.certificateIdHex, "beadfeed")

        let spendResponse = try await client.submitOfflineSpendReceipts(receiptsRequest)
        XCTAssertEqual(spendResponse.receiptCount, 1)

        let settlementResponse = try await client.submitOfflineSettlement(settlementRequest)
        XCTAssertEqual(settlementResponse.bundleIdHex, "beef")

        let state = try await client.getOfflineState()
        XCTAssertEqual(state.nowMs, 123)

        let proof = try await client.requestOfflineTransferProof(proofRequest)
        if case let .object(object) = proof {
            XCTAssertNotNil(object["header"])
        } else {
            XCTFail("expected proof response object")
        }

        XCTAssertTrue(queue.isEmpty)
    }

    func testOfflineTopUpRequests() async throws {
        let accountId = "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs"
        let assetId = "rose#wonderland#\(accountId)"
        let issuePayload = """
        {
          "certificate_id_hex": "cafe",
          "certificate": {
            "controller": "\(accountId)",
            "operator": "\(accountId)",
            "allowance": {
              "asset": "\(assetId)",
              "amount": "25",
              "commitment": [1, 2, 3]
            },
            "spend_public_key": "ed0120deadbeef",
            "attestation_report": [4, 5, 6],
            "issued_at_ms": 100,
            "expires_at_ms": 200,
            "policy": {
              "max_balance": "25",
              "max_tx_value": "5",
              "expires_at_ms": 200
            },
            "operator_signature": "AA",
            "metadata": {},
            "verdict_id_hex": null,
            "attestation_nonce_hex": null,
            "refresh_at_ms": null
          }
        }
        """.data(using: .utf8)!
        let registerPayload = """
        {"certificate_id_hex": "cafe"}
        """.data(using: .utf8)!

        let draft = OfflineWalletCertificateDraft(
            controller: accountId,
            allowance: OfflineAllowanceCommitment(assetId: assetId,
                                                  amount: "25",
                                                  commitment: Data([0x01, 0x02])),
            spendPublicKey: "ed0120deadbeef",
            attestationReport: Data([0xAA, 0xBB]),
            issuedAtMs: 100,
            expiresAtMs: 200,
            policy: OfflineWalletPolicy(maxBalance: "25", maxTxValue: "5", expiresAtMs: 200)
        )
        let attestationNonce = Data(repeating: 0xAB, count: 32)

        var queue: [ExpectedRequest] = [
            ExpectedRequest(method: "POST", path: "/v1/offline/certificates/issue", responseBody: issuePayload, assertBody: { data in
                let body = try XCTUnwrap(data)
                let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
                let certificate = try XCTUnwrap(json["certificate"] as? [String: Any])
                XCTAssertNil(certificate["operator"])
                XCTAssertNil(certificate["operator_signature"])
                XCTAssertNotNil(certificate["allowance"] as? [String: Any])
                let attestationNonceLiteral = try XCTUnwrap(certificate["attestation_nonce"] as? String)
                let attestationNonceHex = attestationNonce.map { String(format: "%02X", $0) }.joined()
                XCTAssertTrue(attestationNonceLiteral.hasPrefix("hash:"))
                XCTAssertTrue(attestationNonceLiteral.contains(attestationNonceHex))
            }),
            ExpectedRequest(method: "POST", path: "/v1/offline/allowances", responseBody: registerPayload, assertBody: { data in
                let body = try XCTUnwrap(data)
                let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
                XCTAssertEqual(json["authority"] as? String, accountId)
                XCTAssertNotNil(json["private_key"] as? String)
                let certificate = try XCTUnwrap(json["certificate"] as? [String: Any])
                XCTAssertNotNil(certificate["operator_signature"] as? String)
            })
        ]

        OfflineStubURLProtocol.handler = { request in
            guard !queue.isEmpty else {
                throw URLError(.badServerResponse)
            }
            let expected = queue.removeFirst()
            XCTAssertEqual(request.httpMethod, expected.method)
            XCTAssertEqual(request.url?.path, expected.path)
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            let body = try self.requestBody(from: request)
            try expected.assertBody?(body)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, expected.responseBody)
        }

        let client = makeClient()
        let result = try await client.topUpOfflineAllowance(draft: draft,
                                                            authority: accountId,
                                                            privateKey: "ed0120deadbeef",
                                                            attestationNonce: attestationNonce)
        XCTAssertEqual(result.certificate.certificateIdHex, "cafe")
        XCTAssertEqual(result.registration.certificateIdHex, "cafe")
        XCTAssertTrue(queue.isEmpty)
    }

    func testOfflineTopUpRenewalRequests() async throws {
        let accountId = "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs"
        let assetId = "rose#wonderland#\(accountId)"
        let issuePayload = """
        {
          "certificate_id_hex": "beadfeed",
          "certificate": {
            "controller": "\(accountId)",
            "allowance": {
              "asset": "\(assetId)",
              "amount": "30",
              "commitment": [7, 8, 9]
            },
            "spend_public_key": "ed0120cafebabe",
            "attestation_report": [9, 9, 9],
            "issued_at_ms": 300,
            "expires_at_ms": 400,
            "policy": {
              "max_balance": "30",
              "max_tx_value": "6",
              "expires_at_ms": 400
            },
            "operator_signature": "BB",
            "metadata": {},
            "verdict_id_hex": null,
            "attestation_nonce_hex": null,
            "refresh_at_ms": null
          }
        }
        """.data(using: .utf8)!
        let renewPayload = """
        {"certificate_id_hex": "beadfeed"}
        """.data(using: .utf8)!

        let draft = OfflineWalletCertificateDraft(
            controller: accountId,
            allowance: OfflineAllowanceCommitment(assetId: assetId,
                                                  amount: "30",
                                                  commitment: Data([0x07, 0x08])),
            spendPublicKey: "ed0120cafebabe",
            attestationReport: Data([0xCC, 0xDD]),
            issuedAtMs: 300,
            expiresAtMs: 400,
            policy: OfflineWalletPolicy(maxBalance: "30", maxTxValue: "6", expiresAtMs: 400)
        )
        let attestationNonce = Data(repeating: 0xCD, count: 32)

        var queue: [ExpectedRequest] = [
            ExpectedRequest(method: "POST", path: "/v1/offline/certificates/deadbeef/renew/issue", responseBody: issuePayload, assertBody: { data in
                let body = try XCTUnwrap(data)
                let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
                let certificate = try XCTUnwrap(json["certificate"] as? [String: Any])
                XCTAssertNil(certificate["operator"])
                let attestationNonceLiteral = try XCTUnwrap(certificate["attestation_nonce"] as? String)
                let attestationNonceHex = attestationNonce.map { String(format: "%02X", $0) }.joined()
                XCTAssertTrue(attestationNonceLiteral.hasPrefix("hash:"))
                XCTAssertTrue(attestationNonceLiteral.contains(attestationNonceHex))
            }),
            ExpectedRequest(method: "POST", path: "/v1/offline/allowances/deadbeef/renew", responseBody: renewPayload, assertBody: { data in
                let body = try XCTUnwrap(data)
                let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
                XCTAssertEqual(json["authority"] as? String, accountId)
                let certificate = try XCTUnwrap(json["certificate"] as? [String: Any])
                XCTAssertNotNil(certificate["operator_signature"] as? String)
            })
        ]

        OfflineStubURLProtocol.handler = { request in
            guard !queue.isEmpty else {
                throw URLError(.badServerResponse)
            }
            let expected = queue.removeFirst()
            XCTAssertEqual(request.httpMethod, expected.method)
            XCTAssertEqual(request.url?.path, expected.path)
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            let body = try self.requestBody(from: request)
            try expected.assertBody?(body)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, expected.responseBody)
        }

        let client = makeClient()
        let result = try await client.topUpOfflineAllowanceRenewal(certificateIdHex: "deadbeef",
                                                                   draft: draft,
                                                                   authority: accountId,
                                                                   privateKey: "ed0120cafebabe",
                                                                   attestationNonce: attestationNonce)
        XCTAssertEqual(result.certificate.certificateIdHex, "beadfeed")
        XCTAssertEqual(result.registration.certificateIdHex, "beadfeed")
        XCTAssertTrue(queue.isEmpty)
    }

    func testOfflineTopUpRejectsInvalidAttestationNonceLength() async throws {
        let accountId = "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs"
        let assetId = "rose#wonderland#\(accountId)"
        let draft = OfflineWalletCertificateDraft(
            controller: accountId,
            allowance: OfflineAllowanceCommitment(assetId: assetId,
                                                  amount: "25",
                                                  commitment: Data([0x01, 0x02])),
            spendPublicKey: "ed0120deadbeef",
            attestationReport: Data([0xAA, 0xBB]),
            issuedAtMs: 100,
            expiresAtMs: 200,
            policy: OfflineWalletPolicy(maxBalance: "25", maxTxValue: "5", expiresAtMs: 200)
        )
        let client = makeClient()
        do {
            _ = try await client.topUpOfflineAllowance(draft: draft,
                                                       authority: accountId,
                                                       privateKey: "ed0120deadbeef",
                                                       attestationNonce: Data([0x01, 0x02, 0x03]))
            XCTFail("expected invalid payload error")
        } catch let error as ToriiClientError {
            guard case .invalidPayload(let message) = error else {
                XCTFail("unexpected ToriiClientError: \(error)")
                return
            }
            XCTAssertTrue(message.contains("attestationNonce"))
        }
    }

    func testOfflineReprovisionRequestsPreserveAmountAndPolicy() async throws {
        let accountId = "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs"
        let assetId = "rose#wonderland#\(accountId)"
        let existingCertificate = OfflineWalletCertificate(
            controller: accountId,
            operatorId: accountId,
            allowance: OfflineAllowanceCommitment(
                assetId: assetId,
                amount: "45.00",
                commitment: Data(repeating: 0x11, count: 32)
            ),
            spendPublicKey: "ed0120cafebabe",
            attestationReport: Data([0x01, 0x02, 0x03]),
            issuedAtMs: 1_700_000_000_000,
            expiresAtMs: 1_800_000_000_000,
            policy: OfflineWalletPolicy(maxBalance: "100.00", maxTxValue: "25.00", expiresAtMs: 1_800_000_000_000),
            operatorSignature: Data(repeating: 0xCD, count: 64)
        )

        let newCommitment = Data(repeating: 0x7A, count: 32)
        let renewedCertificateIdHex = "beadfeed"
        let renewedPayloadObject: [String: Any] = [
            "controller": existingCertificate.controller,
            "operator": existingCertificate.operatorId,
            "allowance": [
                "asset": existingCertificate.allowance.assetId,
                "amount": existingCertificate.allowance.amount,
                "commitment": newCommitment.map(Int.init)
            ],
            "spend_public_key": existingCertificate.spendPublicKey,
            "attestation_report": existingCertificate.attestationReport.map(Int.init),
            "issued_at_ms": existingCertificate.issuedAtMs,
            "expires_at_ms": existingCertificate.expiresAtMs,
            "policy": [
                "max_balance": existingCertificate.policy.maxBalance,
                "max_tx_value": existingCertificate.policy.maxTxValue,
                "expires_at_ms": existingCertificate.policy.expiresAtMs
            ],
            "operator_signature": "AB",
            "metadata": [:],
            "verdict_id_hex": NSNull(),
            "attestation_nonce_hex": NSNull(),
            "refresh_at_ms": NSNull()
        ]
        let issuePayload = try JSONSerialization.data(
            withJSONObject: [
                "certificate_id_hex": renewedCertificateIdHex,
                "certificate": renewedPayloadObject
            ],
            options: [.sortedKeys]
        )
        let renewPayload = try JSONSerialization.data(
            withJSONObject: ["certificate_id_hex": renewedCertificateIdHex],
            options: [.sortedKeys]
        )

        var queue: [ExpectedRequest] = [
            ExpectedRequest(method: "POST", path: "/v1/offline/certificates/deadbeef/renew/issue", responseBody: issuePayload, assertBody: { data in
                let body = try XCTUnwrap(data)
                let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
                let certificate = try XCTUnwrap(json["certificate"] as? [String: Any])
                let allowance = try XCTUnwrap(certificate["allowance"] as? [String: Any])
                XCTAssertEqual(allowance["asset"] as? String, assetId)
                XCTAssertEqual(allowance["amount"] as? String, "45.00")
                let commitment = try XCTUnwrap(allowance["commitment"] as? [NSNumber])
                XCTAssertEqual(commitment.count, 32)
                XCTAssertEqual(commitment.first?.intValue, 0x7A)
                let policy = try XCTUnwrap(certificate["policy"] as? [String: Any])
                XCTAssertEqual(policy["max_balance"] as? String, "100.00")
                XCTAssertEqual(policy["max_tx_value"] as? String, "25.00")
            }),
            ExpectedRequest(method: "POST", path: "/v1/offline/allowances/deadbeef/renew", responseBody: renewPayload, assertBody: { data in
                let body = try XCTUnwrap(data)
                let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
                XCTAssertEqual(json["authority"] as? String, accountId)
                let certificate = try XCTUnwrap(json["certificate"] as? [String: Any])
                let allowance = try XCTUnwrap(certificate["allowance"] as? [String: Any])
                XCTAssertEqual(allowance["amount"] as? String, "45.00")
                let policy = try XCTUnwrap(certificate["policy"] as? [String: Any])
                XCTAssertEqual(policy["max_balance"] as? String, "100.00")
                XCTAssertNotNil(certificate["operator_signature"] as? String)
            })
        ]

        OfflineStubURLProtocol.handler = { request in
            guard !queue.isEmpty else {
                throw URLError(.badServerResponse)
            }
            let expected = queue.removeFirst()
            XCTAssertEqual(request.httpMethod, expected.method)
            XCTAssertEqual(request.url?.path, expected.path)
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            let body = try self.requestBody(from: request)
            try expected.assertBody?(body)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, expected.responseBody)
        }

        let existingRecordData = try makeRecordData(for: existingCertificate)
        let client = makeClient()
        let result = try await client.reprovisionOfflineAllowance(
            certificateIdHex: "deadbeef",
            currentCertificateRecordData: existingRecordData,
            newCommitment: newCommitment,
            authority: accountId,
            privateKey: "ed0120cafebabe",
            lineage: OfflineCertificateLineage(scope: "test", epoch: 2, prevCertificateIdHex: "deadbeef")
        )
        XCTAssertEqual(result.certificate.certificateIdHex.lowercased(), renewedCertificateIdHex)
        XCTAssertEqual(result.registration.certificateIdHex.lowercased(), renewedCertificateIdHex)
        XCTAssertTrue(queue.isEmpty)
    }

    func testOfflineReprovisionRejectsInvalidCommitmentLength() async throws {
        let accountId = "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs"
        let assetId = "rose#wonderland#\(accountId)"
        let existingCertificate = OfflineWalletCertificate(
            controller: accountId,
            operatorId: accountId,
            allowance: OfflineAllowanceCommitment(
                assetId: assetId,
                amount: "45.00",
                commitment: Data(repeating: 0x11, count: 32)
            ),
            spendPublicKey: "ed0120cafebabe",
            attestationReport: Data([0x01, 0x02, 0x03]),
            issuedAtMs: 1_700_000_000_000,
            expiresAtMs: 1_800_000_000_000,
            policy: OfflineWalletPolicy(maxBalance: "100.00", maxTxValue: "25.00", expiresAtMs: 1_800_000_000_000),
            operatorSignature: Data(repeating: 0xCD, count: 64)
        )

        let existingRecordData = try makeRecordData(for: existingCertificate)
        let client = makeClient()
        do {
            _ = try await client.reprovisionOfflineAllowance(
                certificateIdHex: "deadbeef",
                currentCertificateRecordData: existingRecordData,
                newCommitment: Data([0x01, 0x02, 0x03]),
                authority: accountId,
                privateKey: "ed0120cafebabe",
                lineage: OfflineCertificateLineage(scope: "test", epoch: 2, prevCertificateIdHex: "deadbeef")
            )
            XCTFail("expected invalid payload error")
        } catch let error as ToriiClientError {
            guard case let .invalidPayload(reason) = error else {
                XCTFail("unexpected error: \(error)")
                return
            }
            XCTAssertTrue(reason.contains("newCommitment must be exactly 32 bytes"))
        }
    }

    func testOfflineReprovisionCompletionAPI() async throws {
        let accountId = "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs"
        let assetId = "rose#wonderland#\(accountId)"
        let existingCertificate = OfflineWalletCertificate(
            controller: accountId,
            operatorId: accountId,
            allowance: OfflineAllowanceCommitment(
                assetId: assetId,
                amount: "45.00",
                commitment: Data(repeating: 0x11, count: 32)
            ),
            spendPublicKey: "ed0120cafebabe",
            attestationReport: Data([0x01, 0x02, 0x03]),
            issuedAtMs: 1_700_000_000_000,
            expiresAtMs: 1_800_000_000_000,
            policy: OfflineWalletPolicy(maxBalance: "100.00", maxTxValue: "25.00", expiresAtMs: 1_800_000_000_000),
            operatorSignature: Data(repeating: 0xCD, count: 64)
        )

        let newCommitment = Data(repeating: 0x6B, count: 32)
        let renewedCertificateIdHex = "feedbead"
        let renewedPayloadObject: [String: Any] = [
            "controller": existingCertificate.controller,
            "operator": existingCertificate.operatorId,
            "allowance": [
                "asset": existingCertificate.allowance.assetId,
                "amount": existingCertificate.allowance.amount,
                "commitment": newCommitment.map(Int.init)
            ],
            "spend_public_key": existingCertificate.spendPublicKey,
            "attestation_report": existingCertificate.attestationReport.map(Int.init),
            "issued_at_ms": existingCertificate.issuedAtMs,
            "expires_at_ms": existingCertificate.expiresAtMs,
            "policy": [
                "max_balance": existingCertificate.policy.maxBalance,
                "max_tx_value": existingCertificate.policy.maxTxValue,
                "expires_at_ms": existingCertificate.policy.expiresAtMs
            ],
            "operator_signature": "AB",
            "metadata": [:],
            "verdict_id_hex": NSNull(),
            "attestation_nonce_hex": NSNull(),
            "refresh_at_ms": NSNull()
        ]
        let issuePayload = try JSONSerialization.data(
            withJSONObject: [
                "certificate_id_hex": renewedCertificateIdHex,
                "certificate": renewedPayloadObject
            ],
            options: [.sortedKeys]
        )
        let renewPayload = try JSONSerialization.data(
            withJSONObject: ["certificate_id_hex": renewedCertificateIdHex],
            options: [.sortedKeys]
        )

        var queue: [ExpectedRequest] = [
            ExpectedRequest(method: "POST",
                            path: "/v1/offline/certificates/deadbeef/renew/issue",
                            responseBody: issuePayload,
                            assertBody: nil),
            ExpectedRequest(method: "POST",
                            path: "/v1/offline/allowances/deadbeef/renew",
                            responseBody: renewPayload,
                            assertBody: nil)
        ]

        OfflineStubURLProtocol.handler = { request in
            guard !queue.isEmpty else {
                throw URLError(.badServerResponse)
            }
            let expected = queue.removeFirst()
            XCTAssertEqual(request.httpMethod, expected.method)
            XCTAssertEqual(request.url?.path, expected.path)
            XCTAssertEqual(request.value(forHTTPHeaderField: "Content-Type"), "application/json")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, expected.responseBody)
        }

        let existingRecordData = try makeRecordData(for: existingCertificate)
        let client = makeClient()
        let result = try await withCheckedThrowingContinuation { continuation in
            _ = client.reprovisionOfflineAllowance(
                certificateIdHex: "deadbeef",
                currentCertificateRecordData: existingRecordData,
                newCommitment: newCommitment,
                authority: accountId,
                privateKey: "ed0120cafebabe",
                lineage: OfflineCertificateLineage(scope: "test", epoch: 2, prevCertificateIdHex: "deadbeef")
            ) { completion in
                continuation.resume(with: completion)
            }
        }
        XCTAssertEqual(result.certificate.certificateIdHex.lowercased(), renewedCertificateIdHex)
        XCTAssertEqual(result.registration.certificateIdHex.lowercased(), renewedCertificateIdHex)
        XCTAssertTrue(queue.isEmpty)
    }

    func testGetSettlementStatusReturnsSettledWhenSettlementRecordExists() async throws {
        let accountId = "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs"
        let assetId = "rose#wonderland#\(accountId)"
        let settlementPayload = """
        {
          "items": [{
            "bundle_id_hex": "aa",
            "controller_id": "\(accountId)",
            "controller_display": "\(accountId)",
            "receiver_id": "\(accountId)",
            "receiver_display": "\(accountId)",
            "deposit_account_id": "\(accountId)",
            "deposit_account_display": "\(accountId)",
            "asset_id": "\(assetId)",
            "receipt_count": 1,
            "total_amount": "10",
            "claimed_delta": "10",
            "status": "settled",
            "recorded_at_ms": 123,
            "recorded_at_height": 456,
            "transfer": { "bundle_id": "aa" }
          }],
          "total": 1
        }
        """.data(using: .utf8)!
        var queue = [settlementPayload]

        OfflineStubURLProtocol.handler = { request in
            guard request.httpMethod == "POST",
                  request.url?.path == "/v1/offline/settlements/query",
                  let payload = queue.first else {
                throw URLError(.badServerResponse)
            }
            queue.removeFirst()
            let body = try XCTUnwrap(self.requestBody(from: request))
            let envelope = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
            let filter = try XCTUnwrap(envelope["filter"] as? [String: Any])
            XCTAssertEqual(filter["op"] as? String, "eq")
            let args = try XCTUnwrap(filter["args"] as? [Any])
            XCTAssertEqual(args.count, 2)
            XCTAssertEqual(args[0] as? String, "bundle_id_hex")
            XCTAssertEqual(args[1] as? String, "aa")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, payload)
        }

        let client = makeClient()
        let status = try await client.getSettlementStatus(bundleIdHex: "aa")
        XCTAssertEqual(status, .settled)
        XCTAssertTrue(queue.isEmpty)
    }

    func testGetSettlementStatusReturnsPendingAfterAcceptedSubmit() async throws {
        let accountId = "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs"
        let submitPayload = """
        {"bundle_id_hex": "aa"}
        """.data(using: .utf8)!
        let emptyListPayload = """
        {"items": [], "total": 0}
        """.data(using: .utf8)!
        var step = 0

        OfflineStubURLProtocol.handler = { request in
            step += 1
            switch step {
            case 1:
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.url?.path, "/v1/offline/settlements")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, submitPayload)
            case 2:
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.url?.path, "/v1/offline/settlements/query")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, emptyListPayload)
            default:
                throw URLError(.badServerResponse)
            }
        }

        let request = ToriiOfflineSettlementSubmitRequest(
            authority: accountId,
            privateKey: "ed0120deadbeef",
            transfer: .object(["bundle_id": .string("aa")])
        )
        let client = makeClient()
        _ = try await client.submitOfflineSettlement(request)
        let status = try await client.getOfflineSettlementStatus(bundleIdHex: "aa")
        XCTAssertEqual(status, .pending)
        XCTAssertEqual(step, 2)
    }

    func testSubmitOfflineSettlementEncodesBuildClaimOverridesAndRepairFlag() async throws {
        let accountId = "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs"
        let txIdHex = "hash:\(String(repeating: "AB", count: 32))#B99E"
        let submitPayload = """
        {"bundle_id_hex": "aa"}
        """.data(using: .utf8)!

        OfflineStubURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.path, "/v1/offline/settlements")
            let body = try XCTUnwrap(self.requestBody(from: request))
            let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
            let overrides = try XCTUnwrap(json["build_claim_overrides"] as? [[String: Any]])
            XCTAssertEqual(overrides.count, 1)
            XCTAssertEqual(overrides.first?["tx_id_hex"] as? String, String(repeating: "ab", count: 32))
            XCTAssertEqual(overrides.first?["app_id"] as? String, "com.example.ios")
            XCTAssertEqual(overrides.first?["build_number"] as? NSNumber, 77)
            XCTAssertEqual(json["repair_existing_build_claims"] as? Bool, true)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, submitPayload)
        }

        let request = ToriiOfflineSettlementSubmitRequest(
            authority: accountId,
            privateKey: "ed0120deadbeef",
            transfer: .object(["bundle_id": .string("aa")]),
            buildClaimOverrides: [
                ToriiOfflineSettlementBuildClaimOverride(
                    txIdHex: txIdHex,
                    appId: "com.example.ios",
                    buildNumber: 77
                ),
            ],
            repairExistingBuildClaims: true
        )
        let client = makeClient()
        let response = try await client.submitOfflineSettlement(request)
        XCTAssertEqual(response.bundleIdHex, "aa")
    }

    func testSubmitOfflineSettlementAndWaitPollsPipelineStatus() async throws {
        let accountId = "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs"
        let txHashHex = String(repeating: "ca", count: 32)
        var step = 0
        OfflineStubURLProtocol.handler = { request in
            step += 1
            switch step {
            case 1:
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.url?.path, "/v1/offline/settlements")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                let payload = """
                {"bundle_id_hex":"aa","transaction_hash_hex":"\(txHashHex)"}
                """.data(using: .utf8)!
                return (response, payload)
            case 2:
                XCTAssertEqual(request.httpMethod, "GET")
                XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions/status")
                let hash = URLComponents(url: request.url!, resolvingAgainstBaseURL: false)?
                    .queryItems?
                    .first(where: { $0.name == "hash" })?
                    .value
                XCTAssertEqual(hash, txHashHex)
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                let payload = """
                {
                  "kind":"Transaction",
                  "content":{"hash":"\(txHashHex)","status":{"kind":"Pending","content":null}}
                }
                """.data(using: .utf8)!
                return (response, payload)
            case 3:
                XCTAssertEqual(request.httpMethod, "GET")
                XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions/status")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                let payload = """
                {
                  "kind":"Transaction",
                  "content":{"hash":"\(txHashHex)","status":{"kind":"Committed","content":"YQ=="}}
                }
                """.data(using: .utf8)!
                return (response, payload)
            default:
                throw URLError(.badServerResponse)
            }
        }

        let request = ToriiOfflineSettlementSubmitRequest(
            authority: accountId,
            privateKey: "ed0120deadbeef",
            transfer: .object(["bundle_id": .string("aa")])
        )
        var poll = PipelineStatusPollOptions.default
        poll.pollInterval = 0
        poll.maxAttempts = 5
        poll.timeout = 5
        let client = makeClient()
        let response = try await client.submitOfflineSettlementAndWait(request, pollOptions: poll)
        XCTAssertEqual(response.bundleIdHex, "aa")
        XCTAssertEqual(response.txHashHex, txHashHex)
        XCTAssertEqual(step, 3)
    }

    func testWaitForTransactionStatusSupportsTaskCancellation() async throws {
        let txHashHex = String(repeating: "dd", count: 32)
        let firstPollStarted = DispatchSemaphore(value: 0)
        var statusCalls = 0
        OfflineStubURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "GET")
            XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions/status")
            statusCalls += 1
            if statusCalls == 1 {
                firstPollStarted.signal()
            }
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let payload = """
            {
              "kind":"Transaction",
              "content":{"hash":"\(txHashHex)","status":{"kind":"Pending","content":null}}
            }
            """.data(using: .utf8)!
            return (response, payload)
        }

        var poll = PipelineStatusPollOptions.default
        poll.pollInterval = 30
        poll.timeout = 120
        let client = makeClient()
        let task = Task {
            try await client.waitForTransactionStatus(hashHex: txHashHex, pollOptions: poll)
        }

        XCTAssertEqual(firstPollStarted.wait(timeout: .now() + 1), .success, "first poll did not start")
        task.cancel()
        do {
            _ = try await task.value
            XCTFail("expected cancellation")
        } catch is CancellationError {
            // expected
        }
        XCTAssertEqual(statusCalls, 1, "cancellation should stop subsequent polls")
    }

    func testSubmitOfflineSettlementAndWaitSupportsTaskCancellation() async throws {
        let accountId = "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs"
        let txHashHex = String(repeating: "de", count: 32)
        let firstPollStarted = DispatchSemaphore(value: 0)
        var step = 0
        OfflineStubURLProtocol.handler = { request in
            step += 1
            switch step {
            case 1:
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.url?.path, "/v1/offline/settlements")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                let payload = """
                {"bundle_id_hex":"aa","transaction_hash_hex":"\(txHashHex)"}
                """.data(using: .utf8)!
                return (response, payload)
            case 2:
                XCTAssertEqual(request.httpMethod, "GET")
                XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions/status")
                firstPollStarted.signal()
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                let payload = """
                {
                  "kind":"Transaction",
                  "content":{"hash":"\(txHashHex)","status":{"kind":"Pending","content":null}}
                }
                """.data(using: .utf8)!
                return (response, payload)
            default:
                throw URLError(.badServerResponse)
            }
        }

        let request = ToriiOfflineSettlementSubmitRequest(
            authority: accountId,
            privateKey: "ed0120deadbeef",
            transfer: .object(["bundle_id": .string("aa")])
        )
        var poll = PipelineStatusPollOptions.default
        poll.pollInterval = 30
        poll.timeout = 120
        let client = makeClient()
        let task = Task {
            try await client.submitOfflineSettlementAndWait(request, pollOptions: poll)
        }

        XCTAssertEqual(firstPollStarted.wait(timeout: .now() + 1), .success, "status poll did not start")
        task.cancel()
        do {
            _ = try await task.value
            XCTFail("expected cancellation")
        } catch is CancellationError {
            // expected
        }
        XCTAssertEqual(step, 2, "cancellation should stop settlement polling loop")
    }

    func testSubmitOfflineSettlementAndWaitSurfacesRejectionReason() async throws {
        let accountId = "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs"
        let txHashHex = String(repeating: "cb", count: 32)
        OfflineStubURLProtocol.handler = { request in
            if request.url?.path == "/v1/offline/settlements" {
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                let payload = """
                {"bundle_id_hex":"aa","transaction_hash_hex":"\(txHashHex)"}
                """.data(using: .utf8)!
                return (response, payload)
            }
            XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions/status")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let payload = """
            {
              "kind":"Transaction",
              "content":{
                "hash":"\(txHashHex)",
                "status":{"kind":"Rejected","content":null,"rejection_reason":"build_claim_missing"}
              }
            }
            """.data(using: .utf8)!
            return (response, payload)
        }

        let request = ToriiOfflineSettlementSubmitRequest(
            authority: accountId,
            privateKey: "ed0120deadbeef",
            transfer: .object(["bundle_id": .string("aa")])
        )
        var poll = PipelineStatusPollOptions.default
        poll.pollInterval = 0
        poll.maxAttempts = 1
        poll.timeout = 5
        let client = makeClient()
        do {
            _ = try await client.submitOfflineSettlementAndWait(request, pollOptions: poll)
            XCTFail("expected pipeline rejection")
        } catch let error as PipelineStatusError {
            guard case .failure(_, let status, _) = error else {
                XCTFail("unexpected pipeline status error: \(error)")
                return
            }
            XCTAssertEqual(status, "Rejected")
            XCTAssertEqual(error.rejectionReason, "build_claim_missing")
            XCTAssertTrue(error.localizedDescription.contains("build_claim_missing"))
        }
    }

    func testSubmitOfflineSettlementAndWaitCachesRejectedStatusForBundle() async throws {
        let accountId = "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs"
        let txHashHex = String(repeating: "cc", count: 32)
        var step = 0
        OfflineStubURLProtocol.handler = { request in
            step += 1
            switch step {
            case 1:
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.url?.path, "/v1/offline/settlements")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                let payload = """
                {"bundle_id_hex":"aa","transaction_hash_hex":"\(txHashHex)"}
                """.data(using: .utf8)!
                return (response, payload)
            case 2:
                XCTAssertEqual(request.httpMethod, "GET")
                XCTAssertEqual(request.url?.path, "/v1/pipeline/transactions/status")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                let payload = """
                {
                  "kind":"Transaction",
                  "content":{
                    "hash":"\(txHashHex)",
                    "status":{"kind":"Rejected","content":null,"rejection_reason":"build_claim_missing"}
                  }
                }
                """.data(using: .utf8)!
                return (response, payload)
            case 3:
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.url?.path, "/v1/offline/settlements/query")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                let payload = """
                {"items":[],"total":0}
                """.data(using: .utf8)!
                return (response, payload)
            default:
                throw URLError(.badServerResponse)
            }
        }

        let request = ToriiOfflineSettlementSubmitRequest(
            authority: accountId,
            privateKey: "ed0120deadbeef",
            transfer: .object(["bundle_id": .string("aa")])
        )
        var poll = PipelineStatusPollOptions.default
        poll.pollInterval = 0
        poll.maxAttempts = 1
        poll.timeout = 5
        let client = makeClient()
        do {
            _ = try await client.submitOfflineSettlementAndWait(request, pollOptions: poll)
            XCTFail("expected pipeline rejection")
        } catch {
            // expected
        }
        let status = try await client.getSettlementStatus(bundleIdHex: "aa")
        XCTAssertEqual(status, .rejected(reason: "build_claim_missing"))
        XCTAssertEqual(step, 3)
    }

    func testSubmitOfflineSettlementAndWaitRejectsMissingTxHashBeforePolling() async throws {
        let accountId = "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs"
        var calls = 0
        OfflineStubURLProtocol.handler = { request in
            calls += 1
            XCTAssertEqual(request.url?.path, "/v1/offline/settlements")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            let payload = """
            {"bundle_id_hex":"aa"}
            """.data(using: .utf8)!
            return (response, payload)
        }

        let request = ToriiOfflineSettlementSubmitRequest(
            authority: accountId,
            privateKey: "ed0120deadbeef",
            transfer: .object(["bundle_id": .string("aa")])
        )
        let client = makeClient()
        do {
            _ = try await client.submitOfflineSettlementAndWait(request)
            XCTFail("expected invalid payload")
        } catch let error as ToriiClientError {
            guard case let .invalidPayload(message) = error else {
                XCTFail("unexpected error: \(error)")
                return
            }
            XCTAssertTrue(message.contains("transaction_hash_hex"))
        }
        XCTAssertEqual(calls, 1, "pipeline polling should not run without tx hash")
    }

    func testIssueOfflineBuildClaimPostsRequestAndParsesResponse() async throws {
        let claimIdHex = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef"
        let issuePayload = """
        {
          "claim_id_hex": "\(claimIdHex)",
          "build_claim": {
            "claim_id": "hash:DEADBEEFDEADBEEFDEADBEEFDEADBEEFDEADBEEFDEADBEEFDEADBEEFDEADBEEF#4A8E",
            "nonce": "hash:ABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCD#C9C5",
            "platform": "Apple",
            "app_id": "com.example.ios",
            "build_number": 77,
            "issued_at_ms": 1700000000000,
            "expires_at_ms": 1700000100000,
            "operator_signature": "AA"
          }
        }
        """.data(using: .utf8)!

        let requestBody = ToriiOfflineBuildClaimIssueRequest(
            certificateIdHex: String(repeating: "ab", count: 32),
            txIdHex: String(repeating: "cd", count: 32),
            platform: "apple",
            appId: "com.example.ios",
            buildNumber: 77
        )

        OfflineStubURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.path, "/v1/offline/build-claims/issue")
            let body = try XCTUnwrap(self.requestBody(from: request))
            let json = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
            XCTAssertEqual(json["certificate_id_hex"] as? String, requestBody.certificateIdHex)
            XCTAssertEqual(json["tx_id_hex"] as? String, requestBody.txIdHex)
            XCTAssertEqual(json["platform"] as? String, "apple")
            XCTAssertEqual(json["app_id"] as? String, "com.example.ios")
            XCTAssertEqual(json["build_number"] as? NSNumber, 77)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, issuePayload)
        }

        let client = makeClient()
        let response = try await client.issueOfflineBuildClaim(requestBody)
        XCTAssertEqual(response.claimIdHex, claimIdHex)
        XCTAssertFalse(try response.encodedBuildClaimData().isEmpty)
        let buildClaim = try response.buildClaimObject()
        XCTAssertEqual(buildClaim.platform, .apple)
        XCTAssertEqual(buildClaim.appId, "com.example.ios")
        XCTAssertEqual(buildClaim.buildNumber, 77)
    }

    func testOfflineBuildClaimDecodesLowercasePlatformVariant() throws {
        let payload = """
        {
          "claim_id": "hash:FEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACEFEEDFACE#CD31",
          "nonce": "hash:ABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCDABCD#C9C5",
          "platform": "android",
          "app_id": "com.example.android",
          "build_number": 99,
          "issued_at_ms": 1700000000000,
          "expires_at_ms": 1700000100000,
          "operator_signature": "AA"
        }
        """.data(using: .utf8)!

        let decoded = try JSONDecoder().decode(ToriiOfflineBuildClaim.self, from: payload)
        XCTAssertEqual(decoded.platform, .android)
    }

    func testIssueOfflineBuildClaimRejectsUnsupportedPlatformBeforeNetworkCall() async throws {
        OfflineStubURLProtocol.handler = { _ in
            XCTFail("network should not be called for invalid platform")
            throw URLError(.badServerResponse)
        }

        let requestBody = ToriiOfflineBuildClaimIssueRequest(
            certificateIdHex: String(repeating: "ab", count: 32),
            txIdHex: String(repeating: "cd", count: 32),
            platform: "windows-phone"
        )

        let client = makeClient()
        do {
            _ = try await client.issueOfflineBuildClaim(requestBody)
            XCTFail("expected invalid payload error")
        } catch let error as ToriiClientError {
            guard case let .invalidPayload(message) = error else {
                XCTFail("unexpected error: \(error)")
                return
            }
            XCTAssertTrue(message.contains("platform"))
        }
    }

    func testSubmitOfflineSettlementRejectsInvalidBuildClaimOverrideTxIdBeforeNetworkCall() async throws {
        OfflineStubURLProtocol.handler = { _ in
            XCTFail("network should not be called for invalid override tx id")
            throw URLError(.badServerResponse)
        }

        let request = ToriiOfflineSettlementSubmitRequest(
            authority: "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs",
            privateKey: "ed0120deadbeef",
            transfer: .object(["bundle_id": .string("aa")]),
            buildClaimOverrides: [
                ToriiOfflineSettlementBuildClaimOverride(
                    txIdHex: "not-a-hash",
                    appId: "com.example.ios"
                ),
            ]
        )

        let client = makeClient()
        do {
            _ = try await client.submitOfflineSettlement(request)
            XCTFail("expected invalid payload error")
        } catch let error as ToriiClientError {
            guard case let .invalidPayload(message) = error else {
                XCTFail("unexpected error: \(error)")
                return
            }
            XCTAssertTrue(message.contains("tx_id_hex"))
        }
    }

    func testGetSettlementStatusReturnsRejectedFromSubmitRejectCode() async throws {
        let accountId = "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs"
        let emptyListPayload = """
        {"items": [], "total": 0}
        """.data(using: .utf8)!
        let rejectCode = "offline_reason::commitment_mismatch"
        var step = 0

        OfflineStubURLProtocol.handler = { request in
            step += 1
            switch step {
            case 1:
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.url?.path, "/v1/offline/settlements")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 422,
                                               httpVersion: nil,
                                               headerFields: ["x-iroha-reject-code": rejectCode])!
                return (response, Data())
            case 2:
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.url?.path, "/v1/offline/settlements/query")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, emptyListPayload)
            default:
                throw URLError(.badServerResponse)
            }
        }

        let request = ToriiOfflineSettlementSubmitRequest(
            authority: accountId,
            privateKey: "ed0120deadbeef",
            transfer: .object(["bundle_id": .string("aa")])
        )
        let client = makeClient()
        do {
            _ = try await client.submitOfflineSettlement(request)
            XCTFail("expected submit to fail")
        } catch let error as ToriiClientError {
            guard case let .httpStatus(code, _, headerRejectCode) = error else {
                XCTFail("unexpected error type: \(error)")
                return
            }
            XCTAssertEqual(code, 422)
            XCTAssertEqual(headerRejectCode, rejectCode)
        }

        let status = try await client.getSettlementStatus(bundleIdHex: "aa")
        XCTAssertEqual(status, .rejected(reason: rejectCode))
        XCTAssertEqual(step, 2)
    }

    func testGetSettlementStatusReturnsRejectedFromDuplicateBundleSubmitRejectCode() async throws {
        let accountId = "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs"
        let emptyListPayload = """
        {"items": [], "total": 0}
        """.data(using: .utf8)!
        let rejectCode = "duplicate_bundle"
        var step = 0

        OfflineStubURLProtocol.handler = { request in
            step += 1
            switch step {
            case 1:
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.url?.path, "/v1/offline/settlements")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 400,
                                               httpVersion: nil,
                                               headerFields: ["x-iroha-reject-code": rejectCode])!
                return (response, Data())
            case 2:
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.url?.path, "/v1/offline/settlements/query")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, emptyListPayload)
            default:
                throw URLError(.badServerResponse)
            }
        }

        let request = ToriiOfflineSettlementSubmitRequest(
            authority: accountId,
            privateKey: "ed0120deadbeef",
            transfer: .object(["bundle_id": .string("aa")])
        )
        let client = makeClient()
        do {
            _ = try await client.submitOfflineSettlement(request)
            XCTFail("expected submit to fail")
        } catch let error as ToriiClientError {
            guard case let .httpStatus(code, _, headerRejectCode) = error else {
                XCTFail("unexpected error type: \(error)")
                return
            }
            XCTAssertEqual(code, 400)
            XCTAssertEqual(headerRejectCode, rejectCode)
        }

        let status = try await client.getSettlementStatus(bundleIdHex: "aa")
        XCTAssertEqual(status, .rejected(reason: rejectCode))
        XCTAssertEqual(step, 2)
    }

    func testGetSettlementStatusReturnsRejectedAfterAcceptedSubmitWhenServerRowAppears() async throws {
        let accountId = "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs"
        let submitPayload = """
        {"bundle_id_hex": "aa"}
        """.data(using: .utf8)!
        let settlementPayload = """
        {
          "items": [{
            "bundle_id_hex": "aa",
            "controller_id": "\(accountId)",
            "controller_display": "\(accountId)",
            "receiver_id": "\(accountId)",
            "receiver_display": "\(accountId)",
            "deposit_account_id": "\(accountId)",
            "deposit_account_display": "\(accountId)",
            "receipt_count": 1,
            "total_amount": "10",
            "claimed_delta": "10",
            "status": "rejected",
            "rejection_reason": "allowance_exceeded",
            "recorded_at_ms": 123,
            "recorded_at_height": 456,
            "transfer": {}
          }],
          "total": 1
        }
        """.data(using: .utf8)!
        var step = 0

        OfflineStubURLProtocol.handler = { request in
            step += 1
            switch step {
            case 1:
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.url?.path, "/v1/offline/settlements")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, submitPayload)
            case 2:
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.url?.path, "/v1/offline/settlements/query")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, settlementPayload)
            default:
                throw URLError(.badServerResponse)
            }
        }

        let request = ToriiOfflineSettlementSubmitRequest(
            authority: accountId,
            privateKey: "ed0120deadbeef",
            transfer: .object(["bundle_id": .string("aa")])
        )
        let client = makeClient()
        _ = try await client.submitOfflineSettlement(request)
        let status = try await client.getSettlementStatus(bundleIdHex: "aa")
        XCTAssertEqual(status, .rejected(reason: "allowance_exceeded"))
        XCTAssertEqual(step, 2)
    }

    func testGetSettlementStatusReturnsSettledAfterAcceptedSubmitWhenServerRowAppears() async throws {
        let accountId = "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs"
        let submitPayload = """
        {"bundle_id_hex": "aa"}
        """.data(using: .utf8)!
        let settlementPayload = """
        {
          "items": [{
            "bundle_id_hex": "aa",
            "controller_id": "\(accountId)",
            "controller_display": "\(accountId)",
            "receiver_id": "\(accountId)",
            "receiver_display": "\(accountId)",
            "deposit_account_id": "\(accountId)",
            "deposit_account_display": "\(accountId)",
            "receipt_count": 1,
            "total_amount": "10",
            "claimed_delta": "10",
            "status": "settled",
            "recorded_at_ms": 123,
            "recorded_at_height": 456,
            "transfer": {}
          }],
          "total": 1
        }
        """.data(using: .utf8)!
        var step = 0

        OfflineStubURLProtocol.handler = { request in
            step += 1
            switch step {
            case 1:
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.url?.path, "/v1/offline/settlements")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, submitPayload)
            case 2:
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.url?.path, "/v1/offline/settlements/query")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, settlementPayload)
            default:
                throw URLError(.badServerResponse)
            }
        }

        let request = ToriiOfflineSettlementSubmitRequest(
            authority: accountId,
            privateKey: "ed0120deadbeef",
            transfer: .object(["bundle_id": .string("aa")])
        )
        let client = makeClient()
        _ = try await client.submitOfflineSettlement(request)
        let status = try await client.getSettlementStatus(bundleIdHex: "aa")
        XCTAssertEqual(status, .settled)
        XCTAssertEqual(step, 2)
    }

    func testGetSettlementStatusReturnsRejectedReasonFromRevocation() async throws {
        let accountId = "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs"
        let settlementPayload = """
        {
          "items": [{
            "bundle_id_hex": "aa",
            "controller_id": "\(accountId)",
            "controller_display": "\(accountId)",
            "receiver_id": "\(accountId)",
            "receiver_display": "\(accountId)",
            "deposit_account_id": "\(accountId)",
            "deposit_account_display": "\(accountId)",
            "receipt_count": 1,
            "total_amount": "10",
            "claimed_delta": "10",
            "status": "rejected",
            "recorded_at_ms": 123,
            "recorded_at_height": 456,
            "verdict_id_hex": "beef",
            "transfer": {}
          }],
          "total": 1
        }
        """.data(using: .utf8)!
        let revocationPayload = """
        {
          "items": [{
            "verdict_id_hex": "beef",
            "issuer_id": "\(accountId)",
            "issuer_display": "\(accountId)",
            "revoked_at_ms": 124,
            "reason": "policy_violation",
            "record": {"reason": "policy_violation"}
          }],
          "total": 1
        }
        """.data(using: .utf8)!
        var step = 0

        OfflineStubURLProtocol.handler = { request in
            step += 1
            switch step {
            case 1:
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.url?.path, "/v1/offline/settlements/query")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, settlementPayload)
            case 2:
                XCTAssertEqual(request.httpMethod, "POST")
                XCTAssertEqual(request.url?.path, "/v1/offline/revocations/query")
                let body = try XCTUnwrap(self.requestBody(from: request))
                let envelope = try XCTUnwrap(JSONSerialization.jsonObject(with: body) as? [String: Any])
                let filter = try XCTUnwrap(envelope["filter"] as? [String: Any])
                let args = try XCTUnwrap(filter["args"] as? [Any])
                XCTAssertEqual(args.count, 2)
                XCTAssertEqual(args[0] as? String, "verdict_id_hex")
                XCTAssertEqual(args[1] as? String, "beef")
                let response = HTTPURLResponse(url: request.url!,
                                               statusCode: 200,
                                               httpVersion: nil,
                                               headerFields: ["Content-Type": "application/json"])!
                return (response, revocationPayload)
            default:
                throw URLError(.badServerResponse)
            }
        }

        let client = makeClient()
        let status = try await client.getSettlementStatus(bundleIdHex: "aa")
        XCTAssertEqual(status, .rejected(reason: "policy_violation"))
        XCTAssertEqual(step, 2)
    }

    func testGetSettlementStatusReturnsRejectedReasonFromTopLevelField() async throws {
        let accountId = "34mSYnDgbaJM58rbLoif4Tkp7G7pptR1KNF52GyuvUNd2XGP5NJ7ERtfk7Pbj5Fhtv2BW74vs"
        let settlementPayload = """
        {
          "items": [{
            "bundle_id_hex": "aa",
            "controller_id": "\(accountId)",
            "controller_display": "\(accountId)",
            "receiver_id": "\(accountId)",
            "receiver_display": "\(accountId)",
            "deposit_account_id": "\(accountId)",
            "deposit_account_display": "\(accountId)",
            "receipt_count": 1,
            "total_amount": "10",
            "claimed_delta": "10",
            "status": "rejected",
            "rejection_reason": "allowance_exceeded",
            "recorded_at_ms": 123,
            "recorded_at_height": 456,
            "transfer": {}
          }],
          "total": 1
        }
        """.data(using: .utf8)!
        var calls = 0

        OfflineStubURLProtocol.handler = { request in
            calls += 1
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.path, "/v1/offline/settlements/query")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, settlementPayload)
        }

        let client = makeClient()
        let status = try await client.getSettlementStatus(bundleIdHex: "aa")
        XCTAssertEqual(status, .rejected(reason: "allowance_exceeded"))
        XCTAssertEqual(calls, 1)
    }

    func testGetSettlementStatusReturnsUnknownForUnobservedBundle() async throws {
        let emptyListPayload = """
        {"items": [], "total": 0}
        """.data(using: .utf8)!
        var invoked = false

        OfflineStubURLProtocol.handler = { request in
            invoked = true
            XCTAssertEqual(request.httpMethod, "POST")
            XCTAssertEqual(request.url?.path, "/v1/offline/settlements/query")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, emptyListPayload)
        }

        let client = makeClient()
        let status = try await client.getSettlementStatus(bundleIdHex: "aa")
        XCTAssertEqual(status, .unknown)
        XCTAssertTrue(invoked)
    }
}
