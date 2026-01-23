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
        let renewPayload = """
        {
          "certificate_id_hex": "beadfeed",
          "certificate": {
            "controller": "\(accountId)",
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
                XCTAssertNil(certificate["operator_signature"])
                XCTAssertNotNil(certificate["allowance"] as? [String: Any])
                XCTAssertNotNil(certificate["attestation_report"] as? [Any])
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
}
