import XCTest
@testable import IrohaSwift

private final class OfflineWalletStubURLProtocol: URLProtocol {
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
final class OfflineWalletRevocationTests: XCTestCase {
    override func tearDown() {
        OfflineWalletStubURLProtocol.handler = nil
        super.tearDown()
    }

    func testRecordRevocationsUpdatesWalletDenyList() throws {
        let verdictHex = String(repeating: "c", count: 64)
        let issuerId = try sampleIssuerId(seed: 0xAB)
        let list = try makeRevocationList(verdictHex: verdictHex, issuerId: issuerId)
        let wallet = try makeWallet(client: ToriiClient(baseURL: URL(string: "https://example.invalid")!))
        let inserted = try wallet.recordRevocations(from: list, recordedAt: 999)
        XCTAssertEqual(inserted.count, 1)
        XCTAssertTrue(wallet.isVerdictRevoked(verdictIdHex: verdictHex.uppercased()))
        let entry = try XCTUnwrap(wallet.revocation(for: verdictHex))
        XCTAssertEqual(entry.revokedAtMs, 777)
        XCTAssertEqual(entry.recordedAtMs, 999)
    }

    func testSyncOfflineStatePopulatesRevocations() async throws {
        let verdictHex = String(repeating: "d", count: 64)
        let hashLiteral = try SocialKeyedHash(pepperId: "pepper", digest: verdictHex).digest
        let issuerId = try sampleIssuerId(seed: 0xCD)
        let statePayload = """
        {
          "allowances": [],
          "transfers": [],
          "summaries": [],
          "revocations": [{
            "verdict_id_hex": "\(hashLiteral)",
            "issuer_id": "\(issuerId)",
            "revoked_at_ms": 321,
            "reason": "compromised"
          }],
          "now_ms": 555
        }
        """.data(using: .utf8)!

        OfflineWalletStubURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "GET")
            XCTAssertEqual(request.url?.path, "/v1/offline/state")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, statePayload)
        }

        let client = makeStubClient()
        let wallet = try makeWallet(client: client)
        let state = try await wallet.syncOfflineState()
        XCTAssertEqual(state.nowMs, 555)
        XCTAssertTrue(wallet.isVerdictRevoked(verdictIdHex: verdictHex))
        let entry = try XCTUnwrap(wallet.revocation(for: verdictHex))
        XCTAssertEqual(entry.recordedAtMs, 555)
    }

    func testSyncOfflineStatePopulatesVerdictsFromAllowances() async throws {
        let certificate = try normalizedCertificate(OfflineWalletCertificate.load(from: fixtureURL("certificate.json")))
        let certificateIdHex = try certificate.certificateIdHex().lowercased()
        let verdictHex = String(repeating: "b", count: 64)
        let verdictLiteral = try SocialKeyedHash(pepperId: "pepper", digest: verdictHex).digest
        let record = try makeRawAllowanceRecord(certificate: certificate,
                                                verdictLiteral: verdictLiteral,
                                                remainingAmount: "99")
        let state: [String: Any] = [
            "allowances": [record],
            "transfers": [],
            "summaries": [],
            "revocations": [],
            "now_ms": NSNumber(value: 123)
        ]
        let statePayload = try JSONSerialization.data(withJSONObject: state, options: [.sortedKeys])

        OfflineWalletStubURLProtocol.handler = { request in
            XCTAssertEqual(request.httpMethod, "GET")
            XCTAssertEqual(request.url?.path, "/v1/offline/state")
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, statePayload)
        }

        let client = makeStubClient()
        let wallet = try makeWallet(client: client)
        _ = try await wallet.syncOfflineState()
        let metadata = try XCTUnwrap(wallet.verdictMetadata(for: certificateIdHex))
        XCTAssertEqual(metadata.verdictIdHex, verdictHex)
        XCTAssertEqual(metadata.attestationNonceHex, verdictHex)
        XCTAssertEqual(metadata.remainingAmount, "99")
        XCTAssertEqual(metadata.recordedAtMs, 123)
    }

    // MARK: - Helpers

    private func makeStubClient() -> ToriiClient {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [OfflineWalletStubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        return ToriiClient(baseURL: URL(string: "https://example.com")!, session: session)
    }

    private func makeWallet(client: ToriiClient) throws -> OfflineWallet {
        let auditURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: false)
        let verdictURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: false)
        let revocationURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: false)
        let counterURL = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: false)
        let verdictJournal = try OfflineVerdictJournal(storageURL: verdictURL)
        let revocationJournal = try OfflineRevocationJournal(storageURL: revocationURL)
        let counterJournal = try OfflineCounterJournal(storageURL: counterURL)
        return try OfflineWallet(
            toriiClient: client,
            auditLoggingEnabled: false,
            auditStorageURL: auditURL,
            auditLogger: nil,
            circulationMode: .ledgerReconcilable,
            onCirculationModeChange: nil,
            verdictJournal: verdictJournal,
            revocationJournal: revocationJournal,
            counterJournal: counterJournal
        )
    }

    private func makeRevocationList(verdictHex: String, issuerId: String) throws -> ToriiOfflineRevocationList {
        let hashLiteral = try SocialKeyedHash(pepperId: "pepper", digest: verdictHex).digest
        let record: [String: Any] = [
            "verdict_id_hex": hashLiteral,
            "issuer_id": issuerId,
            "revoked_at_ms": NSNumber(value: 777),
            "reason": "compromised"
        ]
        let item: [String: Any] = [
            "verdict_id_hex": verdictHex,
            "issuer_id": issuerId,
            "issuer_display": "Alice",
            "revoked_at_ms": NSNumber(value: 777),
            "reason": "compromised",
            "record": record
        ]
        let root: [String: Any] = [
            "items": [item],
            "total": NSNumber(value: 1)
        ]
        let data = try JSONSerialization.data(withJSONObject: root, options: [.sortedKeys])
        return try JSONDecoder().decode(ToriiOfflineRevocationList.self, from: data)
    }

    private func sampleIssuerId(seed: UInt8) throws -> String {
        let publicKey = Data(repeating: seed, count: 32)
        return try AccountId.makeIH58(publicKey: publicKey, domain: "wonderland")
    }

    private func fixtureURL(_ name: String) -> URL {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 {
            url.deleteLastPathComponent()
        }
        return url.appendingPathComponent("fixtures/offline_allowance/ios-demo/\(name)")
    }

    private func makeRawAllowanceRecord(certificate: OfflineWalletCertificate,
                                        verdictLiteral: String,
                                        remainingAmount: String) throws -> [String: Any] {
        let normalized = try normalizedCertificate(certificate)
        let certificateData = try JSONEncoder().encode(normalized)
        guard let certificateObject = try JSONSerialization.jsonObject(with: certificateData) as? [String: Any] else {
            throw XCTSkip("certificate fixture failed to encode as JSON object")
        }
        return [
            "certificate": certificateObject,
            "current_commitment": [NSNumber(value: 1), NSNumber(value: 2), NSNumber(value: 3)],
            "registered_at_ms": NSNumber(value: 123),
            "remaining_amount": remainingAmount,
            "counter_state": [
                "apple_key_counters": [:],
                "android_series_counters": [:]
            ],
            "verdict_id_hex": verdictLiteral,
            "attestation_nonce_hex": verdictLiteral,
            "refresh_at_ms": NSNumber(value: 777)
        ]
    }

    private func normalizedCertificate(_ certificate: OfflineWalletCertificate) throws -> OfflineWalletCertificate {
        let allowance = OfflineAllowanceCommitment(
            assetId: try makeNoritoAssetId(name: "xor", domain: "sora", accountId: certificate.controller),
            amount: certificate.allowance.amount,
            commitment: certificate.allowance.commitment
        )
        return OfflineWalletCertificate(
            controller: certificate.controller,
            operatorId: certificate.operatorId,
            allowance: allowance,
            spendPublicKey: certificate.spendPublicKey,
            attestationReport: certificate.attestationReport,
            issuedAtMs: certificate.issuedAtMs,
            expiresAtMs: certificate.expiresAtMs,
            policy: certificate.policy,
            operatorSignature: certificate.operatorSignature,
            metadata: certificate.metadata,
            verdictId: certificate.verdictId,
            attestationNonce: certificate.attestationNonce,
            refreshAtMs: certificate.refreshAtMs
        )
    }

    private func makeNoritoAssetId(name: String,
                                   domain: String,
                                   accountId: String) throws -> String {
        var writer = OfflineNoritoWriter()
        writer.writeField(try OfflineNorito.encodeAssetDefinitionId(name: name, domain: domain))
        writer.writeField(try OfflineNorito.encodeAccountId(accountId))
        return "norito:\(writer.data.hexLowercased())"
    }
}
