import Foundation

public struct OfflineAuditEntry: Codable, Sendable, Equatable {
    public let txId: String
    public let senderId: String
    public let receiverId: String
    public let assetId: String
    public let amount: String
    public let timestampMs: UInt64

    public init(txId: String,
                senderId: String,
                receiverId: String,
                assetId: String,
                amount: String,
                timestampMs: UInt64) {
        self.txId = txId
        self.senderId = senderId
        self.receiverId = receiverId
        self.assetId = assetId
        self.amount = amount
        self.timestampMs = timestampMs
    }
}

public enum OfflineAuditLoggerError: Swift.Error {
    case storageUnavailable(URL)
    case serializationFailed(Swift.Error)
}

public final class OfflineAuditLogger: @unchecked Sendable {
    private let queue = DispatchQueue(label: "org.hyperledger.iroha.offline-audit-logger", qos: .utility)
    private var entries: [OfflineAuditEntry]
    private let encoder: JSONEncoder
    private let decoder: JSONDecoder
    private let fileURL: URL
    private var dirty = false

    private var isEnabled: Bool

    public init(storageURL: URL? = nil, isEnabled: Bool = false) throws {
        self.isEnabled = isEnabled
        self.fileURL = storageURL ?? OfflineAuditLogger.defaultStorageURL()
        self.encoder = JSONEncoder()
        self.encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        self.decoder = JSONDecoder()
        self.entries = []

        try FileManager.default.createDirectory(at: fileURL.deletingLastPathComponent(),
                                                withIntermediateDirectories: true,
                                                attributes: nil)
        if FileManager.default.fileExists(atPath: fileURL.path) {
            let data = try Data(contentsOf: fileURL)
            if !data.isEmpty {
                self.entries = try decoder.decode([OfflineAuditEntry].self, from: data)
            }
        }
    }

    public static func defaultStorageURL() -> URL {
        let directory = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first
            ?? FileManager.default.temporaryDirectory
        return directory.appendingPathComponent("offline_audit_log.json", isDirectory: false)
    }

    public var storageURL: URL {
        fileURL
    }

    public func currentEnabled() -> Bool {
        queue.sync { isEnabled }
    }

    public func setEnabled(_ enabled: Bool) {
        queue.sync {
            self.isEnabled = enabled
            if !enabled {
                self.dirty = false
            }
        }
    }

    public func record(entry: OfflineAuditEntry) {
        queue.async {
            guard self.isEnabled else { return }
            self.entries.append(entry)
            self.dirty = true
            try? self.persistIfNeeded()
        }
    }

    public func exportEntries() -> [OfflineAuditEntry] {
        queue.sync { entries }
    }

    public func exportJSON(prettyPrinted: Bool = true) throws -> Data {
        try queue.sync {
            if prettyPrinted {
                encoder.outputFormatting.insert(.prettyPrinted)
            } else {
                encoder.outputFormatting.remove(.prettyPrinted)
            }
            do {
                return try encoder.encode(entries)
            } catch {
                throw OfflineAuditLoggerError.serializationFailed(error)
            }
        }
    }

    public func clear() throws {
        try queue.sync {
            entries.removeAll()
            dirty = true
            try persistIfNeeded(force: true)
        }
    }

    private func persistIfNeeded(force: Bool = false) throws {
        if dirty || force {
            do {
                let data = try encoder.encode(entries)
                try data.write(to: fileURL, options: [.atomic])
                dirty = false
            } catch {
                throw OfflineAuditLoggerError.serializationFailed(error)
            }
        }
    }
}
