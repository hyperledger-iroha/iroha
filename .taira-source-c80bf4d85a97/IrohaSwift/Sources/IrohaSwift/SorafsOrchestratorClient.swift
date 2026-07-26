import Foundation
#if canImport(Darwin)
import Darwin
#elseif canImport(Glibc)
import Glibc
#endif

/// Errors surfaced by the Swift orchestrator wrapper.
public enum SorafsOrchestratorError: Error {
    /// The native bridge declined to execute the fetch (missing symbols or non-zero status).
    case bridgeUnavailable
    /// The native bridge returned a report that could not be decoded as JSON.
    case reportDecodingFailed(Error)
    /// Typed gateway requests must contain between one and 256 providers.
    case invalidProviderCount
}

extension SorafsOrchestratorClient: SorafsGatewayFetching {
    public func fetchGatewayPayload(
        plan: ToriiJSONValue,
        providers: [SorafsGatewayProvider],
        options: SorafsGatewayFetchOptions?,
        cancellationHandler: (@Sendable () -> Void)?
    ) async throws -> SorafsGatewayFetchResult {
        guard !providers.isEmpty, providers.count <= 256 else {
            throw SorafsOrchestratorError.invalidProviderCount
        }
        return try await fetch(
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
    private static let maximumStreamTokenEncodedBytes = 4 * 1_024
    private static let maximumStreamTokenDecodedBytes = 2 * 1_024

    public enum Error: Swift.Error {
        case invalidName
        case invalidProviderIdHex
        case invalidGatewayPublicKeyHex
        case invalidBaseURL
        case invalidStreamToken
    }

    public let name: String
    public let providerIdHex: String
    public let gatewayPublicKeyHex: String
    public let baseURL: URL
    public let streamTokenB64: String
    public let privacyEventsURL: URL?

    public init(name: String,
                providerIdHex: String,
                gatewayPublicKeyHex: String,
                baseURL: URL,
                streamTokenB64: String,
                privacyEventsURL: URL? = nil) throws {
        guard !name.isEmpty,
              name.utf8.count <= 128,
              name.utf8.allSatisfy({ byte in
                  (byte >= 48 && byte <= 57) ||
                  (byte >= 65 && byte <= 90) ||
                  (byte >= 97 && byte <= 122) ||
                  byte == 45 || byte == 46 || byte == 58 || byte == 95
              }) else {
            throw Error.invalidName
        }
        guard SorafsGatewayProvider.isCanonicalHex32(providerIdHex) else {
            throw Error.invalidProviderIdHex
        }
        guard SorafsGatewayProvider.isCanonicalHex32(gatewayPublicKeyHex) else {
            throw Error.invalidGatewayPublicKeyHex
        }
        guard SorafsGatewayProvider.isCanonicalGatewayURL(baseURL, path: "/"),
              privacyEventsURL.map({
                  SorafsGatewayProvider.isCanonicalGatewayURL($0, path: "/privacy/events")
              }) ?? true else {
            throw Error.invalidBaseURL
        }
        guard streamTokenB64.utf8.count <= Self.maximumStreamTokenEncodedBytes,
              let token = Data(base64Encoded: streamTokenB64),
              !token.isEmpty,
              token.count <= Self.maximumStreamTokenDecodedBytes,
              token.base64EncodedString() == streamTokenB64 else {
            throw Error.invalidStreamToken
        }
        self.name = name
        self.providerIdHex = providerIdHex
        self.gatewayPublicKeyHex = gatewayPublicKeyHex
        self.baseURL = baseURL
        self.streamTokenB64 = streamTokenB64
        self.privacyEventsURL = privacyEventsURL
    }

    private static func isCanonicalHex32(_ value: String) -> Bool {
        value.utf8.count == 64 &&
            value.utf8.contains(where: { $0 != 48 }) &&
            value.utf8.allSatisfy { byte in
                (byte >= 48 && byte <= 57) || (byte >= 97 && byte <= 102)
            }
    }

    private static func isCanonicalGatewayURL(_ url: URL, path: String) -> Bool {
        guard let host = url.host,
              url.absoluteString.utf8.count <= 2_048,
              url.scheme == "https",
              url.user == nil,
              url.password == nil,
              url.query == nil,
              url.fragment == nil,
              url.port == nil,
              host == host.lowercased(),
              isPublicGatewayHost(host) else {
            return false
        }
        let authorityHost = host.contains(":") ? "[\(host)]" : host
        let origin = "https://\(authorityHost)"
        if path == "/" {
            return url.absoluteString == origin || url.absoluteString == origin + "/"
        }
        return url.absoluteString == origin + path
    }

    private static func isPublicGatewayHost(_ host: String) -> Bool {
        if let octets = parseCanonicalIPv4(host) {
            return isPublicIPv4(octets)
        }
        if host.allSatisfy({ $0.isNumber || $0 == "." }) {
            return false
        }
        if host.contains(":") {
            return isPublicIPv6Literal(host)
        }
        if host == "localhost" || host.hasSuffix(".localhost") ||
            host.hasSuffix(".local") || host.hasSuffix(".internal") ||
            host.hasSuffix(".lan") || host.hasSuffix(".") || host.utf8.count > 253 {
            return false
        }
        return host.split(separator: ".", omittingEmptySubsequences: false).allSatisfy { label in
            guard !label.isEmpty, label.utf8.count <= 63,
                  let first = label.utf8.first, let last = label.utf8.last,
                  isASCIILowerAlphanumeric(first), isASCIILowerAlphanumeric(last) else {
                return false
            }
            return label.utf8.allSatisfy { isASCIILowerAlphanumeric($0) || $0 == 45 }
        }
    }

    private static func parseCanonicalIPv4(_ host: String) -> [UInt8]? {
        let parts = host.split(separator: ".", omittingEmptySubsequences: false)
        guard parts.count == 4 else { return nil }
        var octets: [UInt8] = []
        octets.reserveCapacity(4)
        for part in parts {
            guard !part.isEmpty,
                  part.utf8.allSatisfy({ $0 >= 48 && $0 <= 57 }),
                  part.count == 1 || part.first != "0",
                  let value = UInt8(String(part)) else {
                return nil
            }
            octets.append(value)
        }
        return octets
    }

    private static func isPublicIPv4(_ octets: [UInt8]) -> Bool {
        guard octets.count == 4 else { return false }
        let first = octets[0]
        let second = octets[1]
        let third = octets[2]
        let fourth = octets[3]
        return first != 0 && first != 10 && first != 127 && first < 224 &&
            !(first == 100 && second >= 64 && second <= 127) &&
            !(first == 169 && second == 254) &&
            !(first == 172 && second >= 16 && second <= 31) &&
            !(first == 192 && second == 0 && third == 0) &&
            !(first == 192 && second == 0 && third == 2) &&
            !(first == 192 && second == 88 && third == 99) &&
            !(first == 192 && second == 168) &&
            !(first == 198 && (second == 18 || second == 19)) &&
            !(first == 198 && second == 51 && third == 100) &&
            !(first == 203 && second == 0 && third == 113) &&
            !(first == 255 && second == 255 && third == 255 && fourth == 255)
    }

    private static func isPublicIPv6Literal(_ host: String) -> Bool {
        #if canImport(Darwin) || canImport(Glibc)
        var address = in6_addr()
        let parsed = host.withCString { inet_pton(AF_INET6, $0, &address) }
        guard parsed == 1 else { return false }
        let bytes = withUnsafeBytes(of: address) { Array($0) }
        let first = (UInt16(bytes[0]) << 8) | UInt16(bytes[1])
        let second = (UInt16(bytes[2]) << 8) | UInt16(bytes[3])
        let globalUnicast = first & 0xe000 == 0x2000
        let documentation = (first == 0x2001 && second == 0x0db8) ||
            (first == 0x3fff && second & 0xf000 == 0)
        let specialPurpose = first == 0x2001 && second <= 0x01ff
        return globalUnicast && !documentation && !specialPurpose && first != 0x2002
        #else
        return false
        #endif
    }

    private static func isASCIILowerAlphanumeric(_ byte: UInt8) -> Bool {
        (byte >= 48 && byte <= 57) || (byte >= 97 && byte <= 122)
    }

    enum CodingKeys: String, CodingKey {
        case name
        case providerIdHex = "provider_id_hex"
        case gatewayPublicKeyHex = "gateway_public_key_hex"
        case baseURL = "base_url"
        case streamTokenB64 = "stream_token_b64"
        case privacyEventsURL = "privacy_events_url"
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(name, forKey: .name)
        try container.encode(providerIdHex, forKey: .providerIdHex)
        try container.encode(gatewayPublicKeyHex, forKey: .gatewayPublicKeyHex)
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
