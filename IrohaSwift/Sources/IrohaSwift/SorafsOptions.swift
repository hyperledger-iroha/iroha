import Foundation

/// Helper for composing SoraFS gateway fetch options that can be passed to the native bridge.
public struct SorafsGatewayFetchOptions: Encodable, Sendable {
    public var telemetryRegion: String?
    public var rolloutPhase: String?
    public var transportPolicy: String?
    public var anonymityPolicy: String?
    public var writeMode: String?
    public var maxPeers: Int?
    public var retryBudget: Int?
    public var policyOverride: PolicyOverride?
    public var taikaiCache: SorafsTaikaiCacheOptions?
    public var chunkerHandle: String?

    public struct PolicyOverride: Encodable, Sendable {
        public var transportPolicy: String?
        public var anonymityPolicy: String?

        public init(
            transportPolicy: String? = nil,
            anonymityPolicy: String? = nil
        ) {
            self.transportPolicy = transportPolicy
            self.anonymityPolicy = anonymityPolicy
        }

        private enum CodingKeys: String, CodingKey {
            case transportPolicy = "transport_policy"
            case anonymityPolicy = "anonymity_policy"
        }
    }

    public init(
        telemetryRegion: String? = nil,
        rolloutPhase: String? = nil,
        transportPolicy: String? = nil,
        anonymityPolicy: String? = nil,
        writeMode: String? = nil,
        maxPeers: Int? = nil,
        retryBudget: Int? = nil,
        policyOverride: PolicyOverride? = nil,
        taikaiCache: SorafsTaikaiCacheOptions? = nil,
        chunkerHandle: String? = nil
    ) {
        self.telemetryRegion = telemetryRegion
        self.rolloutPhase = rolloutPhase
        self.transportPolicy = transportPolicy
        self.anonymityPolicy = anonymityPolicy
        self.writeMode = writeMode
        self.maxPeers = maxPeers
        self.retryBudget = retryBudget
        self.policyOverride = policyOverride
        self.taikaiCache = taikaiCache
        self.chunkerHandle = chunkerHandle
    }

    private enum CodingKeys: String, CodingKey {
        case telemetryRegion = "telemetry_region"
        case rolloutPhase = "rollout_phase"
        case transportPolicy = "transport_policy"
        case anonymityPolicy = "anonymity_policy"
        case writeMode = "write_mode"
        case maxPeers = "max_peers"
        case retryBudget = "retry_budget"
        case policyOverride = "policy_override"
        case taikaiCache = "taikai_cache"
        case chunkerHandle = "chunker_handle"
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        if let region = telemetryRegion?.trimmingCharacters(in: .whitespacesAndNewlines),
           !region.isEmpty {
            try container.encode(region, forKey: .telemetryRegion)
        }
        if let rolloutPhase {
            try container.encode(rolloutPhase, forKey: .rolloutPhase)
        }
        if let transportPolicy {
            try container.encode(transportPolicy, forKey: .transportPolicy)
        }
        if let anonymityPolicy {
            try container.encode(anonymityPolicy, forKey: .anonymityPolicy)
        }
        if let writeMode {
            let normalized = writeMode.trimmingCharacters(in: .whitespacesAndNewlines)
            if !normalized.isEmpty {
                try container.encode(normalized, forKey: .writeMode)
            }
        }
        if let maxPeers {
            try container.encode(maxPeers, forKey: .maxPeers)
        }
        if let retryBudget {
            try container.encode(retryBudget, forKey: .retryBudget)
        }
        if let policyOverride {
            try container.encode(policyOverride, forKey: .policyOverride)
        }
        if let taikaiCache {
            try container.encode(taikaiCache, forKey: .taikaiCache)
        }
        if let chunkerHandle {
            try container.encode(chunkerHandle, forKey: .chunkerHandle)
        }
    }

    /// Render the options as a JSON string using the canonical key names.
    public func jsonString(prettyPrinted: Bool = false) throws -> String {
        let encoder = JSONEncoder()
        var formatting: JSONEncoder.OutputFormatting = [.sortedKeys]
        if prettyPrinted {
            formatting.insert(.prettyPrinted)
        }
        encoder.outputFormatting = formatting
        let data = try encoder.encode(self)
        guard let json = String(data: data, encoding: .utf8) else {
            throw SorafsOptionsEncodingError.utf8ConversionFailed
        }
        return json
    }
}

/// QoS configuration for the Taikai cache override.
public struct SorafsTaikaiCacheQosOptions: Encodable, Sendable {
    public var priorityRateBps: UInt64
    public var standardRateBps: UInt64
    public var bulkRateBps: UInt64
    public var burstMultiplier: UInt32

    public init(
        priorityRateBps: UInt64,
        standardRateBps: UInt64,
        bulkRateBps: UInt64,
        burstMultiplier: UInt32
    ) {
        self.priorityRateBps = priorityRateBps
        self.standardRateBps = standardRateBps
        self.bulkRateBps = bulkRateBps
        self.burstMultiplier = burstMultiplier
    }

    private enum CodingKeys: String, CodingKey {
        case priorityRateBps = "priority_rate_bps"
        case standardRateBps = "standard_rate_bps"
        case bulkRateBps = "bulk_rate_bps"
        case burstMultiplier = "burst_multiplier"
    }
}

/// Reliability tuning for Taikai cache shard selection.
public struct SorafsTaikaiReliabilityOptions: Encodable, Sendable {
    public var failuresToTrip: UInt32?
    public var openSecs: UInt64?

    public init(failuresToTrip: UInt32? = nil, openSecs: UInt64? = nil) {
        self.failuresToTrip = failuresToTrip
        self.openSecs = openSecs
    }

    private enum CodingKeys: String, CodingKey {
        case failuresToTrip = "failures_to_trip"
        case openSecs = "open_secs"
    }
}

/// Taikai cache tier configuration mirrored from `TaikaiCacheConfig`.
public struct SorafsTaikaiCacheOptions: Encodable, Sendable {
    public var hotCapacityBytes: UInt64
    public var hotRetentionSecs: UInt64
    public var warmCapacityBytes: UInt64
    public var warmRetentionSecs: UInt64
    public var coldCapacityBytes: UInt64
    public var coldRetentionSecs: UInt64
    public var qos: SorafsTaikaiCacheQosOptions
    public var reliability: SorafsTaikaiReliabilityOptions?

    public init(
        hotCapacityBytes: UInt64,
        hotRetentionSecs: UInt64,
        warmCapacityBytes: UInt64,
        warmRetentionSecs: UInt64,
        coldCapacityBytes: UInt64,
        coldRetentionSecs: UInt64,
        qos: SorafsTaikaiCacheQosOptions,
        reliability: SorafsTaikaiReliabilityOptions? = nil
    ) {
        self.hotCapacityBytes = hotCapacityBytes
        self.hotRetentionSecs = hotRetentionSecs
        self.warmCapacityBytes = warmCapacityBytes
        self.warmRetentionSecs = warmRetentionSecs
        self.coldCapacityBytes = coldCapacityBytes
        self.coldRetentionSecs = coldRetentionSecs
        self.qos = qos
        self.reliability = reliability
    }

    private enum CodingKeys: String, CodingKey {
        case hotCapacityBytes = "hot_capacity_bytes"
        case hotRetentionSecs = "hot_retention_secs"
        case warmCapacityBytes = "warm_capacity_bytes"
        case warmRetentionSecs = "warm_retention_secs"
        case coldCapacityBytes = "cold_capacity_bytes"
        case coldRetentionSecs = "cold_retention_secs"
        case qos
        case reliability
    }
}

public enum SorafsOptionsEncodingError: Error {
    case utf8ConversionFailed
}
