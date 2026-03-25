import XCTest
@testable import IrohaSwift

private final class OfflineCashStubURLProtocol: URLProtocol {
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
final class ToriiOfflineCashEndpointsTests: XCTestCase {
    private struct ExpectedRequest {
        let method: String
        let path: String
        let responseBody: Data
        let assertBody: (Data?) throws -> Void
    }

    override func tearDown() {
        OfflineCashStubURLProtocol.handler = nil
        super.tearDown()
    }

    func testOfflineCashEndpointsUseExpectedRoutesAndPayloads() async throws {
        let client = makeClient()
        let deviceBinding = ToriiOfflineDeviceBinding(
            platform: "ios",
            attestationKeyId: "attest-key",
            deviceId: "device-1",
            offlinePublicKey: "offline-public-key",
            attestationReportBase64: "YXR0ZXN0YXRpb24tcmVwb3J0",
            iosTeamId: "TEAMID1234",
            iosBundleId: "io.example.wallet",
            iosEnvironment: "development"
        )
        let deviceProof = ToriiOfflineDeviceProof(
            platform: "ios",
            attestationKeyId: "attest-key",
            challengeHashHex: "challenge-hash",
            assertionBase64: "YXNzZXJ0aW9u",
            counter: 7
        )
        let authorization = ToriiOfflineSpendAuthorization(
            authorizationId: "auth-1",
            lineageId: "lineage-1",
            accountId: "alice@hbl",
            deviceId: "device-1",
            offlinePublicKey: "offline-public-key",
            verdictId: "verdict-1",
            policyMaxBalance: "1000.00",
            policyMaxTxValue: "200.00",
            issuedAtMs: 1_700_000_000_000,
            refreshAtMs: 1_700_003_600_000,
            expiresAtMs: 1_700_086_400_000,
            appAttestKeyId: "attest-key",
            issuerSignatureBase64: "authorization-signature"
        )
        let lineageState = ToriiOfflineCashState(
            lineageId: "lineage-1",
            accountId: "alice@hbl",
            deviceId: "device-1",
            offlinePublicKey: "offline-public-key",
            assetDefinitionId: "xor#pk",
            balance: "97.50",
            lockedBalance: "0",
            serverRevision: 4,
            serverStateHash: "server-hash",
            pendingLocalRevision: 2,
            authorization: authorization,
            issuerSignatureBase64: "cash-signature"
        )
        let envelopeData = try JSONEncoder().encode(ToriiOfflineCashEnvelope(lineageState: lineageState))
        let revocationListData = Data(
            """
            {
              "items": [
                {
                  "verdict_id_hex": "verdict-2",
                  "issuer_id": "issuer@hbl",
                  "issuer_display": "issuer@hbl",
                  "revoked_at_ms": 1700000000000,
                  "reason": "device_compromised",
                  "note": "device lost",
                  "metadata": null,
                  "record": {}
                }
              ],
              "total": 1
            }
            """.utf8
        )
        let bundleData = try JSONEncoder().encode(
            ToriiOfflineRevocationBundle(
                issuedAtMs: 1_700_000_000_000,
                expiresAtMs: 1_700_003_600_000,
                verdictIds: ["verdict-2", "verdict-9"],
                issuerSignatureBase64: "bundle-signature"
            )
        )
        let receipt = ToriiOfflineTransferReceipt(
            transferId: "transfer-1",
            direction: .outgoing,
            lineageId: "lineage-1",
            accountId: "alice@hbl",
            deviceId: "device-1",
            offlinePublicKey: "offline-public-key",
            preBalance: "97.50",
            postBalance: "67.25",
            preLockedBalance: "0",
            postLockedBalance: "0",
            preStateHash: "pre-hash",
            postStateHash: "post-hash",
            localRevision: 3,
            counterpartyLineageId: "lineage-2",
            counterpartyAccountId: "bob@hbl",
            counterpartyDeviceId: "device-2",
            counterpartyOfflinePublicKey: "receiver-public-key",
            amount: "30.25",
            authorization: authorization,
            deviceProof: deviceProof,
            sourcePayload: nil,
            senderSignatureBase64: "sender-signature",
            createdAtMs: 1_700_000_123_456
        )

        var queue: [ExpectedRequest] = [
            ExpectedRequest(
                method: "POST",
                path: "/v1/offline/cash/setup",
                responseBody: envelopeData,
                assertBody: { body in
                    let request = try XCTUnwrap(body).decoded(ToriiOfflineCashSetupRequest.self)
                    XCTAssertEqual(request.accountId, "alice@hbl")
                    XCTAssertEqual(request.assetDefinitionId, "xor#pk")
                    XCTAssertEqual(request.deviceBinding, deviceBinding)
                    XCTAssertEqual(request.deviceProof, deviceProof)
                }
            ),
            ExpectedRequest(
                method: "POST",
                path: "/v1/offline/cash/load",
                responseBody: envelopeData,
                assertBody: { body in
                    let request = try XCTUnwrap(body).decoded(ToriiOfflineCashLoadRequest.self)
                    XCTAssertEqual(request.operationId, "load-1")
                    XCTAssertEqual(request.lineageId, "lineage-1")
                    XCTAssertEqual(request.assetDefinitionId, "xor#pk")
                    XCTAssertEqual(request.amount, "100.00")
                    XCTAssertEqual(request.deviceBinding, deviceBinding)
                    XCTAssertEqual(request.deviceProof, deviceProof)
                }
            ),
            ExpectedRequest(
                method: "POST",
                path: "/v1/offline/cash/refresh",
                responseBody: envelopeData,
                assertBody: { body in
                    let request = try XCTUnwrap(body).decoded(ToriiOfflineCashRefreshRequest.self)
                    XCTAssertEqual(request.operationId, "refresh-1")
                    XCTAssertEqual(request.lineageId, "lineage-1")
                    XCTAssertEqual(request.deviceBinding, deviceBinding)
                    XCTAssertEqual(request.deviceProof, deviceProof)
                }
            ),
            ExpectedRequest(
                method: "POST",
                path: "/v1/offline/cash/sync",
                responseBody: envelopeData,
                assertBody: { body in
                    let request = try XCTUnwrap(body).decoded(ToriiOfflineCashSyncRequest.self)
                    XCTAssertEqual(request.operationId, "sync-1")
                    XCTAssertEqual(request.deviceBinding, deviceBinding)
                    XCTAssertEqual(request.deviceProof, deviceProof)
                    XCTAssertEqual(request.receipts, [receipt])
                }
            ),
            ExpectedRequest(
                method: "POST",
                path: "/v1/offline/cash/redeem",
                responseBody: envelopeData,
                assertBody: { body in
                    let request = try XCTUnwrap(body).decoded(ToriiOfflineCashRedeemRequest.self)
                    XCTAssertEqual(request.operationId, "redeem-1")
                    XCTAssertEqual(request.deviceBinding, deviceBinding)
                    XCTAssertEqual(request.deviceProof, deviceProof)
                    XCTAssertEqual(request.amount, "30.00")
                    XCTAssertEqual(request.receipts, [receipt])
                }
            ),
            ExpectedRequest(
                method: "GET",
                path: "/v1/offline/revocations",
                responseBody: revocationListData,
                assertBody: { body in
                    XCTAssertNil(body)
                }
            ),
            ExpectedRequest(
                method: "GET",
                path: "/v1/offline/revocations/bundle",
                responseBody: bundleData,
                assertBody: { body in
                    XCTAssertNil(body)
                }
            )
        ]

        OfflineCashStubURLProtocol.handler = { [self] request in
            let expected = try XCTUnwrap(queue.isEmpty ? nil : queue.removeFirst())
            XCTAssertEqual(request.httpMethod, expected.method)
            XCTAssertEqual(request.url?.path, expected.path)
            if expected.method != "GET" {
                XCTAssertFalse((request.value(forHTTPHeaderField: "Idempotency-Key") ?? "").isEmpty)
            }
            try expected.assertBody(try self.requestBody(from: request))
            let response = try XCTUnwrap(
                HTTPURLResponse(
                    url: request.url!,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )
            )
            return (response, expected.responseBody)
        }

        let setup = try await client.setupOfflineCash(
            ToriiOfflineCashSetupRequest(
                accountId: "alice@hbl",
                assetDefinitionId: "xor#pk",
                deviceBinding: deviceBinding,
                deviceProof: deviceProof
            )
        )
        XCTAssertEqual(setup.lineageState.lineageId, "lineage-1")

        let topUp = try await client.loadOfflineCash(
            ToriiOfflineCashLoadRequest(
                operationId: "load-1",
                lineageId: "lineage-1",
                accountId: "alice@hbl",
                assetDefinitionId: "xor#pk",
                amount: "100.00",
                deviceBinding: deviceBinding,
                deviceProof: deviceProof
            )
        )
        XCTAssertEqual(topUp.lineageState.balance, "97.50")

        let renewed = try await client.refreshOfflineCash(
            ToriiOfflineCashRefreshRequest(
                operationId: "refresh-1",
                lineageId: "lineage-1",
                accountId: "alice@hbl",
                deviceBinding: deviceBinding,
                deviceProof: deviceProof
            )
        )
        XCTAssertEqual(renewed.lineageState.authorization.authorizationId, "auth-1")

        let synced = try await client.syncOfflineCash(
            ToriiOfflineCashSyncRequest(
                operationId: "sync-1",
                lineageId: "lineage-1",
                accountId: "alice@hbl",
                deviceBinding: deviceBinding,
                deviceProof: deviceProof,
                receipts: [receipt]
            )
        )
        XCTAssertEqual(synced.lineageState.serverStateHash, "server-hash")

        let defunded = try await client.redeemOfflineCash(
            ToriiOfflineCashRedeemRequest(
                operationId: "redeem-1",
                lineageId: "lineage-1",
                accountId: "alice@hbl",
                deviceBinding: deviceBinding,
                deviceProof: deviceProof,
                amount: "30.00",
                receipts: [receipt]
            )
        )
        XCTAssertEqual(defunded.lineageState.pendingLocalRevision, 2)

        let revocations = try await client.listOfflineRevocations()
        XCTAssertEqual(revocations.total, 1)
        XCTAssertEqual(revocations.items.map(\.verdictIdHex), ["verdict-2"])

        let bundle = try await client.getOfflineRevocationBundle()
        XCTAssertEqual(bundle.verdictIds, ["verdict-2", "verdict-9"])
        XCTAssertTrue(queue.isEmpty)
    }
}

@available(iOS 15.0, macOS 12.0, *)
private extension ToriiOfflineCashEndpointsTests {
    func makeClient() -> ToriiClient {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [OfflineCashStubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        return ToriiClient(baseURL: URL(string: "https://example.com")!, session: session)
    }

    func requestBody(from request: URLRequest) throws -> Data? {
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
}

private extension Data {
    func decoded<T: Decodable>(_ type: T.Type) throws -> T {
        try JSONDecoder().decode(type, from: self)
    }
}
