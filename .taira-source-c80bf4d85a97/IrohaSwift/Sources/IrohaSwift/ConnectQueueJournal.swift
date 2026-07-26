import Foundation

/// Journal record persisted for Connect queue replay.
public struct ConnectJournalRecord: Equatable, Sendable {
    static let schemaName = "ConnectJournalRecordV1"
    static let schemaHash = noritoSchemaHash(forTypeName: schemaName)
    static let headerLength = 40
    static let maxHeaderPadding = 64

    public let direction: ConnectDirection
    public let sequence: UInt64
    public let payloadHash: Data
    public let ciphertext: Data
    public let receivedAtMs: UInt64
    public let expiresAtMs: UInt64

    init(direction: ConnectDirection,
         sequence: UInt64,
         payloadHash: Data,
         ciphertext: Data,
         receivedAtMs: UInt64,
         expiresAtMs: UInt64) throws {
        guard payloadHash.count == 32 else {
            throw ConnectQueueError.corrupted
        }
        guard ciphertext.count <= Int(UInt32.max) else {
            throw ConnectQueueError.corrupted
        }
        self.direction = direction
        self.sequence = sequence
        self.payloadHash = payloadHash
        self.ciphertext = ciphertext
        self.receivedAtMs = receivedAtMs
        self.expiresAtMs = expiresAtMs
    }

    var encodedSize: Int {
        let payloadLength = 1 + 8 + 8 + 8 + 4 + 32 + ciphertext.count
        return Self.headerLength + payloadLength
    }

    func encodeEnvelope() throws -> Data {
        var payload = Data(capacity: encodedSize - Self.headerLength)
        payload.append(direction == .appToWallet ? 0 : 1)
        payload.appendLittleEndian(sequence)
        payload.appendLittleEndian(receivedAtMs)
        payload.appendLittleEndian(expiresAtMs)
        payload.appendLittleEndian(UInt32(ciphertext.count))
        payload.append(payloadHash)
        payload.append(ciphertext)
        return noritoEncode(typeName: Self.schemaName, payload: payload)
    }

    static func decode(from data: Data, offset: Int) throws -> (record: ConnectJournalRecord, bytes: Int) {
        guard data.count >= offset + headerLength else {
            throw ConnectQueueError.corrupted
        }
        var cursor = offset
        guard data[cursor..<(cursor + 4)] == NoritoHeader.magic else {
            throw ConnectQueueError.corrupted
        }
        cursor += 4
        let major = data[cursor]
        let minor = data[cursor + 1]
        guard major == NoritoHeader.versionMajor, minor == NoritoHeader.versionMinor else {
            throw ConnectQueueError.corrupted
        }
        cursor += 2
        let schemaSlice = data[cursor..<(cursor + 16)]
        guard Array(schemaSlice) == schemaHash else {
            throw ConnectQueueError.corrupted
        }
        cursor += 16
        let compression = data[cursor]
        cursor += 1
        guard compression == NoritoCompression.none.rawValue else {
            throw ConnectQueueError.corrupted
        }
        let payloadLength = try data.readUInt64LE(at: cursor)
        cursor += 8
        let checksum = try data.readUInt64LE(at: cursor)
        cursor += 8
        cursor += 1 // flags
        let (payload, padding) = try payloadWithPadding(data: data,
                                                        offset: offset,
                                                        payloadLength: payloadLength,
                                                        checksum: checksum)

        var payloadCursor = payload.startIndex
        guard payload.count >= 1 + 8 + 8 + 8 + 4 + 32 else {
            throw ConnectQueueError.corrupted
        }
        let directionByte = payload[payloadCursor]
        payloadCursor += 1
        let direction: ConnectDirection
        switch directionByte {
        case 0: direction = .appToWallet
        case 1: direction = .walletToApp
        default: throw ConnectQueueError.corrupted
        }
        let sequence = try payload.readUInt64LE(at: payloadCursor - payload.startIndex)
        payloadCursor += 8
        let receivedAt = try payload.readUInt64LE(at: payloadCursor - payload.startIndex)
        payloadCursor += 8
        let expiresAt = try payload.readUInt64LE(at: payloadCursor - payload.startIndex)
        payloadCursor += 8
        let ciphertextLength = try payload.readUInt32LE(at: payloadCursor - payload.startIndex)
        payloadCursor += 4
        let hashStart = payloadCursor
        let hashEnd = payloadCursor + 32
        guard payload.count >= hashEnd else {
            throw ConnectQueueError.corrupted
        }
        let hash = Data(payload[hashStart..<hashEnd])
        payloadCursor = hashEnd
        guard payload.count >= payloadCursor + Int(ciphertextLength) else {
            throw ConnectQueueError.corrupted
        }
        let ciphertext = Data(payload[payloadCursor..<(payloadCursor + Int(ciphertextLength))])
        let record = try ConnectJournalRecord(direction: direction,
                                              sequence: sequence,
                                              payloadHash: hash,
                                              ciphertext: ciphertext,
                                              receivedAtMs: receivedAt,
                                              expiresAtMs: expiresAt)
        return (record, headerLength + padding + Int(payloadLength))
    }

    private static func payloadWithPadding(data: Data,
                                           offset: Int,
                                           payloadLength: UInt64,
                                           checksum: UInt64) throws -> (payload: Data, padding: Int) {
        guard payloadLength <= UInt64(Int.max) else {
            throw ConnectQueueError.corrupted
        }
        let headerEnd = offset + headerLength
        let payloadLen = Int(payloadLength)
        let maxAvailable = data.count - headerEnd - payloadLen
        guard maxAvailable >= 0 else {
            throw ConnectQueueError.corrupted
        }
        let maxPadding = min(maxHeaderPadding, maxAvailable)
        if maxPadding < 0 {
            throw ConnectQueueError.corrupted
        }
        for padding in 0...maxPadding {
            if padding > 0 {
                let padRange = headerEnd..<(headerEnd + padding)
                if data[padRange].contains(where: { $0 != 0 }) {
                    continue
                }
            }
            let payloadStart = headerEnd + padding
            let payloadEnd = payloadStart + payloadLen
            if payloadEnd > data.count {
                break
            }
            let payload = Data(data[payloadStart..<payloadEnd])
            if crc64ECMA(payload) == checksum {
                return (payload, padding)
            }
        }
        throw ConnectQueueError.corrupted
    }
}

/// Append-only journal wiring for Connect queue replay.
public final class ConnectQueueJournal {
    public struct Configuration: Sendable {
        public var rootDirectory: URL
        public var maxRecordsPerQueue: Int
        public var maxBytesPerQueue: Int
        public var retentionInterval: TimeInterval

        public init(rootDirectory: URL = ConnectSessionDiagnostics.defaultRootDirectory(),
                    maxRecordsPerQueue: Int = 32,
                    maxBytesPerQueue: Int = 1 << 20,
                    retentionInterval: TimeInterval = 24 * 60 * 60) {
            self.rootDirectory = rootDirectory
            self.maxRecordsPerQueue = maxRecordsPerQueue
            self.maxBytesPerQueue = maxBytesPerQueue
            self.retentionInterval = retentionInterval
        }
    }

    private let storage: ConnectQueueStorage
    private let configuration: Configuration
    private let appFile: ConnectJournalFile
    private let walletFile: ConnectJournalFile

    public init(sessionID: Data,
                configuration: Configuration = Configuration(),
                fileManager: FileManager = .default) {
        let diagnosticsConfig = ConnectSessionDiagnostics.Configuration(rootDirectory: configuration.rootDirectory)
        self.storage = ConnectQueueStorage(sessionID: sessionID,
                                           configuration: diagnosticsConfig,
                                           fileManager: fileManager)
        self.configuration = configuration
        self.appFile = ConnectJournalFile(direction: .appToWallet,
                                          storage: storage,
                                          configuration: configuration,
                                          fileManager: fileManager)
        self.walletFile = ConnectJournalFile(direction: .walletToApp,
                                             storage: storage,
                                             configuration: configuration,
                                             fileManager: fileManager)
    }

    /// Append a ciphertext frame to the on-disk journal.
    public func append(direction: ConnectDirection,
                       sequence: UInt64,
                       ciphertext: Data,
                       receivedAtMs: UInt64 = ConnectQueueJournal.timestampNow(),
                       ttlOverrideMs: UInt64? = nil) throws {
        let ttlMs = ttlOverrideMs ?? Self.clampedRetentionIntervalMs(configuration.retentionInterval)
        let (expiresAt, overflow) = receivedAtMs.addingReportingOverflow(ttlMs)
        let expiresAtMs = overflow ? UInt64.max : expiresAt
        let digest = try ConnectQueueJournal.payloadHash(ciphertext)
        let record = try ConnectJournalRecord(direction: direction,
                                              sequence: sequence,
                                              payloadHash: digest,
                                              ciphertext: ciphertext,
                                              receivedAtMs: receivedAtMs,
                                              expiresAtMs: expiresAtMs)
        try file(for: direction).append(record: record, nowMs: receivedAtMs)
    }

    /// Return all non-expired records for the requested direction.
    public func records(direction: ConnectDirection,
                        nowMs: UInt64 = ConnectQueueJournal.timestampNow()) throws -> [ConnectJournalRecord] {
        try file(for: direction).records(nowMs: nowMs)
    }

    /// Remove and return up to `count` of the oldest records for the requested direction.
    @discardableResult
    public func popOldest(direction: ConnectDirection,
                          count: Int,
                          nowMs: UInt64 = ConnectQueueJournal.timestampNow()) throws -> [ConnectJournalRecord] {
        guard count > 0 else {
            throw ConnectQueueError.invalidCount(count)
        }
        return try file(for: direction).popOldest(count: count, nowMs: nowMs)
    }

    public static func timestampNow() -> UInt64 {
        UInt64(Date().timeIntervalSince1970 * 1000)
    }

    private static func clampedRetentionIntervalMs(_ interval: TimeInterval) -> UInt64 {
        if interval.isNaN {
            return 0
        }
        if interval.isInfinite {
            return interval > 0 ? UInt64.max : 0
        }
        let millis = interval * 1000
        if millis <= 0 {
            return 0
        }
        if millis >= Double(UInt64.max) {
            return UInt64.max
        }
        return UInt64(millis.rounded(.towardZero))
    }

    private func file(for direction: ConnectDirection) -> ConnectJournalFile {
        switch direction {
        case .appToWallet: return appFile
        case .walletToApp: return walletFile
        }
    }

    private static func payloadHash(_ ciphertext: Data) throws -> Data {
        guard let digest = NoritoNativeBridge.shared.blake3Hash(data: ciphertext),
              digest.count == 32 else {
            throw ConnectQueueError.hashUnavailable
        }
        return digest
    }
}

private final class ConnectJournalFile {
    private let direction: ConnectDirection
    private let storage: ConnectQueueStorage
    private let configuration: ConnectQueueJournal.Configuration
    private let fileManager: FileManager
    private let queue: DispatchQueue

    init(direction: ConnectDirection,
         storage: ConnectQueueStorage,
         configuration: ConnectQueueJournal.Configuration,
         fileManager: FileManager) {
        self.direction = direction
        self.storage = storage
        self.configuration = configuration
        self.fileManager = fileManager
        self.queue = DispatchQueue(label: "org.hyperledger.iroha.connect.queue.\(direction)")
    }

    func append(record: ConnectJournalRecord, nowMs: UInt64) throws {
        try queue.sync {
            try storage.ensureSessionDirectory()
            var records = try readLocked()
            records.append(record)
            let (clamped, _) = clamp(records: records, nowMs: nowMs)
            try writeLocked(records: clamped)
        }
    }

    func records(nowMs: UInt64) throws -> [ConnectJournalRecord] {
        try queue.sync {
            var records = try readLocked()
            let (clamped, changed) = clamp(records: records, nowMs: nowMs)
            records = clamped
            if changed {
                try writeLocked(records: records)
            }
            return records
        }
    }

    func popOldest(count: Int, nowMs: UInt64) throws -> [ConnectJournalRecord] {
        try queue.sync {
            var records = try readLocked()
            let (clamped, changed) = clamp(records: records, nowMs: nowMs)
            records = clamped
            let slice = Array(records.prefix(count))
            records.removeFirst(min(count, records.count))
            if changed || !slice.isEmpty {
                try writeLocked(records: records)
            }
            return slice
        }
    }

    private var fileURL: URL {
        switch direction {
        case .appToWallet:
            return storage.appQueueURL
        case .walletToApp:
            return storage.walletQueueURL
        }
    }

    private func readLocked() throws -> [ConnectJournalRecord] {
        guard fileManager.fileExists(atPath: fileURL.path) else {
            return []
        }
        let attributes = try fileManager.attributesOfItem(atPath: fileURL.path)
        if let fileSize = attributes[.size] as? NSNumber,
           fileSize.intValue > configuration.maxBytesPerQueue {
            throw ConnectQueueError.overflow(limit: configuration.maxBytesPerQueue)
        }
        let contents = try Data(contentsOf: fileURL)
        if contents.count > configuration.maxBytesPerQueue {
            throw ConnectQueueError.overflow(limit: configuration.maxBytesPerQueue)
        }
        var records: [ConnectJournalRecord] = []
        var offset = 0
        while offset < contents.count {
            if records.count >= configuration.maxRecordsPerQueue {
                throw ConnectQueueError.overflow(limit: configuration.maxRecordsPerQueue)
            }
            let (record, consumed) = try ConnectJournalRecord.decode(from: contents, offset: offset)
            guard consumed > 0 else {
                throw ConnectQueueError.corrupted
            }
            offset += consumed
            records.append(record)
        }
        return records
    }

    private func writeLocked(records: [ConnectJournalRecord]) throws {
        guard !records.isEmpty else {
            if fileManager.fileExists(atPath: fileURL.path) {
                try fileManager.removeItem(at: fileURL)
            }
            return
        }
        var payload = Data()
        payload.reserveCapacity(records.reduce(0) { $0 + $1.encodedSize })
        for record in records {
            payload.append(try record.encodeEnvelope())
        }
        try payload.write(to: fileURL, options: .atomic)
    }

    private func clamp(records: [ConnectJournalRecord],
                       nowMs: UInt64) -> ([ConnectJournalRecord], Bool) {
        let filtered = records.filter { $0.expiresAtMs > nowMs }
        var changed = filtered.count != records.count
        var clamped = filtered
        while clamped.count > configuration.maxRecordsPerQueue {
            clamped.removeFirst()
            changed = true
        }
        while totalEncodedSize(clamped) > configuration.maxBytesPerQueue, !clamped.isEmpty {
            clamped.removeFirst()
            changed = true
        }
        return (clamped, changed)
    }

    private func totalEncodedSize(_ records: [ConnectJournalRecord]) -> Int {
        records.reduce(0) { $0 + $1.encodedSize }
    }

}

private extension Data {
    mutating func appendLittleEndian<T: FixedWidthInteger>(_ value: T) {
        var le = value.littleEndian
        Swift.withUnsafeBytes(of: &le) { append(contentsOf: $0) }
    }

    func readUInt64LE(at offset: Int) throws -> UInt64 {
        guard offset >= 0 else {
            throw ConnectQueueError.corrupted
        }
        guard let start = index(startIndex, offsetBy: offset, limitedBy: endIndex),
              let end = index(start, offsetBy: 8, limitedBy: endIndex) else {
            throw ConnectQueueError.corrupted
        }
        var value: UInt64 = 0
        self[start..<end].withUnsafeBytes { buffer in
            guard let base = buffer.baseAddress else { return }
            memcpy(&value, base, 8)
        }
        return UInt64(littleEndian: value)
    }

    func readUInt32LE(at offset: Int) throws -> UInt32 {
        guard offset >= 0 else {
            throw ConnectQueueError.corrupted
        }
        guard let start = index(startIndex, offsetBy: offset, limitedBy: endIndex),
              let end = index(start, offsetBy: 4, limitedBy: endIndex) else {
            throw ConnectQueueError.corrupted
        }
        var value: UInt32 = 0
        self[start..<end].withUnsafeBytes { buffer in
            guard let base = buffer.baseAddress else { return }
            memcpy(&value, base, 4)
        }
        return UInt32(littleEndian: value)
    }
}
