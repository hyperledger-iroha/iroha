import CryptoKit
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

    func testOfflineCashEndpointsPostRequestsAndKeepRevocationReads() async throws {
        let client = makeClient()
        let aliceId = "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"
        let bobId = "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
        let assetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
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
            accountId: aliceId,
            verdictId: "verdict-1",
            policyMaxBalance: "1000.00",
            policyMaxTxValue: "200.00",
            issuedAtMs: 1_700_000_000_000,
            refreshAtMs: 1_700_003_600_000,
            expiresAtMs: 1_700_086_400_000,
            deviceBinding: deviceBinding,
            issuerSignatureBase64: "authorization-signature"
        )
        let cashState = ToriiOfflineCashState(
            lineageId: "lineage-1",
            accountId: aliceId,
            deviceId: "device-1",
            offlinePublicKey: "offline-public-key",
            assetDefinitionId: assetDefinitionId,
            balance: "97.50",
            lockedBalance: "0",
            serverRevision: 4,
            serverStateHash: String(repeating: "a", count: 64),
            pendingLocalRevision: 2,
            authorization: authorization,
            issuerSignatureBase64: "cash-signature"
        )
        let cashEnvelope = ToriiOfflineCashEnvelope(lineageState: cashState)
        let cashEnvelopeData = try JSONEncoder().encode(cashEnvelope)
        let revocationListData = Data(
            """
            {
              "items": [
                {
                  "verdict_id_hex": "verdict-2",
                  "issuer_id": "\(bobId)",
                  "issuer_display": "\(bobId)",
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
            accountId: aliceId,
            deviceId: "device-1",
            offlinePublicKey: "offline-public-key",
            preBalance: "97.50",
            postBalance: "67.25",
            preLockedBalance: "0",
            postLockedBalance: "0",
            preStateHash: String(repeating: "b", count: 64),
            postStateHash: String(repeating: "c", count: 64),
            localRevision: 3,
            counterpartyLineageId: "lineage-2",
            counterpartyAccountId: bobId,
            counterpartyDeviceId: "device-2",
            counterpartyOfflinePublicKey: "receiver-public-key",
            amount: "30.25",
            authorization: authorization,
            deviceProof: deviceProof,
            sourcePayload: nil,
            senderSignatureBase64: "sender-signature",
            createdAtMs: 1_700_000_123_456
        )
        let setupRequest = ToriiOfflineCashSetupRequest(
            accountId: aliceId,
            assetDefinitionId: assetDefinitionId,
            deviceBinding: deviceBinding,
            deviceProof: deviceProof
        )
        let loadRequest = ToriiOfflineCashLoadRequest(
            operationId: "load-1",
            lineageId: "lineage-1",
            accountId: aliceId,
            assetDefinitionId: assetDefinitionId,
            amount: "100.00",
            deviceBinding: deviceBinding,
            deviceProof: deviceProof
        )
        let refreshRequest = ToriiOfflineCashRefreshRequest(
            operationId: "refresh-1",
            lineageId: "lineage-1",
            accountId: aliceId,
            deviceBinding: deviceBinding,
            deviceProof: deviceProof
        )
        let syncRequest = ToriiOfflineCashSyncRequest(
            operationId: "sync-1",
            lineageId: "lineage-1",
            accountId: aliceId,
            deviceBinding: deviceBinding,
            deviceProof: deviceProof,
            receipts: [receipt]
        )
        let redeemProof = try ToriiOfflineSettlementProofs.buildRedeemRequestProof(
            operationId: "redeem-1",
            accountId: aliceId,
            lineageId: "lineage-1",
            assetDefinitionId: assetDefinitionId,
            amount: "30.00",
            offlinePublicKey: deviceBinding.offlinePublicKey,
            authorizationId: authorization.authorizationId,
            preStateHash: String(repeating: "a", count: 64),
            receipts: [receipt]
        )
        let redeemRequest = ToriiOfflineCashRedeemRequest(
            operationId: "redeem-1",
            lineageId: "lineage-1",
            accountId: aliceId,
            deviceBinding: deviceBinding,
            deviceProof: deviceProof,
            amount: "30.00",
            receipts: [receipt],
            redeemProof: redeemProof
        )

        var queue: [ExpectedRequest] = [
            ExpectedRequest(
                method: "POST",
                path: "/v1/offline/cash/setup",
                responseBody: cashEnvelopeData,
                assertBody: { body in
                    let payload = try XCTUnwrap(body).decoded(ToriiOfflineCashSetupRequest.self)
                    XCTAssertEqual(payload, setupRequest)
                }
            ),
            ExpectedRequest(
                method: "POST",
                path: "/v1/offline/cash/load",
                responseBody: cashEnvelopeData,
                assertBody: { body in
                    let payload = try XCTUnwrap(body).decoded(ToriiOfflineCashLoadRequest.self)
                    XCTAssertEqual(payload, loadRequest)
                }
            ),
            ExpectedRequest(
                method: "POST",
                path: "/v1/offline/cash/refresh",
                responseBody: cashEnvelopeData,
                assertBody: { body in
                    let payload = try XCTUnwrap(body).decoded(ToriiOfflineCashRefreshRequest.self)
                    XCTAssertEqual(payload, refreshRequest)
                }
            ),
            ExpectedRequest(
                method: "POST",
                path: "/v1/offline/cash/sync",
                responseBody: cashEnvelopeData,
                assertBody: { body in
                    let payload = try XCTUnwrap(body).decoded(ToriiOfflineCashSyncRequest.self)
                    XCTAssertEqual(payload, syncRequest)
                }
            ),
            ExpectedRequest(
                method: "POST",
                path: "/v1/offline/cash/redeem",
                responseBody: cashEnvelopeData,
                assertBody: { body in
                    let payload = try XCTUnwrap(body).decoded(ToriiOfflineCashRedeemRequest.self)
                    XCTAssertEqual(payload, redeemRequest)
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
                let requestBody = try self.requestBody(from: request)
                XCTAssertEqual(
                    request.value(forHTTPHeaderField: "Content-Type"),
                    "application/json"
                )
                XCTAssertFalse((request.value(forHTTPHeaderField: "X-Request-Id") ?? "").isEmpty)
                XCTAssertEqual(
                    request.value(forHTTPHeaderField: "Idempotency-Key"),
                    try expectedIdempotencyKey(for: expected.path, requestBody: requestBody)
                )
                try expected.assertBody(requestBody)
            } else {
                try expected.assertBody(try self.requestBody(from: request))
            }
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

        let setupEnvelope = try await client.setupOfflineCash(setupRequest)
        let loadEnvelope = try await client.loadOfflineCash(loadRequest)
        let refreshEnvelope = try await client.refreshOfflineCash(refreshRequest)
        let syncEnvelope = try await client.syncOfflineCash(syncRequest)
        let redeemEnvelope = try await client.redeemOfflineCash(redeemRequest)

        XCTAssertEqual(setupEnvelope, cashEnvelope)
        XCTAssertEqual(loadEnvelope, cashEnvelope)
        XCTAssertEqual(refreshEnvelope, cashEnvelope)
        XCTAssertEqual(syncEnvelope, cashEnvelope)
        XCTAssertEqual(redeemEnvelope, cashEnvelope)

        let revocations = try await client.listOfflineRevocations()
        XCTAssertEqual(revocations.total, 1)
        XCTAssertEqual(revocations.items.map(\.verdictIdHex), ["verdict-2"])

        let bundle = try await client.getOfflineRevocationBundle()
        XCTAssertEqual(bundle.verdictIds, ["verdict-2", "verdict-9"])
        XCTAssertTrue(queue.isEmpty)
    }

    func testOfflineCashPostHeadersUseStableIdempotencyKeysAndUniqueRequestIds() async throws {
        let client = makeClient()
        let aliceId = "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"
        let bobId = "sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D"
        let assetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
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
        let alternateDeviceBinding = ToriiOfflineDeviceBinding(
            platform: "ios",
            attestationKeyId: "attest-key",
            deviceId: "device-2",
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
        let alternateDeviceProof = ToriiOfflineDeviceProof(
            platform: "ios",
            attestationKeyId: "attest-key",
            challengeHashHex: "challenge-hash-2",
            assertionBase64: "YXNzZXJ0aW9uLTI=",
            counter: 9
        )
        let authorization = ToriiOfflineSpendAuthorization(
            authorizationId: "auth-1",
            lineageId: "lineage-1",
            accountId: aliceId,
            verdictId: "verdict-1",
            policyMaxBalance: "1000.00",
            policyMaxTxValue: "200.00",
            issuedAtMs: 1_700_000_000_000,
            refreshAtMs: 1_700_003_600_000,
            expiresAtMs: 1_700_086_400_000,
            deviceBinding: deviceBinding,
            issuerSignatureBase64: "authorization-signature"
        )
        let cashState = ToriiOfflineCashState(
            lineageId: "lineage-1",
            accountId: aliceId,
            deviceId: "device-1",
            offlinePublicKey: "offline-public-key",
            assetDefinitionId: assetDefinitionId,
            balance: "97.50",
            lockedBalance: "0",
            serverRevision: 4,
            serverStateHash: String(repeating: "a", count: 64),
            pendingLocalRevision: 2,
            authorization: authorization,
            issuerSignatureBase64: "cash-signature"
        )
        let responseBody = try JSONEncoder().encode(ToriiOfflineCashEnvelope(lineageState: cashState))
        let receiptA = ToriiOfflineTransferReceipt(
            transferId: "transfer-1",
            direction: .outgoing,
            lineageId: "lineage-1",
            accountId: aliceId,
            deviceId: "device-1",
            offlinePublicKey: "offline-public-key",
            preBalance: "97.50",
            postBalance: "67.25",
            preLockedBalance: "0",
            postLockedBalance: "0",
            preStateHash: String(repeating: "b", count: 64),
            postStateHash: String(repeating: "c", count: 64),
            localRevision: 3,
            counterpartyLineageId: "lineage-2",
            counterpartyAccountId: bobId,
            counterpartyDeviceId: "device-2",
            counterpartyOfflinePublicKey: "receiver-public-key",
            amount: "30.25",
            authorization: authorization,
            deviceProof: deviceProof,
            sourcePayload: nil,
            senderSignatureBase64: "sender-signature",
            createdAtMs: 1_700_000_123_456
        )
        let receiptB = ToriiOfflineTransferReceipt(
            transferId: "transfer-2",
            direction: .incoming,
            lineageId: "lineage-1",
            accountId: aliceId,
            deviceId: "device-1",
            offlinePublicKey: "offline-public-key",
            preBalance: "67.25",
            postBalance: "77.25",
            preLockedBalance: "0",
            postLockedBalance: "0",
            preStateHash: String(repeating: "d", count: 64),
            postStateHash: String(repeating: "e", count: 64),
            localRevision: 4,
            counterpartyLineageId: "lineage-3",
            counterpartyAccountId: bobId,
            counterpartyDeviceId: "device-3",
            counterpartyOfflinePublicKey: "receiver-public-key-2",
            amount: "10.00",
            authorization: authorization,
            deviceProof: alternateDeviceProof,
            sourcePayload: nil,
            senderSignatureBase64: "sender-signature-2",
            createdAtMs: 1_700_000_223_456
        )

        let setupRequest = ToriiOfflineCashSetupRequest(
            accountId: aliceId,
            assetDefinitionId: assetDefinitionId,
            deviceBinding: deviceBinding,
            deviceProof: deviceProof
        )
        let changedSetupRequest = ToriiOfflineCashSetupRequest(
            accountId: aliceId,
            assetDefinitionId: assetDefinitionId,
            deviceBinding: alternateDeviceBinding,
            deviceProof: deviceProof
        )
        let loadRequestA = ToriiOfflineCashLoadRequest(
            operationId: "load-stable",
            lineageId: "lineage-1",
            accountId: aliceId,
            assetDefinitionId: assetDefinitionId,
            amount: "100.00",
            deviceBinding: deviceBinding,
            deviceProof: deviceProof
        )
        let loadRequestB = ToriiOfflineCashLoadRequest(
            operationId: "load-stable",
            lineageId: "lineage-1",
            accountId: aliceId,
            assetDefinitionId: assetDefinitionId,
            amount: "250.00",
            deviceBinding: alternateDeviceBinding,
            deviceProof: alternateDeviceProof
        )
        let refreshRequestA = ToriiOfflineCashRefreshRequest(
            operationId: "refresh-stable",
            lineageId: "lineage-1",
            accountId: aliceId,
            deviceBinding: deviceBinding,
            deviceProof: deviceProof
        )
        let refreshRequestB = ToriiOfflineCashRefreshRequest(
            operationId: "refresh-stable",
            lineageId: "lineage-2",
            accountId: aliceId,
            deviceBinding: alternateDeviceBinding,
            deviceProof: alternateDeviceProof
        )
        let syncRequestA = ToriiOfflineCashSyncRequest(
            operationId: "sync-stable",
            lineageId: "lineage-1",
            accountId: aliceId,
            deviceBinding: deviceBinding,
            deviceProof: deviceProof,
            receipts: [receiptA]
        )
        let syncRequestB = ToriiOfflineCashSyncRequest(
            operationId: "sync-stable",
            lineageId: "lineage-1",
            accountId: aliceId,
            deviceBinding: deviceBinding,
            deviceProof: alternateDeviceProof,
            receipts: [receiptA, receiptB]
        )
        let redeemProofA = try ToriiOfflineSettlementProofs.buildRedeemRequestProof(
            operationId: "redeem-stable",
            accountId: aliceId,
            lineageId: "lineage-1",
            assetDefinitionId: assetDefinitionId,
            amount: "30.00",
            offlinePublicKey: deviceBinding.offlinePublicKey,
            authorizationId: authorization.authorizationId,
            preStateHash: String(repeating: "a", count: 64),
            receipts: [receiptA]
        )
        let redeemProofB = try ToriiOfflineSettlementProofs.buildRedeemRequestProof(
            operationId: "redeem-stable",
            accountId: aliceId,
            lineageId: "lineage-1",
            assetDefinitionId: assetDefinitionId,
            amount: "40.00",
            offlinePublicKey: deviceBinding.offlinePublicKey,
            authorizationId: authorization.authorizationId,
            preStateHash: String(repeating: "f", count: 64),
            receipts: [receiptA, receiptB]
        )
        let redeemRequestA = ToriiOfflineCashRedeemRequest(
            operationId: "redeem-stable",
            lineageId: "lineage-1",
            accountId: aliceId,
            deviceBinding: deviceBinding,
            deviceProof: deviceProof,
            amount: "30.00",
            receipts: [receiptA],
            redeemProof: redeemProofA
        )
        let redeemRequestB = ToriiOfflineCashRedeemRequest(
            operationId: "redeem-stable",
            lineageId: "lineage-1",
            accountId: aliceId,
            deviceBinding: alternateDeviceBinding,
            deviceProof: alternateDeviceProof,
            amount: "40.00",
            receipts: [receiptA, receiptB],
            redeemProof: redeemProofB
        )

        let observations = OfflineCashRequestObservationStore()
        OfflineCashStubURLProtocol.handler = { request in
            let url = try XCTUnwrap(request.url)
            let idempotencyKey = try XCTUnwrap(request.value(forHTTPHeaderField: "Idempotency-Key"))
            let requestId = try XCTUnwrap(request.value(forHTTPHeaderField: "X-Request-Id"))
            observations.append(path: url.path, requestId: requestId, idempotencyKey: idempotencyKey)

            let response = try XCTUnwrap(
                HTTPURLResponse(
                    url: url,
                    statusCode: 200,
                    httpVersion: nil,
                    headerFields: ["Content-Type": "application/json"]
                )
            )
            return (response, responseBody)
        }

        _ = try await client.setupOfflineCash(setupRequest)
        _ = try await client.setupOfflineCash(setupRequest)
        _ = try await client.setupOfflineCash(changedSetupRequest)
        _ = try await client.loadOfflineCash(loadRequestA)
        _ = try await client.loadOfflineCash(loadRequestB)
        _ = try await client.refreshOfflineCash(refreshRequestA)
        _ = try await client.refreshOfflineCash(refreshRequestB)
        _ = try await client.syncOfflineCash(syncRequestA)
        _ = try await client.syncOfflineCash(syncRequestB)
        _ = try await client.redeemOfflineCash(redeemRequestA)
        _ = try await client.redeemOfflineCash(redeemRequestB)

        let headersByPath = observations.idempotencyKeysByPath()
        let setupHeaders = try XCTUnwrap(headersByPath["/v1/offline/cash/setup"])
        XCTAssertEqual(setupHeaders.count, 3)
        XCTAssertEqual(setupHeaders[0], setupHeaders[1])
        XCTAssertNotEqual(setupHeaders[1], setupHeaders[2])

        let loadHeaders = try XCTUnwrap(headersByPath["/v1/offline/cash/load"])
        XCTAssertEqual(loadHeaders, ["offline-cash:load-stable", "offline-cash:load-stable"])

        let refreshHeaders = try XCTUnwrap(headersByPath["/v1/offline/cash/refresh"])
        XCTAssertEqual(refreshHeaders, ["offline-cash:refresh-stable", "offline-cash:refresh-stable"])

        let syncHeaders = try XCTUnwrap(headersByPath["/v1/offline/cash/sync"])
        XCTAssertEqual(syncHeaders, ["offline-cash:sync-stable", "offline-cash:sync-stable"])

        let redeemHeaders = try XCTUnwrap(headersByPath["/v1/offline/cash/redeem"])
        XCTAssertEqual(redeemHeaders, ["offline-cash:redeem-stable", "offline-cash:redeem-stable"])

        let requestIds = observations.requestIds()
        XCTAssertEqual(requestIds.count, 11)
        XCTAssertEqual(Set(requestIds).count, requestIds.count)
        XCTAssertTrue(requestIds.allSatisfy { !$0.isEmpty })
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

    func expectedIdempotencyKey(
        for path: String,
        requestBody: Data?
    ) throws -> String {
        switch path {
        case "/v1/offline/cash/load":
            return "offline-cash:load-1"
        case "/v1/offline/cash/refresh":
            return "offline-cash:refresh-1"
        case "/v1/offline/cash/sync":
            return "offline-cash:sync-1"
        case "/v1/offline/cash/redeem":
            return "offline-cash:redeem-1"
        case "/v1/offline/cash/setup":
            let body = try XCTUnwrap(requestBody)
            var digestInput = Data(path.utf8)
            digestInput.append(0)
            digestInput.append(body)
            let digest = SHA256.hash(data: digestInput)
            let hexDigest = digest.map { String(format: "%02x", $0) }.joined()
            return "offline-cash:setup:\(hexDigest)"
        default:
            XCTFail("Unexpected offline cash path: \(path)")
            return ""
        }
    }
}

private extension Data {
    func decoded<T: Decodable>(_ type: T.Type) throws -> T {
        try JSONDecoder().decode(type, from: self)
    }
}

private final class OfflineCashRequestObservationStore {
    private let queue = DispatchQueue(label: "OfflineCashRequestObservationStore.queue")
    private var observations: [(path: String, requestId: String, idempotencyKey: String)] = []

    func append(path: String, requestId: String, idempotencyKey: String) {
        queue.sync {
            observations.append((path, requestId, idempotencyKey))
        }
    }

    func idempotencyKeysByPath() -> [String: [String]] {
        queue.sync {
            observations.reduce(into: [:]) { partialResult, observation in
                partialResult[observation.path, default: []].append(observation.idempotencyKey)
            }
        }
    }

    func requestIds() -> [String] {
        queue.sync {
            observations.map(\.requestId)
        }
    }
}
