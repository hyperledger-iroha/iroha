import Foundation

/// Sole native status protocol revision.
public let sumeragiStatusProtocolVersion: UInt16 = 8

private func requireNativeStatusFields(_ decoder: Decoder, _ fields: [String]) throws {
    let container = try decoder.container(keyedBy: NativeStatusCodingKey.self)
    guard Set(container.allKeys.map(\.stringValue)) == Set(fields) else {
        throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath,
            debugDescription: "native status requires every field and rejects unknown fields"))
    }
}
private struct NativeStatusCodingKey: CodingKey {
    let stringValue: String
    let intValue: Int? = nil
    init?(stringValue: String) { self.stringValue = stringValue }
    init?(intValue: Int) { return nil }
}

// Every numeric field in native status is unsigned. Reject the sign token before
// Foundation can normalize JSON -0 to the same UInt64 as canonical zero.
private func rejectNativeStatusSignedNumbers(_ data: Data) throws {
    var inString = false
    var escaped = false
    for byte in data {
        if inString {
            if escaped { escaped = false }
            else if byte == 0x5c { escaped = true }
            else if byte == 0x22 { inString = false }
        } else if byte == 0x22 {
            inString = true
        } else if byte == 0x2d {
            throw DecodingError.dataCorrupted(.init(codingPath: [],
                debugDescription: "native status requires unsigned numeric tokens"))
        }
    }
}

private func nativeStatusPublicKey(_ literal: String?, codingPath: [CodingKey]) throws -> String? {
    guard let literal else { return nil }
    let parsed = try governanceKagemushaPublicKeyOrderV1(literal, codingPath: codingPath)
    guard let algorithm = SigningAlgorithm(noritoDiscriminant: parsed.algorithm) else {
        throw DecodingError.dataCorrupted(.init(codingPath: codingPath, debugDescription: "unsupported public key"))
    }
    _ = try AccountAddress.fromAccount(publicKey: Data(parsed.payload), algorithm: algorithm.wireName)
    return literal
}

/// Unsigned native core memory counters.
public struct ToriiSumeragiFootprint: Decodable, Sendable, Equatable {
    public let votes: UInt64
    public let timeouts: UInt64
    public let blocks: UInt64
    public let execEntries: UInt64
    public let wants: UInt64
    public let pendingApply: UInt64
    public let syncEntries: UInt64
    public let syncBytes: UInt64
    public let peers: UInt64
    public let recentHeaders: UInt64
    public let configs: UInt64
    public let certCache: UInt64
    public let evidenceKeys: UInt64
    public let probe: UInt64
    private enum CodingKeys: String, CodingKey {
        case votes
        case timeouts
        case blocks
        case execEntries = "exec_entries"
        case wants
        case pendingApply = "pending_apply"
        case syncEntries = "sync_entries"
        case syncBytes = "sync_bytes"
        case peers
        case recentHeaders = "recent_headers"
        case configs
        case certCache = "cert_cache"
        case evidenceKeys = "evidence_keys"
        case probe
    }
    public init(from decoder: Decoder) throws {
        try requireNativeStatusFields(decoder, ["votes", "timeouts", "blocks", "exec_entries", "wants", "pending_apply", "sync_entries", "sync_bytes", "peers", "recent_headers", "configs", "cert_cache", "evidence_keys", "probe"])
        let c = try decoder.container(keyedBy: CodingKeys.self)
        votes = try c.decode(UInt64.self, forKey: .votes)
        timeouts = try c.decode(UInt64.self, forKey: .timeouts)
        blocks = try c.decode(UInt64.self, forKey: .blocks)
        execEntries = try c.decode(UInt64.self, forKey: .execEntries)
        wants = try c.decode(UInt64.self, forKey: .wants)
        pendingApply = try c.decode(UInt64.self, forKey: .pendingApply)
        syncEntries = try c.decode(UInt64.self, forKey: .syncEntries)
        syncBytes = try c.decode(UInt64.self, forKey: .syncBytes)
        peers = try c.decode(UInt64.self, forKey: .peers)
        recentHeaders = try c.decode(UInt64.self, forKey: .recentHeaders)
        configs = try c.decode(UInt64.self, forKey: .configs)
        certCache = try c.decode(UInt64.self, forKey: .certCache)
        evidenceKeys = try c.decode(UInt64.self, forKey: .evidenceKeys)
        probe = try c.decode(UInt64.self, forKey: .probe)
    }
}

/// Applied-cut readiness from the sole native pulse owner.
public struct ToriiSumeragiBeaconHorizon: Decodable, Sendable, Equatable {
    public let epochLengthBlocks: UInt64
    public let nextRequiredPulseHeight: UInt64?
    public let activeSessionId: String?
    public let sessionCoversNextPulse: Bool
    public let localProviderReady: Bool
    private enum CodingKeys: String, CodingKey {
        case epochLengthBlocks = "epoch_length_blocks"
        case nextRequiredPulseHeight = "next_required_pulse_height"
        case activeSessionId = "active_session_id"
        case sessionCoversNextPulse = "session_covers_next_pulse"
        case localProviderReady = "local_provider_ready"
    }
    public init(from decoder: Decoder) throws {
        try requireNativeStatusFields(decoder, ["epoch_length_blocks", "next_required_pulse_height", "active_session_id", "session_covers_next_pulse", "local_provider_ready"])
        let c = try decoder.container(keyedBy: CodingKeys.self)
        epochLengthBlocks = try c.decode(UInt64.self, forKey: .epochLengthBlocks)
        nextRequiredPulseHeight = try c.decodeIfPresent(UInt64.self, forKey: .nextRequiredPulseHeight)
        activeSessionId = try c.decodeIfPresent(String.self, forKey: .activeSessionId)
        sessionCoversNextPulse = try c.decode(Bool.self, forKey: .sessionCoversNextPulse)
        localProviderReady = try c.decode(Bool.self, forKey: .localProviderReady)
        guard activeSessionId.map({ $0.count == 64 && $0.utf8.allSatisfy { (48...57).contains($0) || (65...70).contains($0) } }) ?? true,
              !sessionCoversNextPulse || (nextRequiredPulseHeight != nil && activeSessionId != nil),
              !localProviderReady || activeSessionId != nil else {
            throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath, debugDescription: "invalid native beacon readiness"))
        }
    }
}

/// Closed halt cases reported by the native core.
public enum ToriiSumeragiHaltReason: Decodable, Sendable, Equatable {
    case safetyRecordCorrupt, safetyRecordInconsistent, driverAnomaly
    case safetyViolation(UInt64), applyDiverged(UInt64), publicationRecoveryRequired(UInt64)
    private enum CodingKeys: String, CodingKey { case reason, details }
    public init(from decoder: Decoder) throws {
        try requireNativeStatusFields(decoder, ["reason", "details"])
        let c = try decoder.container(keyedBy: CodingKeys.self)
        let reason = try c.decode(String.self, forKey: .reason)
        switch reason {
        case "safety_violation": self = .safetyViolation(try c.decode(UInt64.self, forKey: .details))
        case "apply_diverged": self = .applyDiverged(try c.decode(UInt64.self, forKey: .details))
        case "publication_recovery_required": self = .publicationRecoveryRequired(try c.decode(UInt64.self, forKey: .details))
        default:
            guard try c.decodeNil(forKey: .details) else {
                throw DecodingError.dataCorruptedError(forKey: .details, in: c, debugDescription: "unit halt requires null details")
            }
            switch reason {
            case "safety_record_corrupt": self = .safetyRecordCorrupt
            case "safety_record_inconsistent": self = .safetyRecordInconsistent
            case "driver_anomaly": self = .driverAnomaly
            default: throw DecodingError.dataCorruptedError(forKey: .reason, in: c, debugDescription: "unknown native halt reason")
            }
        }
    }
}

/// Sole native status observation; it confers no finality authority.
public struct ToriiSumeragiStatusSnapshot: Decodable, Sendable, Equatable {
    /// Bounded, duplicate-key and non-integral-token rejecting JSON decoder.
    public static func parseJSON(_ data: Data) throws -> Self {
        guard !data.isEmpty, data.count <= 1_048_576 else { throw CocoaError(.fileReadTooLarge) }
        try rejectNativeStatusSignedNumbers(data)
        try StrictJSONDuplicateKeyRejector.rejectDuplicateObjectKeys(in: data, requireAllNumbersInteger: true)
        return try JSONDecoder().decode(Self.self, from: data)
    }

    public let protocolVersion: UInt16
    public let configFingerprint: String
    public let beaconHorizon: ToriiSumeragiBeaconHorizon?
    public let instance: String
    public let height: UInt64
    public let view: UInt64
    public let stage: UInt8
    public let leader: String?
    public let proxyTail: String?
    public let highQcView: UInt64?
    public let level: UInt32
    public let startLevel: UInt32
    public let tRetxMs: UInt64
    public let committedHeight: UInt64
    public let appliedHeight: UInt64
    public let awaiting: Bool
    public let signer: String?
    public let unanchored: Bool
    public let abstaining: Bool
    public let halted: ToriiSumeragiHaltReason?
    public let footprint: ToriiSumeragiFootprint
    public var isHalted: Bool { halted != nil }
    public var isSigning: Bool { signer != nil && !abstaining && !unanchored }
    public var applyLag: UInt64 { committedHeight >= appliedHeight ? committedHeight - appliedHeight : 0 }
    private enum CodingKeys: String, CodingKey {
        case protocolVersion = "protocol_version"
        case configFingerprint = "config_fingerprint"
        case beaconHorizon = "beacon_horizon"
        case instance
        case height
        case view
        case stage
        case leader
        case proxyTail = "proxy_tail"
        case highQcView = "high_qc_view"
        case level
        case startLevel = "start_level"
        case tRetxMs = "t_retx_ms"
        case committedHeight = "committed_height"
        case appliedHeight = "applied_height"
        case awaiting
        case signer
        case unanchored
        case abstaining
        case halted
        case footprint
    }
    public init(from decoder: Decoder) throws {
        try requireNativeStatusFields(decoder, ["protocol_version", "config_fingerprint", "beacon_horizon", "instance", "height", "view", "stage", "leader", "proxy_tail", "high_qc_view", "level", "start_level", "t_retx_ms", "committed_height", "applied_height", "awaiting", "signer", "unanchored", "abstaining", "halted", "footprint"])
        let c = try decoder.container(keyedBy: CodingKeys.self)
        protocolVersion = try c.decode(UInt16.self, forKey: .protocolVersion)
        configFingerprint = try c.decode(String.self, forKey: .configFingerprint)
        beaconHorizon = try c.decodeIfPresent(ToriiSumeragiBeaconHorizon.self, forKey: .beaconHorizon)
        instance = try c.decode(String.self, forKey: .instance)
        height = try c.decode(UInt64.self, forKey: .height)
        view = try c.decode(UInt64.self, forKey: .view)
        stage = try c.decode(UInt8.self, forKey: .stage)
        leader = try nativeStatusPublicKey(try c.decodeIfPresent(String.self, forKey: .leader), codingPath: decoder.codingPath)
        proxyTail = try nativeStatusPublicKey(try c.decodeIfPresent(String.self, forKey: .proxyTail), codingPath: decoder.codingPath)
        highQcView = try c.decodeIfPresent(UInt64.self, forKey: .highQcView)
        level = try c.decode(UInt32.self, forKey: .level)
        startLevel = try c.decode(UInt32.self, forKey: .startLevel)
        tRetxMs = try c.decode(UInt64.self, forKey: .tRetxMs)
        committedHeight = try c.decode(UInt64.self, forKey: .committedHeight)
        appliedHeight = try c.decode(UInt64.self, forKey: .appliedHeight)
        awaiting = try c.decode(Bool.self, forKey: .awaiting)
        signer = try nativeStatusPublicKey(try c.decodeIfPresent(String.self, forKey: .signer), codingPath: decoder.codingPath)
        unanchored = try c.decode(Bool.self, forKey: .unanchored)
        abstaining = try c.decode(Bool.self, forKey: .abstaining)
        halted = try c.decodeIfPresent(ToriiSumeragiHaltReason.self, forKey: .halted)
        footprint = try c.decode(ToriiSumeragiFootprint.self, forKey: .footprint)
        guard protocolVersion == sumeragiStatusProtocolVersion,
              ToriiCanonicalWire.isCanonicalHash(configFingerprint), stage <= 2,
              instance.count == 64,
              instance.utf8.allSatisfy({ (48...57).contains($0) || (97...102).contains($0) }) else {
            throw DecodingError.dataCorrupted(.init(codingPath: decoder.codingPath, debugDescription: "invalid native status protocol, fingerprint, stage or instance"))
        }
    }
}
