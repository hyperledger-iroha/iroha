import XCTest
@testable import IrohaSwift

private final class OfflineWalletTopUpStubURLProtocol: URLProtocol {
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
final class OfflineWalletTopUpTests: XCTestCase {
    override func tearDown() {
        OfflineWalletTopUpStubURLProtocol.handler = nil
        super.tearDown()
    }

    func testTopUpAllowanceRecordsVerdictMetadata() async throws {
        let certificate = try makeCertificate()
        let certificateIdHex = try certificate.certificateIdHex().lowercased()
        let draft = makeDraft(from: certificate)
        let issuePayload = try makeIssuePayload(certificate: certificate)
        let registerPayload = try makeRegisterPayload(certificateIdHex: certificateIdHex)

        var queue: [ExpectedRequest] = [
            ExpectedRequest(method: "POST",
                            path: "/v1/offline/certificates/issue",
                            responseBody: issuePayload),
            ExpectedRequest(method: "POST",
                            path: "/v1/offline/allowances",
                            responseBody: registerPayload)
        ]

        OfflineWalletTopUpStubURLProtocol.handler = { request in
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

        let wallet = try makeWallet()
        let response = try await wallet.topUpAllowance(draft: draft,
                                                       authority: certificate.controller,
                                                       privateKey: "ed0120deadbeef",
                                                       recordVerdict: true,
                                                       recordedAt: 777)
        XCTAssertEqual(response.certificate.certificateIdHex.lowercased(), certificateIdHex)
        let metadata = try XCTUnwrap(wallet.verdictMetadata(for: certificateIdHex))
        XCTAssertEqual(metadata.verdictIdHex, certificate.verdictId?.hexUppercased().lowercased())
        XCTAssertEqual(metadata.attestationNonceHex, certificate.attestationNonce?.hexUppercased().lowercased())
        XCTAssertEqual(metadata.remainingAmount, certificate.allowance.amount)
        XCTAssertEqual(metadata.recordedAtMs, 777)
        XCTAssertTrue(queue.isEmpty)
    }

    func testTopUpAllowanceSkipsVerdictRecordingWhenDisabled() async throws {
        let certificate = try makeCertificate()
        let certificateIdHex = try certificate.certificateIdHex().lowercased()
        let draft = makeDraft(from: certificate)
        let issuePayload = try makeIssuePayload(certificate: certificate)
        let registerPayload = try makeRegisterPayload(certificateIdHex: certificateIdHex)

        var queue: [ExpectedRequest] = [
            ExpectedRequest(method: "POST",
                            path: "/v1/offline/certificates/issue",
                            responseBody: issuePayload),
            ExpectedRequest(method: "POST",
                            path: "/v1/offline/allowances",
                            responseBody: registerPayload)
        ]

        OfflineWalletTopUpStubURLProtocol.handler = { request in
            guard !queue.isEmpty else {
                throw URLError(.badServerResponse)
            }
            let expected = queue.removeFirst()
            XCTAssertEqual(request.httpMethod, expected.method)
            XCTAssertEqual(request.url?.path, expected.path)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, expected.responseBody)
        }

        let wallet = try makeWallet()
        _ = try await wallet.topUpAllowance(draft: draft,
                                            authority: certificate.controller,
                                            privateKey: "ed0120deadbeef",
                                            recordVerdict: false)
        XCTAssertNil(wallet.verdictMetadata(for: certificateIdHex))
        XCTAssertTrue(queue.isEmpty)
    }

    func testTopUpAllowanceRenewalRecordsVerdictMetadata() async throws {
        let certificate = try makeCertificate()
        let certificateIdHex = try certificate.certificateIdHex().lowercased()
        let draft = makeDraft(from: certificate)
        let issuePayload = try makeIssuePayload(certificate: certificate)
        let registerPayload = try makeRegisterPayload(certificateIdHex: certificateIdHex)

        var queue: [ExpectedRequest] = [
            ExpectedRequest(method: "POST",
                            path: "/v1/offline/certificates/deadbeef/renew/issue",
                            responseBody: issuePayload),
            ExpectedRequest(method: "POST",
                            path: "/v1/offline/allowances/deadbeef/renew",
                            responseBody: registerPayload)
        ]

        OfflineWalletTopUpStubURLProtocol.handler = { request in
            guard !queue.isEmpty else {
                throw URLError(.badServerResponse)
            }
            let expected = queue.removeFirst()
            XCTAssertEqual(request.httpMethod, expected.method)
            XCTAssertEqual(request.url?.path, expected.path)
            let response = HTTPURLResponse(url: request.url!,
                                           statusCode: 200,
                                           httpVersion: nil,
                                           headerFields: ["Content-Type": "application/json"])!
            return (response, expected.responseBody)
        }

        let wallet = try makeWallet()
        _ = try await wallet.topUpAllowanceRenewal(certificateIdHex: "deadbeef",
                                                   draft: draft,
                                                   authority: certificate.controller,
                                                   privateKey: "ed0120deadbeef",
                                                   recordVerdict: true,
                                                   recordedAt: 888)
        let metadata = try XCTUnwrap(wallet.verdictMetadata(for: certificateIdHex))
        XCTAssertEqual(metadata.recordedAtMs, 888)
        XCTAssertTrue(queue.isEmpty)
    }

    func testReprovisionAllowanceRecordsVerdictMetadata() async throws {
        let existing = try makeCertificate()
        let renewed = OfflineWalletCertificate(
            controller: existing.controller,
            operatorId: existing.operatorId,
            allowance: OfflineAllowanceCommitment(
                assetId: existing.allowance.assetId,
                amount: existing.allowance.amount,
                commitment: Data(repeating: 0x44, count: 32)
            ),
            spendPublicKey: existing.spendPublicKey,
            attestationReport: existing.attestationReport,
            issuedAtMs: existing.issuedAtMs,
            expiresAtMs: existing.expiresAtMs,
            policy: existing.policy,
            operatorSignature: Data(repeating: 0xEF, count: 64),
            metadata: existing.metadata,
            verdictId: existing.verdictId,
            attestationNonce: existing.attestationNonce,
            refreshAtMs: existing.refreshAtMs
        )
        let renewedCertificateIdHex = try renewed.certificateIdHex().lowercased()
        let issuePayload = try makeIssuePayload(certificate: renewed)
        let renewPayload = try makeRegisterPayload(certificateIdHex: renewedCertificateIdHex)

        var queue: [ExpectedRequest] = [
            ExpectedRequest(method: "POST",
                            path: "/v1/offline/certificates/deadbeef/renew/issue",
                            responseBody: issuePayload),
            ExpectedRequest(method: "POST",
                            path: "/v1/offline/allowances/deadbeef/renew",
                            responseBody: renewPayload)
        ]

        OfflineWalletTopUpStubURLProtocol.handler = { request in
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

        let wallet = try makeWallet()
        let response = try await wallet.reprovisionAllowance(
            certificateIdHex: "deadbeef",
            currentCertificate: existing,
            newCommitment: Data(repeating: 0x44, count: 32),
            authority: existing.controller,
            privateKey: "ed0120deadbeef",
            recordVerdict: true,
            recordedAt: 999
        )
        XCTAssertEqual(response.certificate.certificateIdHex.lowercased(), renewedCertificateIdHex)
        let metadata = try XCTUnwrap(wallet.verdictMetadata(for: renewedCertificateIdHex))
        XCTAssertEqual(metadata.recordedAtMs, 999)
        XCTAssertEqual(metadata.remainingAmount, existing.allowance.amount)
        XCTAssertTrue(queue.isEmpty)
    }

    // MARK: - Helpers

    private struct ExpectedRequest {
        let method: String
        let path: String
        let responseBody: Data
    }

    private func makeWallet() throws -> OfflineWallet {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [OfflineWalletTopUpStubURLProtocol.self]
        let session = URLSession(configuration: configuration)
        let client = ToriiClient(baseURL: URL(string: "https://example.invalid")!, session: session)
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

    private func makeCertificate() throws -> OfflineWalletCertificate {
        let publicKey = Data(repeating: 0xAB, count: 32)
        let controller = try AccountId.makeIH58(publicKey: publicKey, domain: "wonderland")
        let assetId = "rose#wonderland#\(controller)"
        let allowance = OfflineAllowanceCommitment(assetId: assetId,
                                                   amount: "42",
                                                   commitment: Data(repeating: 0x11, count: 32))
        let policy = OfflineWalletPolicy(maxBalance: "50", maxTxValue: "10", expiresAtMs: 999)
        return OfflineWalletCertificate(
            controller: controller,
            operatorId: controller,
            allowance: allowance,
            spendPublicKey: "ed0120deadbeef",
            attestationReport: Data([0x01, 0x02, 0x03]),
            issuedAtMs: 100,
            expiresAtMs: 200,
            policy: policy,
            operatorSignature: Data(repeating: 0xCD, count: 64),
            metadata: [:],
            verdictId: Data(repeating: 0x23, count: 32),
            attestationNonce: Data(repeating: 0x33, count: 32),
            refreshAtMs: 333
        )
    }

    private func makeDraft(from certificate: OfflineWalletCertificate) -> OfflineWalletCertificateDraft {
        OfflineWalletCertificateDraft(
            controller: certificate.controller,
            operatorId: certificate.operatorId,
            allowance: certificate.allowance,
            spendPublicKey: certificate.spendPublicKey,
            attestationReport: certificate.attestationReport,
            issuedAtMs: certificate.issuedAtMs,
            expiresAtMs: certificate.expiresAtMs,
            policy: certificate.policy,
            metadata: certificate.metadata,
            verdictId: certificate.verdictId,
            attestationNonce: certificate.attestationNonce,
            refreshAtMs: certificate.refreshAtMs
        )
    }

    private func makeIssuePayload(certificate: OfflineWalletCertificate) throws -> Data {
        let certificateIdHex = try certificate.certificateIdHex()
        let toriiCertificate = try certificate.toriiJSON()
        let certificateData = try toriiCertificate.encodedData()
        guard let certificateObject = try JSONSerialization.jsonObject(with: certificateData) as? [String: Any] else {
            throw XCTSkip("certificate payload must be a JSON object")
        }
        let root: [String: Any] = [
            "certificate_id_hex": certificateIdHex,
            "certificate": certificateObject
        ]
        return try JSONSerialization.data(withJSONObject: root, options: [.sortedKeys])
    }

    private func makeRegisterPayload(certificateIdHex: String) throws -> Data {
        let root: [String: Any] = [
            "certificate_id_hex": certificateIdHex
        ]
        return try JSONSerialization.data(withJSONObject: root, options: [.sortedKeys])
    }
}
