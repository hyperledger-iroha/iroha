import Foundation
import XCTest

@testable import IrohaSwift

final class OfflineWalletReceiptTests: XCTestCase {
    private let chainId = "testnet"
    private let appAttestKeyId = Data("swift-tests".utf8).base64EncodedString()

    func testBuildSignedReceiptRecordsJournalAndAudit() throws {
        let logger = try OfflineAuditLogger(storageURL: temporaryLogURL(), isEnabled: true)
        let counterJournal = try OfflineCounterJournal(storageURL: temporaryCounterURL())
        let wallet = try OfflineWallet(
            toriiClient: ToriiClient(baseURL: URL(string: "https://example.invalid")!),
            auditLoggingEnabled: true,
            auditLogger: logger,
            counterJournal: counterJournal
        )
        let journal = try OfflineJournal(url: temporaryJournalURL(), key: journalKey())
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x22, count: 32))
        let certificate = try makeCertificate(signingKey: signingKey)
        let txId = IrohaHash.hash(Data("wallet-receipt".utf8))
        let amount = "10.00"
        let invoiceId = "inv-wallet"
        let issuedAtMs = validIssuedAtMs(for: certificate)
        let proof = try makeProof(txId: txId,
                                  receiver: certificate.controller,
                                  assetId: certificate.allowance.assetId,
                                  amount: amount,
                                  issuedAtMs: issuedAtMs,
                                  invoiceId: invoiceId)

        let receipt = try wallet.buildSignedReceipt(
            txId: txId,
            chainId: chainId,
            receiverAccountId: certificate.controller,
            amount: amount,
            invoiceId: invoiceId,
            platformProof: proof,
            senderCertificate: certificate,
            signingKey: signingKey,
            journal: journal,
            recordedAt: issuedAtMs
        )

        let pending = journal.pendingEntries()
        XCTAssertEqual(pending.count, 1)
        XCTAssertEqual(pending.first?.txId, receipt.txId)

        let entry = try XCTUnwrap(waitForAuditEntries({ wallet.exportAuditEntries() }, expected: 1).first)
        XCTAssertEqual(entry.txId, receipt.txId.hexUppercased())
        XCTAssertEqual(entry.timestampMs, issuedAtMs)

        let checkpoint = try XCTUnwrap(counterJournal.checkpoint(for: try certificate.certificateIdHex()))
        XCTAssertEqual(checkpoint.appleKeyCounters[appAttestKeyId], 1)
    }

    func testBuildSignedReceiptRejectsCounterJump() throws {
        let counterJournal = try OfflineCounterJournal(storageURL: temporaryCounterURL())
        let wallet = try OfflineWallet(
            toriiClient: ToriiClient(baseURL: URL(string: "https://example.invalid")!),
            auditLoggingEnabled: false,
            counterJournal: counterJournal
        )
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x22, count: 32))
        let certificate = try makeCertificate(signingKey: signingKey)
        let certId = try certificate.certificateIdHex()
        _ = try counterJournal.updateCounter(
            certificateIdHex: certId,
            controllerId: certificate.controller,
            controllerDisplay: nil,
            platform: .appleKey,
            scope: appAttestKeyId,
            counter: 1,
            recordedAtMs: 1
        )

        let txId = IrohaHash.hash(Data("wallet-receipt-jump".utf8))
        let amount = "10.00"
        let invoiceId = "inv-wallet-jump"
        let issuedAtMs = validIssuedAtMs(for: certificate, offset: 2_000)
        let proof = try makeProof(txId: txId,
                                  receiver: certificate.controller,
                                  assetId: certificate.allowance.assetId,
                                  amount: amount,
                                  issuedAtMs: issuedAtMs,
                                  invoiceId: invoiceId,
                                  counter: 3)
        XCTAssertThrowsError(
            try wallet.buildSignedReceipt(
                txId: txId,
                chainId: chainId,
                receiverAccountId: certificate.controller,
                amount: amount,
                invoiceId: invoiceId,
                platformProof: proof,
                senderCertificate: certificate,
                signingKey: signingKey,
                recordedAt: issuedAtMs
            )
        ) { error in
            guard case OfflineCounterError.counterJump = error else {
                return XCTFail("expected counter jump error")
            }
        }
    }

    // MARK: - Helpers

    private func makeProof(txId: Data,
                           receiver: String,
                           assetId: String,
                           amount: String,
                           issuedAtMs: UInt64,
                           invoiceId: String,
                           counter: UInt64 = 1) throws -> OfflinePlatformProof {
        let challenge = try challengeHash(txId: txId,
                                          chainId: chainId,
                                          receiver: receiver,
                                          assetId: assetId,
                                          amount: amount,
                                          issuedAtMs: issuedAtMs,
                                          invoiceId: invoiceId)
        let proof = AppleAppAttestProof(keyId: appAttestKeyId,
                                        counter: counter,
                                        assertion: Data([0xAA]),
                                        challengeHash: challenge)
        return .appleAppAttest(proof)
    }

    private func challengeHash(txId: Data,
                               chainId: String,
                               receiver: String,
                               assetId: String,
                               amount: String,
                               issuedAtMs: UInt64,
                               invoiceId: String) throws -> Data {
        let preimage = OfflineReceiptChallengePreimage(
            invoiceId: invoiceId,
            receiverAccountId: receiver,
            assetId: assetId,
            amount: amount,
            issuedAtMs: issuedAtMs,
            nonceHex: txId.hexUppercased()
        )
        let payload = try preimage.noritoPayload()
        let bytes = OfflineNorito.wrap(typeName: OfflineReceiptChallengePreimage.noritoTypeName,
                                       payload: payload)
        let context = IrohaHash.hash(Data(chainId.utf8))
        var hashInput = Data()
        hashInput.reserveCapacity(context.count + bytes.count)
        hashInput.append(context)
        hashInput.append(bytes)
        return IrohaHash.hash(hashInput)
    }

    private func fixtureURL(_ name: String) -> URL {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 {
            url.deleteLastPathComponent()
        }
        return url.appendingPathComponent("fixtures/offline_allowance/ios-demo/\(name)")
    }

    private func makeCertificate(signingKey: SigningKey) throws -> OfflineWalletCertificate {
        let base = try OfflineWalletCertificate.load(from: fixtureURL("certificate.json"))
        let spendKey = "ed0120" + (try signingKey.publicKey()).hexUppercased()
        return OfflineWalletCertificate(
            controller: base.controller,
            allowance: base.allowance,
            spendPublicKey: spendKey,
            attestationReport: base.attestationReport,
            issuedAtMs: base.issuedAtMs,
            expiresAtMs: base.expiresAtMs,
            policy: base.policy,
            operatorSignature: base.operatorSignature,
            metadata: base.metadata,
            verdictId: base.verdictId,
            attestationNonce: base.attestationNonce,
            refreshAtMs: base.refreshAtMs
        )
    }

    private func validIssuedAtMs(for certificate: OfflineWalletCertificate,
                                 offset: UInt64 = 1_000) -> UInt64 {
        let expiryBound = min(certificate.expiresAtMs, certificate.policy.expiresAtMs)
        let candidate = certificate.issuedAtMs &+ offset
        return candidate > expiryBound ? expiryBound : candidate
    }

    private func waitForAuditEntries(_ fetch: () -> [OfflineAuditEntry],
                                     expected: Int,
                                     timeout: TimeInterval = 0.5) -> [OfflineAuditEntry] {
        var snapshot = fetch()
        let deadline = Date().addingTimeInterval(timeout)
        while snapshot.count < expected && Date() < deadline {
            RunLoop.current.run(mode: .default, before: Date().addingTimeInterval(0.01))
            snapshot = fetch()
        }
        return snapshot
    }

    private func temporaryJournalURL() -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".ijournal")
        addTeardownBlock {
            try? FileManager.default.removeItem(at: url)
        }
        return url
    }

    private func temporaryLogURL() -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".json")
        addTeardownBlock {
            try? FileManager.default.removeItem(at: url)
        }
        return url
    }

    private func temporaryCounterURL() -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".counter.json")
        addTeardownBlock {
            try? FileManager.default.removeItem(at: url)
        }
        return url
    }

    private func journalKey() -> OfflineJournalKey {
        OfflineJournalKey.derive(from: Data("wallet-receipt-key".utf8))
    }
}
