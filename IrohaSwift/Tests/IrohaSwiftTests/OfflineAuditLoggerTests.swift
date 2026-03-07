import Foundation
import XCTest
@testable import IrohaSwift

final class OfflineAuditLoggerTests: XCTestCase {
    private let fixtureAccountA = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"
    private let fixtureAccountB = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"
    private let fixtureAccountC = "6cmzPVPX8j6hZ4dceSGxHX65vr3iK4RG3em7w24HeJ9BbQXmKFVrykC"

    func testRecordsEntriesWhenEnabled() throws {
        let logger = try OfflineAuditLogger(storageURL: temporaryLogURL(), isEnabled: true)
        let assetId = try makeNoritoAssetId(name: "xor", domain: "sora", accountId: fixtureAccountA)
        let entry = OfflineAuditEntry(txId: "bundle",
                                      senderId: fixtureAccountA,
                                      receiverId: fixtureAccountB,
                                      assetId: assetId,
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
                                               senderId: fixtureAccountA,
                                               receiverId: fixtureAccountB,
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
        let assetId = try makeNoritoAssetId(name: "xor", domain: "sora", accountId: fixtureAccountA)

        let transfer = makeTransferItem(
            transfer: makeReceiptPayload(sender: fixtureAccountA,
                                         receiver: fixtureAccountB,
                                         asset: assetId,
                                         amount: "42.5")
        )

        wallet.recordTransferAudit(transfer)

        let entry = try XCTUnwrap(wallet.exportAuditEntries().first)
        XCTAssertEqual(entry.senderId, fixtureAccountA)
        XCTAssertEqual(entry.receiverId, fixtureAccountB)
        XCTAssertEqual(entry.assetId, assetId)
        XCTAssertEqual(entry.amount, "42.5")
        XCTAssertEqual(entry.txId, transfer.bundleIdHex)
    }

    func testRecordReceiptAuditCapturesReceiptFields() throws {
        let logger = try OfflineAuditLogger(storageURL: temporaryLogURL(), isEnabled: true)
        let wallet = try makeWallet(logger: logger, auditLoggingEnabled: true)
        let certificate = try normalizedCertificate(OfflineWalletCertificate.load(from: fixtureURL("certificate.json")))
        let txId = IrohaHash.hash(Data("receipt-audit".utf8))
        let proof = OfflinePlatformProof.appleAppAttest(
            AppleAppAttestProof(keyId: Data("swift-tests".utf8).base64EncodedString(),
                                counter: 1,
                                assertion: Data([0xAA]),
                                challengeHash: IrohaHash.hash(Data("challenge".utf8)))
        )
        let assetId = try makeNoritoAssetId(name: "xor", domain: "sora", accountId: certificate.controller)
        let receipt = OfflineSpendReceipt(
            txId: txId,
            from: certificate.controller,
            to: certificate.controller,
            assetId: assetId,
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
        XCTAssertEqual(entry.assetId, assetId)
        XCTAssertEqual(entry.amount, "25")
        XCTAssertEqual(entry.timestampMs, 42)
    }

    func testRecordTransferAuditFallsBackWhenSummaryMissing() throws {
        let logger = try OfflineAuditLogger(storageURL: temporaryLogURL(), isEnabled: true)
        let wallet = try makeWallet(logger: logger, auditLoggingEnabled: true)

        let transfer = makeTransferItem(
            receiverId: fixtureAccountB,
            depositAccountId: fixtureAccountC,
            assetId: nil,
            totalAmount: "100",
            claimedDelta: "  ",
            transfer: .object([:])
        )

        wallet.recordTransferAudit(transfer)

        let entry = try XCTUnwrap(wallet.exportAuditEntries().first)
        XCTAssertEqual(entry.senderId, fixtureAccountB)
        XCTAssertEqual(entry.receiverId, fixtureAccountC)
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
                                  controllerId: String = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
                                  controllerDisplay: String = "Merchant",
                                  receiverId: String = "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
                                  receiverDisplay: String = "Receiver",
                                  depositAccountId: String = "6cmzPVPX8j6hZ4dceSGxHX65vr3iK4RG3em7w24HeJ9BbQXmKFVrykC",
                                  depositAccountDisplay: String = "Deposit",
                                  assetId: String? = nil,
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

    private func makeNoritoAssetId(name: String,
                                   domain: String,
                                   accountId: String) throws -> String {
        var writer = OfflineNoritoWriter()
        writer.writeField(try OfflineNorito.encodeAssetDefinitionId(name: name, domain: domain))
        writer.writeField(try OfflineNorito.encodeAccountId(accountId))
        return "norito:\(writer.data.hexLowercased())"
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

    private func temporaryLogURL() -> URL {
        let directory = FileManager.default.temporaryDirectory
        let url = directory.appendingPathComponent("offline-audit-\(UUID().uuidString).json")
        addTeardownBlock {
            try? FileManager.default.removeItem(at: url)
        }
        return url
    }
}
