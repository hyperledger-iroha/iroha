import Foundation

/// Errors raised when first-release SCCP values are not canonical.
public enum SccpV1Error: Error, Equatable, CustomStringConvertible {
    case invalid(String)

    public var description: String {
        switch self {
        case let .invalid(message): message
        }
    }
}

/// Closed first-release SCCP network inventory.
public enum SccpNetworkV1: String, CaseIterable, Sendable {
    case soraTaira = "sora-taira"
    case ethereumMainnet = "ethereum-mainnet"
    case bscMainnet = "bsc-mainnet"
    case tronMainnet = "tron-mainnet"
    case tonMainnet = "ton-mainnet"

    public var tag: UInt8 {
        switch self {
        case .soraTaira: 0x40
        case .ethereumMainnet: 0x41
        case .bscMainnet: 0x42
        case .tronMainnet: 0x43
        case .tonMainnet: 0x44
        }
    }

    public var domainId: UInt32 {
        switch self {
        case .soraTaira: 0
        case .ethereumMainnet: 1
        case .bscMainnet: 2
        case .tronMainnet: 3
        case .tonMainnet: 4
        }
    }

    public var isSora: Bool { domainId == 0 }
    public var isExternal: Bool { !isSora }

    static func fromTag(_ tag: UInt8) -> SccpNetworkV1? {
        allCases.first { $0.tag == tag }
    }
}

/// Directed lane joining one exact SORA profile and one exact external profile.
public struct SccpLaneIdV1: Equatable, Hashable, Sendable {
    public let source: SccpNetworkV1
    public let target: SccpNetworkV1

    public init(source: SccpNetworkV1, target: SccpNetworkV1) throws {
        guard source.isSora != target.isSora, source.domainId != target.domainId else {
            throw SccpV1Error.invalid("SCCP lane must join exactly one SORA profile and one external profile")
        }
        self.source = source
        self.target = target
    }

    public var isOutbound: Bool { source.isSora && target.isExternal }
    public var isInbound: Bool { source.isExternal && target.isSora }
}

/// Closed first-release SCCP binary codec inventory.
public enum SccpCodecV1: UInt8, CaseIterable, Sendable {
    case canonicalText = 0
    case evmAddress20 = 1
    case tronAddress21 = 2
    case tonAccount36 = 3

    public var wireKey: String {
        switch self {
        case .canonicalText: "canonical_text"
        case .evmAddress20: "evm_address20"
        case .tronAddress21: "tron_address21"
        case .tonAccount36: "ton_account36"
        }
    }

    /// Validate and defensively copy one canonical codec value.
    public func validate(_ value: Data) throws -> Data {
        let valid: Bool
        switch self {
        case .canonicalText:
            guard (1...256).contains(value.count) else {
                throw SccpV1Error.invalid("value does not match SCCP codec \(wireKey)")
            }
            if value.allSatisfy({ (0x21...0x7e).contains($0) }) {
                valid = true
            } else if let literal = String(data: value, encoding: .utf8),
                      let prefix = try? AccountAddress.inspectI105NetworkPrefix(literal),
                      let address = try? AccountAddress.fromI105(literal),
                      let canonical = try? address.toI105(networkPrefix: prefix.chainDiscriminant)
            {
                valid = canonical == literal
            } else {
                valid = false
            }
        case .evmAddress20:
            valid = value.count == 20 && value.contains { $0 != 0 }
        case .tronAddress21:
            valid = value.count == 21 && value.first == 0x41 && value.dropFirst().contains { $0 != 0 }
        case .tonAccount36:
            valid = value.count == 36 && value.prefix(4).allSatisfy { $0 == 0 }
                && value.dropFirst(4).contains { $0 != 0 }
        }
        guard valid else {
            throw SccpV1Error.invalid("value does not match SCCP codec \(wireKey)")
        }
        return Data(value)
    }
}

/// Canonical native verifier backends admitted by SCCP V1.
public enum SccpNativeBackendV1: String, CaseIterable, Sendable {
    case ethereumBeacon = "ethereum_beacon_v1"
    case bscParlia = "bsc_parlia_v1"
    case tronDpos = "tron_dpos_v1"
    case tonMasterchain = "ton_masterchain_v1"

    public var backendLabel: String {
        switch self {
        case .ethereumBeacon: "bridge/sccp/native/ethereum-beacon-v1"
        case .bscParlia: "bridge/sccp/native/bsc-parlia-v1"
        case .tronDpos: "bridge/sccp/native/tron-dpos-v1"
        case .tonMasterchain: "bridge/sccp/native/ton-masterchain-v1"
        }
    }

    public func supports(_ network: SccpNetworkV1) -> Bool {
        switch self {
        case .ethereumBeacon: network == .ethereumMainnet
        case .bscParlia: network == .bscMainnet
        case .tronDpos: network == .tronMainnet
        case .tonMasterchain: network == .tonMainnet
        }
    }
}

/// Canonical raw TON account identity. SCCP value-moving V1 contracts use workchain zero.
public struct SccpTonAddressV1: Equatable, Hashable, Sendable {
    public let workchain: Int32
    public let account: Data

    public init(workchain: Int32, account: Data) throws {
        guard account.count == 32, account.contains(where: { $0 != 0 }) else {
            throw SccpV1Error.invalid("TON account must be a nonzero 32-byte value")
        }
        self.workchain = workchain
        self.account = Data(account)
    }

    public var isSccpBasechainContract: Bool { workchain == 0 }

    /// Signed big-endian workchain followed by the raw account id, as used by codec 3.
    public func canonicalAccount36() throws -> Data {
        guard isSccpBasechainContract else {
            throw SccpV1Error.invalid("SCCP TON contracts must use basechain workchain 0")
        }
        var value = workchain.bigEndian
        return withUnsafeBytes(of: &value) { Data($0) } + account
    }
}

/// Exact source-emitter identity. EVM and TRON identities bind governed route configuration,
/// never a mutable owner address.
public enum SccpSourceEmitterV1: Equatable, Sendable {
    case evm(address: Data, runtimeCodeHash: Data, routeConfigHash: Data)
    case tron(address: Data, runtimeCodeHash: Data, routeConfigHash: Data)
    case ton(address: SccpTonAddressV1, codeHash: Data, routeConfigHash: Data)

    public static func validatedEvm(address: Data, runtimeCodeHash: Data, routeConfigHash: Data) throws -> Self {
        try requireRole(address, count: 20, name: "address")
        try requireRole(runtimeCodeHash, count: 32, name: "runtime_code_hash")
        try requireRole(routeConfigHash, count: 32, name: "route_config_hash")
        try requireDistinct([runtimeCodeHash, routeConfigHash], label: "EVM emitter hash roles")
        return .evm(address: Data(address), runtimeCodeHash: Data(runtimeCodeHash), routeConfigHash: Data(routeConfigHash))
    }

    public static func validatedTron(address: Data, runtimeCodeHash: Data, routeConfigHash: Data) throws -> Self {
        try requireRole(address, count: 20, name: "address")
        try requireRole(runtimeCodeHash, count: 32, name: "runtime_code_hash")
        try requireRole(routeConfigHash, count: 32, name: "route_config_hash")
        try requireDistinct([runtimeCodeHash, routeConfigHash], label: "TRON emitter hash roles")
        return .tron(address: Data(address), runtimeCodeHash: Data(runtimeCodeHash), routeConfigHash: Data(routeConfigHash))
    }

    public static func validatedTon(address: SccpTonAddressV1, codeHash: Data, routeConfigHash: Data) throws -> Self {
        guard address.isSccpBasechainContract else {
            throw SccpV1Error.invalid("SCCP TON source emitter must use basechain workchain 0")
        }
        try requireRole(codeHash, count: 32, name: "code_hash")
        try requireRole(routeConfigHash, count: 32, name: "route_config_hash")
        try requireDistinct([codeHash, routeConfigHash], label: "TON emitter hash roles")
        return .ton(address: address, codeHash: Data(codeHash), routeConfigHash: Data(routeConfigHash))
    }

    private static func requireRole(_ value: Data, count: Int, name: String) throws {
        guard value.count == count, value.contains(where: { $0 != 0 }) else {
            throw SccpV1Error.invalid("\(name) must be a nonzero \(count)-byte value")
        }
    }

    private static func requireDistinct(_ values: [Data], label: String) throws {
        for left in values.indices {
            for right in values.indices where right > left {
                guard values[left] != values[right] else {
                    throw SccpV1Error.invalid("\(label) must be distinct")
                }
            }
        }
    }
}

/// Consensus-compatible exact-lane hashing and fixed layouts for SCCP V1.
public enum SccpV1 {
    /// Exact I105 discriminant used by the public SORA Taira SCCP endpoint.
    public static let tairaI105DiscriminantV1: UInt16 = 369

    private static let laneHashPrefix = Data("sccp:lane-id:v1".utf8)
    private static let messageIdPrefix = Data("sccp:lane-message-id:v1".utf8)
    private static let payloadHashPrefix = Data("sccp:payload:v1".utf8)
    private static let sourceEventPrefix = Data("sccp:source:event:v1".utf8)

    /// Canonical profile bytes independent of Swift enum layout.
    public static func canonicalNetworkBytes(_ network: SccpNetworkV1) -> Data {
        var out = Data([1, network.tag])
        appendUInt32LE(network.domainId, to: &out)
        switch network {
        case .soraTaira:
            out.append(try! decodeLowerHex("fc56984b2be7431d840e21514d1883f0"))
        case .ethereumMainnet:
            appendUInt64LE(1, to: &out)
        case .bscMainnet:
            appendUInt64LE(56, to: &out)
        case .tronMainnet:
            appendUInt32LE(0x2b66_53dc, to: &out)
        case .tonMainnet:
            appendInt32LE(-239, to: &out)
            appendInt32LE(-1, to: &out)
            appendUInt64LE(0x8000_0000_0000_0000, to: &out)
            appendUInt32LE(0, to: &out)
            out.append(try! decodeLowerHex("17a3a92992aabea785a7a090985a265cd31f323d849da51239737e321fb05569"))
            out.append(try! decodeLowerHex("5e994fcf4d425c0a6ce6a792594b7173205f740a39cd56f537defd28b48a0f6e"))
        }
        return out
    }

    /// Canonical directed exact-lane bytes.
    public static func canonicalLaneBytes(_ lane: SccpLaneIdV1) -> Data {
        var out = Data([1])
        appendBytes(canonicalNetworkBytes(lane.source), to: &out)
        appendBytes(canonicalNetworkBytes(lane.target), to: &out)
        return out
    }

    /// Blake2b-256 of the domain-separated lane.
    public static func laneHash(_ lane: SccpLaneIdV1) -> Data {
        Blake2b.hash256(laneHashPrefix + canonicalLaneBytes(lane))
    }

    /// Hash canonical payload bytes exactly as consensus does.
    public static func payloadHash(_ canonicalPayload: Data) throws -> Data {
        guard !canonicalPayload.isEmpty else {
            throw SccpV1Error.invalid("canonical SCCP payload must not be empty")
        }
        return Blake2b.hash256(payloadHashPrefix + canonicalPayload)
    }

    /// Compute a lane-bound message id. Destination binding is deliberately excluded.
    public static func messageId(lane: SccpLaneIdV1, canonicalPayload: Data) throws -> Data {
        guard !canonicalPayload.isEmpty else {
            throw SccpV1Error.invalid("canonical SCCP payload must not be empty")
        }
        var body = Data([1])
        appendBytes(canonicalLaneBytes(lane), to: &body)
        appendBytes(canonicalPayload, to: &body)
        let digest = irohaKeccak256(messageIdPrefix + body)
        guard digest.contains(where: { $0 != 0 }) else {
            throw SccpV1Error.invalid("SCCP message id must be nonzero")
        }
        return digest
    }

    /// Canonical contract-computable source-event bytes after the domain prefix.
    public static func canonicalSourceEventBytes(
        lane: SccpLaneIdV1,
        messageId: Data,
        payloadHash: Data
    ) throws -> Data {
        let laneDigest = laneHash(lane)
        try requireHash(messageId, field: "messageId")
        try requireHash(payloadHash, field: "payloadHash")
        let roles = [laneDigest, messageId, payloadHash]
        guard Set(roles).count == roles.count else {
            throw SccpV1Error.invalid("SCCP lane, message, and payload hash roles must be distinct")
        }
        return Data([1]) + laneDigest + messageId + payloadHash
    }

    /// Keccak-256(`sccp:source:event:v1 || 0x01 || lane_hash || message_id || payload_hash`).
    public static func sourceEventDigest(
        lane: SccpLaneIdV1,
        messageId: Data,
        payloadHash: Data
    ) throws -> Data {
        irohaKeccak256(sourceEventPrefix + (try canonicalSourceEventBytes(
            lane: lane,
            messageId: messageId,
            payloadHash: payloadHash
        )))
    }

    /// Strict prefixless lowercase hexadecimal decoder.
    public static func decodeLowerHex(_ value: String) throws -> Data {
        guard value.count.isMultiple(of: 2), !value.isEmpty,
              value.allSatisfy({ $0.isNumber || ("a"..."f").contains(String($0)) }),
              let data = Data(hexString: value)
        else {
            throw SccpV1Error.invalid("hex must be canonical lowercase without 0x")
        }
        return data
    }

    /// Prefixless lowercase hexadecimal encoder.
    public static func encodeLowerHex(_ value: Data) -> String {
        value.map { String(format: "%02x", $0) }.joined()
    }

    private static func requireHash(_ value: Data, field: String) throws {
        guard value.count == 32, value.contains(where: { $0 != 0 }) else {
            throw SccpV1Error.invalid("\(field) must be a nonzero 32-byte value")
        }
    }

    private static func appendBytes(_ value: Data, to out: inout Data) {
        appendUInt32LE(UInt32(value.count), to: &out)
        out.append(value)
    }

    private static func appendUInt32LE(_ value: UInt32, to out: inout Data) {
        var little = value.littleEndian
        withUnsafeBytes(of: &little) { out.append(contentsOf: $0) }
    }

    private static func appendInt32LE(_ value: Int32, to out: inout Data) {
        var little = value.littleEndian
        withUnsafeBytes(of: &little) { out.append(contentsOf: $0) }
    }

    private static func appendUInt64LE(_ value: UInt64, to out: inout Data) {
        var little = value.littleEndian
        withUnsafeBytes(of: &little) { out.append(contentsOf: $0) }
    }

}
