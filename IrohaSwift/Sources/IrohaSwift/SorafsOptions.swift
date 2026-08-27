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

public enum SorafsOptionsEncodingError: Error {
    case utf8ConversionFailed
}
