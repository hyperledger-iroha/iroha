import Foundation

public struct OfflineVerdictRevocationEntry: Codable, Sendable, Equatable {
    public let verdictIdHex: String
    public let issuerId: String?
    public let issuerDisplay: String?
    public let revokedAtMs: UInt64
    public let reason: String?
    public let note: String?
    public let metadata: ToriiJSONValue?
    public let recordedAtMs: UInt64

    public init(verdictIdHex: String,
                issuerId: String?,
                issuerDisplay: String?,
                revokedAtMs: UInt64,
                reason: String?,
                note: String?,
                metadata: ToriiJSONValue?,
                recordedAtMs: UInt64) {
        self.verdictIdHex = verdictIdHex
        self.issuerId = issuerId
        self.issuerDisplay = issuerDisplay
        self.revokedAtMs = revokedAtMs
        self.reason = reason
        self.note = note
        self.metadata = metadata
        self.recordedAtMs = recordedAtMs
    }
}

/// Thread-safe store of verdict revocations for offline denial checks.
public final class OfflineRevocationJournal {
    private let queue = DispatchQueue(label: "org.hyperledger.iroha.offline-revocation-journal",
                                      qos: .utility)
    private var entries: [String: OfflineVerdictRevocationEntry]
    private let encoder: JSONEncoder
    private let decoder: JSONDecoder
    public let storageURL: URL

    public init(storageURL: URL? = nil) throws {
        self.storageURL = storageURL ?? OfflineRevocationJournal.defaultStorageURL()
        self.entries = [:]
        self.encoder = JSONEncoder()
        self.encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        self.decoder = JSONDecoder()
        try loadFromDisk()
    }

    public static func defaultStorageURL() -> URL {
        let base = FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? FileManager.default.temporaryDirectory
        return base
            .appendingPathComponent("iroha_offline_revocations", isDirectory: true)
            .appendingPathComponent("journal.json", isDirectory: false)
    }

    @discardableResult
    public func upsert(revocations: [ToriiOfflineVerdictRevocation],
                       recordedAtMs: UInt64) throws -> [OfflineVerdictRevocationEntry] {
        try queue.sync {
            var inserted: [OfflineVerdictRevocationEntry] = []
            for revocation in revocations {
                guard let verdictIdHex = Self.normalizeVerdictIdHex(revocation.verdictIdHex) else {
                    continue
                }
                let entry = OfflineVerdictRevocationEntry(
                    verdictIdHex: verdictIdHex,
                    issuerId: revocation.issuerId,
                    issuerDisplay: revocation.issuerDisplay,
                    revokedAtMs: revocation.revokedAtMs,
                    reason: revocation.reason,
                    note: revocation.note,
                    metadata: revocation.metadata,
                    recordedAtMs: recordedAtMs
                )
                entries[verdictIdHex] = entry
                inserted.append(entry)
            }
            try persistLocked()
            return inserted
        }
    }

    @discardableResult
    public func upsert(rawRevocations: [ToriiJSONValue],
                       recordedAtMs: UInt64) throws -> [OfflineVerdictRevocationEntry] {
        try queue.sync {
            var inserted: [OfflineVerdictRevocationEntry] = []
            for revocation in rawRevocations {
                guard let entry = Self.parseRawRevocation(revocation, recordedAtMs: recordedAtMs) else {
                    continue
                }
                entries[entry.verdictIdHex] = entry
                inserted.append(entry)
            }
            try persistLocked()
            return inserted
        }
    }

    public func isRevoked(verdictIdHex: String) -> Bool {
        queue.sync {
            guard let normalized = Self.normalizeVerdictIdHex(verdictIdHex) else {
                return false
            }
            return entries[normalized] != nil
        }
    }

    public func revocation(for verdictIdHex: String) -> OfflineVerdictRevocationEntry? {
        queue.sync {
            guard let normalized = Self.normalizeVerdictIdHex(verdictIdHex) else {
                return nil
            }
            return entries[normalized]
        }
    }

    public func snapshot() -> [OfflineVerdictRevocationEntry] {
        queue.sync { Array(entries.values) }
    }

    private func loadFromDisk() throws {
        guard FileManager.default.fileExists(atPath: storageURL.path) else {
            entries = [:]
            return
        }
        let data = try Data(contentsOf: storageURL)
        if data.isEmpty {
            entries = [:]
            return
        }
        entries = try decoder.decode([String: OfflineVerdictRevocationEntry].self, from: data)
    }

    private func persistLocked() throws {
        let directory = storageURL.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: directory,
                                                withIntermediateDirectories: true,
                                                attributes: nil)
        let data = try encoder.encode(entries)
        try data.write(to: storageURL, options: [.atomic])
    }

    private static func parseRawRevocation(_ value: ToriiJSONValue,
                                           recordedAtMs: UInt64) -> OfflineVerdictRevocationEntry? {
        guard case let .object(object) = value else { return nil }
        let verdictValue = object["verdict_id_hex"]
        guard let verdictIdHex = normalizeVerdictIdHex(verdictValue) else { return nil }
        guard let revokedAtMs = object["revoked_at_ms"]?.normalizedUInt64 else {
            return nil
        }
        let issuerId = object["issuer_id"]?.normalizedString
        let issuerDisplay = object["issuer_display"]?.normalizedString
        let reason = object["reason"]?.normalizedString
        let note = object["note"]?.normalizedString
        let metadata = object["metadata"]

        return OfflineVerdictRevocationEntry(
            verdictIdHex: verdictIdHex,
            issuerId: issuerId,
            issuerDisplay: issuerDisplay,
            revokedAtMs: revokedAtMs,
            reason: reason,
            note: note,
            metadata: metadata,
            recordedAtMs: recordedAtMs
        )
    }

    private static func normalizeVerdictIdHex(_ value: ToriiJSONValue?) -> String? {
        guard let raw = value?.normalizedString else { return nil }
        return normalizeVerdictIdHex(raw)
    }

    private static func normalizeVerdictIdHex(_ raw: String?) -> String? {
        guard var trimmed = raw?.trimmingCharacters(in: .whitespacesAndNewlines),
              !trimmed.isEmpty else {
            return nil
        }
        if trimmed.lowercased().hasPrefix("hash:") {
            return parseHashLiteralBody(trimmed)
        }
        if trimmed.hasPrefix("0x") || trimmed.hasPrefix("0X") {
            trimmed = String(trimmed.dropFirst(2))
        }
        guard trimmed.count == 64, Data(hexString: trimmed) != nil else {
            return nil
        }
        return trimmed.lowercased()
    }

    private static func parseHashLiteralBody(_ literal: String) -> String? {
        let trimmed = literal.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.lowercased().hasPrefix("hash:"),
              let separator = trimmed.lastIndex(of: "#") else {
            return nil
        }
        let bodyStart = trimmed.index(trimmed.startIndex, offsetBy: 5)
        let body = String(trimmed[bodyStart..<separator])
        let checksum = String(trimmed[trimmed.index(after: separator)...])
        guard body.count == 64, Data(hexString: body) != nil else {
            return nil
        }
        guard checksum.count == 4, let provided = UInt16(checksum, radix: 16) else {
            return nil
        }
        let expected = crc16(tag: "hash", body: body.uppercased())
        guard expected == provided else {
            return nil
        }
        return body.lowercased()
    }

    private static func crc16(tag: String, body: String) -> UInt16 {
        var crc: UInt16 = 0xFFFF
        for byte in tag.utf8 {
            crc = updateCrc(crc, value: byte)
        }
        crc = updateCrc(crc, value: Character(":").asciiValue ?? 0)
        for byte in body.utf8 {
            crc = updateCrc(crc, value: byte)
        }
        return crc
    }

    private static func updateCrc(_ crc: UInt16, value: UInt8) -> UInt16 {
        var current = crc ^ UInt16(value) << 8
        for _ in 0..<8 {
            if current & 0x8000 != 0 {
                current = (current << 1) ^ 0x1021
            } else {
                current <<= 1
            }
        }
        return current & 0xFFFF
    }
}
