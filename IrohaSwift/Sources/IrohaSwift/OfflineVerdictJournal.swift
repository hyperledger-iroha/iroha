import Foundation

public struct OfflineVerdictMetadata: Codable, Sendable, Equatable {
    public let certificateIdHex: String
    public let controllerId: String
    public let controllerDisplay: String
    public let verdictIdHex: String?
    public let attestationNonceHex: String?
    public let expiresAtMs: UInt64
    public let policyExpiresAtMs: UInt64
    public let refreshAtMs: UInt64?
    public let remainingAmount: String
    public let recordedAtMs: UInt64
    public let integrityPolicy: String?
    public let playIntegrityMetadata: OfflineVerdictPlayIntegrityMetadata?
    public let hmsSafetyDetectMetadata: OfflineVerdictHmsSafetyDetectMetadata?
    public let provisionedMetadata: OfflineVerdictProvisionedMetadata?

    public init(certificateIdHex: String,
                controllerId: String,
                controllerDisplay: String,
                verdictIdHex: String?,
                attestationNonceHex: String?,
                expiresAtMs: UInt64,
                policyExpiresAtMs: UInt64,
                refreshAtMs: UInt64?,
                remainingAmount: String,
                recordedAtMs: UInt64,
                integrityPolicy: String? = nil,
                playIntegrityMetadata: OfflineVerdictPlayIntegrityMetadata? = nil,
                hmsSafetyDetectMetadata: OfflineVerdictHmsSafetyDetectMetadata? = nil,
                provisionedMetadata: OfflineVerdictProvisionedMetadata? = nil) {
        self.certificateIdHex = certificateIdHex
        self.controllerId = controllerId
        self.controllerDisplay = controllerDisplay
        self.verdictIdHex = verdictIdHex
        self.attestationNonceHex = attestationNonceHex
        self.expiresAtMs = expiresAtMs
        self.policyExpiresAtMs = policyExpiresAtMs
        self.refreshAtMs = refreshAtMs
        self.remainingAmount = remainingAmount
        self.recordedAtMs = recordedAtMs
        self.integrityPolicy = integrityPolicy
        self.playIntegrityMetadata = playIntegrityMetadata
        self.hmsSafetyDetectMetadata = hmsSafetyDetectMetadata
        self.provisionedMetadata = provisionedMetadata
    }
}

public struct OfflineVerdictProvisionedMetadata: Codable, Sendable, Equatable {
    public let inspectorPublicKeyHex: String
    public let manifestSchema: String
    public let manifestVersion: Int?
    public let maxManifestAgeMs: UInt64?
    public let manifestDigestHex: String?

    public init(inspectorPublicKeyHex: String,
                manifestSchema: String,
                manifestVersion: Int?,
                maxManifestAgeMs: UInt64?,
                manifestDigestHex: String?) {
        self.inspectorPublicKeyHex = inspectorPublicKeyHex
        self.manifestSchema = manifestSchema
        self.manifestVersion = manifestVersion
        self.maxManifestAgeMs = maxManifestAgeMs
        self.manifestDigestHex = manifestDigestHex
    }
}

public struct OfflineVerdictPlayIntegrityMetadata: Codable, Sendable, Equatable {
    public let cloudProjectNumber: UInt64
    public let environment: String
    public let packageNames: [String]
    public let signingDigestsSha256: [String]
    public let allowedAppVerdicts: [String]
    public let allowedDeviceVerdicts: [String]
    public let maxTokenAgeMs: UInt64?

    public init(cloudProjectNumber: UInt64,
                environment: String,
                packageNames: [String],
                signingDigestsSha256: [String],
                allowedAppVerdicts: [String],
                allowedDeviceVerdicts: [String],
                maxTokenAgeMs: UInt64?) {
        self.cloudProjectNumber = cloudProjectNumber
        self.environment = environment
        self.packageNames = packageNames
        self.signingDigestsSha256 = signingDigestsSha256
        self.allowedAppVerdicts = allowedAppVerdicts
        self.allowedDeviceVerdicts = allowedDeviceVerdicts
        self.maxTokenAgeMs = maxTokenAgeMs
    }
}

public struct OfflineVerdictHmsSafetyDetectMetadata: Codable, Sendable, Equatable {
    public let appId: String
    public let packageNames: [String]
    public let signingDigestsSha256: [String]
    public let requiredEvaluations: [String]
    public let maxTokenAgeMs: UInt64?

    public init(appId: String,
                packageNames: [String],
                signingDigestsSha256: [String],
                requiredEvaluations: [String],
                maxTokenAgeMs: UInt64?) {
        self.appId = appId
        self.packageNames = packageNames
        self.signingDigestsSha256 = signingDigestsSha256
        self.requiredEvaluations = requiredEvaluations
        self.maxTokenAgeMs = maxTokenAgeMs
    }
}

public struct OfflineVerdictWarning: Equatable, Sendable {
    public enum DeadlineKind: String, Sendable {
        case refresh
        case policy
        case certificate
    }

    public enum State: String, Sendable {
        case warning
        case expired
    }

    public let certificateIdHex: String
    public let controllerId: String
    public let controllerDisplay: String
    public let verdictIdHex: String?
    public let deadlineKind: DeadlineKind
    public let deadlineMs: UInt64
    public let millisecondsRemaining: Int64
    public let state: State
    public let headline: String
    public let details: String
}

/// Errors thrown when cached verdict metadata is missing or stale.
public enum OfflineVerdictError: Error, Equatable, Sendable {
    case metadataMissing(certificateIdHex: String)
    case nonceMismatch(certificateIdHex: String, expectedNonceHex: String?, providedNonceHex: String?)
    case expired(certificateIdHex: String, deadlineKind: OfflineVerdictWarning.DeadlineKind, deadlineMs: UInt64)
}

/// Thread-safe verdict cache that wallets use to enforce offline policies.
public final class OfflineVerdictJournal {
    public static let defaultWarningThresholdMs: UInt64 = 86_400_000

    private let queue = DispatchQueue(label: "org.hyperledger.iroha.offline-verdict-journal",
                                      qos: .utility)
    private var entries: [String: OfflineVerdictMetadata]
    private let encoder: JSONEncoder
    private let decoder: JSONDecoder
    public let storageURL: URL

    public init(storageURL: URL? = nil) throws {
        self.storageURL = storageURL ?? OfflineVerdictJournal.defaultStorageURL()
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
            .appendingPathComponent("iroha_offline_verdicts", isDirectory: true)
            .appendingPathComponent("journal.json", isDirectory: false)
    }

    @discardableResult
    public func upsert(allowances: [ToriiOfflineAllowanceItem],
                       recordedAtMs: UInt64) throws -> [OfflineVerdictMetadata] {
        try queue.sync {
            var inserted: [OfflineVerdictMetadata] = []
            for allowance in allowances {
                let snapshot = OfflineVerdictJournal.parseIntegritySnapshot(from: allowance.record)
                let metadata = OfflineVerdictMetadata(
                    certificateIdHex: allowance.certificateIdHex.lowercased(),
                    controllerId: allowance.controllerId,
                    controllerDisplay: allowance.controllerDisplay,
                    verdictIdHex: allowance.verdictIdHex,
                    attestationNonceHex: allowance.attestationNonceHex,
                    expiresAtMs: allowance.expiresAtMs,
                    policyExpiresAtMs: allowance.policyExpiresAtMs,
                    refreshAtMs: allowance.refreshAtMs,
                    remainingAmount: allowance.remainingAmount,
                    recordedAtMs: recordedAtMs,
                    integrityPolicy: snapshot.policy,
                    playIntegrityMetadata: snapshot.playIntegrityMetadata,
                    hmsSafetyDetectMetadata: snapshot.hmsSafetyDetectMetadata,
                    provisionedMetadata: snapshot.provisionedMetadata
                )
                entries[metadata.certificateIdHex] = metadata
                inserted.append(metadata)
            }
            try persistLocked()
            return inserted
        }
    }

    @discardableResult
    public func upsert(rawAllowances: [ToriiJSONValue],
                       recordedAtMs: UInt64) throws -> [OfflineVerdictMetadata] {
        try queue.sync {
            var inserted: [OfflineVerdictMetadata] = []
            for record in rawAllowances {
                guard let metadata = try Self.parseRawAllowance(record, recordedAtMs: recordedAtMs) else {
                    continue
                }
                entries[metadata.certificateIdHex] = metadata
                inserted.append(metadata)
            }
            try persistLocked()
            return inserted
        }
    }

    public func warnings(nowMs: UInt64,
                         warningThresholdMs: UInt64) -> [OfflineVerdictWarning] {
        queue.sync {
            entries.values.compactMap { metadata in
                OfflineVerdictJournal.buildWarning(for: metadata,
                                                   nowMs: nowMs,
                                                   warningThresholdMs: warningThresholdMs)
            }
        }
    }

    public func warning(for certificateIdHex: String,
                        nowMs: UInt64,
                        warningThresholdMs: UInt64) -> OfflineVerdictWarning? {
        queue.sync {
            guard let metadata = entries[certificateIdHex.lowercased()] else {
                return nil
            }
            return OfflineVerdictJournal.buildWarning(for: metadata,
                                                      nowMs: nowMs,
                                                      warningThresholdMs: warningThresholdMs)
        }
    }

    public func metadata(for certificateIdHex: String) -> OfflineVerdictMetadata? {
        queue.sync { entries[certificateIdHex.lowercased()] }
    }

    public func snapshot() -> [OfflineVerdictMetadata] {
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
        entries = try decoder.decode([String: OfflineVerdictMetadata].self, from: data)
    }

    private func persistLocked() throws {
        let directory = storageURL.deletingLastPathComponent()
        try FileManager.default.createDirectory(at: directory,
                                                withIntermediateDirectories: true,
                                                attributes: nil)
        let data = try encoder.encode(entries)
        try data.write(to: storageURL, options: [.atomic])
    }

    private static func buildWarning(for metadata: OfflineVerdictMetadata,
                                     nowMs: UInt64,
                                     warningThresholdMs: UInt64) -> OfflineVerdictWarning? {
        guard let (kind, deadlineMs) = OfflineVerdictJournal.deadline(for: metadata) else {
            return nil
        }
        let delta = Int64(deadlineMs) - Int64(nowMs)
        let state: OfflineVerdictWarning.State
        if delta <= 0 {
            state = .expired
        } else if UInt64(delta) <= warningThresholdMs {
            state = .warning
        } else {
            return nil
        }

        let headline: String
        switch (state, kind) {
        case (.expired, .refresh):
            headline = "Cached verdict expired"
        case (.expired, .policy):
            headline = "Policy expired"
        case (.expired, .certificate):
            headline = "Allowance expired"
        case (.warning, .refresh):
            headline = "Refresh cached verdict soon"
        case (.warning, .policy):
            headline = "Policy expiry approaching"
        case (.warning, .certificate):
            headline = "Allowance expiry approaching"
        }
        let remaining = OfflineVerdictJournal.formatDuration(milliseconds: UInt64(abs(delta)))
        let verdictLabel = metadata.verdictIdHex ?? "unknown verdict"
        let details = """
        Certificate \(metadata.certificateIdHex) (\(metadata.controllerDisplay)) \
        \(state == .expired ? "missed" : "must meet") its \(kind.rawValue) deadline at \
        \(OfflineVerdictJournal.iso8601String(for: deadlineMs)). Verdict=\(verdictLabel) \
        Remaining=\(remaining) Amount=\(metadata.remainingAmount)
        """

        return OfflineVerdictWarning(
            certificateIdHex: metadata.certificateIdHex,
            controllerId: metadata.controllerId,
            controllerDisplay: metadata.controllerDisplay,
            verdictIdHex: metadata.verdictIdHex,
            deadlineKind: kind,
            deadlineMs: deadlineMs,
            millisecondsRemaining: delta,
            state: state,
            headline: headline,
            details: details
        )
    }

    private static func deadline(for metadata: OfflineVerdictMetadata)
        -> (OfflineVerdictWarning.DeadlineKind, UInt64)? {
        if let refresh = metadata.refreshAtMs, refresh > 0 {
            return (.refresh, refresh)
        }
        if metadata.policyExpiresAtMs > 0 {
            return (.policy, metadata.policyExpiresAtMs)
        }
        if metadata.expiresAtMs > 0 {
            return (.certificate, metadata.expiresAtMs)
        }
        return nil
    }

    private static func parseIntegritySnapshot(from record: ToriiJSONValue) -> IntegritySnapshot {
        guard case let .object(root) = record,
              let certificateValue = root["certificate"],
              case let .object(certificate) = certificateValue,
              let metadataValue = certificate["metadata"],
              case let .object(metadata) = metadataValue else {
            return .empty
        }
        let parsed = OfflineCertificateParsedMetadata(from: metadata)
        guard let policy = parsed.platformPolicy else {
            return .empty
        }
        var provisioned: OfflineVerdictProvisionedMetadata?
        var playIntegrity: OfflineVerdictPlayIntegrityMetadata?
        var hmsSafetyDetect: OfflineVerdictHmsSafetyDetectMetadata?
        switch policy {
        case .provisioned:
            if let p = parsed.androidProvisioned,
               let inspector = p.inspectorPublicKey,
               let schema = p.manifestSchema {
                provisioned = OfflineVerdictProvisionedMetadata(
                    inspectorPublicKeyHex: inspector,
                    manifestSchema: schema,
                    manifestVersion: p.manifestVersion,
                    maxManifestAgeMs: p.maxManifestAgeMs,
                    manifestDigestHex: p.manifestDigestHex)
            }
        case .playIntegrity:
            if let p = parsed.androidPlayIntegrity,
               let projectNumber = p.cloudProjectNumber,
               let env = p.environment,
               !p.packageNames.isEmpty, !p.signingDigestsSha256.isEmpty,
               !p.allowedAppVerdicts.isEmpty, !p.allowedDeviceVerdicts.isEmpty {
                playIntegrity = OfflineVerdictPlayIntegrityMetadata(
                    cloudProjectNumber: projectNumber,
                    environment: env,
                    packageNames: p.packageNames,
                    signingDigestsSha256: p.signingDigestsSha256,
                    allowedAppVerdicts: p.allowedAppVerdicts,
                    allowedDeviceVerdicts: p.allowedDeviceVerdicts,
                    maxTokenAgeMs: p.maxTokenAgeMs)
            }
        case .hmsSafetyDetect:
            if let h = parsed.androidHmsSafetyDetect,
               let appId = h.appId,
               !h.packageNames.isEmpty, !h.signingDigestsSha256.isEmpty {
                hmsSafetyDetect = OfflineVerdictHmsSafetyDetectMetadata(
                    appId: appId,
                    packageNames: h.packageNames,
                    signingDigestsSha256: h.signingDigestsSha256,
                    requiredEvaluations: h.requiredEvaluations,
                    maxTokenAgeMs: h.maxTokenAgeMs)
            }
        case .markerKey:
            break
        }
        return IntegritySnapshot(policy: policy.rawValue,
                                 playIntegrityMetadata: playIntegrity,
                                 hmsSafetyDetectMetadata: hmsSafetyDetect,
                                 provisionedMetadata: provisioned)
    }

    private static func parseRawAllowance(_ value: ToriiJSONValue,
                                          recordedAtMs: UInt64) throws -> OfflineVerdictMetadata? {
        guard case let .object(object) = value,
              let certificateValue = object["certificate"] else {
            return nil
        }
        let certificate: OfflineWalletCertificate
        do {
            certificate = try certificateValue.decode(as: OfflineWalletCertificate.self)
        } catch {
            return nil
        }
        let certificateIdHex = (try? certificate.certificateIdHex().lowercased()) ?? ""
        guard !certificateIdHex.isEmpty else { return nil }
        let controllerId = certificate.controller
        let controllerDisplay = object["controller_display"]?.normalizedString ?? controllerId
        let verdictIdHex = parseHashHex(in: object, key: "verdict_id_hex")
        let attestationNonceHex = parseHashHex(in: object, key: "attestation_nonce_hex")
        let remainingAmount = object["remaining_amount"]?.normalizedString
            ?? certificate.allowance.amount
        let refreshAtMs = object["refresh_at_ms"]?.normalizedUInt64
        let snapshot = parseIntegritySnapshot(from: value)
        return OfflineVerdictMetadata(
            certificateIdHex: certificateIdHex,
            controllerId: controllerId,
            controllerDisplay: controllerDisplay,
            verdictIdHex: verdictIdHex,
            attestationNonceHex: attestationNonceHex,
            expiresAtMs: certificate.expiresAtMs,
            policyExpiresAtMs: certificate.policy.expiresAtMs,
            refreshAtMs: refreshAtMs,
            remainingAmount: remainingAmount,
            recordedAtMs: recordedAtMs,
            integrityPolicy: snapshot.policy,
            playIntegrityMetadata: snapshot.playIntegrityMetadata,
            hmsSafetyDetectMetadata: snapshot.hmsSafetyDetectMetadata,
            provisionedMetadata: snapshot.provisionedMetadata
        )
    }

    private static func parseHashHex(in object: [String: ToriiJSONValue],
                                     key: String) -> String? {
        guard let value = object[key] else { return nil }
        return parseHashHex(value)
    }

    private static func parseHashHex(_ value: ToriiJSONValue) -> String? {
        switch value {
        case .null:
            return nil
        case .string(let raw):
            return normalizeHashHex(raw)
        case .array(let items):
            var bytes = Data(capacity: items.count)
            for item in items {
                guard case let .number(number) = item, number.isFinite else {
                    return nil
                }
                let rounded = number.rounded(.towardZero)
                guard rounded == number, rounded >= 0, rounded <= 255 else {
                    return nil
                }
                bytes.append(UInt8(rounded))
            }
            guard bytes.count == 32 else {
                return nil
            }
            do {
                _ = try OfflineNorito.encodeHash(bytes)
            } catch {
                return nil
            }
            return bytes.hexUppercased().lowercased()
        default:
            return nil
        }
    }

    private static func normalizeHashHex(_ raw: String) -> String? {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return nil
        }
        if trimmed.lowercased().hasPrefix("hash:") {
            guard let separator = trimmed.lastIndex(of: "#") else {
                return nil
            }
            let bodyStart = trimmed.index(trimmed.startIndex, offsetBy: 5)
            let body = String(trimmed[bodyStart..<separator])
            let checksum = String(trimmed[trimmed.index(after: separator)...])
            guard body.count == 64, let data = Data(hexString: body) else {
                return nil
            }
            guard checksum.count == 4, let checksumValue = UInt16(checksum, radix: 16) else {
                return nil
            }
            let expected = crc16(tag: "hash", body: body.uppercased())
            guard expected == checksumValue else {
                return nil
            }
            if (try? OfflineNorito.encodeHash(data)) == nil {
                return nil
            }
            return body.lowercased()
        }
        var hex = trimmed
        if hex.hasPrefix("0x") || hex.hasPrefix("0X") {
            hex = String(hex.dropFirst(2))
        }
        guard hex.count == 64, let data = Data(hexString: hex) else {
            return nil
        }
        if (try? OfflineNorito.encodeHash(data)) == nil {
            return nil
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

    private struct IntegritySnapshot {
        static let empty = IntegritySnapshot(policy: nil,
                                             playIntegrityMetadata: nil,
                                             hmsSafetyDetectMetadata: nil,
                                             provisionedMetadata: nil)

        let policy: String?
        let playIntegrityMetadata: OfflineVerdictPlayIntegrityMetadata?
        let hmsSafetyDetectMetadata: OfflineVerdictHmsSafetyDetectMetadata?
        let provisionedMetadata: OfflineVerdictProvisionedMetadata?
    }

    private static func formatDuration(milliseconds: UInt64) -> String {
        let seconds = milliseconds / 1_000
        let minutes = seconds / 60
        let hours = minutes / 60
        let days = hours / 24
        if days > 0 {
            return "\(days)d \(hours % 24)h"
        } else if hours > 0 {
            return "\(hours)h \(minutes % 60)m"
        } else if minutes > 0 {
            return "\(minutes)m \(seconds % 60)s"
        } else {
            return "\(seconds)s"
        }
    }

    private static func iso8601String(for timestampMs: UInt64) -> String {
        let date = Date(timeIntervalSince1970: TimeInterval(timestampMs) / 1_000)
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter.string(from: date)
    }
}
