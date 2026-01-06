import Foundation
import XCTest

@testable import IrohaSwift

final class OfflineJournalTests: XCTestCase {
    private func tempURL() -> URL {
        let directory = FileManager.default.temporaryDirectory
        let filename = UUID().uuidString + ".ijournal"
        return directory.appendingPathComponent(filename, isDirectory: false)
    }

    private func key() -> OfflineJournalKey {
        OfflineJournalKey.derive(from: Data("test-key".utf8))
    }

    func testRoundTrip() throws {
        let url = tempURL()
        var journal: OfflineJournal? = try OfflineJournal(url: url, key: key())
        let txId = Data(repeating: 0x11, count: 32)
        let payload = Data("pending-envelope".utf8)
        let entry = try journal?.appendPending(txId: txId, payload: payload, timestampMs: 42)
        XCTAssertEqual(entry?.hashChain.count, 32)
        journal = nil

        let reopened = try OfflineJournal(url: url, key: key())
        let pending = reopened.pendingEntries()
        XCTAssertEqual(pending.count, 1)
        XCTAssertEqual(pending[0].payload, payload)
        XCTAssertEqual(pending[0].recordedAtMs, 42)
        XCTAssertNotEqual(pending[0].hashChain, Data(repeating: 0, count: 32))
    }

    func testMarkCommittedClearsPending() throws {
        let url = tempURL()
        let journal = try OfflineJournal(url: url, key: key())
        let txId = Data(repeating: 0x22, count: 32)
        try journal.appendPending(txId: txId, payload: Data([0xAA]), timestampMs: 1)
        try journal.markCommitted(txId: txId, timestampMs: 2)
        XCTAssertTrue(journal.pendingEntries().isEmpty)
    }

    func testTamperingDetected() throws {
        let url = tempURL()
        do {
            let journal = try OfflineJournal(url: url, key: key())
            let txId = Data(repeating: 0x33, count: 32)
            try journal.appendPending(txId: txId, payload: Data("payload".utf8), timestampMs: 5)
        }

        var bytes = try Data(contentsOf: url)
        if let ascii = Character("p").asciiValue, let idx = bytes.firstIndex(of: ascii) {
            bytes[idx] ^= 0xFF
            try bytes.write(to: url)
        } else {
            XCTFail("expected payload byte to mutate")
            return
        }

        XCTAssertThrowsError(try OfflineJournal(url: url, key: key())) { error in
            guard case OfflineJournalError.integrityViolation(let reason) = error else {
                XCTFail("unexpected error \(error)")
                return
            }
            XCTAssertTrue(reason.contains("HMAC") || reason.contains("hash chain"))
        }
    }

    func testRejectsOversizedRecord() throws {
        let url = tempURL()
        let config = OfflineJournalConfiguration(maxRecords: 8, maxBytes: 80)
        let journal = try OfflineJournal(url: url, key: key(), configuration: config)
        let txId = Data(repeating: 0x44, count: 32)
        let payload = Data(repeating: 0xAA, count: 100)

        XCTAssertThrowsError(try journal.appendPending(txId: txId, payload: payload, timestampMs: 1)) { error in
            guard case OfflineJournalError.integrityViolation(let reason) = error else {
                return XCTFail("expected integrity violation, got \(error)")
            }
            XCTAssertTrue(reason.contains("max size"))
        }
    }

    func testRejectsExcessRecords() throws {
        let url = tempURL()
        let config = OfflineJournalConfiguration(maxRecords: 1, maxBytes: 1 << 20)
        let journal = try OfflineJournal(url: url, key: key(), configuration: config)
        let tx1 = Data(repeating: 0x10, count: 32)
        let tx2 = Data(repeating: 0x20, count: 32)
        try journal.appendPending(txId: tx1, payload: Data([0x01]), timestampMs: 1)
        XCTAssertThrowsError(try journal.appendPending(txId: tx2, payload: Data([0x02]), timestampMs: 2)) { error in
            guard case OfflineJournalError.integrityViolation(let reason) = error else {
                return XCTFail("expected integrity violation, got \(error)")
            }
            XCTAssertTrue(reason.contains("record limit"))
        }
    }

    func testRejectsTruncatedJournalOnReopen() throws {
        let url = tempURL()
        _ = try OfflineJournal(url: url, key: key())
        let handle = try FileHandle(forWritingTo: url)
        try handle.seekToEnd()
        try handle.write(contentsOf: [0x01])
        try handle.close()

        XCTAssertThrowsError(try OfflineJournal(url: url, key: key())) { error in
            guard case OfflineJournalError.integrityViolation = error else {
                return XCTFail("expected integrity violation, got \(error)")
            }
        }
    }

    func testReceiptHelpersPersistNoritoPayload() throws {
        let url = tempURL()
        let journal = try OfflineJournal(url: url, key: key())
        let certificate = try OfflineWalletCertificate.load(from: fixtureURL("certificate.json"))
        let txId = IrohaHash.hash(Data("receipt".utf8))
        let challengeHash = IrohaHash.hash(Data("challenge".utf8))
        let proof = OfflinePlatformProof.appleAppAttest(
            AppleAppAttestProof(keyId: Data("swift-tests".utf8).base64EncodedString(),
                                counter: 1,
                                assertion: Data([0xAA]),
                                challengeHash: challengeHash)
        )
        let receipt = OfflineSpendReceipt(
            txId: txId,
            from: certificate.controller,
            to: certificate.controller,
            assetId: certificate.allowance.assetId,
            amount: certificate.allowance.amount,
            issuedAtMs: certificate.issuedAtMs + 1000,
            invoiceId: "inv-1",
            platformProof: proof,
            platformSnapshot: nil,
            senderCertificate: certificate,
            senderSignature: Data(repeating: 0x01, count: 64)
        )

        let entry = try journal.appendPending(receipt: receipt, timestampMs: 10)
        XCTAssertEqual(entry.payload, try receipt.noritoEncoded())
        try journal.markCommitted(receipt: receipt, timestampMs: 20)
        XCTAssertTrue(journal.pendingEntries().isEmpty)
    }

    private func fixtureURL(_ name: String) -> URL {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 {
            url.deleteLastPathComponent()
        }
        return url.appendingPathComponent("fixtures/offline_allowance/ios-demo/\(name)")
    }
}
