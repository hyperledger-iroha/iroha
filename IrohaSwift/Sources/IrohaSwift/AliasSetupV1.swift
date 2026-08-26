import Foundation

/// Validation failures raised by the catalog-free alias and planner models.
public enum AliasSetupModelError: Error, Equatable, Sendable, CustomStringConvertible {
    case invalidName(field: String)
    case invalidValue(field: String)
    case planValidation([String])

    public var description: String {
        switch self {
        case let .invalidName(field):
            return "invalid alias name field: \(field)"
        case let .invalidValue(field):
            return "invalid alias setup field: \(field)"
        case let .planValidation(codes):
            return codes.sorted().joined(separator: ",")
        }
    }
}

private func canonicalAliasSegment(_ raw: String, field: String) throws -> String {
    guard !raw.isEmpty,
          raw.utf8.count <= 255,
          raw == raw.trimmingCharacters(in: .whitespacesAndNewlines),
          !raw.unicodeScalars.contains(where: {
              CharacterSet.whitespacesAndNewlines.contains($0)
                  || CharacterSet.controlCharacters.contains($0)
                  || isAliasBidiControl($0)
          }),
          !raw.contains(where: { "@#$.".contains($0) }) else {
        throw AliasSetupModelError.invalidName(field: field)
    }
    let normalized = raw.precomposedStringWithCanonicalMapping
    guard !normalized.unicodeScalars.contains(where: { (0x1E00...0x1EFF).contains($0.value) }) else {
        throw AliasSetupModelError.invalidName(field: field)
    }
    var components = URLComponents()
    components.scheme = "https"
    components.host = normalized
    guard let ascii = components.url?.host?.lowercased(),
          !ascii.isEmpty,
          ascii.utf8.count <= 63,
          ascii.unicodeScalars.allSatisfy({ scalar in
              let value = scalar.value
              return (48...57).contains(value) || (97...122).contains(value) || value == 45 || value == 95
          }),
          !ascii.hasPrefix("-"),
          !ascii.hasSuffix("-"),
          ascii.count < 4
              || ascii[ascii.index(ascii.startIndex, offsetBy: 2)] != "-"
              || ascii[ascii.index(ascii.startIndex, offsetBy: 3)] != "-"
              || ascii.hasPrefix("xn--"),
          !ascii.hasPrefix("xn--") || isValidCanonicalACELabel(ascii, components: components) else {
        throw AliasSetupModelError.invalidName(field: field)
    }
    return ascii
}

private func isAliasBidiControl(_ scalar: UnicodeScalar) -> Bool {
    switch scalar.value {
    case 0x061C, 0x200E, 0x200F, 0x202A...0x202E, 0x2066...0x2069:
        return true
    default:
        return false
    }
}

private func isValidCanonicalACELabel(_ ascii: String, components: URLComponents) -> Bool {
    guard let percentEncodedUnicode = components.percentEncodedHost,
          let unicode = percentEncodedUnicode.removingPercentEncoding,
          unicode != ascii else {
        return false
    }

    var roundTrip = URLComponents()
    roundTrip.scheme = "https"
    roundTrip.host = unicode
    return roundTrip.url?.host?.lowercased() == ascii
}

private func canonicalAliasAccountId(_ raw: String, field: String) throws -> String {
    guard !raw.isEmpty,
          raw.utf8.elementsEqual(
              raw.trimmingCharacters(in: .whitespacesAndNewlines).utf8
          ),
          !raw.contains("@"),
          !raw.contains("#"),
          !raw.contains("$"),
          let chainDiscriminant = try? AccountAddress
            .inspectI105NetworkPrefix(raw).chainDiscriminant,
          let address = try? AccountAddress.parseEncodedSwiftOnly(
              raw,
              expectedPrefix: chainDiscriminant
          ),
          let canonical = try? address.toI105(
              networkPrefix: chainDiscriminant
          ),
          canonical.utf8.elementsEqual(raw.utf8) else {
        throw AliasSetupModelError.invalidValue(field: field)
    }
    return raw
}

private func canonicalAliasPaymentAsset(_ raw: String, field: String) throws -> String {
    guard AssetDefinitionAddressCodec.canonicalDefinitionLiteral(raw) == raw else {
        throw AliasSetupModelError.invalidValue(field: field)
    }
    return raw
}

private func canonicalAliasText(_ raw: String, field: String, allowWhitespace: Bool) throws -> String {
    guard !raw.isEmpty,
          raw == raw.trimmingCharacters(in: .whitespacesAndNewlines),
          !raw.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) }),
          allowWhitespace || !raw.unicodeScalars.contains(where: { CharacterSet.whitespacesAndNewlines.contains($0) }) else {
        throw AliasSetupModelError.invalidValue(field: field)
    }
    return raw
}

private func canonicalAliasToken(_ raw: String, field: String) throws -> String {
    try canonicalAliasText(raw, field: field, allowWhitespace: false)
}

private let aliasLeaseYearMs: UInt64 = 31_536_000_000

/// Catalog-free textual account alias.
///
/// `merchant@banka.paynet` has a domain while `merchant@paynet` is rooted
/// directly in a dataspace.
public struct AccountAliasName: Codable, Equatable, Hashable, Sendable, CustomStringConvertible {
    public let label: String
    public let domain: String?
    public let dataspace: String

    public init(label: String, domain: String? = nil, dataspace: String) throws {
        self.label = try canonicalAliasSegment(label, field: "label")
        self.domain = try domain.map { try canonicalAliasSegment($0, field: "domain") }
        self.dataspace = try canonicalAliasSegment(dataspace, field: "dataspace")
    }

    /// Parses `label@domain.dataspace` or `label@dataspace` without a catalog.
    public init(parsing literal: String) throws {
        guard !literal.isEmpty,
              literal == literal.trimmingCharacters(in: .whitespacesAndNewlines),
              !literal.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) }) else {
            throw AliasSetupModelError.invalidName(field: "account_alias")
        }
        let atParts = literal.split(separator: "@", omittingEmptySubsequences: false)
        guard atParts.count == 2, !atParts[0].isEmpty, !atParts[1].isEmpty else {
            throw AliasSetupModelError.invalidName(field: "account_alias")
        }
        let scopeParts = atParts[1].split(separator: ".", omittingEmptySubsequences: false)
        switch scopeParts.count {
        case 1:
            try self.init(label: String(atParts[0]), dataspace: String(scopeParts[0]))
        case 2 where !scopeParts[0].isEmpty && !scopeParts[1].isEmpty:
            try self.init(
                label: String(atParts[0]),
                domain: String(scopeParts[0]),
                dataspace: String(scopeParts[1])
            )
        default:
            throw AliasSetupModelError.invalidName(field: "account_alias")
        }
    }

    public var canonicalText: String {
        if let domain {
            return "\(label)@\(domain).\(dataspace)"
        }
        return "\(label)@\(dataspace)"
    }

    public var description: String { canonicalText }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            label: container.decode(String.self, forKey: .label),
            domain: container.decodeIfPresent(String.self, forKey: .domain),
            dataspace: container.decode(String.self, forKey: .dataspace)
        )
    }

    private enum CodingKeys: String, CodingKey { case label, domain, dataspace }
}

/// Canonical dataspace text paired with its expected numeric ID.
public struct ResolvedDataSpaceV1: Codable, Equatable, Hashable, Sendable {
    public let canonicalName: String
    public let dataspaceId: UInt64

    public init(canonicalName: String, dataspaceId: UInt64) throws {
        self.canonicalName = try canonicalAliasSegment(canonicalName, field: "canonical_name")
        self.dataspaceId = dataspaceId
    }

    fileprivate init(validatedCanonicalName: String, dataspaceId: UInt64) {
        self.canonicalName = validatedCanonicalName
        self.dataspaceId = dataspaceId
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            canonicalName: container.decode(String.self, forKey: .canonicalName),
            dataspaceId: container.decode(UInt64.self, forKey: .dataspaceId)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case canonicalName = "canonical_name"
        case dataspaceId = "dataspace_id"
    }
}

/// Canonical `domain.dataspace` text paired with its expected dataspace ID.
public struct ResolvedDomainV1: Codable, Equatable, Hashable, Sendable {
    public let canonicalName: String
    public let dataspaceId: UInt64

    public init(canonicalName: String, dataspaceId: UInt64) throws {
        let parts = canonicalName.split(separator: ".", omittingEmptySubsequences: false)
        guard parts.count == 2, !parts[0].isEmpty, !parts[1].isEmpty else {
            throw AliasSetupModelError.invalidName(field: "canonical_name")
        }
        let domain = try canonicalAliasSegment(String(parts[0]), field: "domain")
        let dataspace = try canonicalAliasSegment(String(parts[1]), field: "dataspace")
        self.canonicalName = "\(domain).\(dataspace)"
        self.dataspaceId = dataspaceId
    }

    public var parentDataspace: ResolvedDataSpaceV1 {
        ResolvedDataSpaceV1(
            validatedCanonicalName: String(canonicalName.split(separator: ".")[1]),
            dataspaceId: dataspaceId
        )
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            canonicalName: container.decode(String.self, forKey: .canonicalName),
            dataspaceId: container.decode(UInt64.self, forKey: .dataspaceId)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case canonicalName = "canonical_name"
        case dataspaceId = "dataspace_id"
    }
}

/// Canonical account-alias text paired with its expected dataspace ID.
public struct ResolvedAccountAliasV1: Codable, Equatable, Hashable, Sendable {
    public let canonicalName: AccountAliasName
    public let dataspaceId: UInt64

    public init(canonicalName: AccountAliasName, dataspaceId: UInt64) {
        self.canonicalName = canonicalName
        self.dataspaceId = dataspaceId
    }

    public init(canonicalName: String, dataspaceId: UInt64) throws {
        try self.init(canonicalName: AccountAliasName(parsing: canonicalName), dataspaceId: dataspaceId)
    }

    public var parentDomain: ResolvedDomainV1? {
        guard let domain = canonicalName.domain else { return nil }
        return try? ResolvedDomainV1(
            canonicalName: "\(domain).\(canonicalName.dataspace)",
            dataspaceId: dataspaceId
        )
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.init(
            canonicalName: try container.decode(AccountAliasName.self, forKey: .canonicalName),
            dataspaceId: try container.decode(UInt64.self, forKey: .dataspaceId)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case canonicalName = "canonical_name"
        case dataspaceId = "dataspace_id"
    }
}

protocol AliasTaggedUnit: RawRepresentable, Codable where RawValue == String {}

extension AliasTaggedUnit {
    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: AliasTaggedUnitKeys.self)
        let raw = try container.decode(String.self, forKey: .kind)
        guard let value = Self(rawValue: raw) else {
            throw DecodingError.dataCorruptedError(forKey: .kind, in: container, debugDescription: "unknown alias enum value")
        }
        self = value
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: AliasTaggedUnitKeys.self)
        try container.encode(rawValue, forKey: .kind)
        try container.encodeNil(forKey: .value)
    }
}

private enum AliasTaggedUnitKeys: String, CodingKey { case kind, value }

/// Account provisioning behavior requested by an account-alias intent.
public enum AccountProvisionV1: String, AliasTaggedUnit, Sendable {
    case existing
    case create
}

/// Whether an account alias is primary or additional.
public enum AccountAliasRoleV1: String, AliasTaggedUnit, Sendable {
    case primary
    case additional
}

/// Lease terms used only when setup classifies a resource as absent.
public struct AliasLeaseAcquisitionV1: Codable, Equatable, Sendable {
    public let termYears: UInt8
    public let pricingClassHint: UInt8?

    public init(termYears: UInt8, pricingClassHint: UInt8? = nil) throws {
        guard termYears > 0 else { throw AliasSetupModelError.invalidValue(field: "term_years") }
        self.termYears = termYears
        self.pricingClassHint = pricingClassHint
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            termYears: container.decode(UInt8.self, forKey: .termYears),
            pricingClassHint: container.decodeIfPresent(UInt8.self, forKey: .pricingClassHint)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case termYears = "term_years"
        case pricingClassHint = "pricing_class_hint"
    }
}

/// Policy, asset, cap, and deadline guard for one lease operation.
public struct AliasQuoteGuardV1: Codable, Equatable, Sendable {
    public let expectedPolicyVersion: UInt16
    public let expectedPaymentAsset: String
    public let maxAmount: String
    public let validUntilMs: UInt64

    public init(
        expectedPolicyVersion: UInt16,
        expectedPaymentAsset: String,
        maxAmount: String,
        validUntilMs: UInt64
    ) throws {
        guard AliasPlanVerifier.isCanonicalQuantity(maxAmount) else {
            throw AliasSetupModelError.invalidValue(field: "max_amount")
        }
        guard validUntilMs > 0 else {
            throw AliasSetupModelError.invalidValue(field: "valid_until_ms")
        }
        self.expectedPolicyVersion = expectedPolicyVersion
        self.expectedPaymentAsset = try canonicalAliasPaymentAsset(
            expectedPaymentAsset,
            field: "expected_payment_asset"
        )
        self.maxAmount = maxAmount
        self.validUntilMs = validUntilMs
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            expectedPolicyVersion: container.decode(UInt16.self, forKey: .expectedPolicyVersion),
            expectedPaymentAsset: container.decode(String.self, forKey: .expectedPaymentAsset),
            maxAmount: container.decode(String.self, forKey: .maxAmount),
            validUntilMs: container.decode(UInt64.self, forKey: .validUntilMs)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case expectedPolicyVersion = "expected_policy_version"
        case expectedPaymentAsset = "expected_payment_asset"
        case maxAmount = "max_amount"
        case validUntilMs = "valid_until_ms"
    }
}

public struct AliasDataSpaceIntentV1: Codable, Equatable, Sendable {
    public let dataspace: ResolvedDataSpaceV1
    public let owner: String

    public init(dataspace: ResolvedDataSpaceV1, owner: String) throws {
        self.dataspace = dataspace
        self.owner = try canonicalAliasAccountId(owner, field: "owner")
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            dataspace: container.decode(ResolvedDataSpaceV1.self, forKey: .dataspace),
            owner: container.decode(String.self, forKey: .owner)
        )
    }

    private enum CodingKeys: String, CodingKey { case dataspace, owner }
}

public struct AliasDomainIntentV1: Codable, Equatable, Sendable {
    public let domain: ResolvedDomainV1
    public let owner: String

    public init(domain: ResolvedDomainV1, owner: String) throws {
        self.domain = domain
        self.owner = try canonicalAliasAccountId(owner, field: "owner")
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            domain: container.decode(ResolvedDomainV1.self, forKey: .domain),
            owner: container.decode(String.self, forKey: .owner)
        )
    }

    private enum CodingKeys: String, CodingKey { case domain, owner }
}

public struct AliasAccountIntentV1: Codable, Equatable, Sendable {
    public let alias: ResolvedAccountAliasV1
    public let targetAccount: String
    public let provision: AccountProvisionV1
    public let role: AccountAliasRoleV1

    public init(
        alias: ResolvedAccountAliasV1,
        targetAccount: String,
        provision: AccountProvisionV1,
        role: AccountAliasRoleV1
    ) throws {
        self.alias = alias
        self.targetAccount = try canonicalAliasAccountId(targetAccount, field: "target_account")
        self.provision = provision
        self.role = role
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            alias: container.decode(ResolvedAccountAliasV1.self, forKey: .alias),
            targetAccount: container.decode(String.self, forKey: .targetAccount),
            provision: container.decode(AccountProvisionV1.self, forKey: .provision),
            role: container.decode(AccountAliasRoleV1.self, forKey: .role)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case alias
        case targetAccount = "target_account"
        case provision
        case role
    }
}

/// Declarative desired state for one alias/SNS resource.
public enum AliasIntentV1: Codable, Equatable, Sendable {
    case dataspace(AliasDataSpaceIntentV1)
    case domain(AliasDomainIntentV1)
    case accountAlias(AliasAccountIntentV1)

    public var dependencyRank: Int {
        switch self {
        case .dataspace: return 0
        case .domain: return 1
        case .accountAlias: return 2
        }
    }

    fileprivate var resourceKey: String {
        switch self {
        case let .dataspace(value):
            return "dataspace\0\(value.dataspace.canonicalName)"
        case let .domain(value):
            return "domain\0\(value.domain.canonicalName)"
        case let .accountAlias(value):
            return "account_alias\0\(value.alias.canonicalName.canonicalText)"
        }
    }

    private enum CodingKeys: String, CodingKey { case kind, intent }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "dataspace": self = .dataspace(try container.decode(AliasDataSpaceIntentV1.self, forKey: .intent))
        case "domain": self = .domain(try container.decode(AliasDomainIntentV1.self, forKey: .intent))
        case "account_alias": self = .accountAlias(try container.decode(AliasAccountIntentV1.self, forKey: .intent))
        default: throw DecodingError.dataCorruptedError(forKey: .kind, in: container, debugDescription: "unknown alias intent")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .dataspace(intent):
            try container.encode("dataspace", forKey: .kind)
            try container.encode(intent, forKey: .intent)
        case let .domain(intent):
            try container.encode("domain", forKey: .kind)
            try container.encode(intent, forKey: .intent)
        case let .accountAlias(intent):
            try container.encode("account_alias", forKey: .kind)
            try container.encode(intent, forKey: .intent)
        }
    }
}

/// Exact resolved resource supported by setup and lifecycle operations.
public enum AliasTargetV1: Codable, Equatable, Sendable {
    case dataspace(ResolvedDataSpaceV1)
    case domain(ResolvedDomainV1)
    case accountAlias(ResolvedAccountAliasV1)

    private enum CodingKeys: String, CodingKey { case kind, resource }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "dataspace": self = .dataspace(try container.decode(ResolvedDataSpaceV1.self, forKey: .resource))
        case "domain": self = .domain(try container.decode(ResolvedDomainV1.self, forKey: .resource))
        case "account_alias": self = .accountAlias(try container.decode(ResolvedAccountAliasV1.self, forKey: .resource))
        default: throw DecodingError.dataCorruptedError(forKey: .kind, in: container, debugDescription: "unknown alias target")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .dataspace(resource):
            try container.encode("dataspace", forKey: .kind)
            try container.encode(resource, forKey: .resource)
        case let .domain(resource):
            try container.encode("domain", forKey: .kind)
            try container.encode(resource, forKey: .resource)
        case let .accountAlias(resource):
            try container.encode("account_alias", forKey: .kind)
            try container.encode(resource, forKey: .resource)
        }
    }
}

/// Exact scope carried by account-alias manage, delegate, and resolve permissions.
public enum AccountAliasPermissionScope: Codable, Equatable, Sendable {
    case domain(String)
    case dataspace(UInt64)
    case alias(ResolvedAccountAliasV1)

    private enum CodingKeys: String, CodingKey { case scope, value }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .scope) {
        case "domain":
            let domain = try container.decode(String.self, forKey: .value)
            _ = try ResolvedDomainV1(canonicalName: domain, dataspaceId: 0)
            self = .domain(domain.lowercased())
        case "dataspace":
            self = .dataspace(try container.decode(UInt64.self, forKey: .value))
        case "alias":
            self = .alias(try container.decode(ResolvedAccountAliasV1.self, forKey: .value))
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .scope,
                in: container,
                debugDescription: "unknown account alias permission scope"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .domain(domain):
            _ = try ResolvedDomainV1(canonicalName: domain, dataspaceId: 0)
            try container.encode("domain", forKey: .scope)
            try container.encode(domain.lowercased(), forKey: .value)
        case let .dataspace(dataspaceId):
            try container.encode("dataspace", forKey: .scope)
            try container.encode(dataspaceId, forKey: .value)
        case let .alias(alias):
            try container.encode("alias", forKey: .scope)
            try container.encode(alias, forKey: .value)
        }
    }
}

/// One `iroha.alias.ensure` instruction.
public struct EnsureAlias: Codable, Equatable, Sendable {
    public static let wireId = "iroha.alias.ensure"

    public let intent: AliasIntentV1
    public let acquisition: AliasLeaseAcquisitionV1
    public let quoteGuard: AliasQuoteGuardV1

    public init(
        intent: AliasIntentV1,
        acquisition: AliasLeaseAcquisitionV1,
        quoteGuard: AliasQuoteGuardV1
    ) {
        self.intent = intent
        self.acquisition = acquisition
        self.quoteGuard = quoteGuard
    }

    private enum CodingKeys: String, CodingKey {
        case intent
        case acquisition
        case quoteGuard = "quote_guard"
    }
}

/// Canonical request body for planning one indivisible alias transaction.
public struct AliasSetupPlanRequestV1: Codable, Equatable, Sendable {
    public static let version: UInt8 = 1

    public let schemaVersion: UInt8
    public let intents: [EnsureAlias]

    public init(intents: [EnsureAlias]) throws {
        guard !intents.isEmpty else {
            throw AliasSetupModelError.invalidValue(field: "intents")
        }
        let resourceKeys = intents.map(\.intent.resourceKey)
        guard Set(resourceKeys).count == resourceKeys.count else {
            throw AliasSetupModelError.invalidValue(field: "intents.duplicate_resource")
        }
        self.schemaVersion = Self.version
        self.intents = intents
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let schemaVersion = try container.decode(UInt8.self, forKey: .schemaVersion)
        guard schemaVersion == Self.version else {
            throw AliasSetupModelError.invalidValue(field: "schema_version")
        }
        try self.init(intents: container.decode([EnsureAlias].self, forKey: .intents))
    }

    private enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case intents
    }
}

/// Expiry-CAS alias lease renewal; the transaction authority is the payer.
public struct RenewAliasLease: Codable, Equatable, Sendable {
    public static let wireId = "iroha.alias.lease.renew"

    public let target: AliasTargetV1
    public let expectedCurrentExpiryMs: UInt64
    public let targetExpiryMs: UInt64
    public let quoteGuard: AliasQuoteGuardV1

    public init(
        target: AliasTargetV1,
        expectedCurrentExpiryMs: UInt64,
        targetExpiryMs: UInt64,
        quoteGuard: AliasQuoteGuardV1
    ) throws {
        guard targetExpiryMs > expectedCurrentExpiryMs else {
            throw AliasSetupModelError.invalidValue(field: "target_expiry_ms")
        }
        self.target = target
        self.expectedCurrentExpiryMs = expectedCurrentExpiryMs
        self.targetExpiryMs = targetExpiryMs
        self.quoteGuard = quoteGuard
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            target: container.decode(AliasTargetV1.self, forKey: .target),
            expectedCurrentExpiryMs: container.decode(UInt64.self, forKey: .expectedCurrentExpiryMs),
            targetExpiryMs: container.decode(UInt64.self, forKey: .targetExpiryMs),
            quoteGuard: container.decode(AliasQuoteGuardV1.self, forKey: .quoteGuard)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case target
        case expectedCurrentExpiryMs = "expected_current_expiry_ms"
        case targetExpiryMs = "target_expiry_ms"
        case quoteGuard = "quote_guard"
    }
}

/// Owner-configured deterministic native auto-renew policy.
public struct AliasAutoRenewConfigV1: Codable, Equatable, Sendable {
    public let termYears: UInt8
    public let policyVersion: UInt16
    public let paymentAsset: String
    public let maxAmount: String
    public let renewBeforeExpiryMs: UInt64
    public let retryBackoffMs: UInt64
    public let maxFailures: UInt32

    public init(
        termYears: UInt8,
        policyVersion: UInt16,
        paymentAsset: String,
        maxAmount: String,
        renewBeforeExpiryMs: UInt64,
        retryBackoffMs: UInt64,
        maxFailures: UInt32
    ) throws {
        guard termYears > 0 else { throw AliasSetupModelError.invalidValue(field: "term_years") }
        guard AliasPlanVerifier.isCanonicalQuantity(maxAmount) else { throw AliasSetupModelError.invalidValue(field: "max_amount") }
        guard renewBeforeExpiryMs > 0 else { throw AliasSetupModelError.invalidValue(field: "renew_before_expiry_ms") }
        guard retryBackoffMs > 0 else { throw AliasSetupModelError.invalidValue(field: "retry_backoff_ms") }
        guard maxFailures > 0 else { throw AliasSetupModelError.invalidValue(field: "max_failures") }
        let termDurationMs = UInt64(termYears).multipliedReportingOverflow(by: aliasLeaseYearMs)
        guard !termDurationMs.overflow, renewBeforeExpiryMs < termDurationMs.partialValue else {
            throw AliasSetupModelError.invalidValue(field: "renew_before_expiry_ms")
        }
        self.termYears = termYears
        self.policyVersion = policyVersion
        self.paymentAsset = try canonicalAliasPaymentAsset(paymentAsset, field: "payment_asset")
        self.maxAmount = maxAmount
        self.renewBeforeExpiryMs = renewBeforeExpiryMs
        self.retryBackoffMs = retryBackoffMs
        self.maxFailures = maxFailures
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            termYears: container.decode(UInt8.self, forKey: .termYears),
            policyVersion: container.decode(UInt16.self, forKey: .policyVersion),
            paymentAsset: container.decode(String.self, forKey: .paymentAsset),
            maxAmount: container.decode(String.self, forKey: .maxAmount),
            renewBeforeExpiryMs: container.decode(UInt64.self, forKey: .renewBeforeExpiryMs),
            retryBackoffMs: container.decode(UInt64.self, forKey: .retryBackoffMs),
            maxFailures: container.decode(UInt32.self, forKey: .maxFailures)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case termYears = "term_years"
        case policyVersion = "policy_version"
        case paymentAsset = "payment_asset"
        case maxAmount = "max_amount"
        case renewBeforeExpiryMs = "renew_before_expiry_ms"
        case retryBackoffMs = "retry_backoff_ms"
        case maxFailures = "max_failures"
    }
}

/// Revision-CAS instruction that enables or disables native auto-renew.
public struct ConfigureAliasAutoRenew: Codable, Equatable, Sendable {
    public static let wireId = "iroha.alias.auto_renew.configure"

    public let target: AliasTargetV1
    public let expectedRevision: UInt64
    public let config: AliasAutoRenewConfigV1?

    public init(target: AliasTargetV1, expectedRevision: UInt64, config: AliasAutoRenewConfigV1?) {
        self.target = target
        self.expectedRevision = expectedRevision
        self.config = config
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        target = try container.decode(AliasTargetV1.self, forKey: .target)
        expectedRevision = try container.decode(UInt64.self, forKey: .expectedRevision)
        config = try container.decodeIfPresent(AliasAutoRenewConfigV1.self, forKey: .config)
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(target, forKey: .target)
        try container.encode(expectedRevision, forKey: .expectedRevision)
        if let config {
            try container.encode(config, forKey: .config)
        } else {
            try container.encodeNil(forKey: .config)
        }
    }

    private enum CodingKeys: String, CodingKey {
        case target
        case expectedRevision = "expected_revision"
        case config
    }
}

/// Canonical request body for planning one absolute-expiry lease renewal.
public struct AliasLeaseRenewPlanRequestV1: Codable, Equatable, Sendable {
    public static let version: UInt8 = 1

    public let schemaVersion: UInt8
    public let renewal: RenewAliasLease

    public var operation: AliasLifecycleOperationV1 { .renewLease(renewal) }

    public init(renewal: RenewAliasLease) {
        self.schemaVersion = Self.version
        self.renewal = renewal
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let version = try container.decode(UInt8.self, forKey: .schemaVersion)
        guard version == Self.version else {
            throw AliasSetupModelError.invalidValue(field: "schema_version")
        }
        self.schemaVersion = version
        self.renewal = try container.decode(RenewAliasLease.self, forKey: .renewal)
    }

    private enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case renewal
    }
}

/// Canonical request body for planning one owner-only auto-renew CAS.
public struct AliasAutoRenewPlanRequestV1: Codable, Equatable, Sendable {
    public static let version: UInt8 = 1

    public let schemaVersion: UInt8
    public let configuration: ConfigureAliasAutoRenew

    public var operation: AliasLifecycleOperationV1 { .configureAutoRenew(configuration) }

    public init(configuration: ConfigureAliasAutoRenew) {
        self.schemaVersion = Self.version
        self.configuration = configuration
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let version = try container.decode(UInt8.self, forKey: .schemaVersion)
        guard version == Self.version else {
            throw AliasSetupModelError.invalidValue(field: "schema_version")
        }
        self.schemaVersion = version
        self.configuration = try container.decode(ConfigureAliasAutoRenew.self, forKey: .configuration)
    }

    private enum CodingKeys: String, CodingKey {
        case schemaVersion = "schema_version"
        case configuration
    }
}

/// Exact operation committed by a lease-renewal or auto-renew transaction plan.
public enum AliasLifecycleOperationV1: Codable, Equatable, Sendable {
    case renewLease(RenewAliasLease)
    case configureAutoRenew(ConfigureAliasAutoRenew)

    public var target: AliasTargetV1 {
        switch self {
        case let .renewLease(operation): return operation.target
        case let .configureAutoRenew(operation): return operation.target
        }
    }

    private enum CodingKeys: String, CodingKey { case kind, operation }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "renew_lease":
            self = .renewLease(try container.decode(RenewAliasLease.self, forKey: .operation))
        case "configure_auto_renew":
            self = .configureAutoRenew(
                try container.decode(ConfigureAliasAutoRenew.self, forKey: .operation)
            )
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .kind,
                in: container,
                debugDescription: "unknown alias lifecycle operation"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case let .renewLease(operation):
            try container.encode("renew_lease", forKey: .kind)
            try container.encode(operation, forKey: .operation)
        case let .configureAutoRenew(operation):
            try container.encode("configure_auto_renew", forKey: .kind)
            try container.encode(operation, forKey: .operation)
        }
    }
}

/// Exact signed request associated with a lifecycle plan loaded for local apply.
public enum AliasLifecyclePlanRequestV1: Equatable, Sendable {
    case leaseRenewal(AliasLeaseRenewPlanRequestV1)
    case autoRenew(AliasAutoRenewPlanRequestV1)

    public var operation: AliasLifecycleOperationV1 {
        switch self {
        case let .leaseRenewal(request): return request.operation
        case let .autoRenew(request): return request.operation
        }
    }
}

/// Whether a lifecycle plan is an exact no-op or needs one ordinary instruction.
public enum AliasLifecyclePlanDispositionV1: String, AliasTaggedUnit, Sendable {
    case noOp = "no_op"
    case apply
}

/// Target-CAS account-alias rebind instruction; lease state is not accepted.
public struct RebindAccountAlias: Codable, Equatable, Sendable {
    public static let wireId = "iroha.account.alias.rebind"

    public let alias: ResolvedAccountAliasV1
    public let expectedTargetAccount: String
    public let newTargetAccount: String

    public init(
        alias: ResolvedAccountAliasV1,
        expectedTargetAccount: String,
        newTargetAccount: String
    ) throws {
        self.alias = alias
        self.expectedTargetAccount = try canonicalAliasAccountId(
            expectedTargetAccount,
            field: "expected_target_account"
        )
        self.newTargetAccount = try canonicalAliasAccountId(
            newTargetAccount,
            field: "new_target_account"
        )
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            alias: container.decode(ResolvedAccountAliasV1.self, forKey: .alias),
            expectedTargetAccount: container.decode(String.self, forKey: .expectedTargetAccount),
            newTargetAccount: container.decode(String.self, forKey: .newTargetAccount)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case alias
        case expectedTargetAccount = "expected_target_account"
        case newTargetAccount = "new_target_account"
    }
}

/// Primary-alias compare-and-set instruction; lease state is not accepted.
public struct CompareAndSetPrimaryAccountAlias: Codable, Equatable, Sendable {
    public static let wireId = "iroha.account.alias.primary.compare_and_set"

    public let account: String
    public let expectedAlias: ResolvedAccountAliasV1?
    public let newAlias: ResolvedAccountAliasV1?

    public init(
        account: String,
        expectedAlias: ResolvedAccountAliasV1?,
        newAlias: ResolvedAccountAliasV1?
    ) throws {
        self.account = try canonicalAliasAccountId(account, field: "account")
        self.expectedAlias = expectedAlias
        self.newAlias = newAlias
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            account: container.decode(String.self, forKey: .account),
            expectedAlias: container.decodeIfPresent(ResolvedAccountAliasV1.self, forKey: .expectedAlias),
            newAlias: container.decodeIfPresent(ResolvedAccountAliasV1.self, forKey: .newAlias)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case account
        case expectedAlias = "expected_alias"
        case newAlias = "new_alias"
    }
}

public enum AliasPlanDispositionV1: String, AliasTaggedUnit, Sendable {
    case noOp = "no_op"
    case repair
    case create
    case conflict
}

public struct AliasLeaseQuoteV1: Codable, Equatable, Sendable {
    public let target: AliasTargetV1
    public let pricingClass: UInt8
    public let exactAmount: String
    public let quoteGuard: AliasQuoteGuardV1
    public let expiresAtMs: UInt64
    public let graceExpiresAtMs: UInt64
    public let redemptionExpiresAtMs: UInt64

    public init(
        target: AliasTargetV1,
        pricingClass: UInt8,
        exactAmount: String,
        quoteGuard: AliasQuoteGuardV1,
        expiresAtMs: UInt64,
        graceExpiresAtMs: UInt64,
        redemptionExpiresAtMs: UInt64
    ) throws {
        guard AliasPlanVerifier.isCanonicalQuantity(exactAmount) else {
            throw AliasSetupModelError.invalidValue(field: "exact_amount")
        }
        self.target = target
        self.pricingClass = pricingClass
        self.exactAmount = exactAmount
        self.quoteGuard = quoteGuard
        self.expiresAtMs = expiresAtMs
        self.graceExpiresAtMs = graceExpiresAtMs
        self.redemptionExpiresAtMs = redemptionExpiresAtMs
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            target: container.decode(AliasTargetV1.self, forKey: .target),
            pricingClass: container.decode(UInt8.self, forKey: .pricingClass),
            exactAmount: container.decode(String.self, forKey: .exactAmount),
            quoteGuard: container.decode(AliasQuoteGuardV1.self, forKey: .quoteGuard),
            expiresAtMs: container.decode(UInt64.self, forKey: .expiresAtMs),
            graceExpiresAtMs: container.decode(UInt64.self, forKey: .graceExpiresAtMs),
            redemptionExpiresAtMs: container.decode(UInt64.self, forKey: .redemptionExpiresAtMs)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case target
        case pricingClass = "pricing_class"
        case exactAmount = "exact_amount"
        case quoteGuard = "guard"
        case expiresAtMs = "expires_at_ms"
        case graceExpiresAtMs = "grace_expires_at_ms"
        case redemptionExpiresAtMs = "redemption_expires_at_ms"
    }
}

public struct AliasPlanResourceV1: Codable, Equatable, Sendable {
    public let intent: AliasIntentV1
    public let disposition: AliasPlanDispositionV1
    public let quote: AliasLeaseQuoteV1?
    public let instructionIndex: UInt32?

    public init(
        intent: AliasIntentV1,
        disposition: AliasPlanDispositionV1,
        quote: AliasLeaseQuoteV1?,
        instructionIndex: UInt32?
    ) {
        self.intent = intent
        self.disposition = disposition
        self.quote = quote
        self.instructionIndex = instructionIndex
    }

    private enum CodingKeys: String, CodingKey {
        case intent
        case disposition
        case quote
        case instructionIndex = "instruction_index"
    }
}

/// Exact framed Norito instruction returned by the planner.
public struct AliasFramedInstructionV1: Codable, Equatable, Sendable {
    public let wireId: String
    public let framedPayload: Data

    public init(wireId: String, framedPayload: Data) throws {
        self.wireId = try canonicalAliasToken(wireId, field: "wire_id")
        self.framedPayload = framedPayload
    }

    private enum CodingKeys: String, CodingKey {
        case wireId = "wire_id"
        case framedPayload = "framed_payload"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            wireId: container.decode(String.self, forKey: .wireId),
            framedPayload: Data(container.decode([UInt8].self, forKey: .framedPayload))
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(wireId, forKey: .wireId)
        try container.encode(Array(framedPayload), forKey: .framedPayload)
    }
}

/// Typed result from decoding and canonically re-encoding an EnsureAlias frame.
public struct DecodedEnsureAliasFrame: Equatable, Sendable {
    public let instruction: EnsureAlias
    public let reencodedFrame: Data

    public init(instruction: EnsureAlias, reencodedFrame: Data) {
        self.instruction = instruction
        self.reencodedFrame = reencodedFrame
    }
}

/// Typed result from decoding and canonically re-encoding a lifecycle frame.
public struct DecodedAliasLifecycleFrame: Equatable, Sendable {
    public let operation: AliasLifecycleOperationV1
    public let reencodedFrame: Data

    public init(operation: AliasLifecycleOperationV1, reencodedFrame: Data) {
        self.operation = operation
        self.reencodedFrame = reencodedFrame
    }
}

/// Registry-backed codec required to interpret planner-provided alias frames.
///
/// Implementations must decode the typed instruction and then canonically
/// re-encode the complete framed archive. An implementation must never treat
/// opaque input bytes as a successful decode/re-encode round trip.
public protocol AliasNoritoRegistryCodec: Sendable {
    func decodeAndReencodeFrame(wireId: String, framedPayload: Data) throws -> Data

    func decodeAndReencodeEnsureAlias(
        wireId: String,
        framedPayload: Data
    ) throws -> DecodedEnsureAliasFrame

    func decodeAndReencodeLifecycle(
        wireId: String,
        framedPayload: Data
    ) throws -> DecodedAliasLifecycleFrame
}

/// Stable failure returned when no typed alias registry codec is available.
public enum AliasNoritoRegistryCodecError: Error, Equatable, Sendable {
    case unavailable(wireId: String)
    case unsupportedWireId(String)
    case invalidInstruction(wireId: String)
}

/// Fail-closed alias codec for builds without a typed Norito registry adapter.
public struct UnavailableAliasNoritoRegistryCodec: AliasNoritoRegistryCodec {
    public init() {}

    public func decodeAndReencodeFrame(wireId: String, framedPayload _: Data) throws -> Data {
        throw AliasNoritoRegistryCodecError.unavailable(wireId: wireId)
    }

    public func decodeAndReencodeEnsureAlias(
        wireId: String,
        framedPayload _: Data
    ) throws -> DecodedEnsureAliasFrame {
        throw AliasNoritoRegistryCodecError.unavailable(wireId: wireId)
    }

    public func decodeAndReencodeLifecycle(
        wireId: String,
        framedPayload _: Data
    ) throws -> DecodedAliasLifecycleFrame {
        throw AliasNoritoRegistryCodecError.unavailable(wireId: wireId)
    }
}

/// Production alias codec backed by the Rust instruction registry.
///
/// The native bridge performs typed registry decoding and canonical full-frame
/// re-encoding. Missing or stale bridge artifacts fail closed.
public struct NativeAliasNoritoRegistryCodec: AliasNoritoRegistryCodec {
    public static let shared = NativeAliasNoritoRegistryCodec()

    public init() {}

    public func decodeAndReencodeFrame(
        wireId: String,
        framedPayload: Data
    ) throws -> Data {
        try roundTrip(wireId: wireId, framedPayload: framedPayload).framedPayload
    }

    public func decodeAndReencodeEnsureAlias(
        wireId: String,
        framedPayload: Data
    ) throws -> DecodedEnsureAliasFrame {
        guard wireId == EnsureAlias.wireId else {
            throw AliasNoritoRegistryCodecError.unsupportedWireId(wireId)
        }
        let result = try roundTrip(wireId: wireId, framedPayload: framedPayload)
        let instruction: EnsureAlias = try decodeInstruction(
            result.typedJSON,
            wireId: wireId
        )
        return DecodedEnsureAliasFrame(
            instruction: instruction,
            reencodedFrame: result.framedPayload
        )
    }

    public func decodeAndReencodeLifecycle(
        wireId: String,
        framedPayload: Data
    ) throws -> DecodedAliasLifecycleFrame {
        let result = try roundTrip(wireId: wireId, framedPayload: framedPayload)
        let operation: AliasLifecycleOperationV1
        switch wireId {
        case RenewAliasLease.wireId:
            let renewal: RenewAliasLease = try decodeInstruction(
                result.typedJSON,
                wireId: wireId
            )
            operation = .renewLease(renewal)
        case ConfigureAliasAutoRenew.wireId:
            let configuration: ConfigureAliasAutoRenew = try decodeInstruction(
                result.typedJSON,
                wireId: wireId
            )
            operation = .configureAutoRenew(configuration)
        default:
            throw AliasNoritoRegistryCodecError.unsupportedWireId(wireId)
        }
        return DecodedAliasLifecycleFrame(
            operation: operation,
            reencodedFrame: result.framedPayload
        )
    }

    private func roundTrip(
        wireId: String,
        framedPayload: Data
    ) throws -> NativeAliasInstructionRoundTripResult {
        do {
            return try NoritoNativeBridge.shared.roundTripAliasInstruction(
                wireId: wireId,
                framedPayload: framedPayload
            )
        } catch NativeBridgeError.bridgeUnavailable {
            throw AliasNoritoRegistryCodecError.unavailable(wireId: wireId)
        } catch {
            throw AliasNoritoRegistryCodecError.invalidInstruction(wireId: wireId)
        }
    }

    private func decodeInstruction<Instruction: Decodable>(
        _ json: Data,
        wireId: String
    ) throws -> Instruction {
        do {
            let envelope = try JSONDecoder().decode(
                NativeAliasInstructionEnvelope<Instruction>.self,
                from: json
            )
            guard envelope.schema == "iroha.alias_instruction_round_trip.v1",
                  envelope.wireId == wireId else {
                throw AliasNoritoRegistryCodecError.invalidInstruction(wireId: wireId)
            }
            return envelope.instruction
        } catch let error as AliasNoritoRegistryCodecError {
            throw error
        } catch {
            throw AliasNoritoRegistryCodecError.invalidInstruction(wireId: wireId)
        }
    }
}

private struct NativeAliasInstructionEnvelope<Instruction: Decodable>: Decodable {
    let schema: String
    let wireId: String
    let instruction: Instruction

    private enum CodingKeys: String, CodingKey {
        case schema
        case wireId = "wire_id"
        case instruction
    }
}

public struct AliasAssetTotalV1: Codable, Equatable, Sendable {
    public let paymentAsset: String
    public let amount: String

    public init(paymentAsset: String, amount: String) throws {
        guard AliasPlanVerifier.isCanonicalQuantity(amount) else {
            throw AliasSetupModelError.invalidValue(field: "amount")
        }
        self.paymentAsset = try canonicalAliasPaymentAsset(paymentAsset, field: "payment_asset")
        self.amount = amount
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            paymentAsset: container.decode(String.self, forKey: .paymentAsset),
            amount: container.decode(String.self, forKey: .amount)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case paymentAsset = "payment_asset"
        case amount
    }
}

public enum AliasSetupStatusV1: String, Codable, Sendable {
    case ready
    case pending
    case blocked

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: StatusKeys.self)
        let raw = try container.decode(String.self, forKey: .status)
        guard let value = Self(rawValue: raw) else {
            throw DecodingError.dataCorruptedError(
                forKey: .status,
                in: container,
                debugDescription: "unknown setup status"
            )
        }
        self = value
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: StatusKeys.self)
        try container.encode(rawValue, forKey: .status)
        try container.encodeNil(forKey: .value)
    }

    private enum StatusKeys: String, CodingKey { case status, value }
}

public enum AliasSetupValidationPhaseV1: String, Codable, Sendable {
    case config
    case catalog
    case bootstrap
    case worldState = "world_state"
    case planning

    fileprivate var sortOrdinal: Int {
        switch self {
        case .config: return 0
        case .catalog: return 1
        case .bootstrap: return 2
        case .worldState: return 3
        case .planning: return 4
        }
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: PhaseKeys.self)
        let raw = try container.decode(String.self, forKey: .phase)
        guard let value = Self(rawValue: raw) else {
            throw DecodingError.dataCorruptedError(forKey: .phase, in: container, debugDescription: "unknown setup phase")
        }
        self = value
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: PhaseKeys.self)
        try container.encode(rawValue, forKey: .phase)
        try container.encodeNil(forKey: .value)
    }

    private enum PhaseKeys: String, CodingKey { case phase, value }
}

public enum AliasSetupSeverityV1: String, Codable, Sendable {
    case info
    case warning
    case error

    fileprivate var sortOrdinal: Int {
        switch self {
        case .info: return 0
        case .warning: return 1
        case .error: return 2
        }
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: SeverityKeys.self)
        let raw = try container.decode(String.self, forKey: .severity)
        guard let value = Self(rawValue: raw) else {
            throw DecodingError.dataCorruptedError(forKey: .severity, in: container, debugDescription: "unknown setup severity")
        }
        self = value
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: SeverityKeys.self)
        try container.encode(rawValue, forKey: .severity)
        try container.encodeNil(forKey: .value)
    }

    private enum SeverityKeys: String, CodingKey { case severity, value }
}

public struct AliasSetupDiagnosticV1: Codable, Equatable, Sendable {
    public let phase: AliasSetupValidationPhaseV1
    public let code: String
    public let severity: AliasSetupSeverityV1
    public let resource: String?
    public let configPath: String?
    public let expected: String?
    public let actual: String?
    public let remediation: String

    public init(
        phase: AliasSetupValidationPhaseV1,
        code: String,
        severity: AliasSetupSeverityV1,
        resource: String? = nil,
        configPath: String? = nil,
        expected: String? = nil,
        actual: String? = nil,
        remediation: String
    ) throws {
        self.phase = phase
        self.code = try canonicalAliasToken(code, field: "code")
        self.severity = severity
        self.resource = try resource.map {
            try canonicalAliasText($0, field: "resource", allowWhitespace: true)
        }
        self.configPath = try configPath.map {
            try canonicalAliasText($0, field: "config_path", allowWhitespace: true)
        }
        self.expected = try expected.map {
            try canonicalAliasText($0, field: "expected", allowWhitespace: true)
        }
        self.actual = try actual.map {
            try canonicalAliasText($0, field: "actual", allowWhitespace: true)
        }
        self.remediation = try canonicalAliasText(
            remediation,
            field: "remediation",
            allowWhitespace: true
        )
    }

    fileprivate var sortKey: String {
        [String(phase.sortOrdinal), code, String(severity.sortOrdinal), resource ?? "", configPath ?? "", expected ?? "", actual ?? "", remediation]
            .joined(separator: "\0")
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            phase: container.decode(AliasSetupValidationPhaseV1.self, forKey: .phase),
            code: container.decode(String.self, forKey: .code),
            severity: container.decode(AliasSetupSeverityV1.self, forKey: .severity),
            resource: container.decodeIfPresent(String.self, forKey: .resource),
            configPath: container.decodeIfPresent(String.self, forKey: .configPath),
            expected: container.decodeIfPresent(String.self, forKey: .expected),
            actual: container.decodeIfPresent(String.self, forKey: .actual),
            remediation: container.decode(String.self, forKey: .remediation)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case phase, code, severity, resource
        case configPath = "config_path"
        case expected, actual, remediation
    }
}

public struct AliasSetupReportV1: Codable, Equatable, Sendable {
    public static let version: UInt8 = 1

    public let version: UInt8
    public let status: AliasSetupStatusV1
    public let diagnostics: [AliasSetupDiagnosticV1]

    public init(status: AliasSetupStatusV1, diagnostics: [AliasSetupDiagnosticV1]) {
        self.version = Self.version
        self.status = status
        self.diagnostics = diagnostics.sorted { $0.sortKey < $1.sortKey }
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let version = try container.decode(UInt8.self, forKey: .version)
        guard version == Self.version else {
            throw AliasSetupModelError.invalidValue(field: "version")
        }
        self.init(
            status: try container.decode(AliasSetupStatusV1.self, forKey: .status),
            diagnostics: try container.decode([AliasSetupDiagnosticV1].self, forKey: .diagnostics)
        )
    }

    private enum CodingKeys: String, CodingKey { case version, status, diagnostics }
}

public struct AliasPlanAnchorV1: Codable, Equatable, Sendable {
    public let blockHeight: UInt64
    /// Exact canonical checksummed `hash:<64 uppercase hex>#<CRC16>` block hash.
    public let blockHash: String

    public init(blockHeight: UInt64, blockHash: String) throws {
        guard AliasPlanVerifier.isCanonicalHashLiteral(blockHash) else {
            throw AliasSetupModelError.invalidValue(field: "block_hash")
        }
        self.blockHeight = blockHeight
        self.blockHash = blockHash
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            blockHeight: container.decode(UInt64.self, forKey: .blockHeight),
            blockHash: container.decode(String.self, forKey: .blockHash)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case blockHeight = "block_height"
        case blockHash = "block_hash"
    }
}

public struct AliasTransactionPlanBodyV1: Codable, Equatable, Sendable {
    public static let version: UInt8 = 1

    public let version: UInt8
    public let authority: String
    /// Exact genesis-derived identity of the network that produced this plan.
    public let networkId: NetworkId
    public let anchor: AliasPlanAnchorV1
    public let resources: [AliasPlanResourceV1]
    public let instructions: [AliasFramedInstructionV1]
    public let totalsByAsset: [AliasAssetTotalV1]
    public let warnings: [AliasSetupDiagnosticV1]
    public let blockers: [AliasSetupDiagnosticV1]
    public let validUntilMs: UInt64

    public init(
        version: UInt8 = Self.version,
        authority: String,
        networkId: NetworkId,
        anchor: AliasPlanAnchorV1,
        resources: [AliasPlanResourceV1],
        instructions: [AliasFramedInstructionV1],
        totalsByAsset: [AliasAssetTotalV1],
        warnings: [AliasSetupDiagnosticV1],
        blockers: [AliasSetupDiagnosticV1],
        validUntilMs: UInt64
    ) throws {
        self.version = version
        self.authority = try canonicalAliasAccountId(authority, field: "authority")
        self.networkId = networkId
        self.anchor = anchor
        self.resources = resources
        self.instructions = instructions
        self.totalsByAsset = totalsByAsset
        self.warnings = warnings
        self.blockers = blockers
        self.validUntilMs = validUntilMs
    }

    public init(from decoder: Decoder) throws {
        let retired = try decoder.container(keyedBy: RetiredCodingKeys.self)
        if let retiredKey = retired.allKeys.first {
            throw DecodingError.dataCorruptedError(
                forKey: retiredKey,
                in: retired,
                debugDescription: "retired chain identity is forbidden; alias transaction plans require network_id"
            )
        }
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            version: container.decode(UInt8.self, forKey: .version),
            authority: container.decode(String.self, forKey: .authority),
            networkId: container.decode(NetworkId.self, forKey: .networkId),
            anchor: container.decode(AliasPlanAnchorV1.self, forKey: .anchor),
            resources: container.decode([AliasPlanResourceV1].self, forKey: .resources),
            instructions: container.decode([AliasFramedInstructionV1].self, forKey: .instructions),
            totalsByAsset: container.decode([AliasAssetTotalV1].self, forKey: .totalsByAsset),
            warnings: container.decode([AliasSetupDiagnosticV1].self, forKey: .warnings),
            blockers: container.decode([AliasSetupDiagnosticV1].self, forKey: .blockers),
            validUntilMs: container.decode(UInt64.self, forKey: .validUntilMs)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case version, authority
        case networkId = "network_id"
        case anchor, resources, instructions
        case totalsByAsset = "totals_by_asset"
        case warnings, blockers
        case validUntilMs = "valid_until_ms"
    }

    private enum RetiredCodingKeys: String, CodingKey {
        case chain
        case chainId
        case chainIdSnake = "chain_id"
    }
}

public struct AliasTransactionPlanV1: Codable, Equatable, Sendable {
    public let body: AliasTransactionPlanBodyV1
    public let planHash: String

    public init(body: AliasTransactionPlanBodyV1, planHash: String) throws {
        guard AliasPlanVerifier.isCanonicalHashText(planHash) else {
            throw AliasSetupModelError.invalidValue(field: "plan_hash")
        }
        self.body = body
        self.planHash = planHash
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            body: container.decode(AliasTransactionPlanBodyV1.self, forKey: .body),
            planHash: container.decode(String.self, forKey: .planHash)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case body
        case planHash = "plan_hash"
    }
}

/// Canonical body committed by a lifecycle transaction plan hash.
public struct AliasLifecycleTransactionPlanBodyV1: Codable, Equatable, Sendable {
    public static let version: UInt8 = 1

    public let version: UInt8
    public let authority: String
    /// Exact genesis-derived identity of the network that produced this plan.
    public let networkId: NetworkId
    public let anchor: AliasPlanAnchorV1
    public let operation: AliasLifecycleOperationV1
    public let disposition: AliasLifecyclePlanDispositionV1
    public let instruction: AliasFramedInstructionV1?
    public let quote: AliasLeaseQuoteV1?
    public let totalsByAsset: [AliasAssetTotalV1]
    public let warnings: [AliasSetupDiagnosticV1]
    public let blockers: [AliasSetupDiagnosticV1]
    public let validUntilMs: UInt64

    public init(
        version: UInt8 = Self.version,
        authority: String,
        networkId: NetworkId,
        anchor: AliasPlanAnchorV1,
        operation: AliasLifecycleOperationV1,
        disposition: AliasLifecyclePlanDispositionV1,
        instruction: AliasFramedInstructionV1?,
        quote: AliasLeaseQuoteV1?,
        totalsByAsset: [AliasAssetTotalV1],
        warnings: [AliasSetupDiagnosticV1],
        blockers: [AliasSetupDiagnosticV1],
        validUntilMs: UInt64
    ) throws {
        guard version == Self.version else {
            throw AliasSetupModelError.invalidValue(field: "version")
        }
        self.version = version
        self.authority = try canonicalAliasAccountId(authority, field: "authority")
        self.networkId = networkId
        self.anchor = anchor
        self.operation = operation
        self.disposition = disposition
        self.instruction = instruction
        self.quote = quote
        self.totalsByAsset = totalsByAsset
        self.warnings = warnings
        self.blockers = blockers
        self.validUntilMs = validUntilMs
    }

    public init(from decoder: Decoder) throws {
        let retired = try decoder.container(keyedBy: RetiredCodingKeys.self)
        if let retiredKey = retired.allKeys.first {
            throw DecodingError.dataCorruptedError(
                forKey: retiredKey,
                in: retired,
                debugDescription: "retired chain identity is forbidden; alias lifecycle plans require network_id"
            )
        }
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            version: container.decode(UInt8.self, forKey: .version),
            authority: container.decode(String.self, forKey: .authority),
            networkId: container.decode(NetworkId.self, forKey: .networkId),
            anchor: container.decode(AliasPlanAnchorV1.self, forKey: .anchor),
            operation: container.decode(AliasLifecycleOperationV1.self, forKey: .operation),
            disposition: container.decode(AliasLifecyclePlanDispositionV1.self, forKey: .disposition),
            instruction: container.decodeIfPresent(AliasFramedInstructionV1.self, forKey: .instruction),
            quote: container.decodeIfPresent(AliasLeaseQuoteV1.self, forKey: .quote),
            totalsByAsset: container.decode([AliasAssetTotalV1].self, forKey: .totalsByAsset),
            warnings: container.decode([AliasSetupDiagnosticV1].self, forKey: .warnings),
            blockers: container.decode([AliasSetupDiagnosticV1].self, forKey: .blockers),
            validUntilMs: container.decode(UInt64.self, forKey: .validUntilMs)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case version, authority
        case networkId = "network_id"
        case anchor, operation, disposition, instruction, quote
        case totalsByAsset = "totals_by_asset"
        case warnings, blockers
        case validUntilMs = "valid_until_ms"
    }
    private enum RetiredCodingKeys: String, CodingKey {
        case chain
        case chainId
        case chainIdSnake = "chain_id"
    }
}

/// Read-only planner response for one alias lifecycle operation.
public struct AliasLifecycleTransactionPlanV1: Codable, Equatable, Sendable {
    public let body: AliasLifecycleTransactionPlanBodyV1
    public let planHash: String

    public init(body: AliasLifecycleTransactionPlanBodyV1, planHash: String) throws {
        guard AliasPlanVerifier.isCanonicalHashText(planHash) else {
            throw AliasSetupModelError.invalidValue(field: "plan_hash")
        }
        self.body = body
        self.planHash = planHash
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            body: container.decode(AliasLifecycleTransactionPlanBodyV1.self, forKey: .body),
            planHash: container.decode(String.self, forKey: .planHash)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case body
        case planHash = "plan_hash"
    }
}

/// Verification helpers used before locally signing an alias transaction plan.
public enum AliasPlanVerifier {
    private static let hashDomain = Data("iroha:alias-transaction-plan-body:v1\0".utf8)
    private static let lifecycleHashDomain = Data("iroha:alias-lifecycle-transaction-plan-body:v1\0".utf8)

    /// Computes the plan commitment from the exact Norito encoding of the plan body.
    public static func canonicalHash(canonicalBodyNorito: Data) -> Data {
        IrohaHash.hash(hashDomain + canonicalBodyNorito)
    }

    /// Computes a lifecycle-plan commitment from the exact Norito body bytes.
    public static func canonicalLifecycleHash(canonicalBodyNorito: Data) -> Data {
        IrohaHash.hash(lifecycleHashDomain + canonicalBodyNorito)
    }

    /// Verifies the carried plan hash against exact Norito body bytes.
    public static func verifyHash(_ planHash: String, canonicalBodyNorito: Data) -> Bool {
        guard let expected = decodeHash(planHash) else { return false }
        return expected == canonicalHash(canonicalBodyNorito: canonicalBodyNorito)
    }

    public static func verifyHash(_ plan: AliasTransactionPlanV1, canonicalBodyNorito: Data) -> Bool {
        verifyHash(plan.planHash, canonicalBodyNorito: canonicalBodyNorito)
    }

    /// Verifies a lifecycle plan's carried hash against its exact Norito body.
    public static func verifyLifecycleHash(
        _ plan: AliasLifecycleTransactionPlanV1,
        canonicalBodyNorito: Data
    ) -> Bool {
        guard let expected = decodeHash(plan.planHash) else { return false }
        return expected == canonicalLifecycleHash(canonicalBodyNorito: canonicalBodyNorito)
    }

    /// Returns stable validation codes for a plan that is unsafe to submit.
    public static func validateExecutable(_ plan: AliasTransactionPlanV1) -> [String] {
        var errors = Set<String>()
        let body = plan.body
        if body.version != AliasTransactionPlanBodyV1.version { errors.insert("alias.plan.version_unsupported") }
        if !body.blockers.isEmpty { errors.insert("alias.plan.blocked") }
        if body.resources.isEmpty { errors.insert("alias.plan.resources_empty") }
        if body.instructions.count != body.resources.count { errors.insert("alias.plan.instruction_count_mismatch") }
        if decodeHash(plan.planHash) == nil { errors.insert("alias.plan.hash_invalid") }
        if zip(body.resources, body.resources.dropFirst()).contains(where: { $0.0.intent.dependencyRank > $0.1.intent.dependencyRank }) {
            errors.insert("alias.plan.resource_order_invalid")
        }
        if !totalsAreCanonical(body.totalsByAsset) {
            errors.insert("alias.plan.totals_not_canonical")
        }
        if body.warnings.map(\.sortKey) != body.warnings.map(\.sortKey).sorted() ||
            body.blockers.map(\.sortKey) != body.blockers.map(\.sortKey).sorted() {
            errors.insert("alias.plan.diagnostics_not_canonical")
        }
        if body.instructions.contains(where: { $0.wireId != EnsureAlias.wireId || $0.framedPayload.isEmpty }) {
            errors.insert("alias.plan.instruction_invalid")
        }

        var claimed = Set<UInt32>()
        var previousInstructionIndex: UInt32?
        var calculatedTotals: [String: String] = [:]
        var quotedValidUntilMs = UInt64.max
        for resource in body.resources {
            if let index = resource.instructionIndex {
                if Int(index) >= body.instructions.count {
                    errors.insert("alias.plan.instruction_index_invalid")
                } else {
                    if !claimed.insert(index).inserted { errors.insert("alias.plan.instruction_index_duplicate") }
                    if let previousInstructionIndex, index <= previousInstructionIndex {
                        errors.insert("alias.plan.instruction_indexes_not_ordered")
                    }
                    previousInstructionIndex = index
                    if body.instructions[Int(index)].wireId != EnsureAlias.wireId {
                        errors.insert("alias.plan.instruction_wire_id_invalid")
                    }
                }
            }
            switch resource.disposition {
            case .noOp:
                if resource.quote != nil || resource.instructionIndex == nil { errors.insert("alias.plan.no_op_shape_invalid") }
            case .repair:
                if resource.quote != nil || resource.instructionIndex == nil { errors.insert("alias.plan.repair_shape_invalid") }
            case .create:
                if resource.quote == nil || resource.instructionIndex == nil { errors.insert("alias.plan.create_shape_invalid") }
            case .conflict:
                errors.insert("alias.plan.conflict")
                if resource.quote != nil || resource.instructionIndex != nil { errors.insert("alias.plan.conflict_not_empty") }
            }
            if let quote = resource.quote {
                if quote.target != target(for: resource.intent) { errors.insert("alias.plan.quote_target_mismatch") }
                if !amount(quote.exactAmount, isAtMost: quote.quoteGuard.maxAmount) { errors.insert("alias.plan.quote_cap_invalid") }
                if quote.expiresAtMs > quote.graceExpiresAtMs || quote.graceExpiresAtMs > quote.redemptionExpiresAtMs {
                    errors.insert("alias.plan.quote_expiry_order_invalid")
                }
                if resource.disposition == .create {
                    guard let total = addQuantities(
                        calculatedTotals[quote.quoteGuard.expectedPaymentAsset] ?? "0",
                        quote.exactAmount
                    ) else {
                        errors.insert("alias.plan.total_overflow")
                        continue
                    }
                    calculatedTotals[quote.quoteGuard.expectedPaymentAsset] = total
                    quotedValidUntilMs = min(quotedValidUntilMs, quote.quoteGuard.validUntilMs)
                }
            }
        }
        if claimed.count != body.instructions.count { errors.insert("alias.plan.instruction_unreferenced") }
        let expectedTotals = calculatedTotals
            .map { (paymentAsset: $0.key, amount: $0.value) }
            .sorted {
                totalSortKey($0.paymentAsset, amount: $0.amount)
                    .lexicographicallyPrecedes(totalSortKey($1.paymentAsset, amount: $1.amount))
            }
        if body.totalsByAsset.count != expectedTotals.count ||
            zip(body.totalsByAsset, expectedTotals).contains(where: { actual, expected in
                actual.paymentAsset != expected.paymentAsset || actual.amount != expected.amount
            }) {
            errors.insert("alias.plan.total_mismatch")
        }
        if body.validUntilMs == 0 || body.validUntilMs == UInt64.max {
            errors.insert("alias.plan.deadline_invalid")
        } else if body.validUntilMs > quotedValidUntilMs {
            errors.insert("alias.plan.deadline_exceeds_quote")
        }
        return errors.sorted()
    }

    /// Decodes and re-encodes every frame, rejecting any byte-level change.
    public static func verifyExactFrames(
        _ plan: AliasTransactionPlanV1,
        roundTrip: (String, Data) throws -> Data
    ) -> Bool {
        for instruction in plan.body.instructions {
            guard let encoded = try? roundTrip(instruction.wireId, instruction.framedPayload),
                  encoded == instruction.framedPayload else { return false }
        }
        return true
    }

    /// Requires canonical shape, exact body hash, and exact frame round trips.
    public static func requireExecutable(
        _ plan: AliasTransactionPlanV1,
        canonicalBodyNorito: Data,
        roundTrip: (String, Data) throws -> Data
    ) throws {
        var errors = validateExecutable(plan)
        if !verifyHash(plan, canonicalBodyNorito: canonicalBodyNorito) { errors.append("alias.plan.hash_mismatch") }
        if !verifyExactFrames(plan, roundTrip: roundTrip) { errors.append("alias.plan.instruction_roundtrip_mismatch") }
        if !errors.isEmpty { throw AliasSetupModelError.planValidation(Array(Set(errors)).sorted()) }
    }

    /// Requires the plan to be the complete canonical rendering of one signed request.
    public static func requireExecutableForRequest(
        _ request: AliasSetupPlanRequestV1,
        plan: AliasTransactionPlanV1,
        canonicalBodyNorito: Data,
        decodeAndReencode: (String, Data) throws -> DecodedEnsureAliasFrame
    ) throws {
        var errors = validateExecutable(plan)
        if !verifyHash(plan, canonicalBodyNorito: canonicalBodyNorito) {
            errors.append("alias.plan.hash_mismatch")
        }

        var decoded = [EnsureAlias]()
        for frame in plan.body.instructions {
            do {
                let result = try decodeAndReencode(frame.wireId, frame.framedPayload)
                guard result.reencodedFrame == frame.framedPayload else {
                    errors.append("alias.plan.instruction_roundtrip_mismatch")
                    continue
                }
                decoded.append(result.instruction)
            } catch {
                errors.append("alias.plan.instruction_roundtrip_mismatch")
            }
        }

        let expected = request.intents.sorted { left, right in
            if left.intent.dependencyRank != right.intent.dependencyRank {
                return left.intent.dependencyRank < right.intent.dependencyRank
            }
            return left.intent.resourceKey < right.intent.resourceKey
        }
        if decoded != expected {
            errors.append("alias.plan.signed_request_mismatch")
        }
        if plan.body.resources.map(\.intent) != expected.map(\.intent) {
            errors.append("alias.plan.resource_request_mismatch")
        }
        for resource in plan.body.resources {
            guard let index = resource.instructionIndex,
                  Int(index) < decoded.count,
                  let quote = resource.quote else { continue }
            if quote.quoteGuard != decoded[Int(index)].quoteGuard {
                errors.append("alias.plan.quote_guard_instruction_mismatch")
            }
        }
        if !errors.isEmpty {
            throw AliasSetupModelError.planValidation(Array(Set(errors)).sorted())
        }
    }

    /// Registry-codec form of request-bound setup-plan verification.
    public static func requireExecutableForRequest<Codec: AliasNoritoRegistryCodec>(
        _ request: AliasSetupPlanRequestV1,
        plan: AliasTransactionPlanV1,
        canonicalBodyNorito: Data,
        codec: Codec
    ) throws {
        try requireExecutableForRequest(
            request,
            plan: plan,
            canonicalBodyNorito: canonicalBodyNorito,
            decodeAndReencode: codec.decodeAndReencodeEnsureAlias
        )
    }

    /// Returns stable validation codes for a lifecycle plan that is unsafe to submit.
    public static func validateExecutable(_ plan: AliasLifecycleTransactionPlanV1) -> [String] {
        var errors = Set<String>()
        let body = plan.body
        if body.version != AliasLifecycleTransactionPlanBodyV1.version {
            errors.insert("alias.lifecycle.plan.version_unsupported")
        }
        if !body.blockers.isEmpty { errors.insert("alias.lifecycle.plan.blocked") }
        if body.validUntilMs == 0 || body.validUntilMs == UInt64.max {
            errors.insert("alias.lifecycle.plan.deadline_invalid")
        }
        if decodeHash(plan.planHash) == nil { errors.insert("alias.lifecycle.plan.hash_invalid") }
        if body.warnings.map(\.sortKey) != body.warnings.map(\.sortKey).sorted() ||
            body.blockers.map(\.sortKey) != body.blockers.map(\.sortKey).sorted() {
            errors.insert("alias.lifecycle.plan.diagnostics_not_canonical")
        }
        if !totalsAreCanonical(body.totalsByAsset) {
            errors.insert("alias.lifecycle.plan.totals_not_canonical")
        }

        switch body.disposition {
        case .noOp:
            if case .renewLease = body.operation {
                errors.insert("alias.lifecycle.plan.renewal_no_op_invalid")
            }
            if body.instruction != nil || body.quote != nil || !body.totalsByAsset.isEmpty {
                errors.insert("alias.lifecycle.plan.no_op_shape_invalid")
            }
        case .apply:
            guard let instruction = body.instruction else {
                errors.insert("alias.lifecycle.plan.instruction_missing")
                return errors.sorted()
            }
            let expectedWireId: String
            switch body.operation {
            case let .renewLease(renewal):
                expectedWireId = RenewAliasLease.wireId
                guard let quote = body.quote else {
                    errors.insert("alias.lifecycle.plan.renewal_quote_missing")
                    break
                }
                if quote.target != renewal.target ||
                    quote.quoteGuard != renewal.quoteGuard ||
                    quote.expiresAtMs != renewal.targetExpiryMs {
                    errors.insert("alias.lifecycle.plan.renewal_quote_mismatch")
                }
                if !amount(quote.exactAmount, isAtMost: quote.quoteGuard.maxAmount) {
                    errors.insert("alias.lifecycle.plan.quote_cap_invalid")
                }
                if quote.expiresAtMs > quote.graceExpiresAtMs ||
                    quote.graceExpiresAtMs > quote.redemptionExpiresAtMs {
                    errors.insert("alias.lifecycle.plan.quote_expiry_order_invalid")
                }
                if body.totalsByAsset.count != 1 ||
                    body.totalsByAsset[0].paymentAsset != quote.quoteGuard.expectedPaymentAsset ||
                    body.totalsByAsset[0].amount != quote.exactAmount {
                    errors.insert("alias.lifecycle.plan.renewal_total_mismatch")
                }
                if body.validUntilMs != quote.quoteGuard.validUntilMs {
                    errors.insert("alias.lifecycle.plan.renewal_deadline_mismatch")
                }
            case .configureAutoRenew:
                expectedWireId = ConfigureAliasAutoRenew.wireId
                if body.quote != nil || !body.totalsByAsset.isEmpty {
                    errors.insert("alias.lifecycle.plan.auto_renew_charge_invalid")
                }
            }
            if instruction.wireId != expectedWireId || instruction.framedPayload.isEmpty {
                errors.insert("alias.lifecycle.plan.instruction_invalid")
            }
        }
        return errors.sorted()
    }

    /// Requires canonical lifecycle shape, exact hash, and byte-identical frame re-encoding.
    public static func requireExecutable(
        _ plan: AliasLifecycleTransactionPlanV1,
        canonicalBodyNorito: Data,
        roundTrip: (String, Data) throws -> Data
    ) throws {
        var errors = validateExecutable(plan)
        if !verifyLifecycleHash(plan, canonicalBodyNorito: canonicalBodyNorito) {
            errors.append("alias.lifecycle.plan.hash_mismatch")
        }
        if let instruction = plan.body.instruction {
            let reencoded = try? roundTrip(instruction.wireId, instruction.framedPayload)
            if reencoded != instruction.framedPayload {
                errors.append("alias.lifecycle.plan.instruction_roundtrip_mismatch")
            }
        }
        if !errors.isEmpty {
            throw AliasSetupModelError.planValidation(Array(Set(errors)).sorted())
        }
    }

    /// Requires a lifecycle plan to preserve its signed request and exact typed frame.
    public static func requireExecutableForRequest(
        _ request: AliasLifecyclePlanRequestV1,
        plan: AliasLifecycleTransactionPlanV1,
        canonicalBodyNorito: Data,
        decodeAndReencode: (String, Data) throws -> DecodedAliasLifecycleFrame
    ) throws {
        var errors = validateExecutable(plan)
        if !verifyLifecycleHash(plan, canonicalBodyNorito: canonicalBodyNorito) {
            errors.append("alias.lifecycle.plan.hash_mismatch")
        }
        if plan.body.operation != request.operation {
            errors.append("alias.lifecycle.plan.signed_request_mismatch")
        }
        if let frame = plan.body.instruction {
            do {
                let decoded = try decodeAndReencode(frame.wireId, frame.framedPayload)
                if decoded.operation != plan.body.operation ||
                    decoded.reencodedFrame != frame.framedPayload {
                    errors.append("alias.lifecycle.plan.instruction_roundtrip_mismatch")
                }
            } catch {
                errors.append("alias.lifecycle.plan.instruction_roundtrip_mismatch")
            }
        }
        if !errors.isEmpty {
            throw AliasSetupModelError.planValidation(Array(Set(errors)).sorted())
        }
    }

    /// Registry-codec form of request-bound lifecycle-plan verification.
    public static func requireExecutableForRequest<Codec: AliasNoritoRegistryCodec>(
        _ request: AliasLifecyclePlanRequestV1,
        plan: AliasLifecycleTransactionPlanV1,
        canonicalBodyNorito: Data,
        codec: Codec
    ) throws {
        try requireExecutableForRequest(
            request,
            plan: plan,
            canonicalBodyNorito: canonicalBodyNorito,
            decodeAndReencode: codec.decodeAndReencodeLifecycle
        )
    }

    fileprivate static func isCanonicalQuantity(_ value: String) -> Bool {
        guard !value.isEmpty, value == value.trimmingCharacters(in: .whitespacesAndNewlines) else { return false }
        let parts = value.split(separator: ".", omittingEmptySubsequences: false)
        guard parts.count <= 2,
              !parts[0].isEmpty,
              parts[0].utf8.allSatisfy({ (48...57).contains($0) }),
              parts[0].count == 1 || parts[0].first != "0" else { return false }
        if parts.count == 2 {
            guard !parts[1].isEmpty,
                  parts[1].utf8.allSatisfy({ (48...57).contains($0) }),
                  parts[1].last != "0" else { return false }
        }
        return true
    }

    private static func amount(_ exact: String, isAtMost cap: String) -> Bool {
        guard isCanonicalQuantity(exact), isCanonicalQuantity(cap) else { return false }
        let exactParts = exact.split(separator: ".", omittingEmptySubsequences: false)
        let capParts = cap.split(separator: ".", omittingEmptySubsequences: false)
        let exactInteger = Array(exactParts[0].utf8)
        let capInteger = Array(capParts[0].utf8)
        if exactInteger.count != capInteger.count {
            return exactInteger.count < capInteger.count
        }
        if exactInteger != capInteger {
            return exactInteger.lexicographicallyPrecedes(capInteger)
        }
        let exactFraction = exactParts.count == 2 ? Array(exactParts[1].utf8) : []
        let capFraction = capParts.count == 2 ? Array(capParts[1].utf8) : []
        for index in 0..<max(exactFraction.count, capFraction.count) {
            let exactDigit = index < exactFraction.count ? exactFraction[index] : 48
            let capDigit = index < capFraction.count ? capFraction[index] : 48
            if exactDigit != capDigit { return exactDigit < capDigit }
        }
        return true
    }

    private static func totalsAreCanonical(_ totals: [AliasAssetTotalV1]) -> Bool {
        let keys = totals.compactMap { total -> [UInt8]? in
            guard AssetDefinitionAddressCodec.uuidBytes(total.paymentAsset) != nil,
                  isCanonicalQuantity(total.amount) else { return nil }
            return totalSortKey(total.paymentAsset, amount: total.amount)
        }
        guard keys.count == totals.count else { return false }
        return zip(keys, keys.dropFirst()).allSatisfy { previous, next in
            previous == next || previous.lexicographicallyPrecedes(next)
        }
    }

    private static func totalSortKey(_ paymentAsset: String, amount: String) -> [UInt8] {
        Array(AssetDefinitionAddressCodec.uuidBytes(paymentAsset) ?? Data())
            + [0]
            + Array(amount.utf8)
    }

    private static func addQuantities(_ lhs: String, _ rhs: String) -> String? {
        guard isCanonicalQuantity(lhs), isCanonicalQuantity(rhs) else { return nil }
        let lhsParts = lhs.split(separator: ".", omittingEmptySubsequences: false)
        let rhsParts = rhs.split(separator: ".", omittingEmptySubsequences: false)
        let lhsFraction = lhsParts.count == 2 ? Array(lhsParts[1].utf8) : []
        let rhsFraction = rhsParts.count == 2 ? Array(rhsParts[1].utf8) : []
        let scale = max(lhsFraction.count, rhsFraction.count)

        func scaledDigits(_ parts: [Substring], fraction: [UInt8]) -> [UInt8] {
            Array(parts[0].utf8).map { $0 - 48 }
                + fraction.map { $0 - 48 }
                + [UInt8](repeating: 0, count: scale - fraction.count)
        }

        let left = scaledDigits(lhsParts, fraction: lhsFraction)
        let right = scaledDigits(rhsParts, fraction: rhsFraction)
        let width = max(left.count, right.count)
        var result = [UInt8]()
        result.reserveCapacity(width + 1)
        var carry: UInt8 = 0
        for offset in 0..<width {
            let leftDigit = offset < left.count ? left[left.count - 1 - offset] : 0
            let rightDigit = offset < right.count ? right[right.count - 1 - offset] : 0
            let sum = leftDigit + rightDigit + carry
            result.append(sum % 10)
            carry = sum / 10
        }
        if carry > 0 { result.append(carry) }
        result.reverse()
        while result.count <= scale { result.insert(0, at: 0) }

        let integerCount = result.count - scale
        let integer = String(result[..<integerCount].map { Character(String($0)) })
        guard scale > 0 else { return integer }
        var fraction = result[integerCount...]
        while fraction.last == 0 { fraction = fraction.dropLast() }
        guard !fraction.isEmpty else { return integer }
        return integer + "." + String(fraction.map { Character(String($0)) })
    }

    private static func target(for intent: AliasIntentV1) -> AliasTargetV1 {
        switch intent {
        case let .dataspace(value): return .dataspace(value.dataspace)
        case let .domain(value): return .domain(value.domain)
        case let .accountAlias(value): return .accountAlias(value.alias)
        }
    }

    fileprivate static func isCanonicalHashText(_ text: String) -> Bool {
        decodeHash(text) != nil
    }

    fileprivate static func isCanonicalHashLiteral(_ text: String) -> Bool {
        (try? NetworkId(literal: text)) != nil
    }

    private static func decodeHash(_ text: String) -> Data? {
        var value = text
        for prefix in ["blake2b:", "hash:", "0x"] where value.hasPrefix(prefix) {
            value.removeFirst(prefix.count)
            break
        }
        if let checksum = value.firstIndex(of: "#") { value = String(value[..<checksum]) }
        guard value.count == 64 else { return nil }
        return Data(hexString: value)
    }
}
