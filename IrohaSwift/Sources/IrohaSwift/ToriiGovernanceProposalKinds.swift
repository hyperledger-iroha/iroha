import CryptoKit
import Foundation

private let maximumUInt128 = "340282366920938463463374607431768211455"
private let maximumTonCoins = "1329227995784915872903807060280344575"
private let keccak256EmptyBytes = Data(hexString: "C5D2460186F7233C927E7DB2DCC703C0E500B653CA82273B7BFAD8045D85A470")!

let governanceExactIntegerLexemesUserInfoKey = CodingUserInfoKey(
    rawValue: "org.hyperledger.iroha.governance.exact-integer-lexemes"
)!

private func governanceCodingPathKey(_ path: [CodingKey]) -> String {
    path.map { key in
        if let index = key.intValue {
            return "i:\(index)"
        }
        return "k:\(key.stringValue)"
    }.joined(separator: "/")
}

/// Scan one proposal with the SCCP exact JSON parser and retain every integer lexeme.
///
/// `JSONDecoder` routes JSON numbers through Foundation numeric types and therefore
/// cannot preserve all UInt128 values. The proposal entry point installs this table in
/// `Decoder.userInfo`, allowing the SCCP cap decoders to validate the original token.
func governanceExactJSONIntegerLexemes(_ data: Data) throws -> [String: String] {
    let root = try SccpStrictJSON.object(data, label: "governance proposal")
    var lexemes: [String: String] = [:]

    func walk(_ value: Any, path: [GovernanceProposalCodingKey]) throws {
        if value is Bool || value is String || value is NSNull {
            return
        }
        if let exact = value as? SccpStrictJSON.ExactUnsignedInteger {
            let field = path.last?.stringValue
            guard field == "max_wrapped_supply" || field == "max_outstanding_liability" else {
                throw SccpV1Error.invalid(
                    "governance proposal integer is outside the exact first-release JSON range"
                )
            }
            lexemes[governanceCodingPathKey(path)] = exact.text
            return
        }
        if let number = value as? NSNumber,
           CFGetTypeID(number) != CFBooleanGetTypeID(),
           !CFNumberIsFloatType(number)
        {
            let text = number.stringValue
            let field = path.last?.stringValue
            if field != "max_wrapped_supply" && field != "max_outstanding_liability" {
                guard let parsed = UInt64(text), parsed <= 9_007_199_254_740_991 else {
                    throw SccpV1Error.invalid(
                        "governance proposal integer is outside the exact first-release JSON range"
                    )
                }
            }
            lexemes[governanceCodingPathKey(path)] = text
            return
        }
        if let object = value as? [String: Any] {
            for (key, item) in object {
                try walk(item, path: path + [GovernanceProposalCodingKey(key)])
            }
            return
        }
        if let array = value as? [Any] {
            for (index, item) in array.enumerated() {
                try walk(item, path: path + [GovernanceProposalCodingKey(intValue: index)!])
            }
            return
        }
        throw SccpV1Error.invalid("governance proposal contains an unsupported JSON value")
    }

    try walk(root, path: [])
    return lexemes
}

private func governanceExactPositiveInteger<Key: CodingKey>(
    decoder: Decoder,
    container: KeyedDecodingContainer<Key>,
    key: Key,
    maximum: String
) throws -> String {
    let path = decoder.codingPath + [key]
    let pathKey = governanceCodingPathKey(path)
    let text: String
    if let lexemes = decoder.userInfo[governanceExactIntegerLexemesUserInfoKey]
        as? [String: String],
       let exact = lexemes[pathKey]
    {
        text = exact
    } else if let value = try? container.decode(UInt64.self, forKey: key) {
        text = String(value)
    } else {
        throw DecodingError.dataCorruptedError(
            forKey: key,
            in: container,
            debugDescription: "\(key.stringValue) requires its exact unsigned JSON integer lexeme"
        )
    }
    guard !text.isEmpty,
          text.first != "0",
          text.utf8.allSatisfy({ (48...57).contains($0) }),
          text.count < maximum.count || text.count == maximum.count && text <= maximum else {
        throw DecodingError.dataCorruptedError(
            forKey: key,
            in: container,
            debugDescription: "\(key.stringValue) is outside its canonical positive range"
        )
    }
    return text
}

private func governanceMultiplyDecimal(_ value: String, by multiplier: UInt64) -> String {
    var carry: UInt64 = 0
    var digits: [UInt8] = []
    digits.reserveCapacity(value.count + 20)
    for byte in value.utf8.reversed() {
        let product = UInt64(byte - 48) * multiplier + carry
        digits.append(UInt8(product % 10))
        carry = product / 10
    }
    while carry != 0 {
        digits.append(UInt8(carry % 10))
        carry /= 10
    }
    return digits.reversed().map(String.init).joined()
}

let governanceFirstReleaseMaxExactJSONInteger = 9_007_199_254_740_991.0

func governanceRequireExactJSONIntegers(
    _ value: ToriiJSONValue,
    codingPath: [CodingKey],
    context: String,
    exactIntegerLexemes: [String: String]? = nil
) throws {
    switch value {
    case let .array(values):
        for (index, item) in values.enumerated() {
            try governanceRequireExactJSONIntegers(
                item,
                codingPath: codingPath + [GovernanceProposalCodingKey(intValue: index)!],
                context: "\(context)[\(index)]",
                exactIntegerLexemes: exactIntegerLexemes
            )
        }
    case let .object(object):
        for (key, item) in object {
            try governanceRequireExactJSONIntegers(
                item,
                codingPath: codingPath + [GovernanceProposalCodingKey(key)],
                context: "\(context).\(key)",
                exactIntegerLexemes: exactIntegerLexemes
            )
        }
    case let .number(number):
        let isSafelyRepresentable = number.isFinite
            && number.rounded(.towardZero) == number
            && abs(number) <= governanceFirstReleaseMaxExactJSONInteger
        let hasValidatedExactLexeme = exactIntegerLexemes?[governanceCodingPathKey(codingPath)] != nil
        guard isSafelyRepresentable || hasValidatedExactLexeme else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: codingPath,
                    debugDescription: "\(context) is outside the exact first-release JSON integer range"
                )
            )
        }
    case .string, .bool, .null:
        break
    }
}

private struct GovernanceProposalCodingKey: CodingKey {
    let stringValue: String
    let intValue: Int?

    init(_ stringValue: String) {
        self.stringValue = stringValue
        intValue = nil
    }

    init?(stringValue: String) {
        self.init(stringValue)
    }

    init?(intValue: Int) {
        stringValue = String(intValue)
        self.intValue = intValue
    }
}

private func governanceRejectUnknownFields(
    _ decoder: Decoder,
    allowed: Set<String>,
    name: String
) throws {
    let container = try decoder.container(keyedBy: GovernanceProposalCodingKey.self)
    guard container.allKeys.allSatisfy({ allowed.contains($0.stringValue) }) else {
        throw DecodingError.dataCorrupted(
            .init(
                codingPath: decoder.codingPath,
                debugDescription: "\(name) contains an unknown or retired field"
            )
        )
    }
}

private func governanceCanonicalAccount(
    _ raw: String,
    codingPath: [CodingKey],
    field: String
) throws -> String {
    do {
        _ = try exactCanonicalToriiAccountAddress(raw)
        return raw
    } catch {
        throw DecodingError.dataCorrupted(
            .init(
                codingPath: codingPath,
                debugDescription: "\(field) must be an exact canonical account address"
            )
        )
    }
}

private func governanceCanonicalAssetDefinition(
    _ raw: String,
    codingPath: [CodingKey],
    field: String
) throws -> String {
    guard AssetDefinitionAddress.decode(raw) != nil else {
        throw DecodingError.dataCorrupted(
            .init(
                codingPath: codingPath,
                debugDescription: "\(field) must be an exact canonical asset definition address"
            )
        )
    }
    return raw
}

private func governanceCanonicalContractAddress(
    _ raw: String,
    codingPath: [CodingKey],
    field: String
) throws -> String {
    guard raw.utf8.elementsEqual(
        raw.trimmingCharacters(in: .whitespacesAndNewlines).utf8
    ), ContractAddressV1.isCanonical(raw) else {
        throw DecodingError.dataCorrupted(
            .init(
                codingPath: codingPath,
                debugDescription: "\(field) must be an exact canonical ABI V1 contract address"
            )
        )
    }
    return raw
}

private func governanceFixedBytes(
    _ bytes: [UInt8],
    count: Int,
    nonzero: Bool = false,
    codingPath: [CodingKey],
    field: String
) throws -> Data {
    guard bytes.count == count, !nonzero || bytes.contains(where: { $0 != 0 }) else {
        throw DecodingError.dataCorrupted(
            .init(
                codingPath: codingPath,
                debugDescription: "\(field) must contain exactly \(count)\(nonzero ? " non-zero" : "") bytes"
            )
        )
    }
    return Data(bytes)
}

private func governanceLowercaseHash32(
    _ raw: String,
    codingPath: [CodingKey],
    field: String
) throws -> Data {
    guard raw.utf8.count == 64,
          raw.utf8.allSatisfy({ byte in
              (0x30...0x39).contains(byte) || (0x61...0x66).contains(byte)
          }),
          let bytes = Data(hexString: raw),
          bytes.count == 32 else {
        throw DecodingError.dataCorrupted(
            .init(
                codingPath: codingPath,
                debugDescription: "\(field) must be exactly 64 lowercase hexadecimal characters"
            )
        )
    }
    return bytes
}

private func governanceBoundedReason(
    _ raw: String,
    codingPath: [CodingKey],
    field: String
) throws -> String {
    guard !raw.isEmpty,
          raw.utf8.count <= 1_024,
          raw == raw.trimmingCharacters(in: .whitespacesAndNewlines),
          !raw.unicodeScalars.contains(where: CharacterSet.controlCharacters.contains) else {
        throw DecodingError.dataCorrupted(
            .init(
                codingPath: codingPath,
                debugDescription: "\(field) must be bounded canonical public text"
            )
        )
    }
    return raw
}

private func governanceCanonicalBase64(
    _ raw: String,
    codingPath: [CodingKey],
    field: String
) throws -> Data {
    guard let decoded = Data(base64Encoded: raw), decoded.base64EncodedString() == raw else {
        throw DecodingError.dataCorrupted(
            .init(
                codingPath: codingPath,
                debugDescription: "\(field) must use exact canonical base64"
            )
        )
    }
    return decoded
}

private func governanceCanonicalQuantity(
    _ raw: String,
    codingPath: [CodingKey],
    field: String
) throws -> String {
    do {
        let decoded = try KotodamaNumericV1Codec.decodeQuantityJSON(raw)
        guard decoded.canonicalString == raw else {
            throw DecodingError.dataCorrupted(
                .init(codingPath: codingPath, debugDescription: "noncanonical Quantity")
            )
        }
        return raw
    } catch {
        throw DecodingError.dataCorrupted(
            .init(
                codingPath: codingPath,
                debugDescription: "\(field) must be a canonical non-negative Quantity string"
            )
        )
    }
}

private func governanceCanonicalNumeric(
    _ raw: String,
    codingPath: [CodingKey],
    field: String
) throws -> String {
    do {
        let decoded = try KotodamaNumericV1Codec.decodeDecimalJSON(raw)
        guard decoded.canonicalString == raw else {
            throw DecodingError.dataCorrupted(
                .init(codingPath: codingPath, debugDescription: "noncanonical Numeric")
            )
        }
        return raw
    } catch {
        throw DecodingError.dataCorrupted(
            .init(
                codingPath: codingPath,
                debugDescription: "\(field) must be a canonical Numeric string"
            )
        )
    }
}

private func governanceCanonicalUInt64String(
    _ raw: String,
    codingPath: [CodingKey],
    field: String,
    positive: Bool = false
) throws -> String {
    guard !raw.isEmpty,
          raw.allSatisfy({ $0 >= "0" && $0 <= "9" }),
          raw == "0" || raw.first != "0",
          let parsed = UInt64(raw),
          !positive || parsed > 0 else {
        throw DecodingError.dataCorrupted(
            .init(
                codingPath: codingPath,
                debugDescription: "\(field) must be a canonical UInt64 decimal string"
            )
        )
    }
    return raw
}

private func governanceCanonicalKey(_ raw: String) -> Bool {
    let bytes = raw.utf8
    return !bytes.isEmpty
        && bytes.count <= 64
        && bytes.first?.isLetterOrNumber == true
        && bytes.last?.isLetterOrNumber == true
        && bytes.allSatisfy {
            ($0 >= 0x61 && $0 <= 0x7A)
                || ($0 >= 0x30 && $0 <= 0x39)
                || $0 == 0x5F
                || $0 == 0x2D
        }
}

private extension UInt8 {
    var isLetterOrNumber: Bool {
        (self >= 0x41 && self <= 0x5A)
            || (self >= 0x61 && self <= 0x7A)
            || (self >= 0x30 && self <= 0x39)
    }
}

/// Stored payload for a governed runtime-upgrade proposal.
public struct ToriiGovernanceRuntimeUpgradeProposal: Decodable, Sendable, Equatable {
    public let manifest: ToriiGovernanceRuntimeUpgradeManifest

    private enum CodingKeys: String, CodingKey, CaseIterable { case manifest }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "runtime-upgrade proposal"
        )
        manifest = try decoder.container(keyedBy: CodingKeys.self)
            .decode(ToriiGovernanceRuntimeUpgradeManifest.self, forKey: .manifest)
    }
}

/// Exact SBOM digest carried by a governed runtime-upgrade manifest.
public struct ToriiGovernanceRuntimeUpgradeSbomDigest: Decodable, Sendable, Equatable {
    public let algorithm: String
    public let digest: Data

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case algorithm
        case digest
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "runtime-upgrade SBOM digest"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        algorithm = try container.decode(String.self, forKey: .algorithm)
        guard !algorithm.isEmpty,
              algorithm.utf8.elementsEqual(
                  algorithm.trimmingCharacters(in: .whitespacesAndNewlines).utf8
              ) else {
            throw DecodingError.dataCorruptedError(
                forKey: .algorithm,
                in: container,
                debugDescription: "SBOM algorithm must be an exact non-empty string"
            )
        }
        digest = try governanceCanonicalBase64(
            container.decode(String.self, forKey: .digest),
            codingPath: container.codingPath + [CodingKeys.digest],
            field: "digest"
        )
    }
}

/// Canonical V1 runtime-upgrade manifest stored inside a governance proposal.
public struct ToriiGovernanceRuntimeUpgradeManifest: Decodable, Sendable, Equatable {
    public let name: String
    public let description: String
    public let abiVersion: UInt16
    public let abiHash: Data
    public let addedSyscalls: [UInt16]
    public let addedPointerTypes: [UInt16]
    public let startHeight: UInt64
    public let endHeight: UInt64
    public let sbomDigests: [ToriiGovernanceRuntimeUpgradeSbomDigest]
    public let slsaAttestation: Data
    public let provenance: [ToriiContractManifestProvenance]

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case name
        case description
        case abiVersion = "abi_version"
        case abiHash = "abi_hash"
        case addedSyscalls = "added_syscalls"
        case addedPointerTypes = "added_pointer_types"
        case startHeight = "start_height"
        case endHeight = "end_height"
        case sbomDigests = "sbom_digests"
        case slsaAttestation = "slsa_attestation"
        case provenance
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "runtime-upgrade manifest"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        name = try container.decode(String.self, forKey: .name)
        description = try container.decode(String.self, forKey: .description)
        abiVersion = try container.decode(UInt16.self, forKey: .abiVersion)
        guard abiVersion == 1 else {
            throw DecodingError.dataCorruptedError(
                forKey: .abiVersion,
                in: container,
                debugDescription: "runtime-upgrade abi_version must be exactly 1"
            )
        }
        abiHash = try governanceFixedBytes(
            container.decode([UInt8].self, forKey: .abiHash),
            count: 32,
            codingPath: container.codingPath + [CodingKeys.abiHash],
            field: "abi_hash"
        )
        addedSyscalls = try container.decode([UInt16].self, forKey: .addedSyscalls)
        addedPointerTypes = try container.decode([UInt16].self, forKey: .addedPointerTypes)
        guard addedSyscalls.isEmpty, addedPointerTypes.isEmpty else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "runtime-upgrade V1 delta lists must be empty"
                )
            )
        }
        startHeight = try container.decode(UInt64.self, forKey: .startHeight)
        endHeight = try container.decode(UInt64.self, forKey: .endHeight)
        guard startHeight < endHeight else {
            throw DecodingError.dataCorruptedError(
                forKey: .endHeight,
                in: container,
                debugDescription: "runtime-upgrade end_height must be greater than start_height"
            )
        }
        sbomDigests = try container.decode(
            [ToriiGovernanceRuntimeUpgradeSbomDigest].self,
            forKey: .sbomDigests
        )
        slsaAttestation = try governanceCanonicalBase64(
            container.decode(String.self, forKey: .slsaAttestation),
            codingPath: container.codingPath + [CodingKeys.slsaAttestation],
            field: "slsa_attestation"
        )
        provenance = try container.decode(
            [ToriiContractManifestProvenance].self,
            forKey: .provenance
        )
    }
}

/// Stored payload for one SCCP route-registry governance action.
public struct ToriiGovernanceSccpRouteProposal: Decodable, Sendable, Equatable {
    public let anchor: ToriiGovernanceSccpRouteAnchor

    private enum CodingKeys: String, CodingKey, CaseIterable { case anchor }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP route-governance proposal"
        )
        anchor = try decoder.container(keyedBy: CodingKeys.self)
            .decode(ToriiGovernanceSccpRouteAnchor.self, forKey: .anchor)
    }
}

/// Exact network- and action-bound SCCP governance preimage.
public struct ToriiGovernanceSccpRouteAnchor: Decodable, Sendable, Equatable {
    public let networkId: NetworkId
    public let action: ToriiGovernanceSccpRouteAction

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case networkId = "network_id"
        case action
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP route-governance anchor"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        networkId = try container.decode(NetworkId.self, forKey: .networkId)
        action = try container.decode(ToriiGovernanceSccpRouteAction.self, forKey: .action)
    }
}

/// Closed first-release SCCP network inventory used by governance payloads.
public enum ToriiGovernanceSccpNetwork: String, Decodable, Sendable, Equatable {
    case soraTaira = "sora_taira"
    case ethereumMainnet = "ethereum_mainnet"
    case bscMainnet = "bsc_mainnet"
    case tronMainnet = "tron_mainnet"
    case tonMainnet = "ton_mainnet"

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case network
        case profile
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP network"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        guard container.contains(.profile), try container.decodeNil(forKey: .profile) else {
            throw DecodingError.dataCorruptedError(
                forKey: .profile,
                in: container,
                debugDescription: "SCCP V1 network profile must be explicit null"
            )
        }
        let raw = try container.decode(String.self, forKey: .network)
        guard let value = Self(rawValue: raw) else {
            throw DecodingError.dataCorruptedError(
                forKey: .network,
                in: container,
                debugDescription: "unsupported SCCP V1 network"
            )
        }
        self = value
    }

    fileprivate var isSora: Bool { self == .soraTaira }

    fileprivate var family: String {
        switch self {
        case .ethereumMainnet, .bscMainnet: return "evm"
        case .tronMainnet: return "tron"
        case .tonMainnet: return "ton"
        case .soraTaira: return "sora"
        }
    }

    var discoveryValue: SccpNetworkV1 {
        switch self {
        case .soraTaira: .soraTaira
        case .ethereumMainnet: .ethereumMainnet
        case .bscMainnet: .bscMainnet
        case .tronMainnet: .tronMainnet
        case .tonMainnet: .tonMainnet
        }
    }

}

/// Directed SCCP V1 lane used by a governed route.
public struct ToriiGovernanceSccpLane: Decodable, Sendable, Equatable {
    public let source: ToriiGovernanceSccpNetwork
    public let target: ToriiGovernanceSccpNetwork

    private enum CodingKeys: String, CodingKey, CaseIterable { case source, target }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP lane"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        source = try container.decode(ToriiGovernanceSccpNetwork.self, forKey: .source)
        target = try container.decode(ToriiGovernanceSccpNetwork.self, forKey: .target)
        guard source.isSora != target.isSora else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "SCCP lane must join exactly one SORA and one external network"
                )
            )
        }
    }

    fileprivate var isInbound: Bool { !source.isSora && target.isSora }

    var discoveryValue: SccpLaneIdV1 {
        get throws { try SccpLaneIdV1(source: source.discoveryValue, target: target.discoveryValue) }
    }
}

/// Closed directional state of a governed SCCP route revision.
public enum ToriiGovernanceSccpRouteActivation: String, Decodable, Sendable, Equatable {
    case staged
    case bidirectional
    case inboundOnly = "inbound_only"
    case paused
    case retired

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case activation
        case direction
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP route activation"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        guard container.contains(.direction), try container.decodeNil(forKey: .direction) else {
            throw DecodingError.dataCorruptedError(
                forKey: .direction,
                in: container,
                debugDescription: "SCCP V1 activation direction must be explicit null"
            )
        }
        let raw = try container.decode(String.self, forKey: .activation)
        guard let value = Self(rawValue: raw) else {
            throw DecodingError.dataCorruptedError(
                forKey: .activation,
                in: container,
                debugDescription: "unsupported SCCP V1 activation"
            )
        }
        self = value
    }

    fileprivate var isTerminal: Bool { self == .retired }

    fileprivate func canTransition(to next: Self) -> Bool {
        guard self != next, self != .retired else { return false }
        switch (self, next) {
        case (.staged, .bidirectional), (.staged, .inboundOnly), (.staged, .retired),
             (.paused, .bidirectional), (.paused, .inboundOnly), (.paused, .retired),
             (.bidirectional, .inboundOnly), (.bidirectional, .paused),
             (.inboundOnly, .paused), (.inboundOnly, .retired):
            return true
        default:
            return false
        }
    }
}

/// Immutable lookup key for one governed SCCP route revision.
public struct ToriiGovernanceSccpRouteKey: Decodable, Sendable, Equatable {
    public let laneId: ToriiGovernanceSccpLane
    public let routeId: String
    public let assetKey: String
    public let revision: UInt32

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case laneId = "lane_id"
        case routeId = "route_id"
        case assetKey = "asset_key"
        case revision
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP route key"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        laneId = try container.decode(ToriiGovernanceSccpLane.self, forKey: .laneId)
        routeId = try container.decode(String.self, forKey: .routeId)
        assetKey = try container.decode(String.self, forKey: .assetKey)
        revision = try container.decode(UInt32.self, forKey: .revision)
        guard laneId.isInbound,
              governanceCanonicalKey(routeId),
              governanceCanonicalKey(assetKey),
              revision > 0 else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "SCCP route key is not a canonical inbound V1 key"
                )
            )
        }
    }
}

/// Closed native proof backend carried beside a governed SCCP checkpoint.
public enum ToriiGovernanceSccpNativeBackend: String, Decodable, Sendable, Equatable {
    case ethereumBeacon = "ethereum_beacon_v1"
    case bscParlia = "bsc_parlia_v1"
    case tronDpos = "tron_dpos_v1"
    case tonMasterchain = "ton_masterchain_v1"

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case backend
        case protocolPayload = "protocol"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP native backend"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        guard container.contains(.protocolPayload),
              try container.decodeNil(forKey: .protocolPayload) else {
            throw DecodingError.dataCorruptedError(
                forKey: .protocolPayload,
                in: container,
                debugDescription: "SCCP V1 native backend protocol must be explicit null"
            )
        }
        let raw = try container.decode(String.self, forKey: .backend)
        guard let value = Self(rawValue: raw) else {
            throw DecodingError.dataCorruptedError(
                forKey: .backend,
                in: container,
                debugDescription: "unsupported SCCP V1 native backend"
            )
        }
        self = value
    }

    fileprivate func supports(_ network: ToriiGovernanceSccpNetwork) -> Bool {
        switch (self, network) {
        case (.ethereumBeacon, .ethereumMainnet),
             (.bscParlia, .bscMainnet),
             (.tronDpos, .tronMainnet),
             (.tonMasterchain, .tonMainnet):
            return true
        default:
            return false
        }
    }
}

/// Governed native checkpoint for one SCCP lane.
public struct ToriiGovernanceSccpNativeTrustAnchor: Decodable, Sendable, Equatable {
    public let backend: ToriiGovernanceSccpNativeBackend
    public let anchorHash: Data
    public let checkpointHeight: UInt64

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case backend
        case anchorHash = "anchor_hash"
        case checkpointHeight = "checkpoint_height"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP native trust anchor"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        backend = try container.decode(ToriiGovernanceSccpNativeBackend.self, forKey: .backend)
        anchorHash = try governanceFixedBytes(
            container.decode([UInt8].self, forKey: .anchorHash),
            count: 32,
            nonzero: true,
            codingPath: container.codingPath + [CodingKeys.anchorHash],
            field: "anchor_hash"
        )
        checkpointHeight = try container.decode(UInt64.self, forKey: .checkpointHeight)
        guard checkpointHeight > 0 else {
            throw DecodingError.dataCorruptedError(
                forKey: .checkpointHeight,
                in: container,
                debugDescription: "checkpoint_height must be non-zero"
            )
        }
    }
}

/// Authenticated upper boundary for delayed SCCP claims.
public struct ToriiGovernanceSccpInboundFinalityCutoff: Decodable, Sendable, Equatable {
    public let trustAnchorHash: Data
    public let maxAnchorIntervalHeight: UInt64

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case trustAnchorHash = "trust_anchor_hash"
        case maxAnchorIntervalHeight = "max_anchor_interval_height"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP inbound-finality cutoff"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        trustAnchorHash = try governanceFixedBytes(
            container.decode([UInt8].self, forKey: .trustAnchorHash),
            count: 32,
            nonzero: true,
            codingPath: container.codingPath + [CodingKeys.trustAnchorHash],
            field: "trust_anchor_hash"
        )
        maxAnchorIntervalHeight = try container.decode(
            UInt64.self,
            forKey: .maxAnchorIntervalHeight
        )
        guard maxAnchorIntervalHeight > 0 else {
            throw DecodingError.dataCorruptedError(
                forKey: .maxAnchorIntervalHeight,
                in: container,
                debugDescription: "max_anchor_interval_height must be non-zero"
            )
        }
    }
}

/// Exact staged-route registration approved by SCCP governance.
public struct ToriiGovernanceSccpRegisterRoute: Decodable, Sendable, Equatable {
    public let route: ToriiGovernanceSccpGovernedRoute
    public let nativeTrustAnchor: ToriiGovernanceSccpNativeTrustAnchor?

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case route
        case nativeTrustAnchor = "native_trust_anchor"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP route registration"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        route = try container.decode(ToriiGovernanceSccpGovernedRoute.self, forKey: .route)
        guard container.contains(.nativeTrustAnchor) else {
            throw DecodingError.keyNotFound(
                CodingKeys.nativeTrustAnchor,
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "native_trust_anchor must be explicit null or an exact anchor"
                )
            )
        }
        nativeTrustAnchor = try container.decodeIfPresent(
            ToriiGovernanceSccpNativeTrustAnchor.self,
            forKey: .nativeTrustAnchor
        )
        guard route.activation == .staged,
              route.inboundFinalityCutoff == nil,
              nativeTrustAnchor.map({ $0.backend.supports(route.laneId.source) }) ?? true else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "SCCP registration must carry a staged route and matching optional anchor"
                )
            )
        }
    }
}

/// Compare-and-set update of one SCCP route activation.
public struct ToriiGovernanceSccpSetRouteActivation: Decodable, Sendable, Equatable {
    public let key: ToriiGovernanceSccpRouteKey
    public let expectedCurrent: ToriiGovernanceSccpRouteActivation
    public let next: ToriiGovernanceSccpRouteActivation
    public let inboundFinalityCutoff: ToriiGovernanceSccpInboundFinalityCutoff?

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case key
        case expectedCurrent = "expected_current"
        case next
        case inboundFinalityCutoff = "inbound_finality_cutoff"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP route-activation update"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        key = try container.decode(ToriiGovernanceSccpRouteKey.self, forKey: .key)
        expectedCurrent = try container.decode(
            ToriiGovernanceSccpRouteActivation.self,
            forKey: .expectedCurrent
        )
        next = try container.decode(ToriiGovernanceSccpRouteActivation.self, forKey: .next)
        guard container.contains(.inboundFinalityCutoff) else {
            throw DecodingError.keyNotFound(
                CodingKeys.inboundFinalityCutoff,
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "inbound_finality_cutoff must be explicit null or an exact cutoff"
                )
            )
        }
        inboundFinalityCutoff = try container.decodeIfPresent(
            ToriiGovernanceSccpInboundFinalityCutoff.self,
            forKey: .inboundFinalityCutoff
        )
        guard expectedCurrent.canTransition(to: next),
              next.isTerminal == (inboundFinalityCutoff != nil) else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "invalid SCCP route-activation transition"
                )
            )
        }
    }
}

/// Atomic SCCP route-revision cutover.
public struct ToriiGovernanceSccpSwitchRouteRevision: Decodable, Sendable, Equatable {
    public let previousKey: ToriiGovernanceSccpRouteKey
    public let expectedPrevious: ToriiGovernanceSccpRouteActivation
    public let previousNext: ToriiGovernanceSccpRouteActivation
    public let previousInboundFinalityCutoff: ToriiGovernanceSccpInboundFinalityCutoff?
    public let successorKey: ToriiGovernanceSccpRouteKey
    public let successorNext: ToriiGovernanceSccpRouteActivation

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case previousKey = "previous_key"
        case expectedPrevious = "expected_previous"
        case previousNext = "previous_next"
        case previousInboundFinalityCutoff = "previous_inbound_finality_cutoff"
        case successorKey = "successor_key"
        case successorNext = "successor_next"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP route-revision switch"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        previousKey = try container.decode(ToriiGovernanceSccpRouteKey.self, forKey: .previousKey)
        expectedPrevious = try container.decode(
            ToriiGovernanceSccpRouteActivation.self,
            forKey: .expectedPrevious
        )
        previousNext = try container.decode(
            ToriiGovernanceSccpRouteActivation.self,
            forKey: .previousNext
        )
        guard container.contains(.previousInboundFinalityCutoff) else {
            throw DecodingError.keyNotFound(
                CodingKeys.previousInboundFinalityCutoff,
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "previous_inbound_finality_cutoff must be explicit"
                )
            )
        }
        previousInboundFinalityCutoff = try container.decodeIfPresent(
            ToriiGovernanceSccpInboundFinalityCutoff.self,
            forKey: .previousInboundFinalityCutoff
        )
        successorKey = try container.decode(ToriiGovernanceSccpRouteKey.self, forKey: .successorKey)
        successorNext = try container.decode(
            ToriiGovernanceSccpRouteActivation.self,
            forKey: .successorNext
        )
        let sameLineage = previousKey.laneId == successorKey.laneId
            && previousKey.routeId == successorKey.routeId
            && previousKey.assetKey == successorKey.assetKey
        let validPreviousTransition = previousNext.isTerminal
            ? [.bidirectional, .inboundOnly, .paused].contains(expectedPrevious)
            : expectedPrevious.canTransition(to: previousNext)
        guard sameLineage,
              previousKey.revision < UInt32.max,
              successorKey.revision == previousKey.revision + 1,
              validPreviousTransition,
              [.inboundOnly, .paused, .retired].contains(previousNext),
              previousNext.isTerminal == (previousInboundFinalityCutoff != nil),
              successorNext == .bidirectional else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "invalid SCCP route-revision switch"
                )
            )
        }
    }
}

/// Initial native SCCP lane-checkpoint installation.
public struct ToriiGovernanceSccpInitializeTrustAnchor: Decodable, Sendable, Equatable {
    public let laneId: ToriiGovernanceSccpLane
    public let initial: ToriiGovernanceSccpNativeTrustAnchor

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case laneId = "lane_id"
        case expectedCurrent = "expected_current"
        case initial
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP initial trust-anchor action"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        laneId = try container.decode(ToriiGovernanceSccpLane.self, forKey: .laneId)
        guard container.contains(.expectedCurrent),
              try container.decodeNil(forKey: .expectedCurrent) else {
            throw DecodingError.dataCorruptedError(
                forKey: .expectedCurrent,
                in: container,
                debugDescription: "initial trust-anchor expected_current must be explicit null"
            )
        }
        initial = try container.decode(
            ToriiGovernanceSccpNativeTrustAnchor.self,
            forKey: .initial
        )
        guard laneId.isInbound, initial.backend.supports(laneId.source) else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "initial SCCP trust anchor does not match its lane"
                )
            )
        }
    }
}

/// Append-only SCCP lane-checkpoint advance.
public struct ToriiGovernanceSccpAdvanceTrustAnchor: Decodable, Sendable, Equatable {
    public let laneId: ToriiGovernanceSccpLane
    public let expectedCurrent: ToriiGovernanceSccpNativeTrustAnchor
    public let next: ToriiGovernanceSccpNativeTrustAnchor

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case laneId = "lane_id"
        case expectedCurrent = "expected_current"
        case next
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP trust-anchor advance"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        laneId = try container.decode(ToriiGovernanceSccpLane.self, forKey: .laneId)
        expectedCurrent = try container.decode(
            ToriiGovernanceSccpNativeTrustAnchor.self,
            forKey: .expectedCurrent
        )
        next = try container.decode(ToriiGovernanceSccpNativeTrustAnchor.self, forKey: .next)
        guard laneId.isInbound,
              expectedCurrent.backend == next.backend,
              next.backend.supports(laneId.source),
              expectedCurrent.anchorHash != next.anchorHash,
              next.checkpointHeight > expectedCurrent.checkpointHeight else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "invalid SCCP trust-anchor advance"
                )
            )
        }
    }
}

/// Closed SCCP route-registry action inventory.
public enum ToriiGovernanceSccpRouteAction: Decodable, Sendable, Equatable {
    case register(ToriiGovernanceSccpRegisterRoute)
    case setActivation(ToriiGovernanceSccpSetRouteActivation)
    case switchRevision(ToriiGovernanceSccpSwitchRouteRevision)
    case initializeTrustAnchor(ToriiGovernanceSccpInitializeTrustAnchor)
    case advanceTrustAnchor(ToriiGovernanceSccpAdvanceTrustAnchor)
    case remove(ToriiGovernanceSccpRouteKey)

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case action
        case route
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP route-governance action"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .action) {
        case "Register":
            self = .register(
                try container.decode(ToriiGovernanceSccpRegisterRoute.self, forKey: .route)
            )
        case "SetActivation":
            self = .setActivation(
                try container.decode(ToriiGovernanceSccpSetRouteActivation.self, forKey: .route)
            )
        case "SwitchRevision":
            self = .switchRevision(
                try container.decode(ToriiGovernanceSccpSwitchRouteRevision.self, forKey: .route)
            )
        case "InitializeTrustAnchor":
            self = .initializeTrustAnchor(
                try container.decode(
                    ToriiGovernanceSccpInitializeTrustAnchor.self,
                    forKey: .route
                )
            )
        case "AdvanceTrustAnchor":
            self = .advanceTrustAnchor(
                try container.decode(ToriiGovernanceSccpAdvanceTrustAnchor.self, forKey: .route)
            )
        case "Remove":
            self = .remove(
                try container.decode(ToriiGovernanceSccpRouteKey.self, forKey: .route)
            )
        case let tag:
            throw DecodingError.dataCorruptedError(
                forKey: .action,
                in: container,
                debugDescription: "unsupported SCCP route-governance action \(tag)"
            )
        }
    }
}

/// Exact EVM/TRON source-emitter identity in a governed SCCP route.
public struct ToriiGovernanceSccpContractEmitter: Decodable, Sendable, Equatable {
    public let address: Data
    public let runtimeCodeHash: Data
    public let routeConfigHash: Data

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case address
        case runtimeCodeHash = "runtime_code_hash"
        case routeConfigHash = "route_config_hash"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP contract source emitter"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        address = try governanceFixedBytes(
            container.decode([UInt8].self, forKey: .address),
            count: 20,
            nonzero: true,
            codingPath: container.codingPath + [CodingKeys.address],
            field: "address"
        )
        runtimeCodeHash = try governanceFixedBytes(
            container.decode([UInt8].self, forKey: .runtimeCodeHash),
            count: 32,
            nonzero: true,
            codingPath: container.codingPath + [CodingKeys.runtimeCodeHash],
            field: "runtime_code_hash"
        )
        routeConfigHash = try governanceFixedBytes(
            container.decode([UInt8].self, forKey: .routeConfigHash),
            count: 32,
            nonzero: true,
            codingPath: container.codingPath + [CodingKeys.routeConfigHash],
            field: "route_config_hash"
        )
        guard runtimeCodeHash != routeConfigHash else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "SCCP source-emitter hash roles must be distinct"
                )
            )
        }
    }
}

/// Canonical raw TON basechain contract address used in governance payloads.
public struct ToriiGovernanceSccpTonAddress: Decodable, Sendable, Equatable {
    public let workchain: Int32
    public let account: Data

    private enum CodingKeys: String, CodingKey, CaseIterable { case workchain, account }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP TON address"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        workchain = try container.decode(Int32.self, forKey: .workchain)
        account = try governanceFixedBytes(
            container.decode([UInt8].self, forKey: .account),
            count: 32,
            nonzero: true,
            codingPath: container.codingPath + [CodingKeys.account],
            field: "account"
        )
        guard workchain == 0 else {
            throw DecodingError.dataCorruptedError(
                forKey: .workchain,
                in: container,
                debugDescription: "SCCP TON contracts must use basechain workchain 0"
            )
        }
    }
}

/// Exact TON route-contract source identity in a governed SCCP route.
public struct ToriiGovernanceSccpTonEmitter: Decodable, Sendable, Equatable {
    public let address: ToriiGovernanceSccpTonAddress
    public let codeHash: Data
    public let routeConfigHash: Data

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case address
        case codeHash = "code_hash"
        case routeConfigHash = "route_config_hash"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP TON source emitter"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        address = try container.decode(ToriiGovernanceSccpTonAddress.self, forKey: .address)
        func hash(_ key: CodingKeys) throws -> Data {
            try governanceFixedBytes(
                container.decode([UInt8].self, forKey: key),
                count: 32,
                nonzero: true,
                codingPath: container.codingPath + [key],
                field: key.stringValue
            )
        }
        codeHash = try hash(.codeHash)
        routeConfigHash = try hash(.routeConfigHash)
        guard codeHash != routeConfigHash else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "SCCP TON source-emitter hash roles must be distinct"
                )
            )
        }
    }
}

/// Closed source-emitter family stored in a governed SCCP route.
public enum ToriiGovernanceSccpSourceEmitter: Decodable, Sendable, Equatable {
    case evm(ToriiGovernanceSccpContractEmitter)
    case tron(ToriiGovernanceSccpContractEmitter)
    case ton(ToriiGovernanceSccpTonEmitter)

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case emitter
        case identity
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP source emitter"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .emitter) {
        case "evm":
            self = .evm(
                try container.decode(ToriiGovernanceSccpContractEmitter.self, forKey: .identity)
            )
        case "tron":
            self = .tron(
                try container.decode(ToriiGovernanceSccpContractEmitter.self, forKey: .identity)
            )
        case "ton":
            self = .ton(
                try container.decode(ToriiGovernanceSccpTonEmitter.self, forKey: .identity)
            )
        case let tag:
            throw DecodingError.dataCorruptedError(
                forKey: .emitter,
                in: container,
                debugDescription: "unsupported SCCP source-emitter family \(tag)"
            )
        }
    }

    fileprivate var family: String {
        switch self {
        case .evm: return "evm"
        case .tron: return "tron"
        case .ton: return "ton"
        }
    }

    fileprivate var routeConfigurationHash: Data {
        switch self {
        case let .evm(value), let .tron(value): value.routeConfigHash
        case let .ton(value): value.routeConfigHash
        }
    }

    fileprivate func matches(_ destination: ToriiGovernanceSccpDestination) -> Bool {
        switch (self, destination) {
        case let (.evm(source), .evm(deployment)):
            source.address == deployment.routeAddress
                && source.runtimeCodeHash == deployment.routeCodeHash
        case let (.tron(source), .tron(deployment)):
            source.address == deployment.routeAddress
                && source.runtimeCodeHash == deployment.routeCodeHash
        case let (.ton(source), .ton(deployment)):
            source.address == deployment.routeAddress
                && source.codeHash == deployment.routeCodeHash
        default:
            false
        }
    }

}

/// Lane-bound external source identity for a governed SCCP route.
public struct ToriiGovernanceSccpSourceIdentity: Decodable, Sendable, Equatable {
    public let lane: ToriiGovernanceSccpLane
    public let emitter: ToriiGovernanceSccpSourceEmitter

    private enum CodingKeys: String, CodingKey, CaseIterable { case lane, emitter }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP source identity"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        lane = try container.decode(ToriiGovernanceSccpLane.self, forKey: .lane)
        emitter = try container.decode(ToriiGovernanceSccpSourceEmitter.self, forKey: .emitter)
        guard lane.isInbound, lane.source.family == emitter.family else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "SCCP source identity does not match its inbound lane"
                )
            )
        }
    }
}

private let governanceBn254BaseFieldModulus: [UInt8] = [
    0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29,
    0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58, 0x5d,
    0x97, 0x81, 0x6a, 0x91, 0x68, 0x71, 0xca, 0x8d,
    0x3c, 0x20, 0x8c, 0x16, 0xd8, 0x7c, 0xfd, 0x47,
]

private func governanceBn254Coordinate(
    _ bytes: [UInt8],
    codingPath: [CodingKey],
    field: String
) throws -> Data {
    guard bytes.count == 32,
          bytes.lexicographicallyPrecedes(governanceBn254BaseFieldModulus) else {
        throw DecodingError.dataCorrupted(
            .init(
                codingPath: codingPath,
                debugDescription: "\(field) must be one canonical BN254 base-field element"
            )
        )
    }
    return Data(bytes)
}

/// Canonical non-infinity BN254 G1 point in verifier coordinate order.
public struct ToriiGovernanceSccpBn254G1Point: Decodable, Sendable, Equatable {
    public let x: Data
    public let y: Data

    private enum CodingKeys: String, CodingKey, CaseIterable { case x, y }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP BN254 G1 point"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        x = try governanceBn254Coordinate(
            container.decode([UInt8].self, forKey: .x),
            codingPath: container.codingPath + [CodingKeys.x],
            field: "x"
        )
        y = try governanceBn254Coordinate(
            container.decode([UInt8].self, forKey: .y),
            codingPath: container.codingPath + [CodingKeys.y],
            field: "y"
        )
        guard x.contains(where: { $0 != 0 }) || y.contains(where: { $0 != 0 }) else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "SCCP BN254 G1 point must not encode infinity"
                )
            )
        }
    }

    fileprivate var canonicalBytes: Data { x + y }
}

/// Canonical non-infinity BN254 G2 point in verifier limb order.
public struct ToriiGovernanceSccpBn254G2Point: Decodable, Sendable, Equatable {
    public let xC0: Data
    public let xC1: Data
    public let yC0: Data
    public let yC1: Data

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case xC0 = "x_c0"
        case xC1 = "x_c1"
        case yC0 = "y_c0"
        case yC1 = "y_c1"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP BN254 G2 point"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        func coordinate(_ key: CodingKeys) throws -> Data {
            try governanceBn254Coordinate(
                container.decode([UInt8].self, forKey: key),
                codingPath: container.codingPath + [key],
                field: key.stringValue
            )
        }
        xC0 = try coordinate(.xC0)
        xC1 = try coordinate(.xC1)
        yC0 = try coordinate(.yC0)
        yC1 = try coordinate(.yC1)
        guard [xC0, xC1, yC0, yC1].contains(where: { $0.contains(where: { $0 != 0 }) }) else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "SCCP BN254 G2 point must not encode infinity"
                )
            )
        }
    }

    fileprivate var canonicalBytes: Data { xC0 + xC1 + yC0 + yC1 }
}

/// Fixed Groth16 IC vector: one constant point and exactly eleven signal points.
public struct ToriiGovernanceSccpGroth16Ic: Decodable, Sendable, Equatable {
    public let constant: ToriiGovernanceSccpBn254G1Point
    public let signal0: ToriiGovernanceSccpBn254G1Point
    public let signal1: ToriiGovernanceSccpBn254G1Point
    public let signal2: ToriiGovernanceSccpBn254G1Point
    public let signal3: ToriiGovernanceSccpBn254G1Point
    public let signal4: ToriiGovernanceSccpBn254G1Point
    public let signal5: ToriiGovernanceSccpBn254G1Point
    public let signal6: ToriiGovernanceSccpBn254G1Point
    public let signal7: ToriiGovernanceSccpBn254G1Point
    public let signal8: ToriiGovernanceSccpBn254G1Point
    public let signal9: ToriiGovernanceSccpBn254G1Point
    public let signal10: ToriiGovernanceSccpBn254G1Point

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case constant
        case signal0 = "signal_0"
        case signal1 = "signal_1"
        case signal2 = "signal_2"
        case signal3 = "signal_3"
        case signal4 = "signal_4"
        case signal5 = "signal_5"
        case signal6 = "signal_6"
        case signal7 = "signal_7"
        case signal8 = "signal_8"
        case signal9 = "signal_9"
        case signal10 = "signal_10"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP Groth16 IC vector"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        constant = try container.decode(ToriiGovernanceSccpBn254G1Point.self, forKey: .constant)
        signal0 = try container.decode(ToriiGovernanceSccpBn254G1Point.self, forKey: .signal0)
        signal1 = try container.decode(ToriiGovernanceSccpBn254G1Point.self, forKey: .signal1)
        signal2 = try container.decode(ToriiGovernanceSccpBn254G1Point.self, forKey: .signal2)
        signal3 = try container.decode(ToriiGovernanceSccpBn254G1Point.self, forKey: .signal3)
        signal4 = try container.decode(ToriiGovernanceSccpBn254G1Point.self, forKey: .signal4)
        signal5 = try container.decode(ToriiGovernanceSccpBn254G1Point.self, forKey: .signal5)
        signal6 = try container.decode(ToriiGovernanceSccpBn254G1Point.self, forKey: .signal6)
        signal7 = try container.decode(ToriiGovernanceSccpBn254G1Point.self, forKey: .signal7)
        signal8 = try container.decode(ToriiGovernanceSccpBn254G1Point.self, forKey: .signal8)
        signal9 = try container.decode(ToriiGovernanceSccpBn254G1Point.self, forKey: .signal9)
        signal10 = try container.decode(ToriiGovernanceSccpBn254G1Point.self, forKey: .signal10)
    }

    fileprivate var ordered: [ToriiGovernanceSccpBn254G1Point] {
        [constant, signal0, signal1, signal2, signal3, signal4, signal5, signal6,
         signal7, signal8, signal9, signal10]
    }
}

/// Closed BN254 Groth16 verification key used by SCCP V1 destinations.
public struct ToriiGovernanceSccpGroth16VerifyingKey: Decodable, Sendable, Equatable {
    public let version: UInt8
    public let alpha1: ToriiGovernanceSccpBn254G1Point
    public let beta2: ToriiGovernanceSccpBn254G2Point
    public let gamma2: ToriiGovernanceSccpBn254G2Point
    public let delta2: ToriiGovernanceSccpBn254G2Point
    public let ic: ToriiGovernanceSccpGroth16Ic

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case version, alpha1, beta2, gamma2, delta2, ic
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP Groth16 verifying key"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        version = try container.decode(UInt8.self, forKey: .version)
        alpha1 = try container.decode(ToriiGovernanceSccpBn254G1Point.self, forKey: .alpha1)
        beta2 = try container.decode(ToriiGovernanceSccpBn254G2Point.self, forKey: .beta2)
        gamma2 = try container.decode(ToriiGovernanceSccpBn254G2Point.self, forKey: .gamma2)
        delta2 = try container.decode(ToriiGovernanceSccpBn254G2Point.self, forKey: .delta2)
        ic = try container.decode(ToriiGovernanceSccpGroth16Ic.self, forKey: .ic)
        guard version == 1 else {
            throw DecodingError.dataCorruptedError(
                forKey: .version,
                in: container,
                debugDescription: "SCCP V1 verifying-key version must be exactly 1"
            )
        }
    }

    fileprivate var canonicalBytes: Data {
        alpha1.canonicalBytes
            + beta2.canonicalBytes
            + gamma2.canonicalBytes
            + delta2.canonicalBytes
            + ic.ordered.reduce(into: Data()) { $0.append($1.canonicalBytes) }
    }
}

private let governanceBls12381BaseField = Data(hexString:
    "1A0111EA397FE69A4B1BA7B6434BACD764774B84F38512BF6730D2A0F6B0F6241EABFFFEB153FFFFB9FEFFFFFFFFAAAB"
)!

private func governanceBls12381Point(
    _ bytes: [UInt8],
    count: Int,
    codingPath: [CodingKey],
    field: String
) throws -> Data {
    let value = Data(bytes)
    func validG1(_ point: Data) -> Bool {
        guard point.count == 48, point[0] & 0x80 != 0, point[0] & 0x40 == 0 else { return false }
        var x = point
        x[0] &= 0x1f
        return x.lexicographicallyPrecedes(governanceBls12381BaseField)
    }
    let valid = count == 48
        ? validG1(value)
        : count == 96
            && validG1(Data(value.prefix(48)))
            && Data(value.suffix(48)).lexicographicallyPrecedes(governanceBls12381BaseField)
    guard valid else {
        throw DecodingError.dataCorrupted(
            .init(codingPath: codingPath, debugDescription: "\(field) is not a canonical compressed BLS12-381 point")
        )
    }
    return value
}

/// Fixed BLS12-381 Groth16 IC vector with one constant and eleven signal points.
public struct ToriiGovernanceSccpBls12381Ic: Decodable, Sendable, Equatable {
    public let constant: Data
    public let signal0: Data
    public let signal1: Data
    public let signal2: Data
    public let signal3: Data
    public let signal4: Data
    public let signal5: Data
    public let signal6: Data
    public let signal7: Data
    public let signal8: Data
    public let signal9: Data
    public let signal10: Data

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case constant
        case signal0 = "signal_0"
        case signal1 = "signal_1"
        case signal2 = "signal_2"
        case signal3 = "signal_3"
        case signal4 = "signal_4"
        case signal5 = "signal_5"
        case signal6 = "signal_6"
        case signal7 = "signal_7"
        case signal8 = "signal_8"
        case signal9 = "signal_9"
        case signal10 = "signal_10"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP BLS12-381 IC vector"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        func point(_ key: CodingKeys) throws -> Data {
            try governanceBls12381Point(
                container.decode([UInt8].self, forKey: key),
                count: 48,
                codingPath: container.codingPath + [key],
                field: key.stringValue
            )
        }
        constant = try point(.constant)
        signal0 = try point(.signal0)
        signal1 = try point(.signal1)
        signal2 = try point(.signal2)
        signal3 = try point(.signal3)
        signal4 = try point(.signal4)
        signal5 = try point(.signal5)
        signal6 = try point(.signal6)
        signal7 = try point(.signal7)
        signal8 = try point(.signal8)
        signal9 = try point(.signal9)
        signal10 = try point(.signal10)
    }

    fileprivate var ordered: [Data] {
        [constant, signal0, signal1, signal2, signal3, signal4, signal5, signal6,
         signal7, signal8, signal9, signal10]
    }
}

/// Closed compressed BLS12-381 Groth16 verification key used by TON SCCP V1.
public struct ToriiGovernanceSccpGroth16Bls12381VerifyingKey: Decodable, Sendable, Equatable {
    public let version: UInt8
    public let alpha1: Data
    public let beta2: Data
    public let gamma2: Data
    public let delta2: Data
    public let ic: ToriiGovernanceSccpBls12381Ic

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case version, alpha1, beta2, gamma2, delta2, ic
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP BLS12-381 Groth16 verifying key"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        version = try container.decode(UInt8.self, forKey: .version)
        alpha1 = try governanceBls12381Point(
            container.decode([UInt8].self, forKey: .alpha1), count: 48,
            codingPath: container.codingPath + [CodingKeys.alpha1], field: "alpha1"
        )
        func g2(_ key: CodingKeys) throws -> Data {
            try governanceBls12381Point(
                container.decode([UInt8].self, forKey: key), count: 96,
                codingPath: container.codingPath + [key], field: key.stringValue
            )
        }
        beta2 = try g2(.beta2)
        gamma2 = try g2(.gamma2)
        delta2 = try g2(.delta2)
        ic = try container.decode(ToriiGovernanceSccpBls12381Ic.self, forKey: .ic)
        guard version == 1 else {
            throw DecodingError.dataCorruptedError(
                forKey: .version, in: container,
                debugDescription: "SCCP BLS12-381 verifying-key version must be exactly 1"
            )
        }
    }

    fileprivate var canonicalBytes: Data {
        Data([version]) + alpha1 + beta2 + gamma2 + delta2
            + ic.ordered.reduce(into: Data()) { $0.append($1) }
    }
}

private let governanceBn254PublicSignalLabels = [
    "sccp:groth16-bn254:signal:message-id:v1",
    "sccp:groth16-bn254:signal:payload-hash:v1",
    "sccp:groth16-bn254:signal:target-domain:v1",
    "sccp:groth16-bn254:signal:commitment-root:v1",
    "sccp:groth16-bn254:signal:finality-height:v1",
    "sccp:groth16-bn254:signal:finality-block-hash:v1",
    "sccp:groth16-bn254:signal:source-domain:v1",
    "sccp:groth16-bn254:signal:statement-hash:v1",
    "sccp:groth16-bn254:signal:destination-binding-hash:v1",
    "sccp:groth16-bn254:signal:route-configuration-hash:v1",
    "sccp:groth16-bn254:signal:sora-finality-anchor-hash:v1",
]

private let governanceBls12381PublicSignalLabels = [
    "sccp:groth16-bls12381:signal:message-id:v1",
    "sccp:groth16-bls12381:signal:payload-hash:v1",
    "sccp:groth16-bls12381:signal:target-domain:v1",
    "sccp:groth16-bls12381:signal:commitment-root:v1",
    "sccp:groth16-bls12381:signal:finality-height:v1",
    "sccp:groth16-bls12381:signal:finality-block-hash:v1",
    "sccp:groth16-bls12381:signal:source-domain:v1",
    "sccp:groth16-bls12381:signal:statement-hash:v1",
    "sccp:groth16-bls12381:signal:destination-binding-hash:v1",
    "sccp:groth16-bls12381:signal:route-config-hash:v1",
    "sccp:groth16-bls12381:signal:sora-finality-anchor-hash:v1",
]

private let governanceTairaChainId = Data([
    0xfc, 0x56, 0x98, 0x4b, 0x2b, 0xe7, 0x43, 0x1d,
    0x84, 0x0e, 0x21, 0x51, 0x4d, 0x18, 0x83, 0xf0,
])

private var governanceTairaChainIdHash: Data { irohaKeccak256(governanceTairaChainId) }

private func governanceAppendUInt16LE(_ value: UInt16, to output: inout Data) {
    var little = value.littleEndian
    withUnsafeBytes(of: &little) { output.append(contentsOf: $0) }
}

private func governanceAppendUInt32LE(_ value: UInt32, to output: inout Data) {
    var little = value.littleEndian
    withUnsafeBytes(of: &little) { output.append(contentsOf: $0) }
}

private func governanceAppendUInt64LE(_ value: UInt64, to output: inout Data) {
    var little = value.littleEndian
    withUnsafeBytes(of: &little) { output.append(contentsOf: $0) }
}

private func governancePublicSignalSchemaHash(
    labels: [String],
    domain: String,
    sha256: Bool
) -> Data {
    var canonical = Data([1])
    governanceAppendUInt32LE(UInt32(labels.count), to: &canonical)
    for label in labels {
        let bytes = Data(label.utf8)
        governanceAppendUInt32LE(UInt32(bytes.count), to: &canonical)
        canonical.append(bytes)
    }
    let preimage = Data(domain.utf8) + canonical
    return sha256 ? Data(SHA256.hash(data: preimage)) : irohaKeccak256(preimage)
}

private var governanceBn254PublicSignalSchemaHash: Data {
    governancePublicSignalSchemaHash(
        labels: governanceBn254PublicSignalLabels,
        domain: "sccp:groth16-bn254:public-signal-schema:v1",
        sha256: false
    )
}

private var governanceBls12381PublicSignalSchemaHash: Data {
    governancePublicSignalSchemaHash(
        labels: governanceBls12381PublicSignalLabels,
        domain: "sccp:groth16-bls12381:public-signal-schema:v1",
        sha256: true
    )
}

/// Commitments identifying the one audited SCCP V1 semantic circuit.
public struct ToriiGovernanceSccpSemanticCircuit: Decodable, Sendable, Equatable {
    public let version: UInt8
    public let circuitCommitment: Data
    public let witnessGeneratorCommitment: Data
    public let publicSignalSchemaHash: Data

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case version
        case circuitCommitment = "circuit_commitment"
        case witnessGeneratorCommitment = "witness_generator_commitment"
        case publicSignalSchemaHash = "public_signal_schema_hash"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP semantic circuit"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        version = try container.decode(UInt8.self, forKey: .version)
        func hash(_ key: CodingKeys) throws -> Data {
            try governanceFixedBytes(
                container.decode([UInt8].self, forKey: key),
                count: 32,
                nonzero: true,
                codingPath: container.codingPath + [key],
                field: key.stringValue
            )
        }
        circuitCommitment = try hash(.circuitCommitment)
        witnessGeneratorCommitment = try hash(.witnessGeneratorCommitment)
        publicSignalSchemaHash = try hash(.publicSignalSchemaHash)
        guard version == 1,
              Set([circuitCommitment, witnessGeneratorCommitment, publicSignalSchemaHash]).count == 3 else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "invalid or role-reused SCCP semantic-circuit commitments"
                )
            )
        }
    }
}

/// Closed semantic-proof profile accepted by SCCP V1.
public enum ToriiGovernanceSccpSemanticProofProfile: Decodable, Sendable, Equatable {
    case soraTairaFinalityInclusionGroth16Bn254(ToriiGovernanceSccpSemanticCircuit)
    case soraTairaFinalityInclusionGroth16Bls12381(ToriiGovernanceSccpSemanticCircuit)

    private enum CodingKeys: String, CodingKey, CaseIterable { case profile, commitments }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP semantic-proof profile"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .profile) {
        case "sora_taira_finality_inclusion_groth16_bn254":
            let circuit = try container.decode(
                ToriiGovernanceSccpSemanticCircuit.self,
                forKey: .commitments
            )
            guard circuit.publicSignalSchemaHash == governanceBn254PublicSignalSchemaHash else {
                throw DecodingError.dataCorruptedError(
                    forKey: .commitments,
                    in: container,
                    debugDescription: "BN254 proof profile must commit the exact SCCP V1 signal schema"
                )
            }
            self = .soraTairaFinalityInclusionGroth16Bn254(circuit)
        case "sora_taira_finality_inclusion_groth16_bls12381":
            let circuit = try container.decode(
                ToriiGovernanceSccpSemanticCircuit.self,
                forKey: .commitments
            )
            guard circuit.publicSignalSchemaHash == governanceBls12381PublicSignalSchemaHash else {
                throw DecodingError.dataCorruptedError(
                    forKey: .commitments,
                    in: container,
                    debugDescription: "BLS12-381 proof profile must commit the exact SCCP V1 signal schema"
                )
            }
            self = .soraTairaFinalityInclusionGroth16Bls12381(circuit)
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .profile,
                in: container,
                debugDescription: "unsupported SCCP semantic-proof profile"
            )
        }
    }

    fileprivate var commitments: [Data] {
        switch self {
        case let .soraTairaFinalityInclusionGroth16Bn254(circuit),
             let .soraTairaFinalityInclusionGroth16Bls12381(circuit):
            return [
                circuit.circuitCommitment,
                circuit.witnessGeneratorCommitment,
                circuit.publicSignalSchemaHash,
            ]
        }
    }

    fileprivate var isBls12381: Bool {
        if case .soraTairaFinalityInclusionGroth16Bls12381 = self { return true }
        return false
    }

    fileprivate var isBn254: Bool {
        if case .soraTairaFinalityInclusionGroth16Bn254 = self { return true }
        return false
    }

    fileprivate var discoveryValue: SccpSemanticProofProfileV1 {
        let circuit: ToriiGovernanceSccpSemanticCircuit
        let kind: SccpSemanticProofProfileKindV1
        let curveTag: UInt8
        switch self {
        case let .soraTairaFinalityInclusionGroth16Bn254(value):
            circuit = value
            kind = .groth16Bn254
            curveTag = 0
        case let .soraTairaFinalityInclusionGroth16Bls12381(value):
            circuit = value
            kind = .groth16Bls12381
            curveTag = 1
        }
        let canonical = Data([1, curveTag, 1])
            + circuit.circuitCommitment
            + circuit.witnessGeneratorCommitment
            + circuit.publicSignalSchemaHash
        return SccpSemanticProofProfileV1(
            kind: kind,
            circuitCommitment: circuit.circuitCommitment,
            witnessGeneratorCommitment: circuit.witnessGeneratorCommitment,
            publicSignalSchemaHash: circuit.publicSignalSchemaHash,
            profileHash: irohaKeccak256(
                Data("sccp:semantic-proof-profile:v1".utf8) + canonical
            )
        )
    }
}

/// Immutable Taira checkpoint anchoring an SCCP outbound proof policy.
public struct ToriiGovernanceSccpSoraFinalityAnchor: Decodable, Sendable, Equatable {
    public let version: UInt8
    public let sourceNetwork: ToriiGovernanceSccpNetwork
    public let protocolVersion: UInt16
    public let chainIdHash: Data
    public let checkpointHeight: UInt64
    public let checkpointBlockHash: Data
    public let checkpointContextId: Data
    public let checkpointFinalityArtifactHash: Data

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case version
        case sourceNetwork = "source_network"
        case protocolVersion = "protocol_version"
        case chainIdHash = "chain_id_hash"
        case checkpointHeight = "checkpoint_height"
        case checkpointBlockHash = "checkpoint_block_hash"
        case checkpointContextId = "checkpoint_context_id"
        case checkpointFinalityArtifactHash = "checkpoint_finality_artifact_hash"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP SORA finality anchor"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        version = try container.decode(UInt8.self, forKey: .version)
        sourceNetwork = try container.decode(ToriiGovernanceSccpNetwork.self, forKey: .sourceNetwork)
        protocolVersion = try container.decode(UInt16.self, forKey: .protocolVersion)
        checkpointHeight = try container.decode(UInt64.self, forKey: .checkpointHeight)
        func hash(_ key: CodingKeys) throws -> Data {
            try governanceFixedBytes(
                container.decode([UInt8].self, forKey: key),
                count: 32,
                nonzero: true,
                codingPath: container.codingPath + [key],
                field: key.stringValue
            )
        }
        chainIdHash = try hash(.chainIdHash)
        checkpointBlockHash = try hash(.checkpointBlockHash)
        checkpointContextId = try hash(.checkpointContextId)
        checkpointFinalityArtifactHash = try hash(.checkpointFinalityArtifactHash)
        guard version == 1,
              sourceNetwork == .soraTaira,
              protocolVersion == 4,
              chainIdHash == governanceTairaChainIdHash,
              checkpointHeight > 0,
              Set([
                  chainIdHash,
                  checkpointBlockHash,
                  checkpointContextId,
                  checkpointFinalityArtifactHash,
              ]).count == 4 else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "invalid or role-reused SCCP SORA finality anchor"
                )
            )
        }
    }

    fileprivate var discoveryValue: SccpSoraFinalityAnchorV1 {
        var canonical = Data([1, SccpNetworkV1.soraTaira.tag])
        governanceAppendUInt16LE(protocolVersion, to: &canonical)
        canonical.append(chainIdHash)
        governanceAppendUInt64LE(checkpointHeight, to: &canonical)
        canonical.append(checkpointBlockHash)
        canonical.append(checkpointContextId)
        canonical.append(checkpointFinalityArtifactHash)
        return SccpSoraFinalityAnchorV1(
            protocolVersion: protocolVersion,
            chainIdHash: chainIdHash,
            checkpointHeight: checkpointHeight,
            checkpointBlockHash: checkpointBlockHash,
            checkpointContextId: checkpointContextId,
            checkpointFinalityArtifactHash: checkpointFinalityArtifactHash,
            anchorHash: irohaKeccak256(
                Data("sccp:sora-finality-anchor:v1".utf8) + canonical
            )
        )
    }
}

/// Mandatory immutable proof policy of one value-moving destination deployment.
public struct ToriiGovernanceSccpOutboundProofPolicy: Decodable, Sendable, Equatable {
    public let version: UInt8
    public let semanticProfile: ToriiGovernanceSccpSemanticProofProfile
    public let soraFinalityAnchor: ToriiGovernanceSccpSoraFinalityAnchor

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case version
        case semanticProfile = "semantic_profile"
        case soraFinalityAnchor = "sora_finality_anchor"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP outbound proof policy"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        version = try container.decode(UInt8.self, forKey: .version)
        semanticProfile = try container.decode(
            ToriiGovernanceSccpSemanticProofProfile.self,
            forKey: .semanticProfile
        )
        soraFinalityAnchor = try container.decode(
            ToriiGovernanceSccpSoraFinalityAnchor.self,
            forKey: .soraFinalityAnchor
        )
        let semantic = semanticProfile.discoveryValue
        let anchor = soraFinalityAnchor.discoveryValue
        let hashRoles = [
            semantic.circuitCommitment,
            semantic.witnessGeneratorCommitment,
            semantic.publicSignalSchemaHash,
            semantic.profileHash,
            anchor.chainIdHash,
            anchor.checkpointBlockHash,
            anchor.checkpointContextId,
            anchor.checkpointFinalityArtifactHash,
            anchor.anchorHash,
        ]
        guard version == 1, Set(hashRoles).count == hashRoles.count else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "SCCP outbound-proof policy is invalid or reuses a hash role"
                )
            )
        }
    }

    var discoveryValue: SccpOutboundProofPolicyV1 {
        SccpOutboundProofPolicyV1(
            version: version,
            semanticProfile: semanticProfile.discoveryValue,
            soraFinalityAnchor: soraFinalityAnchor.discoveryValue
        )
    }
}

/// Exact EVM verifier, route, and ERC-20 destination deployment.
public struct ToriiGovernanceSccpEvmDestinationDeployment: Decodable, Sendable, Equatable {
    public let tokenAddress: Data
    public let tokenCodeHash: Data
    public let verifierAddress: Data
    public let verifierCodeHash: Data
    public let verifyingKey: ToriiGovernanceSccpGroth16VerifyingKey
    public let verifierKeyHash: Data
    public let outboundProofPolicy: ToriiGovernanceSccpOutboundProofPolicy
    public let routeAddress: Data
    public let routeCodeHash: Data
    public let replayVerifierAddress: Data
    public let replayVerifierCodeHash: Data
    public let mintBreakerAddress: Data
    public let mintBreakerCodeHash: Data
    public let tairaToTokenMultiplier: UInt64
    /// Exact canonical positive UInt128 JSON integer.
    public let maxWrappedSupply: String

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case tokenAddress = "token_address"
        case tokenCodeHash = "token_code_hash"
        case verifierAddress = "verifier_address"
        case verifierCodeHash = "verifier_code_hash"
        case verifyingKey = "verifying_key"
        case verifierKeyHash = "verifier_key_hash"
        case outboundProofPolicy = "outbound_proof_policy"
        case routeAddress = "route_address"
        case routeCodeHash = "route_code_hash"
        case replayVerifierAddress = "replay_verifier_address"
        case replayVerifierCodeHash = "replay_verifier_code_hash"
        case mintBreakerAddress = "mint_breaker_address"
        case mintBreakerCodeHash = "mint_breaker_code_hash"
        case tairaToTokenMultiplier = "taira_to_token_multiplier"
        case maxWrappedSupply = "max_wrapped_supply"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP EVM destination deployment"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        func bytes(_ key: CodingKeys, count: Int) throws -> Data {
            try governanceFixedBytes(
                container.decode([UInt8].self, forKey: key),
                count: count,
                nonzero: true,
                codingPath: container.codingPath + [key],
                field: key.stringValue
            )
        }
        tokenAddress = try bytes(.tokenAddress, count: 20)
        tokenCodeHash = try bytes(.tokenCodeHash, count: 32)
        verifierAddress = try bytes(.verifierAddress, count: 20)
        verifierCodeHash = try bytes(.verifierCodeHash, count: 32)
        verifyingKey = try container.decode(ToriiGovernanceSccpGroth16VerifyingKey.self, forKey: .verifyingKey)
        verifierKeyHash = try bytes(.verifierKeyHash, count: 32)
        outboundProofPolicy = try container.decode(
            ToriiGovernanceSccpOutboundProofPolicy.self,
            forKey: .outboundProofPolicy
        )
        routeAddress = try bytes(.routeAddress, count: 20)
        routeCodeHash = try bytes(.routeCodeHash, count: 32)
        replayVerifierAddress = try bytes(.replayVerifierAddress, count: 20)
        replayVerifierCodeHash = try bytes(.replayVerifierCodeHash, count: 32)
        mintBreakerAddress = try bytes(.mintBreakerAddress, count: 20)
        mintBreakerCodeHash = try bytes(.mintBreakerCodeHash, count: 32)
        tairaToTokenMultiplier = try container.decode(UInt64.self, forKey: .tairaToTokenMultiplier)
        maxWrappedSupply = try governanceExactPositiveInteger(
            decoder: decoder,
            container: container,
            key: .maxWrappedSupply,
            maximum: maximumUInt128
        )
        let proofPolicy = outboundProofPolicy.discoveryValue
        let deploymentHashRoles = [
            tokenCodeHash, verifierCodeHash, verifierKeyHash, routeCodeHash,
            replayVerifierCodeHash, mintBreakerCodeHash,
            proofPolicy.semanticProfile.profileHash,
            proofPolicy.soraFinalityAnchor.anchorHash,
        ]
        guard tairaToTokenMultiplier == 1_000_000_000,
              outboundProofPolicy.semanticProfile.isBn254,
              irohaKeccak256(verifyingKey.canonicalBytes) == verifierKeyHash,
              Set([tokenAddress, verifierAddress, routeAddress, replayVerifierAddress, mintBreakerAddress]).count == 5,
              Set(deploymentHashRoles).count == deploymentHashRoles.count,
              ![tokenCodeHash, verifierCodeHash, routeCodeHash, replayVerifierCodeHash, mintBreakerCodeHash]
                  .contains(keccak256EmptyBytes) else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "invalid or role-reused SCCP EVM destination deployment"
                )
            )
        }
    }
}

/// Exact TRON verifier, route, and TRC-20 destination deployment.
public struct ToriiGovernanceSccpTronDestinationDeployment: Decodable, Sendable, Equatable {
    public let tokenAddress: Data
    public let tokenCodeHash: Data
    public let verifierAddress: Data
    public let verifierCodeHash: Data
    public let verifyingKey: ToriiGovernanceSccpGroth16VerifyingKey
    public let verifierKeyHash: Data
    public let outboundProofPolicy: ToriiGovernanceSccpOutboundProofPolicy
    public let routeAddress: Data
    public let routeCodeHash: Data
    public let replayVerifierAddress: Data
    public let replayVerifierCodeHash: Data
    public let mintBreakerAddress: Data
    public let mintBreakerCodeHash: Data
    public let tairaToTokenMultiplier: UInt64
    /// Exact canonical positive UInt128 JSON integer.
    public let maxWrappedSupply: String

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case tokenAddress = "token_address"
        case tokenCodeHash = "token_code_hash"
        case verifierAddress = "verifier_address"
        case verifierCodeHash = "verifier_code_hash"
        case verifyingKey = "verifying_key"
        case verifierKeyHash = "verifier_key_hash"
        case outboundProofPolicy = "outbound_proof_policy"
        case routeAddress = "route_address"
        case routeCodeHash = "route_code_hash"
        case replayVerifierAddress = "replay_verifier_address"
        case replayVerifierCodeHash = "replay_verifier_code_hash"
        case mintBreakerAddress = "mint_breaker_address"
        case mintBreakerCodeHash = "mint_breaker_code_hash"
        case tairaToTokenMultiplier = "taira_to_token_multiplier"
        case maxWrappedSupply = "max_wrapped_supply"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP TRON destination deployment"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        func bytes(_ key: CodingKeys, count: Int) throws -> Data {
            try governanceFixedBytes(
                container.decode([UInt8].self, forKey: key),
                count: count,
                nonzero: true,
                codingPath: container.codingPath + [key],
                field: key.stringValue
            )
        }
        tokenAddress = try bytes(.tokenAddress, count: 20)
        tokenCodeHash = try bytes(.tokenCodeHash, count: 32)
        verifierAddress = try bytes(.verifierAddress, count: 20)
        verifierCodeHash = try bytes(.verifierCodeHash, count: 32)
        verifyingKey = try container.decode(ToriiGovernanceSccpGroth16VerifyingKey.self, forKey: .verifyingKey)
        verifierKeyHash = try bytes(.verifierKeyHash, count: 32)
        outboundProofPolicy = try container.decode(
            ToriiGovernanceSccpOutboundProofPolicy.self,
            forKey: .outboundProofPolicy
        )
        routeAddress = try bytes(.routeAddress, count: 20)
        routeCodeHash = try bytes(.routeCodeHash, count: 32)
        replayVerifierAddress = try bytes(.replayVerifierAddress, count: 20)
        replayVerifierCodeHash = try bytes(.replayVerifierCodeHash, count: 32)
        mintBreakerAddress = try bytes(.mintBreakerAddress, count: 20)
        mintBreakerCodeHash = try bytes(.mintBreakerCodeHash, count: 32)
        tairaToTokenMultiplier = try container.decode(UInt64.self, forKey: .tairaToTokenMultiplier)
        maxWrappedSupply = try governanceExactPositiveInteger(
            decoder: decoder,
            container: container,
            key: .maxWrappedSupply,
            maximum: maximumUInt128
        )
        let proofPolicy = outboundProofPolicy.discoveryValue
        let deploymentHashRoles = [
            tokenCodeHash, verifierCodeHash, verifierKeyHash, routeCodeHash,
            replayVerifierCodeHash, mintBreakerCodeHash,
            proofPolicy.semanticProfile.profileHash,
            proofPolicy.soraFinalityAnchor.anchorHash,
        ]
        guard tairaToTokenMultiplier == 1_000_000_000,
              outboundProofPolicy.semanticProfile.isBn254,
              irohaKeccak256(verifyingKey.canonicalBytes) == verifierKeyHash,
              Set([tokenAddress, verifierAddress, routeAddress, replayVerifierAddress, mintBreakerAddress]).count == 5,
              Set(deploymentHashRoles).count == deploymentHashRoles.count,
              ![tokenCodeHash, verifierCodeHash, routeCodeHash, replayVerifierCodeHash, mintBreakerCodeHash]
                  .contains(keccak256EmptyBytes) else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "invalid or role-reused SCCP TRON destination deployment"
                )
            )
        }
    }
}

/// Exact ordered five-key TON mint-breaker guardian set in governance payloads.
public struct ToriiGovernanceSccpTonMintBreakerGuardianKeys: Decodable, Sendable, Equatable {
    public let guardian0: Data
    public let guardian1: Data
    public let guardian2: Data
    public let guardian3: Data
    public let guardian4: Data

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case guardian0 = "guardian_0"
        case guardian1 = "guardian_1"
        case guardian2 = "guardian_2"
        case guardian3 = "guardian_3"
        case guardian4 = "guardian_4"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP TON mint-breaker guardian keys"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        func key(_ codingKey: CodingKeys) throws -> Data {
            try governanceFixedBytes(
                container.decode([UInt8].self, forKey: codingKey),
                count: 32,
                nonzero: true,
                codingPath: container.codingPath + [codingKey],
                field: codingKey.stringValue
            )
        }
        guardian0 = try key(.guardian0)
        guardian1 = try key(.guardian1)
        guardian2 = try key(.guardian2)
        guardian3 = try key(.guardian3)
        guardian4 = try key(.guardian4)
        let keys = ordered
        guard zip(keys, keys.dropFirst()).allSatisfy({ $0.lexicographicallyPrecedes($1) }) else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "SCCP TON guardian keys must be strictly increasing"
                )
            )
        }
    }

    fileprivate var ordered: [Data] { [guardian0, guardian1, guardian2, guardian3, guardian4] }
}

private func governanceTonProofProfileCommitment() -> Data {
    let labels = [
        "sccp:groth16-bls12381:signal:message-id:v1",
        "sccp:groth16-bls12381:signal:payload-hash:v1",
        "sccp:groth16-bls12381:signal:target-domain:v1",
        "sccp:groth16-bls12381:signal:commitment-root:v1",
        "sccp:groth16-bls12381:signal:finality-height:v1",
        "sccp:groth16-bls12381:signal:finality-block-hash:v1",
        "sccp:groth16-bls12381:signal:source-domain:v1",
        "sccp:groth16-bls12381:signal:statement-hash:v1",
        "sccp:groth16-bls12381:signal:destination-binding-hash:v1",
        "sccp:groth16-bls12381:signal:route-config-hash:v1",
        "sccp:groth16-bls12381:signal:sora-finality-anchor-hash:v1",
    ]
    func appendUInt32LE(_ value: UInt32, to data: inout Data) {
        var little = value.littleEndian
        withUnsafeBytes(of: &little) { data.append(contentsOf: $0) }
    }
    var schema = Data([1])
    appendUInt32LE(UInt32(labels.count), to: &schema)
    for label in labels {
        let bytes = Data(label.utf8)
        appendUInt32LE(UInt32(bytes.count), to: &schema)
        schema.append(bytes)
    }
    let schemaHash = Data(SHA256.hash(
        data: Data("sccp:groth16-bls12381:public-signal-schema:v1".utf8) + schema
    ))
    var profile = Data("sccp:ton:groth16-bls12381:proof-profile:v1".utf8)
    profile.append(1)
    profile.append(Data("ietf-bls12381-compressed-g1-48-g2-96".utf8))
    profile.append(Data("groth16-a-g1-b-g2-c-g1".utf8))
    profile.append(Data("sha256-sha256-label-value-mod-r".utf8))
    profile.append(Data(hexString: "73EDA753299D7D483339D80809A1D80553BDA402FFFE5BFEFFFFFFFF00000001")!)
    profile.append(schemaHash)
    return Data(SHA256.hash(data: profile))
}

/// Exact TON Jetton route and linked BLS12-381 verifier deployment.
public struct ToriiGovernanceSccpTonDestinationDeployment: Decodable, Sendable, Equatable {
    public let jettonMasterAddress: ToriiGovernanceSccpTonAddress
    public let jettonMasterCodeHash: Data
    public let jettonMasterInitialDataHash: Data
    public let jettonWalletCodeHash: Data
    public let routeAddress: ToriiGovernanceSccpTonAddress
    public let routeCodeHash: Data
    public let routeInitialDataHash: Data
    public let embeddedVerifierCodeHash: Data
    public let verifierCircuitHash: Data
    public let verifyingKey: ToriiGovernanceSccpGroth16Bls12381VerifyingKey
    public let verifierKeyHash: Data
    public let proofProfileCommitment: Data
    public let mintBreakerGuardianKeys: ToriiGovernanceSccpTonMintBreakerGuardianKeys
    public let outboundProofPolicy: ToriiGovernanceSccpOutboundProofPolicy
    public let tairaToTokenMultiplier: UInt64
    /// Exact canonical positive integer no greater than 2^120 - 1.
    public let maxWrappedSupply: String

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case jettonMasterAddress = "jetton_master_address"
        case jettonMasterCodeHash = "jetton_master_code_hash"
        case jettonMasterInitialDataHash = "jetton_master_initial_data_hash"
        case jettonWalletCodeHash = "jetton_wallet_code_hash"
        case routeAddress = "route_address"
        case routeCodeHash = "route_code_hash"
        case routeInitialDataHash = "route_initial_data_hash"
        case embeddedVerifierCodeHash = "embedded_verifier_code_hash"
        case verifierCircuitHash = "verifier_circuit_hash"
        case verifyingKey = "verifying_key"
        case verifierKeyHash = "verifier_key_hash"
        case proofProfileCommitment = "proof_profile_commitment"
        case mintBreakerGuardianKeys = "mint_breaker_guardian_keys"
        case outboundProofPolicy = "outbound_proof_policy"
        case tairaToTokenMultiplier = "taira_to_token_multiplier"
        case maxWrappedSupply = "max_wrapped_supply"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP TON destination deployment"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        func hash(_ key: CodingKeys) throws -> Data {
            try governanceFixedBytes(
                container.decode([UInt8].self, forKey: key),
                count: 32,
                nonzero: true,
                codingPath: container.codingPath + [key],
                field: key.stringValue
            )
        }
        jettonMasterAddress = try container.decode(
            ToriiGovernanceSccpTonAddress.self, forKey: .jettonMasterAddress
        )
        jettonMasterCodeHash = try hash(.jettonMasterCodeHash)
        jettonMasterInitialDataHash = try hash(.jettonMasterInitialDataHash)
        jettonWalletCodeHash = try hash(.jettonWalletCodeHash)
        routeAddress = try container.decode(ToriiGovernanceSccpTonAddress.self, forKey: .routeAddress)
        routeCodeHash = try hash(.routeCodeHash)
        routeInitialDataHash = try hash(.routeInitialDataHash)
        embeddedVerifierCodeHash = try hash(.embeddedVerifierCodeHash)
        verifierCircuitHash = try hash(.verifierCircuitHash)
        verifyingKey = try container.decode(
            ToriiGovernanceSccpGroth16Bls12381VerifyingKey.self, forKey: .verifyingKey
        )
        verifierKeyHash = try hash(.verifierKeyHash)
        proofProfileCommitment = try hash(.proofProfileCommitment)
        mintBreakerGuardianKeys = try container.decode(
            ToriiGovernanceSccpTonMintBreakerGuardianKeys.self,
            forKey: .mintBreakerGuardianKeys
        )
        outboundProofPolicy = try container.decode(
            ToriiGovernanceSccpOutboundProofPolicy.self, forKey: .outboundProofPolicy
        )
        tairaToTokenMultiplier = try container.decode(UInt64.self, forKey: .tairaToTokenMultiplier)
        maxWrappedSupply = try governanceExactPositiveInteger(
            decoder: decoder,
            container: container,
            key: .maxWrappedSupply,
            maximum: maximumTonCoins
        )
        let proofPolicy = outboundProofPolicy.discoveryValue
        let hashes = [
            jettonMasterCodeHash, jettonMasterInitialDataHash, jettonWalletCodeHash,
            routeCodeHash, routeInitialDataHash, embeddedVerifierCodeHash,
            verifierCircuitHash, verifierKeyHash, proofProfileCommitment,
        ] + Array(outboundProofPolicy.semanticProfile.commitments.dropFirst()) + [
            proofPolicy.semanticProfile.profileHash,
            proofPolicy.soraFinalityAnchor.anchorHash,
        ]
        guard jettonMasterAddress != routeAddress,
              Data(SHA256.hash(data: verifyingKey.canonicalBytes)) == verifierKeyHash,
              outboundProofPolicy.semanticProfile.isBls12381,
              verifierCircuitHash == outboundProofPolicy.semanticProfile.commitments[0],
              proofProfileCommitment == governanceTonProofProfileCommitment(),
              tairaToTokenMultiplier == 1,
              Set(hashes).count == hashes.count else {
            throw DecodingError.dataCorrupted(
                .init(codingPath: container.codingPath, debugDescription: "invalid SCCP TON destination deployment")
            )
        }
    }
}


/// Closed destination-deployment family in a governed SCCP route.
public enum ToriiGovernanceSccpDestination: Decodable, Sendable, Equatable {
    case evm(ToriiGovernanceSccpEvmDestinationDeployment)
    case tron(ToriiGovernanceSccpTronDestinationDeployment)
    case ton(ToriiGovernanceSccpTonDestinationDeployment)

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case family
        case deployment
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP destination deployment"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .family) {
        case "evm":
            self = .evm(
                try container.decode(
                    ToriiGovernanceSccpEvmDestinationDeployment.self,
                    forKey: .deployment
                )
            )
        case "tron":
            self = .tron(
                try container.decode(
                    ToriiGovernanceSccpTronDestinationDeployment.self,
                    forKey: .deployment
                )
            )
        case "ton":
            self = .ton(
                try container.decode(
                    ToriiGovernanceSccpTonDestinationDeployment.self,
                    forKey: .deployment
                )
            )
        case let tag:
            throw DecodingError.dataCorruptedError(
                forKey: .family,
                in: container,
                debugDescription: "unsupported SCCP destination family \(tag)"
            )
        }
    }

    fileprivate var family: String {
        switch self {
        case .evm: return "evm"
        case .tron: return "tron"
        case .ton: return "ton"
        }
    }

    fileprivate var tairaToTokenMultiplier: UInt64 {
        switch self {
        case let .evm(deployment): return deployment.tairaToTokenMultiplier
        case let .tron(deployment): return deployment.tairaToTokenMultiplier
        case let .ton(deployment): return deployment.tairaToTokenMultiplier
        }
    }

    fileprivate var maxWrappedSupply: String {
        switch self {
        case let .evm(deployment): return deployment.maxWrappedSupply
        case let .tron(deployment): return deployment.maxWrappedSupply
        case let .ton(deployment): return deployment.maxWrappedSupply
        }
    }

    func discoveryValue(for lane: SccpLaneIdV1) throws -> SccpDestinationDeploymentV1 {
        func contract(
            _ deployment: ToriiGovernanceSccpEvmDestinationDeployment,
            binding: Data
        ) throws -> SccpEvmTronDestinationDeploymentV1 {
            SccpEvmTronDestinationDeploymentV1(
                tokenAddress: deployment.tokenAddress,
                tokenCodeHash: deployment.tokenCodeHash,
                verifierAddress: deployment.verifierAddress,
                verifierCodeHash: deployment.verifierCodeHash,
                verifierKeyHash: deployment.verifierKeyHash,
                outboundProofPolicy: deployment.outboundProofPolicy.discoveryValue,
                routeAddress: deployment.routeAddress,
                routeCodeHash: deployment.routeCodeHash,
                replayVerifierAddress: deployment.replayVerifierAddress,
                replayVerifierCodeHash: deployment.replayVerifierCodeHash,
                mintBreakerAddress: deployment.mintBreakerAddress,
                mintBreakerCodeHash: deployment.mintBreakerCodeHash,
                tairaToTokenMultiplier: deployment.tairaToTokenMultiplier,
                maxWrappedSupply: deployment.maxWrappedSupply,
                destinationBindingHash: binding
            )
        }

        func contract(
            _ deployment: ToriiGovernanceSccpTronDestinationDeployment,
            binding: Data
        ) throws -> SccpEvmTronDestinationDeploymentV1 {
            SccpEvmTronDestinationDeploymentV1(
                tokenAddress: deployment.tokenAddress,
                tokenCodeHash: deployment.tokenCodeHash,
                verifierAddress: deployment.verifierAddress,
                verifierCodeHash: deployment.verifierCodeHash,
                verifierKeyHash: deployment.verifierKeyHash,
                outboundProofPolicy: deployment.outboundProofPolicy.discoveryValue,
                routeAddress: deployment.routeAddress,
                routeCodeHash: deployment.routeCodeHash,
                replayVerifierAddress: deployment.replayVerifierAddress,
                replayVerifierCodeHash: deployment.replayVerifierCodeHash,
                mintBreakerAddress: deployment.mintBreakerAddress,
                mintBreakerCodeHash: deployment.mintBreakerCodeHash,
                tairaToTokenMultiplier: deployment.tairaToTokenMultiplier,
                maxWrappedSupply: deployment.maxWrappedSupply,
                destinationBindingHash: binding
            )
        }

        switch self {
        case let .evm(deployment):
            let partial = try contract(deployment, binding: Data())
            let binding = try SccpExactParser.destinationBindingHash(
                lane: lane,
                destination: .evm(partial)
            )
            return .evm(try contract(deployment, binding: binding))
        case let .tron(deployment):
            let partial = try contract(deployment, binding: Data())
            let binding = try SccpExactParser.destinationBindingHash(
                lane: lane,
                destination: .tron(partial)
            )
            return .tron(try contract(deployment, binding: binding))
        case let .ton(deployment):
            let master = try SccpTonAddressV1(
                workchain: deployment.jettonMasterAddress.workchain,
                account: deployment.jettonMasterAddress.account
            )
            let route = try SccpTonAddressV1(
                workchain: deployment.routeAddress.workchain,
                account: deployment.routeAddress.account
            )
            let keys = deployment.mintBreakerGuardianKeys
            let guardians = try SccpTonMintBreakerGuardianKeysV1(
                guardian0: keys.guardian0,
                guardian1: keys.guardian1,
                guardian2: keys.guardian2,
                guardian3: keys.guardian3,
                guardian4: keys.guardian4
            )
            func ton(binding: Data) throws -> SccpTonDestinationDeploymentV1 {
                SccpTonDestinationDeploymentV1(
                    jettonMasterAddress: master,
                    jettonMasterCodeHash: deployment.jettonMasterCodeHash,
                    jettonMasterInitialDataHash: deployment.jettonMasterInitialDataHash,
                    jettonWalletCodeHash: deployment.jettonWalletCodeHash,
                    routeAddress: route,
                    routeCodeHash: deployment.routeCodeHash,
                    routeInitialDataHash: deployment.routeInitialDataHash,
                    embeddedVerifierCodeHash: deployment.embeddedVerifierCodeHash,
                    verifierCircuitHash: deployment.verifierCircuitHash,
                    verifierKeyHash: deployment.verifierKeyHash,
                    proofProfileCommitment: deployment.proofProfileCommitment,
                    mintBreakerGuardianKeys: guardians,
                    outboundProofPolicy: deployment.outboundProofPolicy.discoveryValue,
                    tairaToTokenMultiplier: deployment.tairaToTokenMultiplier,
                    maxWrappedSupply: deployment.maxWrappedSupply,
                    destinationBindingHash: binding
                )
            }
            let partial = try ton(binding: Data())
            let binding = try SccpExactParser.destinationBindingHash(
                lane: lane,
                destination: .ton(partial)
            )
            return .ton(try ton(binding: binding))
        }
    }
}

/// Governance-registered portable verification-key reference.
public struct ToriiGovernanceSccpVerifyingKeyReference: Decodable, Sendable, Equatable {
    public let backend: String
    public let name: String
    public let version: UInt32
    public let commitment: Data

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case backend
        case name
        case version
        case commitment
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP verification-key reference"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        backend = try container.decode(String.self, forKey: .backend)
        name = try container.decode(String.self, forKey: .name)
        version = try container.decode(UInt32.self, forKey: .version)
        commitment = try governanceFixedBytes(
            container.decode([UInt8].self, forKey: .commitment),
            count: 32,
            nonzero: true,
            codingPath: container.codingPath + [CodingKeys.commitment],
            field: "commitment"
        )
        guard governanceCanonicalKey(backend), governanceCanonicalKey(name), version > 0 else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "SCCP verification-key reference is not canonical"
                )
            )
        }
    }
}

/// Mandatory TAIRA-side execution policy for a governed SCCP route.
public struct ToriiGovernanceSccpOutboundExecutionPolicy: Decodable, Sendable, Equatable {
    public let version: UInt8
    public let semantics: String
    public let contractArtifactSha256: Data
    public let verifyingKeyReference: ToriiGovernanceSccpVerifyingKeyReference
    public let gasLimit: UInt64

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case version
        case semantics
        case contractArtifactSha256 = "contract_artifact_sha256"
        case verifyingKeyReference = "vk_ref"
        case gasLimit = "gas_limit"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP outbound execution policy"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        version = try container.decode(UInt8.self, forKey: .version)
        semantics = try container.decode(String.self, forKey: .semantics)
        contractArtifactSha256 = try governanceFixedBytes(
            container.decode([UInt8].self, forKey: .contractArtifactSha256),
            count: 32,
            nonzero: true,
            codingPath: container.codingPath + [CodingKeys.contractArtifactSha256],
            field: "contract_artifact_sha256"
        )
        verifyingKeyReference = try container.decode(
            ToriiGovernanceSccpVerifyingKeyReference.self,
            forKey: .verifyingKeyReference
        )
        gasLimit = try container.decode(UInt64.self, forKey: .gasLimit)
        guard version == 1,
              semantics == "ivm_proved_record_sccp_message_v1",
              gasLimit > 0,
              gasLimit <= 1_000_000_000 else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "unsupported SCCP outbound execution policy"
                )
            )
        }
    }
}

/// Typed TAIRA settlement policy for a governed SCCP route.
public struct ToriiGovernanceSccpSettlement: Decodable, Sendable, Equatable {
    public let assetDefinitionId: String
    public let payloadAmountScale: UInt32
    /// Exact canonical positive UInt128 JSON integer.
    public let maxOutstandingLiability: String

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case assetDefinitionId = "asset_definition_id"
        case payloadAmountScale = "payload_amount_scale"
        case maxOutstandingLiability = "max_outstanding_liability"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SCCP SORA settlement"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        assetDefinitionId = try governanceCanonicalAssetDefinition(
            container.decode(String.self, forKey: .assetDefinitionId),
            codingPath: container.codingPath + [CodingKeys.assetDefinitionId],
            field: "asset_definition_id"
        )
        payloadAmountScale = try container.decode(UInt32.self, forKey: .payloadAmountScale)
        maxOutstandingLiability = try governanceExactPositiveInteger(
            decoder: decoder,
            container: container,
            key: .maxOutstandingLiability,
            maximum: maximumUInt128
        )
        guard assetDefinitionId == "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
              payloadAmountScale == 9 else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "SCCP V1 settlement must use the canonical TAIRA XOR asset and scale"
                )
            )
        }
    }
}

/// Complete immutable SCCP route stored by a Register action.
public struct ToriiGovernanceSccpGovernedRoute: Decodable, Sendable, Equatable {
    public let laneId: ToriiGovernanceSccpLane
    public let routeId: String
    public let assetKey: String
    public let revision: UInt32
    public let activation: ToriiGovernanceSccpRouteActivation
    public let inboundFinalityCutoff: ToriiGovernanceSccpInboundFinalityCutoff?
    public let sourceIdentity: ToriiGovernanceSccpSourceIdentity
    public let destination: ToriiGovernanceSccpDestination
    public let soraOutboundExecutionPolicy: ToriiGovernanceSccpOutboundExecutionPolicy
    public let settlement: ToriiGovernanceSccpSettlement

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case laneId = "lane_id"
        case routeId = "route_id"
        case assetKey = "asset_key"
        case revision
        case activation
        case inboundFinalityCutoff = "inbound_finality_cutoff"
        case sourceIdentity = "source_identity"
        case destination
        case soraOutboundExecutionPolicy = "sora_outbound_execution_policy"
        case settlement
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "governed SCCP route"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        laneId = try container.decode(ToriiGovernanceSccpLane.self, forKey: .laneId)
        routeId = try container.decode(String.self, forKey: .routeId)
        assetKey = try container.decode(String.self, forKey: .assetKey)
        revision = try container.decode(UInt32.self, forKey: .revision)
        activation = try container.decode(
            ToriiGovernanceSccpRouteActivation.self,
            forKey: .activation
        )
        guard container.contains(.inboundFinalityCutoff) else {
            throw DecodingError.keyNotFound(
                CodingKeys.inboundFinalityCutoff,
                .init(codingPath: container.codingPath, debugDescription: "cutoff must be explicit")
            )
        }
        inboundFinalityCutoff = try container.decodeIfPresent(
            ToriiGovernanceSccpInboundFinalityCutoff.self,
            forKey: .inboundFinalityCutoff
        )
        sourceIdentity = try container.decode(
            ToriiGovernanceSccpSourceIdentity.self,
            forKey: .sourceIdentity
        )
        destination = try container.decode(
            ToriiGovernanceSccpDestination.self,
            forKey: .destination
        )
        soraOutboundExecutionPolicy = try container.decode(
            ToriiGovernanceSccpOutboundExecutionPolicy.self,
            forKey: .soraOutboundExecutionPolicy
        )
        settlement = try container.decode(ToriiGovernanceSccpSettlement.self, forKey: .settlement)
        let discoveryLane = try laneId.discoveryValue
        let discoveryDestination = try destination.discoveryValue(for: discoveryLane)
        let routeConfigurationHash = try SccpExactParser.routeConfigurationHash(
            lane: discoveryLane,
            routeId: routeId,
            assetKey: assetKey,
            revision: revision,
            destination: discoveryDestination
        )
        var governedHashRoles = [
            soraOutboundExecutionPolicy.contractArtifactSha256,
            soraOutboundExecutionPolicy.verifyingKeyReference.commitment,
            routeConfigurationHash,
            discoveryDestination.destinationBindingHash,
        ]
        switch discoveryDestination {
        case let .evm(deployment), let .tron(deployment):
            governedHashRoles.append(contentsOf: [
                deployment.tokenCodeHash,
                deployment.verifierCodeHash,
                deployment.replayVerifierCodeHash,
                deployment.mintBreakerCodeHash,
                deployment.verifierKeyHash,
                deployment.routeCodeHash,
                deployment.outboundProofPolicy.semanticProfile.profileHash,
                deployment.outboundProofPolicy.soraFinalityAnchor.anchorHash,
            ])
        case let .ton(deployment):
            governedHashRoles.append(contentsOf: [
                deployment.jettonMasterCodeHash,
                deployment.jettonMasterInitialDataHash,
                deployment.jettonWalletCodeHash,
                deployment.routeCodeHash,
                deployment.routeInitialDataHash,
                deployment.embeddedVerifierCodeHash,
                deployment.verifierCircuitHash,
                deployment.verifierKeyHash,
                deployment.proofProfileCommitment,
                deployment.outboundProofPolicy.semanticProfile.profileHash,
                deployment.outboundProofPolicy.soraFinalityAnchor.anchorHash,
            ])
        }
        let expectedWrappedSupply = governanceMultiplyDecimal(
            settlement.maxOutstandingLiability,
            by: destination.tairaToTokenMultiplier
        )
        guard laneId.isInbound,
              governanceCanonicalKey(routeId),
              governanceCanonicalKey(assetKey),
              revision > 0,
              sourceIdentity.lane == laneId,
              sourceIdentity.emitter.matches(destination),
              sourceIdentity.emitter.routeConfigurationHash == routeConfigurationHash,
              destination.family == laneId.source.family,
              Set(governedHashRoles).count == governedHashRoles.count,
              expectedWrappedSupply == destination.maxWrappedSupply,
              activation.isTerminal == (inboundFinalityCutoff != nil) else {
            throw DecodingError.dataCorrupted(
                .init(codingPath: container.codingPath, debugDescription: "governed SCCP route is invalid")
            )
        }
    }
}

/// Closed validation-fee charging mode stored in a governed policy.
public enum ToriiGovernanceValidationFeeChargingMode: String, Decodable, Sendable, Equatable {
    case disabled = "DISABLED"
    case perQualifyingTransferInstruction = "PER_QUALIFYING_TRANSFER_INSTRUCTION"

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case chargingMode = "charging_mode"
        case value
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "validation-fee charging mode"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        guard container.contains(.value), try container.decodeNil(forKey: .value) else {
            throw DecodingError.dataCorruptedError(
                forKey: .value,
                in: container,
                debugDescription: "validation-fee charging-mode value must be explicit null"
            )
        }
        let raw = try container.decode(String.self, forKey: .chargingMode)
        guard let value = Self(rawValue: raw) else {
            throw DecodingError.dataCorruptedError(
                forKey: .chargingMode,
                in: container,
                debugDescription: "unsupported validation-fee charging mode"
            )
        }
        self = value
    }
}

/// One exact payout recipient in a governed validation-fee lifecycle.
public struct ToriiGovernanceValidationFeePayoutRecipient: Decodable, Sendable, Equatable {
    public let accountId: String
    public let share: String

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case accountId = "account_id"
        case share
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "validation-fee payout recipient"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        accountId = try governanceCanonicalAccount(
            container.decode(String.self, forKey: .accountId),
            codingPath: container.codingPath + [CodingKeys.accountId],
            field: "account_id"
        )
        share = try governanceCanonicalNumeric(
            container.decode(String.self, forKey: .share),
            codingPath: container.codingPath + [CodingKeys.share],
            field: "share"
        )
        guard share == "0.25" else {
            throw DecodingError.dataCorruptedError(
                forKey: .share,
                in: container,
                debugDescription: "validation-fee payout recipient share must be exactly 0.25"
            )
        }
    }
}

/// Exact contract and six-transfer plan authorized for validation-fee treasury payout.
public struct ToriiGovernanceValidationFeePayoutBinding: Decodable, Sendable, Equatable {
    public let contractAddress: String
    public let codeHash: Data
    public let entrypoint: String
    public let treasuryAccountId: String
    public let dsAssetId: String
    public let xorAssetId: String
    public let poolVaultAccountId: String
    public let batchDs: String
    public let minXorOut: String
    public let maxXorOut: String
    public let recipients: [ToriiGovernanceValidationFeePayoutRecipient]

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case contractAddress = "contract_address"
        case codeHash = "code_hash"
        case entrypoint
        case treasuryAccountId = "treasury_account_id"
        case dsAssetId = "ds_asset_id"
        case xorAssetId = "xor_asset_id"
        case poolVaultAccountId = "pool_vault_account_id"
        case batchDs = "batch_ds"
        case minXorOut = "min_xor_out"
        case maxXorOut = "max_xor_out"
        case recipients
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "validation-fee payout binding"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        contractAddress = try governanceCanonicalContractAddress(
            container.decode(String.self, forKey: .contractAddress),
            codingPath: container.codingPath + [CodingKeys.contractAddress],
            field: "contract_address"
        )
        codeHash = try governanceFixedBytes(
            container.decode([UInt8].self, forKey: .codeHash),
            count: 32,
            nonzero: true,
            codingPath: container.codingPath + [CodingKeys.codeHash],
            field: "code_hash"
        )
        entrypoint = try container.decode(String.self, forKey: .entrypoint)
        treasuryAccountId = try governanceCanonicalAccount(
            container.decode(String.self, forKey: .treasuryAccountId),
            codingPath: container.codingPath + [CodingKeys.treasuryAccountId],
            field: "treasury_account_id"
        )
        dsAssetId = try governanceCanonicalAssetDefinition(
            container.decode(String.self, forKey: .dsAssetId),
            codingPath: container.codingPath + [CodingKeys.dsAssetId],
            field: "ds_asset_id"
        )
        xorAssetId = try governanceCanonicalAssetDefinition(
            container.decode(String.self, forKey: .xorAssetId),
            codingPath: container.codingPath + [CodingKeys.xorAssetId],
            field: "xor_asset_id"
        )
        poolVaultAccountId = try governanceCanonicalAccount(
            container.decode(String.self, forKey: .poolVaultAccountId),
            codingPath: container.codingPath + [CodingKeys.poolVaultAccountId],
            field: "pool_vault_account_id"
        )
        batchDs = try governanceCanonicalQuantity(
            container.decode(String.self, forKey: .batchDs),
            codingPath: container.codingPath + [CodingKeys.batchDs],
            field: "batch_ds"
        )
        minXorOut = try governanceCanonicalQuantity(
            container.decode(String.self, forKey: .minXorOut),
            codingPath: container.codingPath + [CodingKeys.minXorOut],
            field: "min_xor_out"
        )
        maxXorOut = try governanceCanonicalQuantity(
            container.decode(String.self, forKey: .maxXorOut),
            codingPath: container.codingPath + [CodingKeys.maxXorOut],
            field: "max_xor_out"
        )
        recipients = try container.decode(
            [ToriiGovernanceValidationFeePayoutRecipient].self,
            forKey: .recipients
        )
        let recipientAccounts = Set(recipients.map(\.accountId))
        guard entrypoint == "autonomous_validation_fee_tick",
              treasuryAccountId != poolVaultAccountId,
              dsAssetId != xorAssetId,
              batchDs == "10",
              minXorOut == "4",
              maxXorOut == "100",
              recipients.count == 4,
              recipientAccounts.count == 4,
              !recipientAccounts.contains(treasuryAccountId),
              !recipientAccounts.contains(poolVaultAccountId) else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "validation-fee payout binding violates V1 invariants"
                )
            )
        }
    }
}

/// Exact-network validation-fee policy stored in a governance proposal.
public struct ToriiGovernanceValidationFeePolicy: Decodable, Sendable, Equatable {
    public let schemaVersion: UInt16
    public let networkId: NetworkId
    public let policyVersion: String
    public let previousPolicyHash: Data?
    public let dsAssetId: String
    public let dsScale: UInt8
    public let fee: String
    public let treasuryAccountId: String
    public let chargingMode: ToriiGovernanceValidationFeeChargingMode
    public let effectiveFromHeight: String
    public let expiresAfterHeight: String?
    public let exemptionClasses: [String]
    public let treasuryPayoutBinding: ToriiGovernanceValidationFeePayoutBinding?

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case schemaVersion = "schema_version"
        case networkId = "network_id"
        case policyVersion = "policy_version"
        case previousPolicyHash = "previous_policy_hash"
        case dsAssetId = "ds_asset_id"
        case dsScale = "ds_scale"
        case fee
        case treasuryAccountId = "treasury_account_id"
        case chargingMode = "charging_mode"
        case effectiveFromHeight = "effective_from_height"
        case expiresAfterHeight = "expires_after_height"
        case exemptionClasses = "exemption_classes"
        case treasuryPayoutBinding = "treasury_payout_binding"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "validation-fee policy"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        schemaVersion = try container.decode(UInt16.self, forKey: .schemaVersion)
        networkId = try container.decode(NetworkId.self, forKey: .networkId)
        policyVersion = try governanceCanonicalUInt64String(
            container.decode(String.self, forKey: .policyVersion),
            codingPath: container.codingPath + [CodingKeys.policyVersion],
            field: "policy_version",
            positive: true
        )
        guard container.contains(.previousPolicyHash) else {
            throw DecodingError.keyNotFound(
                CodingKeys.previousPolicyHash,
                .init(codingPath: container.codingPath, debugDescription: "previous_policy_hash must be explicit")
            )
        }
        if let bytes = try container.decodeIfPresent([UInt8].self, forKey: .previousPolicyHash) {
            previousPolicyHash = try governanceFixedBytes(
                bytes,
                count: 32,
                nonzero: true,
                codingPath: container.codingPath + [CodingKeys.previousPolicyHash],
                field: "previous_policy_hash"
            )
        } else {
            previousPolicyHash = nil
        }
        dsAssetId = try governanceCanonicalAssetDefinition(
            container.decode(String.self, forKey: .dsAssetId),
            codingPath: container.codingPath + [CodingKeys.dsAssetId],
            field: "ds_asset_id"
        )
        dsScale = try container.decode(UInt8.self, forKey: .dsScale)
        fee = try governanceCanonicalQuantity(
            container.decode(String.self, forKey: .fee),
            codingPath: container.codingPath + [CodingKeys.fee],
            field: "fee"
        )
        treasuryAccountId = try governanceCanonicalAccount(
            container.decode(String.self, forKey: .treasuryAccountId),
            codingPath: container.codingPath + [CodingKeys.treasuryAccountId],
            field: "treasury_account_id"
        )
        chargingMode = try container.decode(
            ToriiGovernanceValidationFeeChargingMode.self,
            forKey: .chargingMode
        )
        effectiveFromHeight = try governanceCanonicalUInt64String(
            container.decode(String.self, forKey: .effectiveFromHeight),
            codingPath: container.codingPath + [CodingKeys.effectiveFromHeight],
            field: "effective_from_height"
        )
        guard container.contains(.expiresAfterHeight) else {
            throw DecodingError.keyNotFound(
                CodingKeys.expiresAfterHeight,
                .init(codingPath: container.codingPath, debugDescription: "expires_after_height must be explicit")
            )
        }
        if let expiry = try container.decodeIfPresent(String.self, forKey: .expiresAfterHeight) {
            expiresAfterHeight = try governanceCanonicalUInt64String(
                expiry,
                codingPath: container.codingPath + [CodingKeys.expiresAfterHeight],
                field: "expires_after_height"
            )
        } else {
            expiresAfterHeight = nil
        }
        exemptionClasses = try container.decode([String].self, forKey: .exemptionClasses)
        guard container.contains(.treasuryPayoutBinding) else {
            throw DecodingError.keyNotFound(
                CodingKeys.treasuryPayoutBinding,
                .init(codingPath: container.codingPath, debugDescription: "treasury_payout_binding must be explicit")
            )
        }
        treasuryPayoutBinding = try container.decodeIfPresent(
            ToriiGovernanceValidationFeePayoutBinding.self,
            forKey: .treasuryPayoutBinding
        )
        let policyNumber = UInt64(policyVersion)!
        let effectiveNumber = UInt64(effectiveFromHeight)!
        let expiryNumber = expiresAfterHeight.flatMap(UInt64.init)
        let exemptionsValid = Set(exemptionClasses).count == exemptionClasses.count
            && exemptionClasses.allSatisfy({ $0 == "TREASURY_PAYOUT" })
        let payoutClassPresent = exemptionClasses.contains("TREASURY_PAYOUT")
        let modeValid: Bool
        switch chargingMode {
        case .disabled:
            modeValid = fee == "0" && exemptionClasses.isEmpty && treasuryPayoutBinding == nil
        case .perQualifyingTransferInstruction:
            modeValid = fee == "0.1"
        }
        guard schemaVersion == 1,
              dsScale == 2,
              (policyNumber == 1) == (previousPolicyHash == nil),
              exemptionsValid,
              payoutClassPresent == (treasuryPayoutBinding != nil),
              treasuryPayoutBinding.map({
                  $0.treasuryAccountId == treasuryAccountId && $0.dsAssetId == dsAssetId
              }) ?? true,
              expiryNumber.map({ $0 > effectiveNumber }) ?? true,
              modeValid else {
            throw DecodingError.dataCorrupted(
                .init(codingPath: container.codingPath, debugDescription: "validation-fee policy violates V1 invariants")
            )
        }
    }
}

/// Stored payload for a governed validation-fee policy proposal.
public struct ToriiGovernanceValidationFeePolicyProposal: Decodable, Sendable, Equatable {
    public let proposalOperator: String
    public let policy: ToriiGovernanceValidationFeePolicy
    public let payoutLifecycleProposalId: Data?

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case proposalOperator = "proposal_operator"
        case policy
        case payoutLifecycleProposalId = "payout_lifecycle_proposal_id"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "validation-fee policy proposal"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        proposalOperator = try governanceCanonicalAccount(
            container.decode(String.self, forKey: .proposalOperator),
            codingPath: container.codingPath + [CodingKeys.proposalOperator],
            field: "proposal_operator"
        )
        policy = try container.decode(ToriiGovernanceValidationFeePolicy.self, forKey: .policy)
        guard container.contains(.payoutLifecycleProposalId) else {
            throw DecodingError.keyNotFound(
                CodingKeys.payoutLifecycleProposalId,
                .init(codingPath: container.codingPath, debugDescription: "payout lifecycle id must be explicit")
            )
        }
        if let bytes = try container.decodeIfPresent(
            [UInt8].self,
            forKey: .payoutLifecycleProposalId
        ) {
            payoutLifecycleProposalId = try governanceFixedBytes(
                bytes,
                count: 32,
                nonzero: true,
                codingPath: container.codingPath + [CodingKeys.payoutLifecycleProposalId],
                field: "payout_lifecycle_proposal_id"
            )
        } else {
            payoutLifecycleProposalId = nil
        }
        guard (policy.treasuryPayoutBinding != nil) == (payoutLifecycleProposalId != nil) else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "payout lifecycle id presence must match the policy payout binding"
                )
            )
        }
    }
}

/// Stored payload authorizing one exact validation-fee payout lifecycle.
public struct ToriiGovernanceValidationFeePayoutLifecycleProposal: Decodable, Sendable, Equatable {
    public let proposalOperator: String
    public let payoutBinding: ToriiGovernanceValidationFeePayoutBinding

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case proposalOperator = "proposal_operator"
        case payoutBinding = "payout_binding"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "validation-fee payout-lifecycle proposal"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        proposalOperator = try governanceCanonicalAccount(
            container.decode(String.self, forKey: .proposalOperator),
            codingPath: container.codingPath + [CodingKeys.proposalOperator],
            field: "proposal_operator"
        )
        payoutBinding = try container.decode(
            ToriiGovernanceValidationFeePayoutBinding.self,
            forKey: .payoutBinding
        )
    }
}

/// Closed Musubi registry-admission mode carried by a Parliament policy action.
public enum ToriiGovernanceMusubiAdmissionMode: String, Decodable, Sendable, Equatable {
    case closed = "Closed"
    case allowlisted = "Allowlisted"
    case open = "Open"

    private enum CodingKeys: String, CodingKey, CaseIterable { case kind, value }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "Musubi registry-admission mode"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        guard container.contains(.value), try container.decodeNil(forKey: .value) else {
            throw DecodingError.dataCorruptedError(
                forKey: .value,
                in: container,
                debugDescription: "Musubi registry-admission mode value must be explicit null"
            )
        }
        let raw = try container.decode(String.self, forKey: .kind)
        guard let value = Self(rawValue: raw) else {
            throw DecodingError.dataCorruptedError(
                forKey: .kind,
                in: container,
                debugDescription: "unsupported Musubi registry-admission mode"
            )
        }
        self = value
    }
}

/// Prospective whole-XOR price schedule for permanent Musubi aliases.
public struct ToriiGovernanceMusubiAliasPricingPolicy: Decodable, Sendable, Equatable {
    public let revision: UInt64
    public let length1Xor: UInt64
    public let length2Xor: UInt64
    public let length3Xor: UInt64
    public let length4Xor: UInt64
    public let length5To32Xor: UInt64

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case revision
        case length1Xor = "length_1_xor"
        case length2Xor = "length_2_xor"
        case length3Xor = "length_3_xor"
        case length4Xor = "length_4_xor"
        case length5To32Xor = "length_5_to_32_xor"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "Musubi alias-pricing policy"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        revision = try container.decode(UInt64.self, forKey: .revision)
        length1Xor = try container.decode(UInt64.self, forKey: .length1Xor)
        length2Xor = try container.decode(UInt64.self, forKey: .length2Xor)
        length3Xor = try container.decode(UInt64.self, forKey: .length3Xor)
        length4Xor = try container.decode(UInt64.self, forKey: .length4Xor)
        length5To32Xor = try container.decode(UInt64.self, forKey: .length5To32Xor)
        guard [revision, length1Xor, length2Xor, length3Xor, length4Xor, length5To32Xor]
            .allSatisfy({ $0 > 0 }) else {
            throw DecodingError.dataCorrupted(
                .init(codingPath: container.codingPath, debugDescription: "Musubi alias prices must be non-zero")
            )
        }
    }
}

/// Complete first-release Musubi registry policy carried by Parliament.
public struct ToriiGovernanceMusubiRegistryPolicy: Decodable, Sendable, Equatable {
    public let version: UInt8
    public let revision: UInt64
    public let mode: ToriiGovernanceMusubiAdmissionMode
    public let allowlistedDataspaces: [UInt64]
    public let aliasPricing: ToriiGovernanceMusubiAliasPricingPolicy

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case version
        case revision
        case mode
        case allowlistedDataspaces = "allowlisted_dataspaces"
        case aliasPricing = "alias_pricing"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "Musubi registry policy"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        version = try container.decode(UInt8.self, forKey: .version)
        revision = try container.decode(UInt64.self, forKey: .revision)
        mode = try container.decode(ToriiGovernanceMusubiAdmissionMode.self, forKey: .mode)
        allowlistedDataspaces = try container.decode(
            [UInt64].self,
            forKey: .allowlistedDataspaces
        )
        aliasPricing = try container.decode(
            ToriiGovernanceMusubiAliasPricingPolicy.self,
            forKey: .aliasPricing
        )
        guard version == 1,
              revision > 0,
              allowlistedDataspaces.count <= 1_024,
              zip(allowlistedDataspaces, allowlistedDataspaces.dropFirst())
                .allSatisfy({ $0.0 < $0.1 }),
              mode == .allowlisted || allowlistedDataspaces.isEmpty else {
            throw DecodingError.dataCorrupted(
                .init(codingPath: container.codingPath, debugDescription: "Musubi registry policy is invalid")
            )
        }
    }
}

/// Parliament package-owner recovery payload.
public struct ToriiGovernanceMusubiRecoverPackageOwners: Decodable, Sendable, Equatable {
    public let package: MusubiPackageIdV1
    public let owners: [String]
    public let expectedRevision: UInt64

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case package
        case owners
        case expectedRevision = "expected_revision"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "Musubi package-owner recovery"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        package = try container.decode(MusubiPackageIdV1.self, forKey: .package)
        let rawOwners = try container.decode([String].self, forKey: .owners)
        owners = try rawOwners.enumerated().map { index, owner in
            try governanceCanonicalAccount(
                owner,
                codingPath: container.codingPath + [CodingKeys.owners, GovernanceProposalCodingKey(intValue: index)!],
                field: "owners"
            )
        }
        expectedRevision = try container.decode(UInt64.self, forKey: .expectedRevision)
        guard !owners.isEmpty,
              owners.count <= 64,
              Set(owners).count == owners.count,
              expectedRevision > 0 else {
            throw DecodingError.dataCorrupted(
                .init(codingPath: container.codingPath, debugDescription: "Musubi owner recovery is invalid")
            )
        }
    }
}

/// Parliament permanent-alias recovery payload.
public struct ToriiGovernanceMusubiRetargetAlias: Decodable, Sendable, Equatable {
    public let alias: MusubiAliasNameV1
    public let target: MusubiPackageIdV1
    public let expectedRevision: UInt64

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case alias
        case target
        case expectedRevision = "expected_revision"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "Musubi alias-retarget action"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        alias = try container.decode(MusubiAliasNameV1.self, forKey: .alias)
        target = try container.decode(MusubiPackageIdV1.self, forKey: .target)
        expectedRevision = try container.decode(UInt64.self, forKey: .expectedRevision)
        guard expectedRevision > 0 else {
            throw DecodingError.dataCorruptedError(
                forKey: .expectedRevision,
                in: container,
                debugDescription: "Musubi alias-retarget revision must be non-zero"
            )
        }
    }
}

/// Parliament immutable-artifact takedown payload.
public struct ToriiGovernanceMusubiTakedownArtifact: Decodable, Sendable, Equatable {
    public let release: MusubiReleaseIdV1
    public let reason: MusubiReasonV1
    public let expectedArtifactGovernanceRevision: UInt64

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case release
        case reason
        case expectedArtifactGovernanceRevision = "expected_artifact_governance_revision"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "Musubi artifact-takedown action"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        release = try container.decode(MusubiReleaseIdV1.self, forKey: .release)
        reason = try container.decode(MusubiReasonV1.self, forKey: .reason)
        expectedArtifactGovernanceRevision = try container.decode(
            UInt64.self,
            forKey: .expectedArtifactGovernanceRevision
        )
        guard expectedArtifactGovernanceRevision > 0 else {
            throw DecodingError.dataCorruptedError(
                forKey: .expectedArtifactGovernanceRevision,
                in: container,
                debugDescription: "Musubi artifact-governance revision must be non-zero"
            )
        }
    }
}

/// Parliament registry-policy replacement payload.
public struct ToriiGovernanceMusubiSetRegistryPolicy: Decodable, Sendable, Equatable {
    public let policy: ToriiGovernanceMusubiRegistryPolicy
    public let expectedRevision: UInt64

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case policy
        case expectedRevision = "expected_revision"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "Musubi registry-policy action"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        policy = try container.decode(ToriiGovernanceMusubiRegistryPolicy.self, forKey: .policy)
        expectedRevision = try container.decode(UInt64.self, forKey: .expectedRevision)
        guard expectedRevision > 0,
              expectedRevision < UInt64.max,
              policy.revision == expectedRevision + 1 else {
            throw DecodingError.dataCorrupted(
                .init(codingPath: container.codingPath, debugDescription: "Musubi policy replacement is not the exact successor")
            )
        }
    }
}

/// Closed Parliament-only Musubi governance action inventory.
public enum ToriiGovernanceMusubiRegistryAction: Decodable, Sendable, Equatable {
    case recoverPackageOwners(ToriiGovernanceMusubiRecoverPackageOwners)
    case retargetAlias(ToriiGovernanceMusubiRetargetAlias)
    case takedownArtifact(ToriiGovernanceMusubiTakedownArtifact)
    case setRegistryPolicy(ToriiGovernanceMusubiSetRegistryPolicy)

    private enum CodingKeys: String, CodingKey, CaseIterable { case kind, value }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "Musubi Parliament action"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "RecoverPackageOwners":
            self = .recoverPackageOwners(
                try container.decode(
                    ToriiGovernanceMusubiRecoverPackageOwners.self,
                    forKey: .value
                )
            )
        case "RetargetAlias":
            self = .retargetAlias(
                try container.decode(ToriiGovernanceMusubiRetargetAlias.self, forKey: .value)
            )
        case "TakedownArtifact":
            self = .takedownArtifact(
                try container.decode(ToriiGovernanceMusubiTakedownArtifact.self, forKey: .value)
            )
        case "SetRegistryPolicy":
            self = .setRegistryPolicy(
                try container.decode(ToriiGovernanceMusubiSetRegistryPolicy.self, forKey: .value)
            )
        case let tag:
            throw DecodingError.dataCorruptedError(
                forKey: .kind,
                in: container,
                debugDescription: "unsupported Musubi Parliament action \(tag)"
            )
        }
    }
}

/// Canonical Norito JSON newtype for one non-zero SoraFS provider id.
public struct ToriiGovernanceSorafsProviderId: Decodable, Sendable, Equatable {
    public let bytes: Data

    public init(from decoder: Decoder) throws {
        var outer = try decoder.unkeyedContainer()
        let raw = try outer.decode([UInt8].self)
        guard outer.isAtEnd else {
            throw DecodingError.dataCorruptedError(
                in: outer,
                debugDescription: "SoraFS provider id must contain one Norito newtype item"
            )
        }
        bytes = try governanceFixedBytes(
            raw,
            count: 32,
            nonzero: true,
            codingPath: decoder.codingPath,
            field: "provider_id"
        )
    }
}

/// Establish one previously unknown SoraFS provider-owner binding.
public struct ToriiGovernanceSorafsEstablishProvider: Decodable, Sendable, Equatable {
    public let providerId: ToriiGovernanceSorafsProviderId
    public let owner: String

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case providerId = "provider_id"
        case owner
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SoraFS provider-owner establish action"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        providerId = try container.decode(ToriiGovernanceSorafsProviderId.self, forKey: .providerId)
        owner = try governanceCanonicalAccount(
            container.decode(String.self, forKey: .owner),
            codingPath: container.codingPath + [CodingKeys.owner],
            field: "owner"
        )
    }
}

/// Compare-and-set one SoraFS provider-owner replacement.
public struct ToriiGovernanceSorafsRebindProvider: Decodable, Sendable, Equatable {
    public let providerId: ToriiGovernanceSorafsProviderId
    public let expectedOwner: String
    public let nextOwner: String

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case providerId = "provider_id"
        case expectedOwner = "expected_owner"
        case nextOwner = "next_owner"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SoraFS provider-owner rebind action"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        providerId = try container.decode(ToriiGovernanceSorafsProviderId.self, forKey: .providerId)
        expectedOwner = try governanceCanonicalAccount(
            container.decode(String.self, forKey: .expectedOwner),
            codingPath: container.codingPath + [CodingKeys.expectedOwner],
            field: "expected_owner"
        )
        nextOwner = try governanceCanonicalAccount(
            container.decode(String.self, forKey: .nextOwner),
            codingPath: container.codingPath + [CodingKeys.nextOwner],
            field: "next_owner"
        )
        guard expectedOwner != nextOwner else {
            throw DecodingError.dataCorrupted(
                .init(codingPath: container.codingPath, debugDescription: "SoraFS rebind must change the owner")
            )
        }
    }
}

/// Compare-and-remove one SoraFS provider-owner binding.
public struct ToriiGovernanceSorafsRemoveProvider: Decodable, Sendable, Equatable {
    public let providerId: ToriiGovernanceSorafsProviderId
    public let expectedOwner: String

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case providerId = "provider_id"
        case expectedOwner = "expected_owner"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SoraFS provider-owner remove action"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        providerId = try container.decode(ToriiGovernanceSorafsProviderId.self, forKey: .providerId)
        expectedOwner = try governanceCanonicalAccount(
            container.decode(String.self, forKey: .expectedOwner),
            codingPath: container.codingPath + [CodingKeys.expectedOwner],
            field: "expected_owner"
        )
    }
}

/// Closed SoraFS provider-owner governance action inventory.
public enum ToriiGovernanceSorafsProviderAction: Decodable, Sendable, Equatable {
    case establish(ToriiGovernanceSorafsEstablishProvider)
    case rebind(ToriiGovernanceSorafsRebindProvider)
    case remove(ToriiGovernanceSorafsRemoveProvider)

    private enum CodingKeys: String, CodingKey, CaseIterable { case action, value }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SoraFS provider governance action"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .action) {
        case "establish":
            self = .establish(
                try container.decode(ToriiGovernanceSorafsEstablishProvider.self, forKey: .value)
            )
        case "rebind":
            self = .rebind(
                try container.decode(ToriiGovernanceSorafsRebindProvider.self, forKey: .value)
            )
        case "remove":
            self = .remove(
                try container.decode(ToriiGovernanceSorafsRemoveProvider.self, forKey: .value)
            )
        case let tag:
            throw DecodingError.dataCorruptedError(
                forKey: .action,
                in: container,
                debugDescription: "unsupported SoraFS provider governance action \(tag)"
            )
        }
    }
}

/// Stored payload for one governed SoraFS provider-owner transition.
public struct ToriiGovernanceSorafsProviderProposal: Decodable, Sendable, Equatable {
    public let action: ToriiGovernanceSorafsProviderAction

    private enum CodingKeys: String, CodingKey, CaseIterable { case action }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "SoraFS provider governance proposal"
        )
        action = try decoder.container(keyedBy: CodingKeys.self)
            .decode(ToriiGovernanceSorafsProviderAction.self, forKey: .action)
    }
}

/// Exact activation payload in a governed contract-lifecycle transition.
public struct ToriiGovernanceContractActivateActionV1: Decodable, Sendable, Equatable {
    public let codeHash: Data
    public let abiHash: Data
    public let abiVersion: UInt16
    public let manifestProvenance: ToriiContractManifestProvenance?

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case codeHash = "code_hash"
        case abiHash = "abi_hash"
        case abiVersion = "abi_version"
        case manifestProvenance = "manifest_provenance"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "contract lifecycle activation"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        codeHash = try governanceLowercaseHash32(
            container.decode(String.self, forKey: .codeHash),
            codingPath: container.codingPath + [CodingKeys.codeHash],
            field: "code_hash"
        )
        abiHash = try governanceLowercaseHash32(
            container.decode(String.self, forKey: .abiHash),
            codingPath: container.codingPath + [CodingKeys.abiHash],
            field: "abi_hash"
        )
        abiVersion = try container.decode(UInt16.self, forKey: .abiVersion)
        guard abiVersion == 1 else {
            throw DecodingError.dataCorruptedError(
                forKey: .abiVersion,
                in: container,
                debugDescription: "contract lifecycle activation abi_version must be exactly 1"
            )
        }
        guard container.contains(.manifestProvenance) else {
            throw DecodingError.keyNotFound(
                CodingKeys.manifestProvenance,
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "manifest_provenance must be explicit, including null"
                )
            )
        }
        manifestProvenance = try container.decodeIfPresent(
            ToriiContractManifestProvenance.self,
            forKey: .manifestProvenance
        )
    }
}

/// Exact deactivation payload in a governed contract-lifecycle transition.
public struct ToriiGovernanceContractDeactivateActionV1: Decodable, Sendable, Equatable {
    public let expectedCodeHash: Data
    public let reason: String?

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case expectedCodeHash = "expected_code_hash"
        case reason
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "contract lifecycle deactivation"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        expectedCodeHash = try governanceLowercaseHash32(
            container.decode(String.self, forKey: .expectedCodeHash),
            codingPath: container.codingPath + [CodingKeys.expectedCodeHash],
            field: "expected_code_hash"
        )
        guard container.contains(.reason) else {
            throw DecodingError.keyNotFound(
                CodingKeys.reason,
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "reason must be explicit, including null"
                )
            )
        }
        if let reason = try container.decodeIfPresent(String.self, forKey: .reason) {
            self.reason = try governanceBoundedReason(
                reason,
                codingPath: container.codingPath + [CodingKeys.reason],
                field: "reason"
            )
        } else {
            self.reason = nil
        }
    }
}

/// Exact ownership-offer payload in a governed contract-lifecycle transition.
public struct ToriiGovernanceContractOfferOwnershipActionV1: Decodable, Sendable, Equatable {
    public let newOwner: String

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case newOwner = "new_owner"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "contract lifecycle ownership offer"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        newOwner = try governanceCanonicalAccount(
            container.decode(String.self, forKey: .newOwner),
            codingPath: container.codingPath + [CodingKeys.newOwner],
            field: "new_owner"
        )
    }
}

/// Exact retained-hold binding and certified finding for an emergency-hold retrospective.
public struct ToriiGovernanceCompleteContractEmergencyHoldRetrospectiveActionV1:
    Decodable, Sendable, Equatable
{
    public let holdProposalContentId: Data
    public let holdGovernanceAttemptId: Data
    public let incidentDigest: Data
    public let retrospectiveFindingRoot: Data

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case holdProposalContentId = "hold_proposal_content_id"
        case holdGovernanceAttemptId = "hold_governance_attempt_id"
        case incidentDigest = "incident_digest"
        case retrospectiveFindingRoot = "retrospective_finding_root"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "contract emergency-hold retrospective"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        holdProposalContentId = try governanceFixedBytes(
            container.decode([UInt8].self, forKey: .holdProposalContentId),
            count: 32,
            nonzero: true,
            codingPath: container.codingPath + [CodingKeys.holdProposalContentId],
            field: "hold_proposal_content_id"
        )
        holdGovernanceAttemptId = try governanceFixedBytes(
            container.decode([UInt8].self, forKey: .holdGovernanceAttemptId),
            count: 32,
            nonzero: true,
            codingPath: container.codingPath + [CodingKeys.holdGovernanceAttemptId],
            field: "hold_governance_attempt_id"
        )
        incidentDigest = try governanceFixedBytes(
            container.decode([UInt8].self, forKey: .incidentDigest),
            count: 32,
            nonzero: true,
            codingPath: container.codingPath + [CodingKeys.incidentDigest],
            field: "incident_digest"
        )
        retrospectiveFindingRoot = try governanceFixedBytes(
            container.decode([UInt8].self, forKey: .retrospectiveFindingRoot),
            count: 32,
            nonzero: true,
            codingPath: container.codingPath + [CodingKeys.retrospectiveFindingRoot],
            field: "retrospective_finding_root"
        )
    }
}

/// Closed owner-consented contract-lifecycle action inventory.
public enum ToriiGovernanceContractLifecycleActionV1: Decodable, Sendable, Equatable {
    case activate(ToriiGovernanceContractActivateActionV1)
    case deactivate(ToriiGovernanceContractDeactivateActionV1)
    case offerOwnership(ToriiGovernanceContractOfferOwnershipActionV1)
    case cancelOwnershipOffer
    case acceptParliamentOwnership
    case completeEmergencyHoldRetrospective(
        ToriiGovernanceCompleteContractEmergencyHoldRetrospectiveActionV1
    )

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case action
        case payload
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "contract lifecycle action"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .action) {
        case "Activate":
            self = .activate(
                try container.decode(
                    ToriiGovernanceContractActivateActionV1.self,
                    forKey: .payload
                )
            )
        case "Deactivate":
            self = .deactivate(
                try container.decode(
                    ToriiGovernanceContractDeactivateActionV1.self,
                    forKey: .payload
                )
            )
        case "OfferOwnership":
            self = .offerOwnership(
                try container.decode(
                    ToriiGovernanceContractOfferOwnershipActionV1.self,
                    forKey: .payload
                )
            )
        case "CancelOwnershipOffer":
            try Self.requireNullPayload(container, tag: "CancelOwnershipOffer")
            self = .cancelOwnershipOffer
        case "AcceptParliamentOwnership":
            try Self.requireNullPayload(container, tag: "AcceptParliamentOwnership")
            self = .acceptParliamentOwnership
        case "CompleteEmergencyHoldRetrospective":
            self = .completeEmergencyHoldRetrospective(
                try container.decode(
                    ToriiGovernanceCompleteContractEmergencyHoldRetrospectiveActionV1.self,
                    forKey: .payload
                )
            )
        case let tag:
            throw DecodingError.dataCorruptedError(
                forKey: .action,
                in: container,
                debugDescription: "unsupported contract lifecycle action \(tag)"
            )
        }
    }

    private static func requireNullPayload(
        _ container: KeyedDecodingContainer<CodingKeys>,
        tag: String
    ) throws {
        guard container.contains(.payload), try container.decodeNil(forKey: .payload) else {
            throw DecodingError.dataCorruptedError(
                forKey: .payload,
                in: container,
                debugDescription: "\(tag) payload must be explicit null"
            )
        }
    }
}

/// Complete compare-and-swap proposal for one governed contract-lifecycle transition.
public struct ToriiGovernanceContractLifecycleProposalV1: Decodable, Sendable, Equatable {
    public let contractAddress: String
    public let expectedRevision: UInt64
    public let action: ToriiGovernanceContractLifecycleActionV1

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case contractAddress = "contract_address"
        case expectedRevision = "expected_revision"
        case action
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "contract lifecycle governance proposal"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        contractAddress = try governanceCanonicalContractAddress(
            container.decode(String.self, forKey: .contractAddress),
            codingPath: container.codingPath + [CodingKeys.contractAddress],
            field: "contract_address"
        )
        expectedRevision = try container.decode(UInt64.self, forKey: .expectedRevision)
        guard (1...9_007_199_254_740_991).contains(expectedRevision) else {
            throw DecodingError.dataCorruptedError(
                forKey: .expectedRevision,
                in: container,
                debugDescription: "expected_revision must be a positive exact first-release JSON integer"
            )
        }
        action = try container.decode(
            ToriiGovernanceContractLifecycleActionV1.self,
            forKey: .action
        )
    }
}

/// Complete time-bounded emergency-containment proposal for one active contract.
public struct ToriiGovernanceContractEmergencyHoldProposalV1: Decodable, Sendable, Equatable {
    public let contractAddress: String
    public let expectedRevision: UInt64
    public let expectedCodeHash: Data
    public let incidentDigest: Data
    public let reason: String
    public let durationBlocks: UInt64

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case contractAddress = "contract_address"
        case expectedRevision = "expected_revision"
        case expectedCodeHash = "expected_code_hash"
        case incidentDigest = "incident_digest"
        case reason
        case durationBlocks = "duration_blocks"
    }

    public init(from decoder: Decoder) throws {
        try governanceRejectUnknownFields(
            decoder,
            allowed: Set(CodingKeys.allCases.map(\.stringValue)),
            name: "contract emergency-hold proposal"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        contractAddress = try governanceCanonicalContractAddress(
            container.decode(String.self, forKey: .contractAddress),
            codingPath: container.codingPath + [CodingKeys.contractAddress],
            field: "contract_address"
        )
        expectedRevision = try container.decode(UInt64.self, forKey: .expectedRevision)
        guard (1...9_007_199_254_740_991).contains(expectedRevision) else {
            throw DecodingError.dataCorruptedError(
                forKey: .expectedRevision,
                in: container,
                debugDescription: "expected_revision must be a positive exact first-release JSON integer"
            )
        }
        expectedCodeHash = try governanceLowercaseHash32(
            container.decode(String.self, forKey: .expectedCodeHash),
            codingPath: container.codingPath + [CodingKeys.expectedCodeHash],
            field: "expected_code_hash"
        )
        incidentDigest = try governanceFixedBytes(
            container.decode([UInt8].self, forKey: .incidentDigest),
            count: 32,
            nonzero: true,
            codingPath: container.codingPath + [CodingKeys.incidentDigest],
            field: "incident_digest"
        )
        reason = try governanceBoundedReason(
            container.decode(String.self, forKey: .reason),
            codingPath: container.codingPath + [CodingKeys.reason],
            field: "reason"
        )
        durationBlocks = try container.decode(UInt64.self, forKey: .durationBlocks)
        guard (1...3_600).contains(durationBlocks) else {
            throw DecodingError.dataCorruptedError(
                forKey: .durationBlocks,
                in: container,
                debugDescription: "duration_blocks must be between 1 and 3600"
            )
        }
    }
}
