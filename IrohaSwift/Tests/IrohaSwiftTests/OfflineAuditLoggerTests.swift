import Foundation
import XCTest
@testable import IrohaSwift

final class OfflineAuditLoggerTests: XCTestCase {

    func testRecordsEntriesWhenEnabled() throws {
        let logger = try OfflineAuditLogger(storageURL: temporaryLogURL(), isEnabled: true)
        let entry = OfflineAuditEntry(txId: "bundle",
                                      senderId: "sender@test",
                                      receiverId: "receiver@test",
                                      assetId: "xor#sora",
                                      amount: "10",
                                      timestampMs: 123)

        logger.record(entry: entry)

        let entries = logger.exportEntries()
        XCTAssertEqual(entries.count, 1)
        XCTAssertEqual(entries.first, entry)

        let exported = try logger.exportJSON()
        XCTAssertFalse(exported.isEmpty)
    }

    func testClearRemovesPersistedEntries() throws {
        let logger = try OfflineAuditLogger(storageURL: temporaryLogURL(), isEnabled: true)
        logger.record(entry: OfflineAuditEntry(txId: "bundle",
                                               senderId: "sender@test",
                                               receiverId: "receiver@test",
                                               assetId: "",
                                               amount: "1",
                                               timestampMs: 1))

        XCTAssertEqual(logger.exportEntries().count, 1)
        try logger.clear()
        XCTAssertTrue(logger.exportEntries().isEmpty)
    }

    func testRecordTransferAuditUsesReceiptSummary() throws {
        let logger = try OfflineAuditLogger(storageURL: temporaryLogURL(), isEnabled: true)
        let wallet = try makeWallet(logger: logger, auditLoggingEnabled: true)

        let transfer = makeTransferItem(
            transfer: makeReceiptPayload(sender: "merchant@test",
                                         receiver: "treasury@test",
                                         asset: "xor#sora",
                                         amount: "42.5")
        )

        wallet.recordTransferAudit(transfer)

        let entry = try XCTUnwrap(wallet.exportAuditEntries().first)
        XCTAssertEqual(entry.senderId, "merchant@test")
        XCTAssertEqual(entry.receiverId, "treasury@test")
        XCTAssertEqual(entry.assetId, "xor#sora")
        XCTAssertEqual(entry.amount, "42.5")
        XCTAssertEqual(entry.txId, transfer.bundleIdHex)
    }

    func testRecordReceiptAuditCapturesReceiptFields() throws {
        let logger = try OfflineAuditLogger(storageURL: temporaryLogURL(), isEnabled: true)
        let wallet = try makeWallet(logger: logger, auditLoggingEnabled: true)
        let certificate = try OfflineWalletCertificate.load(from: fixtureURL("certificate.json"))
        let txId = IrohaHash.hash(Data("receipt-audit".utf8))
        let proof = OfflinePlatformProof.appleAppAttest(
            AppleAppAttestProof(keyId: Data("swift-tests".utf8).base64EncodedString(),
                                counter: 1,
                                assertion: Data([0xAA]),
                                challengeHash: IrohaHash.hash(Data("challenge".utf8)))
        )
        let receipt = OfflineSpendReceipt(
            txId: txId,
            from: certificate.controller,
            to: certificate.controller,
            assetId: certificate.allowance.assetId,
            amount: "25",
            issuedAtMs: certificate.issuedAtMs + 1000,
            invoiceId: "inv-audit",
            platformProof: proof,
            platformSnapshot: nil,
            senderCertificateId: try certificate.certificateId(),
            senderSignature: Data(repeating: 0x01, count: 64)
        )

        wallet.recordReceiptAudit(receipt, timestampMs: 42)

        let entry = try XCTUnwrap(wallet.exportAuditEntries().first)
        XCTAssertEqual(entry.txId, txId.hexUppercased())
        XCTAssertEqual(entry.senderId, certificate.controller)
        XCTAssertEqual(entry.receiverId, certificate.controller)
        XCTAssertEqual(entry.assetId, certificate.allowance.assetId)
        XCTAssertEqual(entry.amount, "25")
        XCTAssertEqual(entry.timestampMs, 42)
    }

    func testRecordTransferAuditFallsBackWhenSummaryMissing() throws {
        let logger = try OfflineAuditLogger(storageURL: temporaryLogURL(), isEnabled: true)
        let wallet = try makeWallet(logger: logger, auditLoggingEnabled: true)

        let transfer = makeTransferItem(
            receiverId: "receiver@test",
            depositAccountId: "deposit@test",
            assetId: nil,
            totalAmount: "100",
            claimedDelta: "  ",
            transfer: .object([:])
        )

        wallet.recordTransferAudit(transfer)

        let entry = try XCTUnwrap(wallet.exportAuditEntries().first)
        XCTAssertEqual(entry.senderId, "receiver@test")
        XCTAssertEqual(entry.receiverId, "deposit@test")
        XCTAssertEqual(entry.assetId, "")
        XCTAssertEqual(entry.amount, "100")
    }

    func testRecordTransferAuditNoOpWhenDisabled() throws {
        let logger = try OfflineAuditLogger(storageURL: temporaryLogURL(), isEnabled: false)
        let wallet = try makeWallet(logger: logger, auditLoggingEnabled: false)

        wallet.recordTransferAudit(makeTransferItem(transfer: .object([:])))

        XCTAssertTrue(wallet.exportAuditEntries().isEmpty)
    }

    // MARK: - Helpers

    private func makeWallet(logger: OfflineAuditLogger,
                            auditLoggingEnabled: Bool) throws -> OfflineWallet {
        let client = ToriiClient(baseURL: URL(string: "https://example.invalid")!)
        return try OfflineWallet(
            toriiClient: client,
            auditLoggingEnabled: auditLoggingEnabled,
            auditLogger: logger
        )
    }

    private func fixtureURL(_ name: String) -> URL {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 {
            url.deleteLastPathComponent()
        }
        return url.appendingPathComponent("fixtures/offline_allowance/ios-demo/\(name)")
    }

    private func makeTransferItem(bundleId: String = UUID().uuidString,
                                  controllerId: String = "merchant@test",
                                  controllerDisplay: String = "Merchant",
                                  receiverId: String = "receiver@test",
                                  receiverDisplay: String = "Receiver",
                                  depositAccountId: String = "deposit@test",
                                  depositAccountDisplay: String = "Deposit",
                                  assetId: String? = "xor#sora",
                                  receiptCount: UInt64 = 1,
                                  totalAmount: String = "10",
                                  claimedDelta: String = "5",
                                  status: String = "settled",
                                  recordedAtMs: UInt64 = 0,
                                  recordedAtHeight: UInt64 = 0,
                                  archivedAtHeight: UInt64? = nil,
                                  certificateIdHex: String? = nil,
                                  certificateExpiresAtMs: UInt64? = nil,
                                  policyExpiresAtMs: UInt64? = nil,
                                  refreshAtMs: UInt64? = nil,
                                  verdictIdHex: String? = nil,
                                  attestationNonceHex: String? = nil,
                                  platformPolicy: ToriiPlatformPolicy? = nil,
                                  platformTokenSnapshot: ToriiOfflinePlatformTokenSnapshot? = nil,
                                  transfer: ToriiJSONValue) -> ToriiOfflineTransferItem {
        ToriiOfflineTransferItem(
            bundleIdHex: bundleId,
            controllerId: controllerId,
            controllerDisplay: controllerDisplay,
            receiverId: receiverId,
            receiverDisplay: receiverDisplay,
            depositAccountId: depositAccountId,
            depositAccountDisplay: depositAccountDisplay,
            assetId: assetId,
            receiptCount: receiptCount,
            totalAmount: totalAmount,
            claimedDelta: claimedDelta,
            status: status,
            recordedAtMs: recordedAtMs,
            recordedAtHeight: recordedAtHeight,
            archivedAtHeight: archivedAtHeight,
            certificateIdHex: certificateIdHex,
            certificateExpiresAtMs: certificateExpiresAtMs,
            policyExpiresAtMs: policyExpiresAtMs,
            refreshAtMs: refreshAtMs,
            verdictIdHex: verdictIdHex,
            attestationNonceHex: attestationNonceHex,
            platformPolicy: platformPolicy,
            platformTokenSnapshot: platformTokenSnapshot,
            transfer: transfer
        )
    }

    private func makeReceiptPayload(sender: String,
                                    receiver: String,
                                    asset: String?,
                                    amount: String) -> ToriiJSONValue {
        var receipt: [String: ToriiJSONValue] = [
            "from": .string(sender),
            "to": .string(receiver),
            "amount": .string(amount)
        ]
        if let asset {
            receipt["asset"] = .string(asset)
        }
        return .object([
            "receipts": .array([.object(receipt)])
        ])
    }

    private func temporaryLogURL() -> URL {
        let directory = FileManager.default.temporaryDirectory
        let url = directory.appendingPathComponent("offline-audit-\(UUID().uuidString).json")
        addTeardownBlock {
            try? FileManager.default.removeItem(at: url)
        }
        return url
    }
}
