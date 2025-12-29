import CryptoKit
import Foundation

private enum OfflineJournalFormat {
    static let headerMagic = Data([0x49, 0x4a, 0x4e, 0x4c]) // "IJNL"
    static let version: UInt8 = 1
    static let headerLength = headerMagic.count + 1
    static let hashLength = 32
    static let hmacLength = 32
}

/// Errors thrown by `OfflineJournal`.
public enum OfflineJournalError: Error, CustomStringConvertible {
    case invalidKeyLength
    case invalidTxIdLength
    case duplicatePending
    case alreadyCommitted
    case notPending
    case io(Error)
    case integrityViolation(String)
    case payloadTooLarge

    public var description: String {
        switch self {
        case .invalidKeyLength:
            return "offline journal key must be exactly 32 bytes"
        case .invalidTxIdLength:
            return "offline journal tx_id must be exactly 32 bytes"
        case .duplicatePending:
            return "transaction already pending in offline journal"
        case .alreadyCommitted:
            return "transaction already committed in offline journal"
        case .notPending:
            return "transaction not found in offline journal pending set"
        case .io(let error):
            return "offline journal I/O error: \(error)"
        case .integrityViolation(let reason):
            return "offline journal integrity violation: \(reason)"
        case .payloadTooLarge:
            return "offline journal payload length exceeds 2^32-1 bytes"
        }
    }
}

public struct OfflineJournalConfiguration: Sendable {
    public var maxRecords: Int
    public var maxBytes: Int

    public init(maxRecords: Int = 4096, maxBytes: Int = 4 << 20) {
        self.maxRecords = maxRecords
        self.maxBytes = maxBytes
    }
}

/// Key material used to authenticate journal records.
public struct OfflineJournalKey: Sendable, Equatable {
    fileprivate let raw: Data

    public init(rawBytes: Data) throws {
        guard rawBytes.count == OfflineJournalFormat.hashLength else {
            throw OfflineJournalError.invalidKeyLength
        }
        self.raw = rawBytes
    }

    public static func derive(from seed: Data) -> OfflineJournalKey {
        let digest = SHA256.hash(data: seed)
        // SHA256 always returns 32 bytes so this init cannot fail.
        return try! OfflineJournalKey(rawBytes: Data(digest))
    }
}

/// Pending offline entry stored by the journal.
public struct OfflineJournalEntry: Sendable, Equatable {
    public let txId: Data
    public let payload: Data
    public let recordedAtMs: UInt64
    public let hashChain: Data
}

private enum OfflineJournalEntryKind: UInt8 {
    case pending = 0
    case committed = 1
}

/// Append-only offline journal shared with the Rust implementation.
///
/// Each record is encoded as:
/// `[kind (1)] [timestamp_le (8)] [payload_len_le (4)] [tx_id (32)] [payload] [chain (32)] [hmac (32)]`
/// where `chain = BLAKE2b-256(prev_chain || tx_id)` and
/// `hmac = HMAC-SHA256(prev_chain || record_without_hmac)`.
public final class OfflineJournal {
    private let queue = DispatchQueue(label: "org.hyperledger.iroha.offline-journal", qos: .utility)
    private let fileURL: URL
    private let configuration: OfflineJournalConfiguration
    private let symmetricKey: SymmetricKey
    private var fileHandle: FileHandle
    private var currentFileSize: Int
    private var recordCount: Int
    private var lastChain: Data
    private var pending: [Data: OfflineJournalEntry]
    private var committed: Set<Data>

    public init(url: URL,
                key: OfflineJournalKey,
                configuration: OfflineJournalConfiguration = .init()) throws {
        self.fileURL = url
        self.configuration = configuration
        self.symmetricKey = SymmetricKey(data: key.raw)
        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true,
            attributes: nil
        )
        if !FileManager.default.fileExists(atPath: url.path) {
            FileManager.default.createFile(atPath: url.path, contents: nil, attributes: nil)
        }
        guard let handle = FileHandle(forUpdatingAtPath: url.path) else {
            throw OfflineJournalError.io(
                NSError(domain: NSCocoaErrorDomain, code: NSFileReadUnknownError, userInfo: [NSFilePathErrorKey: url.path])
            )
        }
        self.fileHandle = handle
        self.currentFileSize = 0
        self.recordCount = 0
        do {
            try OfflineJournal.ensureHeader(handle: handle)
            let fileSize64 = try handle.seekToEnd()
            if fileSize64 > UInt64(configuration.maxBytes) {
                throw OfflineJournalError.integrityViolation("journal exceeds max size \(configuration.maxBytes) bytes")
            }
            guard fileSize64 <= UInt64(Int.max) else {
                throw OfflineJournalError.integrityViolation("journal file size overflow")
            }
            self.currentFileSize = Int(fileSize64)
            try handle.seek(toOffset: UInt64(OfflineJournalFormat.headerLength))
            let state = try OfflineJournal.loadEntries(handle: handle,
                                                       fileSize: currentFileSize,
                                                       key: symmetricKey,
                                                       configuration: configuration)
            self.pending = state.pending
            self.committed = state.committed
            self.lastChain = state.lastChain
            self.recordCount = state.recordCount
        } catch {
            try? handle.close()
            throw error
        }
        let endOffset = try self.fileHandle.seekToEnd()
        currentFileSize = Int(endOffset)
    }

    deinit {
        try? fileHandle.close()
    }

    /// Append a pending entry (uses current timestamp).
    @discardableResult
    public func appendPending(txId: Data, payload: Data, timestampMs: UInt64? = nil) throws -> OfflineJournalEntry {
        try queue.sync {
            let normalizedId = try OfflineJournal.requireTxId(txId)
            if pending[normalizedId] != nil {
                throw OfflineJournalError.duplicatePending
            }
            if committed.contains(normalizedId) {
                throw OfflineJournalError.alreadyCommitted
            }
            if recordCount + 1 > configuration.maxRecords {
                throw OfflineJournalError.integrityViolation("journal exceeds max record limit \(configuration.maxRecords)")
            }
            let recordedAt = timestampMs ?? OfflineJournal.currentTimestamp()
            let chain = OfflineJournal.computeChain(prevChain: lastChain, txId: normalizedId)
            let record = try encodeRecord(
                kind: .pending,
                txId: normalizedId,
                payload: payload,
                timestampMs: recordedAt,
                chain: chain
            )
            try write(record: record)
            recordCount += 1
            lastChain = chain
            let entry = OfflineJournalEntry(txId: normalizedId, payload: payload, recordedAtMs: recordedAt, hashChain: chain)
            pending[normalizedId] = entry
            return entry
        }
    }

    /// Mark a pending entry as committed.
    public func markCommitted(txId: Data, timestampMs: UInt64? = nil) throws {
        try queue.sync {
            let normalizedId = try OfflineJournal.requireTxId(txId)
            guard pending.removeValue(forKey: normalizedId) != nil else {
            if committed.contains(normalizedId) {
                throw OfflineJournalError.alreadyCommitted
            }
            throw OfflineJournalError.notPending
        }
        if recordCount + 1 > configuration.maxRecords {
            throw OfflineJournalError.integrityViolation("journal exceeds max record limit \(configuration.maxRecords)")
        }
        let recordedAt = timestampMs ?? OfflineJournal.currentTimestamp()
        let chain = OfflineJournal.computeChain(prevChain: lastChain, txId: normalizedId)
        let record = try encodeRecord(
            kind: .committed,
            txId: normalizedId,
                payload: Data(),
                timestampMs: recordedAt,
            chain: chain
        )
        try write(record: record)
        recordCount += 1
        lastChain = chain
        committed.insert(normalizedId)
    }
    }

    /// Returns the currently pending entries sorted by `tx_id`.
    public func pendingEntries() -> [OfflineJournalEntry] {
        queue.sync {
            pending.values.sorted { $0.txId.lexicographicallyPrecedes($1.txId) }
        }
    }

    public var path: URL { fileURL }

    // MARK: - Private helpers

    private func encodeRecord(
        kind: OfflineJournalEntryKind,
        txId: Data,
        payload: Data,
        timestampMs: UInt64,
        chain: Data
    ) throws -> Data {
        var record = Data()
        record.append(kind.rawValue)
        record.append(contentsOf: timestampMs.littleEndianBytes)
        guard payload.count <= UInt32.max else {
            throw OfflineJournalError.payloadTooLarge
        }
        record.append(contentsOf: UInt32(payload.count).littleEndianBytes)
        record.append(txId)
        record.append(payload)
        record.append(chain)
        let hmac = OfflineJournal.computeHMAC(key: symmetricKey, prevChain: lastChain, record: record)
        record.append(hmac)
        return record
    }

    private func write(record: Data) throws {
        if currentFileSize + record.count > configuration.maxBytes {
            throw OfflineJournalError.integrityViolation("journal exceeds max size \(configuration.maxBytes) bytes")
        }
        do {
            try fileHandle.write(contentsOf: record)
            try fileHandle.synchronize()
            currentFileSize += record.count
        } catch {
            throw OfflineJournalError.io(error)
        }
    }

    private static func ensureHeader(handle: FileHandle) throws {
        try handle.seek(toOffset: 0)
        if let header = try readExactly(handle: handle, count: OfflineJournalFormat.headerLength) {
            let magic = header.prefix(OfflineJournalFormat.headerMagic.count)
            guard magic == OfflineJournalFormat.headerMagic,
                  header[OfflineJournalFormat.headerMagic.count] == OfflineJournalFormat.version else {
                throw OfflineJournalError.integrityViolation("header mismatch")
            }
        } else {
            try handle.seek(toOffset: 0)
            try handle.write(contentsOf: OfflineJournalFormat.headerMagic)
            try handle.write(contentsOf: [OfflineJournalFormat.version])
            try handle.synchronize()
        }
    }

    private static func loadEntries(
        handle: FileHandle,
        fileSize: Int,
        key: SymmetricKey,
        configuration: OfflineJournalConfiguration
    ) throws -> (pending: [Data: OfflineJournalEntry], committed: Set<Data>, lastChain: Data, recordCount: Int) {
        if fileSize < OfflineJournalFormat.headerLength {
            throw OfflineJournalError.integrityViolation("journal header truncated")
        }
        if fileSize == OfflineJournalFormat.headerLength {
            return ([:], [], Data(repeating: 0, count: OfflineJournalFormat.hashLength), 0)
        }

        try handle.seek(toOffset: UInt64(OfflineJournalFormat.headerLength))
        var offset = OfflineJournalFormat.headerLength
        var prevChain = Data(repeating: 0, count: OfflineJournalFormat.hashLength)
        var pending: [Data: OfflineJournalEntry] = [:]
        var committed = Set<Data>()
        var recordsRead = 0

        while offset < fileSize {
            if recordsRead >= configuration.maxRecords {
                throw OfflineJournalError.integrityViolation("journal exceeds max record limit \(configuration.maxRecords)")
            }
            guard let kindBytes = try readExactly(handle: handle, count: 1),
                  let kind = OfflineJournalEntryKind(rawValue: kindBytes[0]) else {
                throw OfflineJournalError.integrityViolation("unknown journal entry kind")
            }
            offset += 1

            guard let timestampBytes = try readExactly(handle: handle, count: 8) else {
                throw OfflineJournalError.integrityViolation("truncated journal record")
            }
            let recordedAt = timestampBytes.readUInt64LE(at: 0)
            offset += 8

            guard let payloadLenBytes = try readExactly(handle: handle, count: 4) else {
                throw OfflineJournalError.integrityViolation("truncated journal payload length")
            }
            let payloadLen = Int(payloadLenBytes.readUInt32LE(at: 0))
            offset += 4
            if payloadLen > configuration.maxBytes {
                throw OfflineJournalError.integrityViolation("journal payload exceeds max size")
            }

            guard let txId = try readExactly(handle: handle, count: OfflineJournalFormat.hashLength) else {
                throw OfflineJournalError.integrityViolation("truncated journal tx_id")
            }
            offset += OfflineJournalFormat.hashLength

            let remaining = fileSize - offset
            let required = payloadLen + OfflineJournalFormat.hashLength + OfflineJournalFormat.hmacLength
            guard required <= remaining else {
                throw OfflineJournalError.integrityViolation("truncated journal payload")
            }

            guard let payload = try readExactly(handle: handle, count: payloadLen) else {
                throw OfflineJournalError.integrityViolation("truncated journal payload bytes")
            }
            offset += payloadLen

            guard let storedChain = try readExactly(handle: handle, count: OfflineJournalFormat.hashLength) else {
                throw OfflineJournalError.integrityViolation("truncated journal chain")
            }
            offset += OfflineJournalFormat.hashLength

            guard let storedHmac = try readExactly(handle: handle, count: OfflineJournalFormat.hmacLength) else {
                throw OfflineJournalError.integrityViolation("truncated journal hmac")
            }
            offset += OfflineJournalFormat.hmacLength

            var record = Data()
            record.append(kindBytes)
            record.append(contentsOf: recordedAt.littleEndianBytes)
            record.append(contentsOf: UInt32(payloadLen).littleEndianBytes)
            record.append(txId)
            record.append(payload)
            record.append(storedChain)

            let expectedChain = computeChain(prevChain: prevChain, txId: txId)
            guard storedChain == expectedChain else {
                throw OfflineJournalError.integrityViolation("hash chain mismatch for tx \(txId.hexEncodedString())")
            }
            let expectedHmac = computeHMAC(key: key, prevChain: prevChain, record: record)
            guard storedHmac == expectedHmac else {
                throw OfflineJournalError.integrityViolation("HMAC mismatch for tx \(txId.hexEncodedString())")
            }

            switch kind {
            case .pending:
                let entry = OfflineJournalEntry(
                    txId: txId,
                    payload: payload,
                    recordedAtMs: recordedAt,
                    hashChain: storedChain
                )
                pending[txId] = entry
            case .committed:
                pending.removeValue(forKey: txId)
                committed.insert(txId)
            }
            prevChain = storedChain
            recordsRead += 1
        }

        return (pending, committed, prevChain, recordsRead)
    }

    private static func readExactly(handle: FileHandle, count: Int) throws -> Data? {
        if count == 0 {
            return Data()
        }
        var remaining = count
        var buffer = Data(capacity: count)
        while remaining > 0 {
            let chunk = try handle.read(upToCount: remaining)
            guard let chunk, !chunk.isEmpty else {
                if buffer.isEmpty {
                    return nil
                }
                throw OfflineJournalError.integrityViolation("truncated journal record")
            }
            buffer.append(chunk)
            remaining -= chunk.count
        }
        return buffer
    }

    private static func computeChain(prevChain: Data, txId: Data) -> Data {
        var input = Data(capacity: prevChain.count + txId.count)
        input.append(prevChain)
        input.append(txId)
        return Blake2b.hash256(input)
    }

    private static func computeHMAC(key: SymmetricKey, prevChain: Data, record: Data) -> Data {
        var hmac = HMAC<SHA256>(key: key)
        hmac.update(data: prevChain)
        hmac.update(data: record)
        return Data(hmac.finalize())
    }

    private static func currentTimestamp() -> UInt64 {
        let seconds = Date().timeIntervalSince1970
        return UInt64(seconds * 1_000)
    }

    private static func requireTxId(_ txId: Data) throws -> Data {
        guard txId.count == OfflineJournalFormat.hashLength else {
            throw OfflineJournalError.invalidTxIdLength
        }
        return Data(txId)
    }
}

// MARK: - Byte helpers

private extension UInt64 {
    var littleEndianBytes: [UInt8] {
        var value = self.littleEndian
        return withUnsafeBytes(of: &value) { Array($0) }
    }
}

private extension UInt32 {
    var littleEndianBytes: [UInt8] {
        var value = self.littleEndian
        return withUnsafeBytes(of: &value) { Array($0) }
    }
}

private extension Data {
    func readUInt64LE(at index: Int) -> UInt64 {
        precondition(index + 8 <= count)
        var value: UInt64 = 0
        for i in 0..<8 {
            value |= UInt64(self[index + i]) << (UInt64(i) * 8)
        }
        return value
    }

    func readUInt32LE(at index: Int) -> UInt32 {
        precondition(index + 4 <= count)
        var value: UInt32 = 0
        for i in 0..<4 {
            value |= UInt32(self[index + i]) << (UInt32(i) * 8)
        }
        return value
    }

    func hexEncodedString() -> String {
        map { String(format: "%02x", $0) }.joined()
    }
}
