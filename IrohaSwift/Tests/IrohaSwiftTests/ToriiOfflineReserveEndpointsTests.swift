import XCTest
@testable import IrohaSwift

private final class OfflineReserveStubURLProtocol: URLProtocol {
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
final class ToriiOfflineReserveEndpointsTests: XCTestCase {
    private struct ExpectedRequest {
        let method: String
        let path: String
        let responseBody: Data
        let assertBody: (Data?) throws -> Void
    }

    override func tearDown() {
        OfflineReserveStubURLProtocol.handler = nil
        super.tearDown()
    }

    func testOfflineReserveEndpointsUseExpectedRoutesAndPayloads() async throws {
        let client = makeClient()
        let attestation = ToriiOfflineDeviceAttestation(
            keyId: "attest-key",
            counter: 7,
            assertionBase64: "YXNzZXJ0aW9u",
            challengeHashHex: "challenge-hash"
        )
        let authorization = ToriiOfflineSpendAuthorization(
            authorizationId: "auth-1",
            reserveId: "reserve-1",
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
        let reserveState = ToriiOfflineReserveState(
            reserveId: "reserve-1",
            accountId: "alice@hbl",
            deviceId: "device-1",
            offlinePublicKey: "offline-public-key",
            assetDefinitionId: "xor#pk",
            balance: "97.50",
            parkedBalance: "0",
            serverRevision: 4,
            serverStateHash: "server-hash",
            pendingLocalRevision: 2,
            authorization: authorization,
            issuerSignatureBase64: "reserve-signature"
        )
        let envelopeData = try JSONEncoder().encode(ToriiOfflineReserveEnvelope(reserveState: reserveState))
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
            reserveId: "reserve-1",
            accountId: "alice@hbl",
            deviceId: "device-1",
            offlinePublicKey: "offline-public-key",
            preBalance: "97.50",
            postBalance: "67.25",
            preParkedBalance: "0",
            postParkedBalance: "0",
            preStateHash: "pre-hash",
            postStateHash: "post-hash",
            localRevision: 3,
            counterpartyReserveId: "reserve-2",
            counterpartyAccountId: "bob@hbl",
            counterpartyDeviceId: "device-2",
            counterpartyOfflinePublicKey: "receiver-public-key",
            amount: "30.25",
            authorization: authorization,
            attestation: attestation,
            sourcePayload: nil,
            senderSignatureBase64: "sender-signature",
            createdAtMs: 1_700_000_123_456
        )

        var queue: [ExpectedRequest] = [
            ExpectedRequest(
                method: "POST",
                path: "/v1/offline/reserve/setup",
                responseBody: envelopeData,
                assertBody: { body in
                    let request = try XCTUnwrap(body).decoded(ToriiOfflineReserveSetupRequest.self)
                    XCTAssertEqual(request.accountId, "alice@hbl")
                    XCTAssertEqual(request.deviceId, "device-1")
                    XCTAssertEqual(request.offlinePublicKey, "offline-public-key")
                    XCTAssertEqual(request.assetDefinitionId, "xor#pk")
                    XCTAssertEqual(request.appAttestKeyId, "attest-key")
                }
            ),
            ExpectedRequest(
                method: "POST",
                path: "/v1/offline/reserve/topup",
                responseBody: envelopeData,
                assertBody: { body in
                    let request = try XCTUnwrap(body).decoded(ToriiOfflineReserveTopUpRequest.self)
                    XCTAssertEqual(request.operationId, "topup-1")
                    XCTAssertEqual(request.reserveId, "reserve-1")
                    XCTAssertEqual(request.assetDefinitionId, "xor#pk")
                    XCTAssertEqual(request.amount, "100.00")
                }
            ),
            ExpectedRequest(
                method: "POST",
                path: "/v1/offline/reserve/renew",
                responseBody: envelopeData,
                assertBody: { body in
                    let request = try XCTUnwrap(body).decoded(ToriiOfflineReserveRenewRequest.self)
                    XCTAssertEqual(request.operationId, "renew-1")
                    XCTAssertEqual(request.reserveId, "reserve-1")
                }
            ),
            ExpectedRequest(
                method: "POST",
                path: "/v1/offline/reserve/sync",
                responseBody: envelopeData,
                assertBody: { body in
                    let request = try XCTUnwrap(body).decoded(ToriiOfflineReserveSyncRequest.self)
                    XCTAssertEqual(request.operationId, "sync-1")
                    XCTAssertEqual(request.receipts, [receipt])
                }
            ),
            ExpectedRequest(
                method: "POST",
                path: "/v1/offline/reserve/defund",
                responseBody: envelopeData,
                assertBody: { body in
                    let request = try XCTUnwrap(body).decoded(ToriiOfflineReserveDefundRequest.self)
                    XCTAssertEqual(request.operationId, "defund-1")
                    XCTAssertEqual(request.amount, "30.00")
                    XCTAssertEqual(request.receipts, [receipt])
                }
            ),
            ExpectedRequest(
                method: "GET",
                path: "/v1/offline/revocations",
                responseBody: bundleData,
                assertBody: { body in
                    XCTAssertNil(body)
                }
            )
        ]

        OfflineReserveStubURLProtocol.handler = { [self] request in
            let expected = try XCTUnwrap(queue.isEmpty ? nil : queue.removeFirst())
            XCTAssertEqual(request.httpMethod, expected.method)
            XCTAssertEqual(request.url?.path, expected.path)
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

        let setup = try await client.setupOfflineReserve(
            ToriiOfflineReserveSetupRequest(
                accountId: "alice@hbl",
                deviceId: "device-1",
                offlinePublicKey: "offline-public-key",
                assetDefinitionId: "xor#pk",
                appAttestKeyId: "attest-key",
                attestation: attestation
            )
        )
        XCTAssertEqual(setup.reserveState.reserveId, "reserve-1")

        let topUp = try await client.topUpOfflineReserve(
            ToriiOfflineReserveTopUpRequest(
                operationId: "topup-1",
                reserveId: "reserve-1",
                accountId: "alice@hbl",
                deviceId: "device-1",
                offlinePublicKey: "offline-public-key",
                assetDefinitionId: "xor#pk",
                appAttestKeyId: "attest-key",
                amount: "100.00",
                attestation: attestation
            )
        )
        XCTAssertEqual(topUp.reserveState.balance, "97.50")

        let renewed = try await client.renewOfflineReserve(
            ToriiOfflineReserveRenewRequest(
                operationId: "renew-1",
                reserveId: "reserve-1",
                accountId: "alice@hbl",
                deviceId: "device-1",
                offlinePublicKey: "offline-public-key",
                appAttestKeyId: "attest-key",
                attestation: attestation
            )
        )
        XCTAssertEqual(renewed.reserveState.authorization.authorizationId, "auth-1")

        let synced = try await client.syncOfflineReserve(
            ToriiOfflineReserveSyncRequest(
                operationId: "sync-1",
                reserveId: "reserve-1",
                accountId: "alice@hbl",
                deviceId: "device-1",
                offlinePublicKey: "offline-public-key",
                receipts: [receipt]
            )
        )
        XCTAssertEqual(synced.reserveState.serverStateHash, "server-hash")

        let defunded = try await client.defundOfflineReserve(
            ToriiOfflineReserveDefundRequest(
                operationId: "defund-1",
                reserveId: "reserve-1",
                accountId: "alice@hbl",
                deviceId: "device-1",
                offlinePublicKey: "offline-public-key",
                amount: "30.00",
                receipts: [receipt]
            )
        )
        XCTAssertEqual(defunded.reserveState.pendingLocalRevision, 2)

        let bundle = try await client.getOfflineRevocationBundle()
        XCTAssertEqual(bundle.verdictIds, ["verdict-2", "verdict-9"])
        XCTAssertTrue(queue.isEmpty)
    }
}

@available(iOS 15.0, macOS 12.0, *)
private extension ToriiOfflineReserveEndpointsTests {
    func makeClient() -> ToriiClient {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [OfflineReserveStubURLProtocol.self]
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
