import Foundation

public enum FilePendingTransactionQueueError: Error, LocalizedError {
    case corruptedEntry(index: Int)
    case unreadableData
    case overflowRecords(limit: Int)
    case overflowBytes(limit: Int)

    public var errorDescription: String? {
        switch self {
        case .corruptedEntry(let index):
            return "Pending queue entry \(index) is corrupted."
        case .unreadableData:
            return "Pending queue file contains unreadable data."
        case .overflowRecords(let limit):
            return "Pending queue exceeded the configured record limit (\(limit))."
        case .overflowBytes(let limit):
            return "Pending queue file exceeded the configured size limit (\(limit) bytes)."
        }
    }
}

public struct FilePendingTransactionQueueConfiguration: Sendable {
    public var maxRecords: Int
    public var maxBytes: Int

    public init(maxRecords: Int = 256, maxBytes: Int = 1 << 20) {
        self.maxRecords = maxRecords
        self.maxBytes = maxBytes
    }
}

/// File-backed queue that stores base64-encoded JSON records (one per line).
public final class FilePendingTransactionQueue: PendingTransactionQueue {
    private let fileURL: URL
    private let configuration: FilePendingTransactionQueueConfiguration
    private let ioQueue = DispatchQueue(label: "org.hyperledger.irohaSwift.FilePendingTransactionQueue")
    private let encoder = JSONEncoder()
    private let decoder = JSONDecoder()

    public init(fileURL: URL, configuration: FilePendingTransactionQueueConfiguration = .init()) throws {
        self.fileURL = fileURL
        self.configuration = configuration
        try ioQueue.sync {
            try prepareFile()
        }
    }

    public func enqueue(_ envelope: SignedTransactionEnvelope) throws {
        try enqueue(PendingTransactionRecord(envelope: envelope))
    }

    public func enqueue(_ record: PendingTransactionRecord) throws {
        try ioQueue.sync {
            let data = try encoder.encode(record)
            let lineData = data.base64EncodedData()
            let projectedSize = try projectedFileSize(additionalBytes: lineData.count + 1)
            if projectedSize > configuration.maxBytes {
                throw FilePendingTransactionQueueError.overflowBytes(limit: configuration.maxBytes)
            }
            if try currentSize() >= configuration.maxRecords {
                throw FilePendingTransactionQueueError.overflowRecords(limit: configuration.maxRecords)
            }
            try appendLine(lineData)
        }
    }

    public func drain() throws -> [PendingTransactionRecord] {
        try ioQueue.sync {
            guard FileManager.default.fileExists(atPath: fileURL.path) else { return [] }
            var records: [PendingTransactionRecord] = []
            try forEachLine { line, index in
                guard !line.isEmpty else {
                    throw FilePendingTransactionQueueError.corruptedEntry(index: index)
                }
                guard let decoded = Data(base64Encoded: line) else {
                    throw FilePendingTransactionQueueError.corruptedEntry(index: index)
                }
                let record = try decoder.decode(PendingTransactionRecord.self, from: decoded)
                records.append(record)
            }
            try clearFile()
            return records
        }
    }

    public func size() throws -> Int {
        try ioQueue.sync { try currentSize() }
    }

    public func clear() throws {
        try ioQueue.sync {
            try clearFile()
        }
    }

    private func prepareFile() throws {
        let directory = fileURL.deletingLastPathComponent()
        if !FileManager.default.fileExists(atPath: directory.path) {
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        }
        if !FileManager.default.fileExists(atPath: fileURL.path) {
            FileManager.default.createFile(atPath: fileURL.path, contents: nil)
        }
    }

    private func appendLine(_ line: Data) throws {
        var data = line
        data.append(0x0A)
        let handle = try FileHandle(forWritingTo: fileURL)
        defer { handle.closeFile() }
        handle.seekToEndOfFile()
        handle.write(data)
    }

    private func clearFile() throws {
        try Data().write(to: fileURL, options: .atomic)
    }

    private func projectedFileSize(additionalBytes: Int) throws -> Int {
        let attributes = try FileManager.default.attributesOfItem(atPath: fileURL.path)
        let size = (attributes[.size] as? NSNumber)?.intValue ?? 0
        return size + additionalBytes
    }

    private func currentSize() throws -> Int {
        guard FileManager.default.fileExists(atPath: fileURL.path) else { return 0 }
        var count = 0
        try forEachLine { _, _ in
            count += 1
            if count > configuration.maxRecords {
                throw FilePendingTransactionQueueError.overflowRecords(limit: configuration.maxRecords)
            }
        }
        return count
    }

    private func forEachLine(_ body: (Data, Int) throws -> Void) throws {
        let attributes = try FileManager.default.attributesOfItem(atPath: fileURL.path)
        let fileSize = (attributes[.size] as? NSNumber)?.intValue ?? 0
        if fileSize > configuration.maxBytes {
            throw FilePendingTransactionQueueError.overflowBytes(limit: configuration.maxBytes)
        }
        guard fileSize > 0 else { return }

        let handle = try FileHandle(forReadingFrom: fileURL)
        defer { try? handle.close() }
        var buffer = Data()
        var lineIndex = 0
        let newline: UInt8 = 0x0A
        while let chunk = try handle.read(upToCount: 4096), !chunk.isEmpty {
            buffer.append(chunk)
            while let newlineIndex = buffer.firstIndex(of: newline) {
                let line = buffer.prefix(upTo: newlineIndex)
                buffer.removeSubrange(0...newlineIndex)
                if lineIndex >= configuration.maxRecords {
                    throw FilePendingTransactionQueueError.overflowRecords(limit: configuration.maxRecords)
                }
                try body(Data(line), lineIndex)
                lineIndex += 1
            }
            if buffer.count > configuration.maxBytes {
                throw FilePendingTransactionQueueError.overflowBytes(limit: configuration.maxBytes)
            }
        }
        if !buffer.isEmpty {
            if lineIndex >= configuration.maxRecords {
                throw FilePendingTransactionQueueError.overflowRecords(limit: configuration.maxRecords)
            }
            try body(buffer, lineIndex)
        }
    }
}
