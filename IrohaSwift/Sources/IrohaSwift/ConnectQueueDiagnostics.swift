import Foundation
#if canImport(CryptoKit)
import CryptoKit
#endif

/// Queue health state recorded by Connect SDKs.
public enum ConnectQueueState: String, Codable, Sendable {
    case healthy
    case throttled
    case quarantined
    case disabled
}

/// Snapshot of queue statistics for a single direction.
public struct ConnectQueueDirectionStats: Codable, Equatable, Sendable {
    public var depth: Int
    public var bytes: Int
    public var oldestSequence: UInt64?
    public var newestSequence: UInt64?
    public var oldestTimestampMs: UInt64?
    public var newestTimestampMs: UInt64?

    public init(depth: Int = 0,
                bytes: Int = 0,
                oldestSequence: UInt64? = nil,
                newestSequence: UInt64? = nil,
                oldestTimestampMs: UInt64? = nil,
                newestTimestampMs: UInt64? = nil) {
        self.depth = depth
        self.bytes = bytes
        self.oldestSequence = oldestSequence
        self.newestSequence = newestSequence
        self.oldestTimestampMs = oldestTimestampMs
        self.newestTimestampMs = newestTimestampMs
    }

    enum CodingKeys: String, CodingKey {
        case depth
        case bytes
        case oldestSequence = "oldest_sequence"
        case newestSequence = "newest_sequence"
        case oldestTimestampMs = "oldest_timestamp_ms"
        case newestTimestampMs = "newest_timestamp_ms"
    }

    public static let empty = ConnectQueueDirectionStats()
}

/// Snapshot written to disk for CLI/ops tooling.
public struct ConnectQueueSnapshot: Codable, Equatable, Sendable {
    public static let schemaVersion = 1

    public var schemaVersion: Int
    public var sessionIDBase64: String
    public var state: ConnectQueueState
    public var reason: String?
    public var warningWatermark: Double
    public var dropWatermark: Double
    public var lastUpdatedMs: UInt64
    public var appToWallet: ConnectQueueDirectionStats
    public var walletToApp: ConnectQueueDirectionStats

    public init(schemaVersion: Int = ConnectQueueSnapshot.schemaVersion,
                sessionIDBase64: String,
                state: ConnectQueueState,
                reason: String? = nil,
                warningWatermark: Double,
                dropWatermark: Double,
                lastUpdatedMs: UInt64,
                appToWallet: ConnectQueueDirectionStats = .empty,
                walletToApp: ConnectQueueDirectionStats = .empty) {
        self.schemaVersion = schemaVersion
        self.sessionIDBase64 = sessionIDBase64
        self.state = state
        self.reason = reason
        self.warningWatermark = warningWatermark
        self.dropWatermark = dropWatermark
        self.lastUpdatedMs = lastUpdatedMs
        self.appToWallet = appToWallet
        self.walletToApp = walletToApp
    }

    enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case sessionIDBase64 = "session_id_base64"
        case state
        case reason
        case warningWatermark = "warning_watermark"
        case dropWatermark = "drop_watermark"
        case lastUpdatedMs = "last_updated_ms"
        case appToWallet = "app_to_wallet"
        case walletToApp = "wallet_to_app"
    }
}

/// NDJSON telemetry sample appended by SDKs.
public struct ConnectQueueMetricsSample: Codable, Equatable, Sendable {
    public var timestampMs: UInt64
    public var state: ConnectQueueState
    public var appToWalletDepth: Int
    public var walletToAppDepth: Int
    public var reason: String?

    public init(timestampMs: UInt64,
                state: ConnectQueueState,
                appToWalletDepth: Int,
                walletToAppDepth: Int,
                reason: String? = nil) {
        self.timestampMs = timestampMs
        self.state = state
        self.appToWalletDepth = appToWalletDepth
        self.walletToAppDepth = walletToAppDepth
        self.reason = reason
    }

    enum CodingKeys: String, CodingKey {
        case timestampMs = "timestamp_ms"
        case state
        case appToWalletDepth = "app_to_wallet_depth"
        case walletToAppDepth = "wallet_to_app_depth"
        case reason
    }
}

/// Evidence manifest accompanying copied journal files.
public struct ConnectQueueEvidenceManifest: Codable, Equatable, Sendable {
    public var schemaVersion: Int
    public var sessionIDBase64: String
    public var createdAtMs: UInt64
    public var snapshot: ConnectQueueSnapshot
    public var files: ConnectQueueEvidenceFiles

    public init(schemaVersion: Int = ConnectQueueSnapshot.schemaVersion,
                sessionIDBase64: String,
                createdAtMs: UInt64,
                snapshot: ConnectQueueSnapshot,
                files: ConnectQueueEvidenceFiles) {
        self.schemaVersion = schemaVersion
        self.sessionIDBase64 = sessionIDBase64
        self.createdAtMs = createdAtMs
        self.snapshot = snapshot
        self.files = files
    }

    enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case sessionIDBase64 = "session_id_base64"
        case createdAtMs = "created_at_ms"
        case snapshot
        case files
    }
}

public struct ConnectQueueEvidenceFiles: Codable, Equatable, Sendable {
    public var appQueueFilename: String?
    public var walletQueueFilename: String?
    public var metricsFilename: String?

    public init(appQueueFilename: String? = nil,
                walletQueueFilename: String? = nil,
                metricsFilename: String? = nil) {
        self.appQueueFilename = appQueueFilename
        self.walletQueueFilename = walletQueueFilename
        self.metricsFilename = metricsFilename
    }

    enum CodingKeys: String, CodingKey {
        case appQueueFilename = "app_queue_filename"
        case walletQueueFilename = "wallet_queue_filename"
        case metricsFilename = "metrics_filename"
    }
}

/// Lightweight file-backed storage used by both diagnostics and trackers.
struct ConnectQueueStorage {
    let sessionIDBase64: String
    private let sessionDirectoryName: String
    let configuration: ConnectSessionDiagnostics.Configuration
    let fileManager: FileManager

    init(sessionID: Data,
         configuration: ConnectSessionDiagnostics.Configuration,
         fileManager: FileManager) {
        self.sessionIDBase64 = sessionID.base64URLEncodedString()
        self.sessionDirectoryName = ConnectQueueStorage.directoryComponent(for: sessionID)
        self.configuration = configuration
        self.fileManager = fileManager
    }

    var sessionDirectory: URL {
        configuration.rootDirectory.appendingPathComponent(sessionDirectoryComponent(), isDirectory: true)
    }

    var stateURL: URL { sessionDirectory.appendingPathComponent("state.json", isDirectory: false) }
    var metricsURL: URL { sessionDirectory.appendingPathComponent("metrics.ndjson", isDirectory: false) }
    var appQueueURL: URL { sessionDirectory.appendingPathComponent("app_to_wallet.queue", isDirectory: false) }
    var walletQueueURL: URL { sessionDirectory.appendingPathComponent("wallet_to_app.queue", isDirectory: false) }

    func ensureSessionDirectory() throws {
        if !fileManager.fileExists(atPath: sessionDirectory.path) {
            try fileManager.createDirectory(at: sessionDirectory, withIntermediateDirectories: true)
        }
    }

    func loadSnapshot() throws -> ConnectQueueSnapshot? {
        guard fileManager.fileExists(atPath: stateURL.path) else { return nil }
        let data = try Data(contentsOf: stateURL)
        return try JSONDecoder().decode(ConnectQueueSnapshot.self, from: data)
    }

    func saveSnapshot(_ snapshot: ConnectQueueSnapshot) throws {
        try ensureSessionDirectory()
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .withoutEscapingSlashes]
        let data = try encoder.encode(snapshot)
        try data.write(to: stateURL, options: .atomic)
    }

    func appendMetric(_ sample: ConnectQueueMetricsSample) throws {
        try ensureSessionDirectory()
        let encoder = JSONEncoder()
        let data = try encoder.encode(sample)
        guard let line = String(data: data, encoding: .utf8) else {
            throw ConnectCodecError.bridgeUnavailable
        }
        let payload = (line + "\n").data(using: .utf8)!
        if fileManager.fileExists(atPath: metricsURL.path) {
            let handle = try FileHandle(forWritingTo: metricsURL)
            defer { try? handle.close() }
            try handle.seekToEnd()
            try handle.write(contentsOf: payload)
        } else {
            try payload.write(to: metricsURL, options: .atomic)
        }
    }

    func makeDefaultSnapshot() -> ConnectQueueSnapshot {
        ConnectQueueSnapshot(sessionIDBase64: sessionIDBase64,
                             state: .disabled,
                             reason: nil,
                             warningWatermark: configuration.warningWatermark,
                             dropWatermark: configuration.dropWatermark,
                             lastUpdatedMs: ConnectSessionDiagnostics.timestampNow(),
                             appToWallet: .empty,
                             walletToApp: .empty)
    }

    func sessionDirectoryComponent() -> String {
        sessionDirectoryName
    }
}

private extension ConnectQueueStorage {
    static func directoryComponent(for sessionID: Data) -> String {
        #if canImport(CryptoKit)
        let digest = SHA256.hash(data: sessionID)
        return digest.map { String(format: "%02x", $0) }.joined()
        #else
        return sessionID.base64URLEncodedString()
        #endif
    }
}

/// Exposes read-only diagnostics helpers for Connect sessions.
public final class ConnectSessionDiagnostics {
    public struct Configuration: Sendable {
        public var rootDirectory: URL
        public var warningWatermark: Double
        public var dropWatermark: Double

        public init(rootDirectory: URL = ConnectSessionDiagnostics.defaultRootDirectory(),
                    warningWatermark: Double = 0.6,
                    dropWatermark: Double = 0.85) {
            self.rootDirectory = rootDirectory
            self.warningWatermark = warningWatermark
            self.dropWatermark = dropWatermark
        }
    }

    private let storage: ConnectQueueStorage

    public init(sessionID: Data,
                configuration: Configuration = Configuration(),
                fileManager: FileManager = .default) {
        self.storage = ConnectQueueStorage(sessionID: sessionID,
                                           configuration: configuration,
                                           fileManager: fileManager)
    }

    /// Returns the latest snapshot on disk or a synthesized default.
    public func snapshot() throws -> ConnectQueueSnapshot {
        if let stored = try storage.loadSnapshot() {
            return stored
        }
        return storage.makeDefaultSnapshot()
    }

    /// Copies queue journals + manifest into `targetDirectory`.
    @discardableResult
    public func exportJournalBundle(to targetDirectory: URL) throws -> ConnectQueueEvidenceManifest {
        try storage.ensureSessionDirectory()
        let manifestDirectory = targetDirectory
        if !storage.fileManager.fileExists(atPath: manifestDirectory.path) {
            try storage.fileManager.createDirectory(at: manifestDirectory, withIntermediateDirectories: true)
        }

        var files = ConnectQueueEvidenceFiles()
        if storage.fileManager.fileExists(atPath: storage.appQueueURL.path) {
            let target = manifestDirectory.appendingPathComponent("app_to_wallet.queue", isDirectory: false)
            try copyReplacing(itemAt: storage.appQueueURL, to: target, fileManager: storage.fileManager)
            files.appQueueFilename = target.lastPathComponent
        }
        if storage.fileManager.fileExists(atPath: storage.walletQueueURL.path) {
            let target = manifestDirectory.appendingPathComponent("wallet_to_app.queue", isDirectory: false)
            try copyReplacing(itemAt: storage.walletQueueURL, to: target, fileManager: storage.fileManager)
            files.walletQueueFilename = target.lastPathComponent
        }
        if storage.fileManager.fileExists(atPath: storage.metricsURL.path) {
            let target = manifestDirectory.appendingPathComponent("metrics.ndjson", isDirectory: false)
            try copyReplacing(itemAt: storage.metricsURL, to: target, fileManager: storage.fileManager)
            files.metricsFilename = target.lastPathComponent
        }

        let manifest = ConnectQueueEvidenceManifest(sessionIDBase64: storage.sessionIDBase64,
                                                    createdAtMs: ConnectSessionDiagnostics.timestampNow(),
                                                    snapshot: try snapshot(),
                                                    files: files)
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .withoutEscapingSlashes]
        let manifestURL = manifestDirectory.appendingPathComponent("manifest.json", isDirectory: false)
        try encoder.encode(manifest).write(to: manifestURL, options: .atomic)
        return manifest
    }

    /// Copies the metrics NDJSON file, writing an empty file if metrics are unavailable.
    @discardableResult
    public func exportQueueMetrics(to destinationURL: URL) throws -> URL {
        try storage.ensureSessionDirectory()
        if storage.fileManager.fileExists(atPath: storage.metricsURL.path) {
            try copyReplacing(itemAt: storage.metricsURL, to: destinationURL, fileManager: storage.fileManager)
        } else {
            if storage.fileManager.fileExists(atPath: destinationURL.path) {
                try storage.fileManager.removeItem(at: destinationURL)
            }
            try Data().write(to: destinationURL, options: .atomic)
        }
        return destinationURL
    }

    /// Default directory for queue diagnostics (Application Support/IrohaConnect/queues).
    public static func defaultRootDirectory(fileManager: FileManager = .default) -> URL {
        if let base = fileManager.urls(for: .applicationSupportDirectory, in: .userDomainMask).first {
            return base.appendingPathComponent("IrohaConnect/queues", isDirectory: true)
        }
        // Fallback for environments without Application Support (should not hit on iOS).
        return URL(fileURLWithPath: NSTemporaryDirectory(), isDirectory: true)
            .appendingPathComponent("IrohaConnect/queues", isDirectory: true)
    }

    public static func timestampNow() -> UInt64 {
        UInt64(Date().timeIntervalSince1970 * 1000)
    }
}

/// Mutating helper used by Connect queue implementations.
public final class ConnectQueueStateTracker {
    private let storage: ConnectQueueStorage

    public init(sessionID: Data,
                configuration: ConnectSessionDiagnostics.Configuration = .init(),
                fileManager: FileManager = .default) {
        self.storage = ConnectQueueStorage(sessionID: sessionID,
                                           configuration: configuration,
                                           fileManager: fileManager)
    }

    /// Load, mutate, and persist the snapshot in a single call.
    public func updateSnapshot(_ mutate: (inout ConnectQueueSnapshot) -> Void) throws {
        var snapshot = try storage.loadSnapshot() ?? storage.makeDefaultSnapshot()
        mutate(&snapshot)
        snapshot.lastUpdatedMs = ConnectSessionDiagnostics.timestampNow()
        try storage.saveSnapshot(snapshot)
    }

    /// Append a metrics sample to `metrics.ndjson`.
    public func recordMetric(_ sample: ConnectQueueMetricsSample) throws {
        try storage.appendMetric(sample)
    }
}

private func copyReplacing(itemAt source: URL, to destination: URL, fileManager: FileManager) throws {
    if fileManager.fileExists(atPath: destination.path) {
        try fileManager.removeItem(at: destination)
    }
    try fileManager.copyItem(at: source, to: destination)
}

private extension Data {
    func base64URLEncodedString() -> String {
        let base64 = self.base64EncodedString()
        let urlSafe = base64
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
        return urlSafe
    }
}
