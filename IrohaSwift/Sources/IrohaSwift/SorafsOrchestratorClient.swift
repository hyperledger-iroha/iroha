import Foundation

/// Errors surfaced by the Swift orchestrator wrapper.
public enum SorafsOrchestratorError: Error {
    /// The native bridge declined to execute the fetch (missing symbols or non-zero status).
    case bridgeUnavailable
    /// The native bridge returned a report that could not be decoded as JSON.
    case reportDecodingFailed(Error)
}

extension SorafsOrchestratorClient: SorafsGatewayFetching {
    public func fetchGatewayPayload(
        plan: ToriiJSONValue,
        providers: [SorafsGatewayProvider],
        options: SorafsGatewayFetchOptions?,
        cancellationHandler: (@Sendable () -> Void)?
    ) async throws -> SorafsGatewayFetchResult {
        try await fetch(
            plan: plan,
            providers: providers,
            options: options,
            cancellationHandler: cancellationHandler
        )
    }
}

/// Result returned by the Swift orchestrator wrapper.
public struct SorafsGatewayFetchResult: Sendable {
    /// Raw payload bytes reassembled by the orchestrator.
    public let payload: Data
    /// Typed summary describing provider receipts and scoreboard weights.
    public let report: SorafsGatewayFetchReport
    /// Raw JSON string returned by the native bridge.
    public let reportJSON: String
}

/// Provider reports, receipts, and scoreboard snapshot emitted by the orchestrator.
public struct SorafsGatewayFetchReport: Codable, Sendable {
    public struct ProviderReport: Codable, Sendable, Equatable {
        public let provider: String
        public let successes: Int
        public let failures: Int
        public let disabled: Bool
    }

    public struct ChunkReceipt: Codable, Sendable, Equatable {
        public let chunkIndex: Int
        public let provider: String
        public let attempts: Int
        public let latencyMs: Double?
        public let bytes: Int

        enum CodingKeys: String, CodingKey {
            case chunkIndex = "chunk_index"
            case provider
            case attempts
            case latencyMs = "latency_ms"
            case bytes
        }
    }

    public struct ScoreboardEntry: Codable, Sendable, Equatable {
        public let providerID: String
        public let alias: String
        public let rawScore: Double
        public let normalizedWeight: Double
        public let eligibility: String

        enum CodingKeys: String, CodingKey {
            case providerID = "provider_id"
            case alias
            case rawScore = "raw_score"
            case normalizedWeight = "normalized_weight"
            case eligibility
        }
    }

    public struct TaikaiCacheTierCounts: Codable, Sendable, Equatable {
        public let hot: Int
        public let warm: Int
        public let cold: Int
    }

    public struct TaikaiCacheEvictionCounts: Codable, Sendable, Equatable {
        public let expired: Int
        public let capacity: Int
    }

    public struct TaikaiCacheEvictions: Codable, Sendable, Equatable {
        public let hot: TaikaiCacheEvictionCounts
        public let warm: TaikaiCacheEvictionCounts
        public let cold: TaikaiCacheEvictionCounts
    }

    public struct TaikaiCachePromotions: Codable, Sendable, Equatable {
        public let warmToHot: Int
        public let coldToWarm: Int
        public let coldToHot: Int

        enum CodingKeys: String, CodingKey {
            case warmToHot = "warm_to_hot"
            case coldToWarm = "cold_to_warm"
            case coldToHot = "cold_to_hot"
        }
    }

    public struct TaikaiQosCounts: Codable, Sendable, Equatable {
        public let priority: Int
        public let standard: Int
        public let bulk: Int
    }

    public struct TaikaiCacheSummary: Codable, Sendable, Equatable {
        public let hits: TaikaiCacheTierCounts
        public let misses: Int
        public let inserts: TaikaiCacheTierCounts
        public let evictions: TaikaiCacheEvictions
        public let promotions: TaikaiCachePromotions
        public let qosDenials: TaikaiQosCounts

        enum CodingKeys: String, CodingKey {
            case hits
            case misses
            case inserts
            case evictions
            case promotions
            case qosDenials = "qos_denials"
        }
    }

    public struct TaikaiCacheQueue: Codable, Sendable, Equatable {
        public let pendingSegments: Int
        public let pendingBytes: Int
        public let pendingBatches: Int
        public let inFlightBatches: Int
        public let hedgedBatches: Int
        public let shaperDenials: TaikaiQosCounts
        public let droppedSegments: Int
        public let failovers: Int
        public let openCircuits: Int

        enum CodingKeys: String, CodingKey {
            case pendingSegments = "pending_segments"
            case pendingBytes = "pending_bytes"
            case pendingBatches = "pending_batches"
            case inFlightBatches = "in_flight_batches"
            case hedgedBatches = "hedged_batches"
            case shaperDenials = "shaper_denials"
            case droppedSegments = "dropped_segments"
            case failovers
            case openCircuits = "open_circuits"
        }
    }

    public let chunkCount: Int
    public let providerReports: [ProviderReport]
    public let chunkReceipts: [ChunkReceipt]
    public let scoreboard: [ScoreboardEntry]?
    public let telemetryRegion: String?
    public let taikaiCacheSummary: TaikaiCacheSummary?
    public let taikaiCacheQueue: TaikaiCacheQueue?

    enum CodingKeys: String, CodingKey {
        case chunkCount = "chunk_count"
        case providerReports = "provider_reports"
        case chunkReceipts = "chunk_receipts"
        case scoreboard
        case telemetryRegion = "telemetry_region"
        case taikaiCacheSummary = "taikai_cache_summary"
        case taikaiCacheQueue = "taikai_cache_queue"
    }

    public init(
        chunkCount: Int,
        providerReports: [ProviderReport],
        chunkReceipts: [ChunkReceipt],
        scoreboard: [ScoreboardEntry]? = nil,
        telemetryRegion: String? = nil,
        taikaiCacheSummary: TaikaiCacheSummary? = nil,
        taikaiCacheQueue: TaikaiCacheQueue? = nil
    ) {
        self.chunkCount = chunkCount
        self.providerReports = providerReports
        self.chunkReceipts = chunkReceipts
        self.scoreboard = scoreboard
        self.telemetryRegion = telemetryRegion
        self.taikaiCacheSummary = taikaiCacheSummary
        self.taikaiCacheQueue = taikaiCacheQueue
    }

    static func decode(from json: String) throws -> SorafsGatewayFetchReport {
        let decoder = JSONDecoder()
        let data = Data(json.utf8)
        return try decoder.decode(SorafsGatewayFetchReport.self, from: data)
    }
}

/// Gateway descriptors consumed by the orchestrator helper.
public struct SorafsGatewayProvider: Encodable, Sendable, Equatable {
    public enum Error: Swift.Error {
        case invalidProviderIdHex
        case invalidStreamToken
    }

    public let name: String
    public let providerIdHex: String
    public let baseURL: URL
    public let streamTokenB64: String
    public let privacyEventsURL: URL?

    public init(name: String,
                providerIdHex: String,
                baseURL: URL,
                streamTokenB64: String,
                privacyEventsURL: URL? = nil) throws {
        let normalizedId = SorafsGatewayProvider.normalizeHex(providerIdHex)
        guard normalizedId.count == 64, Data(hexString: normalizedId) != nil else {
            throw Error.invalidProviderIdHex
        }
        guard Data(base64Encoded: streamTokenB64) != nil else {
            throw Error.invalidStreamToken
        }
        self.name = name
        self.providerIdHex = normalizedId
        self.baseURL = baseURL
        self.streamTokenB64 = streamTokenB64
        self.privacyEventsURL = privacyEventsURL
    }

    private static func normalizeHex(_ value: String) -> String {
        var trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.hasPrefix("0x") || trimmed.hasPrefix("0X") {
            trimmed = String(trimmed.dropFirst(2))
        }
        return trimmed.lowercased()
    }

    enum CodingKeys: String, CodingKey {
        case name
        case providerIdHex = "provider_id_hex"
        case baseURL = "base_url"
        case streamTokenB64 = "stream_token_b64"
        case privacyEventsURL = "privacy_events_url"
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(name, forKey: .name)
        try container.encode(providerIdHex, forKey: .providerIdHex)
        try container.encode(baseURL.absoluteString, forKey: .baseURL)
        try container.encode(streamTokenB64, forKey: .streamTokenB64)
        if let privacyEventsURL {
            try container.encode(privacyEventsURL.absoluteString, forKey: .privacyEventsURL)
        }
    }
}

/// Abstraction over gateway orchestration so SDKs and tests can swap implementations.
public protocol SorafsGatewayFetching: Sendable {
    func fetchGatewayPayload(
        plan: ToriiJSONValue,
        providers: [SorafsGatewayProvider],
        options: SorafsGatewayFetchOptions?,
        cancellationHandler: (@Sendable () -> Void)?
    ) async throws -> SorafsGatewayFetchResult
}

/// Async wrapper around `connect_norito_sorafs_local_fetch`.
public final class SorafsOrchestratorClient: @unchecked Sendable {
    public init() {}

    /// Execute a fetch using typed plan/providers payloads.
    public func fetch<Plan: Encodable, Providers: Encodable>(
        plan: Plan,
        providers: Providers,
        options: SorafsGatewayFetchOptions? = nil,
        cancellationHandler: (@Sendable () -> Void)? = nil
    ) async throws -> SorafsGatewayFetchResult {
        let planJSON = try SorafsOrchestratorClient.encodeJSONString(plan)
        let providersJSON = try SorafsOrchestratorClient.encodeJSONString(providers)
        let optionsJSON = try options?.jsonString()
        return try await fetchRaw(
            planJSON: planJSON,
            providersJSON: providersJSON,
            optionsJSON: optionsJSON,
            cancellationHandler: cancellationHandler
        )
    }

    /// Execute a fetch using raw JSON strings (mirrors the CLI fixtures exactly).
    public func fetchRaw(
        planJSON: String,
        providersJSON: String,
        optionsJSON: String? = nil,
        cancellationHandler: (@Sendable () -> Void)? = nil
    ) async throws -> SorafsGatewayFetchResult {
        try await withTaskCancellationHandler(operation: {
            let task = Task(priority: .userInitiated) { () throws -> SorafsGatewayFetchResult in
                try Task.checkCancellation()
                guard let output = NoritoNativeBridge.shared.sorafsLocalFetch(
                    planJSON: planJSON,
                    providersJSON: providersJSON,
                    optionsJSON: optionsJSON
                ) else {
                    throw SorafsOrchestratorError.bridgeUnavailable
                }
                do {
                    let report = try SorafsGatewayFetchReport.decode(from: output.reportJSON)
                    return SorafsGatewayFetchResult(payload: output.payload, report: report, reportJSON: output.reportJSON)
                } catch {
                    throw SorafsOrchestratorError.reportDecodingFailed(error)
                }
            }
            let result = try await task.value
            return result
        }, onCancel: {
            cancellationHandler?()
        })
    }

    private static func encodeJSONString<T: Encodable>(_ value: T) throws -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let data = try encoder.encode(value)
        guard let json = String(data: data, encoding: .utf8) else {
            throw SorafsOrchestratorError.bridgeUnavailable
        }
        return json
    }
}
