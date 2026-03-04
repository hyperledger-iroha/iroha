import Foundation

public enum OfflineCounterError: Error, Equatable, Sendable {
    case summaryHashMismatch(certificateIdHex: String, expected: String, actual: String)
    case counterJump(certificateIdHex: String, scope: String, expected: UInt64, actual: UInt64)
    case counterOverflow(certificateIdHex: String, scope: String)
    case invalidScope(String)
    case invalidSummaryHash(String)
}

public enum OfflineCounterPlatform: String, Codable, Sendable {
    case appleKey = "apple_key"
    case androidSeries = "android_series"
}

public struct OfflineCounterCheckpoint: Codable, Sendable, Equatable {
    public let certificateIdHex: String
    public let controllerId: String
    public let controllerDisplay: String?
    public let summaryHashHex: String
    public let appleKeyCounters: [String: UInt64]
    public let androidSeriesCounters: [String: UInt64]
    public let recordedAtMs: UInt64

    public init(certificateIdHex: String,
                controllerId: String,
                controllerDisplay: String?,
                summaryHashHex: String,
                appleKeyCounters: [String: UInt64],
                androidSeriesCounters: [String: UInt64],
                recordedAtMs: UInt64) {
        self.certificateIdHex = certificateIdHex
        self.controllerId = controllerId
        self.controllerDisplay = controllerDisplay
        self.summaryHashHex = summaryHashHex
        self.appleKeyCounters = appleKeyCounters
        self.androidSeriesCounters = androidSeriesCounters
        self.recordedAtMs = recordedAtMs
    }
}

/// Thread-safe store that tracks offline platform counters and summary hashes.
public final class OfflineCounterJournal {
    private let queue = DispatchQueue(label: "org.hyperledger.iroha.offline-counter-journal",
                                      qos: .utility)
    private var entries: [String: OfflineCounterCheckpoint]
    private let encoder: JSONEncoder
    private let decoder: JSONDecoder
    public let storageURL: URL

    public init(storageURL: URL? = nil) throws {
        self.storageURL = storageURL ?? OfflineCounterJournal.defaultStorageURL()
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
            .appendingPathComponent("iroha_offline_counters", isDirectory: true)
            .appendingPathComponent("journal.json", isDirectory: false)
    }

    @discardableResult
    public func upsert(
        summaries: [ToriiOfflineSummaryItem],
        recordedAtMs: UInt64,
        allowSummaryHashMismatch: Bool = false
    ) throws -> [OfflineCounterCheckpoint] {
        try queue.sync {
            var inserted: [OfflineCounterCheckpoint] = []
            for summary in summaries {
                let normalizedCert = summary.certificateIdHex.lowercased()
                let computedHash = try OfflineCounterJournal.computeSummaryHashHex(
                    appleKeyCounters: summary.appleKeyCounters,
                    androidSeriesCounters: summary.androidSeriesCounters
                )
                let expectedHash = try Self.normalizeSummaryHash(summary.summaryHashHex)
                if !allowSummaryHashMismatch, expectedHash.lowercased() != computedHash.lowercased() {
                    throw OfflineCounterError.summaryHashMismatch(certificateIdHex: normalizedCert,
                                                                 expected: expectedHash,
                                                                 actual: computedHash)
                }
                let checkpoint = OfflineCounterCheckpoint(
                    certificateIdHex: normalizedCert,
                    controllerId: summary.controllerId,
                    controllerDisplay: summary.controllerDisplay,
                    summaryHashHex: computedHash.lowercased(),
                    appleKeyCounters: summary.appleKeyCounters,
                    androidSeriesCounters: summary.androidSeriesCounters,
                    recordedAtMs: recordedAtMs
                )
                entries[normalizedCert] = checkpoint
                inserted.append(checkpoint)
            }
            try persistLocked()
            return inserted
        }
    }

    @discardableResult
    public func upsert(
        rawSummaries: [ToriiJSONValue],
        recordedAtMs: UInt64,
        allowSummaryHashMismatch: Bool = false
    ) throws -> [OfflineCounterCheckpoint] {
        try queue.sync {
            var inserted: [OfflineCounterCheckpoint] = []
            for summary in rawSummaries {
                guard let entry = try Self.parseRawSummary(summary,
                                                      recordedAtMs: recordedAtMs,
                                                      allowSummaryHashMismatch: allowSummaryHashMismatch) else {
                    continue
                }
                entries[entry.certificateIdHex] = entry
                inserted.append(entry)
            }
            try persistLocked()
            return inserted
        }
    }

    @discardableResult
    public func advanceCounter(
        certificate: OfflineWalletCertificate,
        platformProof: OfflinePlatformProof,
        recordedAtMs: UInt64
    ) throws -> OfflineCounterCheckpoint {
        let certificateIdHex = try certificate.certificateIdHex().lowercased()
        let controllerId = certificate.controller
        switch platformProof {
        case .appleAppAttest(let proof):
            return try updateCounter(certificateIdHex: certificateIdHex,
                                     controllerId: controllerId,
                                     controllerDisplay: nil,
                                     platform: .appleKey,
                                     scope: proof.keyId,
                                     counter: proof.counter,
                                     recordedAtMs: recordedAtMs)
        case .androidMarkerKey(let proof):
            return try updateCounter(certificateIdHex: certificateIdHex,
                                     controllerId: controllerId,
                                     controllerDisplay: nil,
                                     platform: .androidSeries,
                                     scope: proof.series,
                                     counter: proof.counter,
                                     recordedAtMs: recordedAtMs)
        case .provisioned(let proof):
            let schema = proof.manifestSchema.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !schema.isEmpty, let deviceId = proof.deviceId else {
                throw OfflineCounterError.invalidScope("provisioned manifest schema/device_id missing")
            }
            let scope = "provisioned::\(schema)::\(deviceId)"
            return try updateCounter(certificateIdHex: certificateIdHex,
                                     controllerId: controllerId,
                                     controllerDisplay: nil,
                                     platform: .androidSeries,
                                     scope: scope,
                                     counter: proof.counter,
                                     recordedAtMs: recordedAtMs)
        }
    }

    @discardableResult
    public func updateCounter(certificateIdHex: String,
                              controllerId: String,
                              controllerDisplay: String?,
                              platform: OfflineCounterPlatform,
                              scope: String,
                              counter: UInt64,
                              recordedAtMs: UInt64) throws -> OfflineCounterCheckpoint {
        let normalizedCert = certificateIdHex.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let trimmedScope = scope.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !normalizedCert.isEmpty else {
            throw OfflineCounterError.invalidScope("certificate_id_hex is empty")
        }
        guard !trimmedScope.isEmpty else {
            throw OfflineCounterError.invalidScope("counter scope is empty")
        }
        return try queue.sync {
            let existing = entries[normalizedCert]
            var apple = existing?.appleKeyCounters ?? [:]
            var android = existing?.androidSeriesCounters ?? [:]
            let previous: UInt64?
            switch platform {
            case .appleKey:
                previous = apple[trimmedScope]
            case .androidSeries:
                previous = android[trimmedScope]
            }
            if let previous {
                let (expected, overflow) = previous.addingReportingOverflow(1)
                if overflow {
                    throw OfflineCounterError.counterOverflow(certificateIdHex: normalizedCert,
                                                              scope: trimmedScope)
                }
                if counter != expected {
                    throw OfflineCounterError.counterJump(certificateIdHex: normalizedCert,
                                                          scope: trimmedScope,
                                                          expected: expected,
                                                          actual: counter)
                }
            }
            switch platform {
            case .appleKey:
                apple[trimmedScope] = counter
            case .androidSeries:
                android[trimmedScope] = counter
            }
            let computedHash = try OfflineCounterJournal.computeSummaryHashHex(
                appleKeyCounters: apple,
                androidSeriesCounters: android
            ).lowercased()
            let resolvedControllerDisplay = existing?.controllerDisplay ?? controllerDisplay
            let resolvedControllerId = existing?.controllerId.isEmpty == false
                ? existing?.controllerId ?? controllerId
                : controllerId
            let checkpoint = OfflineCounterCheckpoint(
                certificateIdHex: normalizedCert,
                controllerId: resolvedControllerId,
                controllerDisplay: resolvedControllerDisplay,
                summaryHashHex: computedHash,
                appleKeyCounters: apple,
                androidSeriesCounters: android,
                recordedAtMs: recordedAtMs
            )
            entries[normalizedCert] = checkpoint
            try persistLocked()
            return checkpoint
        }
    }

    public func checkpoint(for certificateIdHex: String) -> OfflineCounterCheckpoint? {
        queue.sync { entries[certificateIdHex.lowercased()] }
    }

    public func snapshot() -> [OfflineCounterCheckpoint] {
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
        entries = try decoder.decode([String: OfflineCounterCheckpoint].self, from: data)
    }

    private func persistLocked() throws {
        let directory = storageURL.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: directory,
                                                withIntermediateDirectories: true,
                                                attributes: nil)
        let data = try encoder.encode(entries)
        try data.write(to: storageURL, options: [.atomic])
    }

    private static func parseRawSummary(_ value: ToriiJSONValue,
                                        recordedAtMs: UInt64,
                                        allowSummaryHashMismatch: Bool) throws -> OfflineCounterCheckpoint? {
        guard case let .object(object) = value else { return nil }
        let certificateId = object["certificate_id_hex"]?.normalizedString
        let controllerId = object["controller_id"]?.normalizedString
        let controllerDisplay = object["controller_display"]?.normalizedString
        let summaryHashRaw = object["summary_hash_hex"]?.normalizedString
        guard let certificateId, let controllerId else { return nil }
        let appleCounters = parseCounterMap(object["apple_key_counters"])
        let androidCounters = parseCounterMap(object["android_series_counters"])
        let computedHash = try computeSummaryHashHex(appleKeyCounters: appleCounters,
                                                     androidSeriesCounters: androidCounters)
        if let summaryHashRaw {
            let expected = try normalizeSummaryHash(summaryHashRaw)
            if !allowSummaryHashMismatch, expected.lowercased() != computedHash.lowercased() {
                throw OfflineCounterError.summaryHashMismatch(certificateIdHex: certificateId,
                                                             expected: expected,
                                                             actual: computedHash)
            }
        }
        return OfflineCounterCheckpoint(
            certificateIdHex: certificateId.lowercased(),
            controllerId: controllerId,
            controllerDisplay: controllerDisplay,
            summaryHashHex: computedHash.lowercased(),
            appleKeyCounters: appleCounters,
            androidSeriesCounters: androidCounters,
            recordedAtMs: recordedAtMs
        )
    }

    private static func parseCounterMap(_ value: ToriiJSONValue?) -> [String: UInt64] {
        guard case let .object(object) = value else { return [:] }
        var result: [String: UInt64] = [:]
        for (key, raw) in object {
            guard let counter = raw.normalizedUInt64 else { continue }
            result[key] = counter
        }
        return result
    }

    static func computeSummaryHashHex(appleKeyCounters: [String: UInt64],
                                      androidSeriesCounters: [String: UInt64]) throws -> String {
        let payload = try computeSummaryHash(appleKeyCounters: appleKeyCounters,
                                             androidSeriesCounters: androidSeriesCounters)
        return payload.hexUppercased()
    }

    static func computeSummaryHash(appleKeyCounters: [String: UInt64],
                                   androidSeriesCounters: [String: UInt64]) throws -> Data {
        let applePayload = try encodeCounterMap(appleKeyCounters)
        let androidPayload = try encodeCounterMap(androidSeriesCounters)
        var writer = Data()
        writer.append(contentsOf: u64LittleEndian(UInt64(applePayload.count)))
        writer.append(applePayload)
        writer.append(contentsOf: u64LittleEndian(UInt64(androidPayload.count)))
        writer.append(androidPayload)
        return IrohaHash.hash(writer)
    }

    private static func encodeCounterMap(_ map: [String: UInt64]) throws -> Data {
        let keys = map.keys.sorted()
        var writer = Data()
        writer.append(contentsOf: u64LittleEndian(UInt64(keys.count)))
        for key in keys {
            let value = map[key] ?? 0
            let keyPayload = encodeString(key)
            writer.append(contentsOf: u64LittleEndian(UInt64(keyPayload.count)))
            writer.append(keyPayload)
            let valuePayload = u64LittleEndian(value)
            writer.append(contentsOf: u64LittleEndian(UInt64(valuePayload.count)))
            writer.append(valuePayload)
        }
        return writer
    }

    private static func encodeString(_ value: String) -> Data {
        var writer = Data()
        let bytes = Data(value.utf8)
        writer.append(contentsOf: u64LittleEndian(UInt64(bytes.count)))
        writer.append(bytes)
        return writer
    }

    private static func u64LittleEndian(_ value: UInt64) -> Data {
        var le = value.littleEndian
        return Data(bytes: &le, count: MemoryLayout<UInt64>.size)
    }

    private static func normalizeSummaryHash(_ raw: String) throws -> String {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw OfflineCounterError.invalidSummaryHash("summary_hash_hex is empty")
        }
        if trimmed.lowercased().hasPrefix("hash:") {
            guard let separator = trimmed.lastIndex(of: "#") else {
                throw OfflineCounterError.invalidSummaryHash("summary_hash_hex missing checksum")
            }
            let bodyStart = trimmed.index(trimmed.startIndex, offsetBy: 5)
            let body = String(trimmed[bodyStart..<separator])
            let checksum = String(trimmed[trimmed.index(after: separator)...])
            guard body.count == 64, Data(hexString: body) != nil else {
                throw OfflineCounterError.invalidSummaryHash("summary_hash_hex invalid body")
            }
            guard checksum.count == 4, UInt16(checksum, radix: 16) != nil else {
                throw OfflineCounterError.invalidSummaryHash("summary_hash_hex invalid checksum")
            }
            let expected = crc16(tag: "hash", body: body.uppercased())
            let provided = UInt16(checksum, radix: 16) ?? 0
            guard expected == provided else {
                throw OfflineCounterError.invalidSummaryHash("summary_hash_hex checksum mismatch")
            }
            return body.lowercased()
        }
        var hex = trimmed
        if hex.hasPrefix("0x") || hex.hasPrefix("0X") {
            hex = String(hex.dropFirst(2))
        }
        guard hex.count == 64, Data(hexString: hex) != nil else {
            throw OfflineCounterError.invalidSummaryHash("summary_hash_hex invalid hex")
        }
        return hex.lowercased()
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
