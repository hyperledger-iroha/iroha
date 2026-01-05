import XCTest
@testable import IrohaSwift
#if canImport(CryptoKit)
import CryptoKit
#endif

final class ConnectQueueJournalTests: XCTestCase {
    func testAppendAndReadRecords() throws {
        try requireBlake3()
        let tmp = makeTemporaryDirectory()
        let journal = ConnectQueueJournal(sessionID: Data("session-1".utf8),
                                          configuration: .init(rootDirectory: tmp,
                                                               maxRecordsPerQueue: 8,
                                                               maxBytesPerQueue: 1 << 20,
                                                               retentionInterval: 60))
        let first = Data([0x01, 0x02, 0x03])
        let second = Data([0xAA, 0xBB])
        try journal.append(direction: .appToWallet,
                           sequence: 1,
                           ciphertext: first,
                           receivedAtMs: 10,
                           ttlOverrideMs: 1_000)
        try journal.append(direction: .walletToApp,
                           sequence: 9,
                           ciphertext: second,
                           receivedAtMs: 12,
                           ttlOverrideMs: 1_000)

        let appRecords = try journal.records(direction: .appToWallet, nowMs: 500)
        XCTAssertEqual(appRecords.count, 1)
        XCTAssertEqual(appRecords.first?.sequence, 1)
        XCTAssertEqual(appRecords.first?.ciphertext, first)

        let walletRecords = try journal.records(direction: .walletToApp, nowMs: 500)
        XCTAssertEqual(walletRecords.count, 1)
        XCTAssertEqual(walletRecords.first?.sequence, 9)
        XCTAssertEqual(walletRecords.first?.ciphertext, second)
    }

    func testEvictsWhenMaxRecordsExceeded() throws {
        try requireBlake3()
        let tmp = makeTemporaryDirectory()
        let journal = ConnectQueueJournal(sessionID: Data("session-2".utf8),
                                          configuration: .init(rootDirectory: tmp,
                                                               maxRecordsPerQueue: 2,
                                                               maxBytesPerQueue: 1 << 20,
                                                               retentionInterval: 60))
        for seq in 1...3 {
            let payload = Data(repeating: UInt8(seq), count: 4)
            try journal.append(direction: .appToWallet,
                               sequence: UInt64(seq),
                               ciphertext: payload,
                               receivedAtMs: UInt64(seq),
                               ttlOverrideMs: 60_000)
        }

        let records = try journal.records(direction: .appToWallet, nowMs: 10)
        XCTAssertEqual(records.map(\.sequence), [2, 3])
    }

    func testPopOldestRemovesRecords() throws {
        try requireBlake3()
        let tmp = makeTemporaryDirectory()
        let journal = ConnectQueueJournal(sessionID: Data("session-3".utf8),
                                          configuration: .init(rootDirectory: tmp,
                                                               maxRecordsPerQueue: 8,
                                                               maxBytesPerQueue: 1 << 20,
                                                               retentionInterval: 60))
        for seq in 10...12 {
            let payload = Data(repeating: UInt8(seq), count: 2)
            try journal.append(direction: .walletToApp,
                               sequence: UInt64(seq),
                               ciphertext: payload,
                               receivedAtMs: UInt64(seq),
                               ttlOverrideMs: 60_000)
        }

        let drained = try journal.popOldest(direction: .walletToApp, count: 2, nowMs: 100)
        XCTAssertEqual(drained.map(\.sequence), [10, 11])

        let remaining = try journal.records(direction: .walletToApp, nowMs: 100)
        XCTAssertEqual(remaining.map(\.sequence), [12])
    }

    func testPayloadHashUsesBlake3() throws {
        try requireBlake3()
        let payload = Data("payload-hash".utf8)
        let expected = try XCTUnwrap(NoritoNativeBridge.shared.blake3Hash(data: payload),
                                     "NoritoBridge blake3 hashing is unavailable")

        let tmp = makeTemporaryDirectory()
        let journal = ConnectQueueJournal(sessionID: Data("session-blake3".utf8),
                                          configuration: .init(rootDirectory: tmp,
                                                               maxRecordsPerQueue: 8,
                                                               maxBytesPerQueue: 1 << 20,
                                                               retentionInterval: 60))
        try journal.append(direction: .appToWallet,
                           sequence: 1,
                           ciphertext: payload,
                           receivedAtMs: 0,
                           ttlOverrideMs: 60_000)

        let records = try journal.records(direction: .appToWallet, nowMs: 1)
        XCTAssertEqual(records.count, 1)
        XCTAssertEqual(records.first?.payloadHash, expected)
    }

    func testExpiredRecordsArePruned() throws {
        try requireBlake3()
        let tmp = makeTemporaryDirectory()
        let journal = ConnectQueueJournal(sessionID: Data("session-4".utf8),
                                          configuration: .init(rootDirectory: tmp,
                                                               maxRecordsPerQueue: 8,
                                                               maxBytesPerQueue: 1 << 20,
                                                               retentionInterval: 1))
        try journal.append(direction: .appToWallet,
                           sequence: 42,
                           ciphertext: Data([0xFF]),
                           receivedAtMs: 0,
                           ttlOverrideMs: 1)
        let records = try journal.records(direction: .appToWallet, nowMs: 5)
        XCTAssertTrue(records.isEmpty)
    }

    func testRejectsOversizedQueueFile() throws {
        let tmp = makeTemporaryDirectory()
        let sessionID = Data("oversized-queue".utf8)
        let config = ConnectQueueJournal.Configuration(rootDirectory: tmp,
                                                       maxRecordsPerQueue: 8,
                                                       maxBytesPerQueue: 64,
                                                       retentionInterval: 60)
        let storage = ConnectQueueStorage(sessionID: sessionID,
                                          configuration: .init(rootDirectory: tmp),
                                          fileManager: .default)
        try storage.ensureSessionDirectory()
        let oversized = Data(repeating: 0xAA, count: 128)
        try oversized.write(to: storage.appQueueURL)

        let journal = ConnectQueueJournal(sessionID: sessionID,
                                          configuration: config,
                                          fileManager: .default)
        XCTAssertThrowsError(try journal.records(direction: .appToWallet, nowMs: 0)) { error in
            guard case ConnectQueueError.overflow(let limit) = error else {
                return XCTFail("expected overflow error, got \(error)")
            }
            XCTAssertEqual(limit, config.maxBytesPerQueue)
        }
    }

    func testRejectsTruncatedHeader() throws {
        let tmp = makeTemporaryDirectory()
        let sessionID = Data("truncated-header".utf8)
        let storage = ConnectQueueStorage(sessionID: sessionID,
                                          configuration: .init(rootDirectory: tmp),
                                          fileManager: .default)
        try storage.ensureSessionDirectory()
        try Data([0x00, 0x01, 0x02]).write(to: storage.walletQueueURL)

        let journal = ConnectQueueJournal(sessionID: sessionID,
                                          configuration: .init(rootDirectory: tmp),
                                          fileManager: .default)
        XCTAssertThrowsError(try journal.records(direction: .walletToApp, nowMs: 0)) { error in
            guard case ConnectQueueError.corrupted = error else {
                return XCTFail("expected corrupted error, got \(error)")
            }
        }
    }

    func testRejectsTruncatedPayload() throws {
        let tmp = makeTemporaryDirectory()
        let sessionID = Data("truncated-payload".utf8)
        let storage = ConnectQueueStorage(sessionID: sessionID,
                                          configuration: .init(rootDirectory: tmp),
                                          fileManager: .default)
        try storage.ensureSessionDirectory()
        let record = try ConnectJournalRecord(direction: .appToWallet,
                                              sequence: 1,
                                              payloadHash: Data(repeating: 0x11, count: 32),
                                              ciphertext: Data([0xAA, 0xBB]),
                                              receivedAtMs: 0,
                                              expiresAtMs: 10_000)
        var envelope = try record.encodeEnvelope()
        envelope.removeLast()
        try envelope.write(to: storage.appQueueURL)

        let journal = ConnectQueueJournal(sessionID: sessionID,
                                          configuration: .init(rootDirectory: tmp),
                                          fileManager: .default)
        XCTAssertThrowsError(try journal.records(direction: .appToWallet, nowMs: 0)) { error in
            guard case ConnectQueueError.corrupted = error else {
                return XCTFail("expected corrupted error, got \(error)")
            }
        }
    }

    func testReadsRecordWithHeaderPadding() throws {
        let tmp = makeTemporaryDirectory()
        let sessionID = Data("padded-record".utf8)
        let storage = ConnectQueueStorage(sessionID: sessionID,
                                          configuration: .init(rootDirectory: tmp),
                                          fileManager: .default)
        try storage.ensureSessionDirectory()
        let record = try ConnectJournalRecord(direction: .appToWallet,
                                              sequence: 7,
                                              payloadHash: Data(repeating: 0x22, count: 32),
                                              ciphertext: Data([0xAB, 0xCD]),
                                              receivedAtMs: 1,
                                              expiresAtMs: 10_000)
        let envelope = try record.encodeEnvelope()
        let headerLen = ConnectJournalRecord.headerLength
        var padded = Data()
        padded.append(envelope.prefix(headerLen))
        padded.append(Data(repeating: 0, count: 8))
        padded.append(envelope.dropFirst(headerLen))
        try padded.write(to: storage.appQueueURL)

        let journal = ConnectQueueJournal(sessionID: sessionID,
                                          configuration: .init(rootDirectory: tmp),
                                          fileManager: .default)
        let records = try journal.records(direction: .appToWallet, nowMs: 0)
        XCTAssertEqual(records.count, 1)
        XCTAssertEqual(records.first?.sequence, record.sequence)
    }

    func testRejectsFileWithTooManyRecords() throws {
        let tmp = makeTemporaryDirectory()
        let sessionID = Data("too-many-records".utf8)
        let storage = ConnectQueueStorage(sessionID: sessionID,
                                          configuration: .init(rootDirectory: tmp),
                                          fileManager: .default)
        try storage.ensureSessionDirectory()
        let first = try ConnectJournalRecord(direction: .appToWallet,
                                             sequence: 1,
                                             payloadHash: Data(repeating: 0x22, count: 32),
                                             ciphertext: Data([0x01]),
                                             receivedAtMs: 0,
                                             expiresAtMs: 10_000)
        let second = try ConnectJournalRecord(direction: .appToWallet,
                                              sequence: 2,
                                              payloadHash: Data(repeating: 0x33, count: 32),
                                              ciphertext: Data([0x02]),
                                              receivedAtMs: 1,
                                              expiresAtMs: 10_000)
        var payload = Data()
        payload.append(try first.encodeEnvelope())
        payload.append(try second.encodeEnvelope())
        try payload.write(to: storage.appQueueURL)

        let config = ConnectQueueJournal.Configuration(rootDirectory: tmp,
                                                       maxRecordsPerQueue: 1,
                                                       maxBytesPerQueue: 1 << 20,
                                                       retentionInterval: 60)
        let journal = ConnectQueueJournal(sessionID: sessionID,
                                          configuration: config,
                                          fileManager: .default)
        XCTAssertThrowsError(try journal.records(direction: .appToWallet, nowMs: 0)) { error in
            guard case ConnectQueueError.overflow(let limit) = error else {
                return XCTFail("expected overflow error, got \(error)")
            }
            XCTAssertEqual(limit, config.maxRecordsPerQueue)
        }
    }

    func testSessionDirectoryUsesSha256Component() throws {
        #if canImport(CryptoKit)
        try requireBlake3()
        let tmp = makeTemporaryDirectory()
        let sessionID = Data("session-5".utf8)
        let journal = ConnectQueueJournal(sessionID: sessionID,
                                          configuration: .init(rootDirectory: tmp,
                                                               maxRecordsPerQueue: 1,
                                                               maxBytesPerQueue: 1 << 20,
                                                               retentionInterval: 60))
        try journal.append(direction: .appToWallet,
                           sequence: 1,
                           ciphertext: Data([0x00]),
                           receivedAtMs: 0,
                           ttlOverrideMs: 10_000)
        let digest = SHA256.hash(data: sessionID)
        let hex = digest.map { String(format: "%02x", $0) }.joined()
        let queueURL = tmp
            .appendingPathComponent(hex, isDirectory: true)
            .appendingPathComponent("app_to_wallet.queue", isDirectory: false)
        XCTAssertTrue(FileManager.default.fileExists(atPath: queueURL.path))
        #else
        throw XCTSkip("CryptoKit unavailable")
        #endif
    }

    private func makeTemporaryDirectory() -> URL {
        let base = FileManager.default.temporaryDirectory
        let url = base.appendingPathComponent("iroha-connect-journal-\(UUID().uuidString)", isDirectory: true)
        try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
        return url
    }

    private func requireBlake3() throws {
        guard NoritoNativeBridge.shared.blake3Hash(data: Data()) != nil else {
            throw XCTSkip("NoritoBridge blake3 hashing unavailable")
        }
    }
}
