import Foundation

/// Validation and decoding failures for the first-release Musubi wire surface.
public enum MusubiV1Error: Error, Equatable, Sendable {
    case invalidValue(String)
    case unsupportedVersion(String)
    case unknownFields(expected: [String], actual: [String])
}

extension MusubiV1Error: LocalizedError {
    public var errorDescription: String? {
        switch self {
        case .invalidValue(let message), .unsupportedVersion(let message):
            return message
        case let .unknownFields(expected, actual):
            return "Musubi V1 object fields differ: expected \(expected), got \(actual)."
        }
    }
}

private struct MusubiDynamicCodingKey: CodingKey, Hashable {
    let stringValue: String
    let intValue: Int? = nil

    init?(stringValue: String) { self.stringValue = stringValue }
    init?(intValue: Int) { return nil }
}

private func musubiRequireExactKeys(_ decoder: Decoder, _ expected: Set<String>) throws {
    let container = try decoder.container(keyedBy: MusubiDynamicCodingKey.self)
    let actual = Set(container.allKeys.map(\.stringValue))
    guard actual == expected else {
        throw MusubiV1Error.unknownFields(
            expected: expected.sorted(),
            actual: actual.sorted()
        )
    }
}

private func musubiRequireExactText(_ value: String, field: String) throws {
    guard !value.isEmpty,
          value == value.trimmingCharacters(in: .whitespacesAndNewlines),
          value.unicodeScalars.allSatisfy({ $0.properties.generalCategory != .control }) else {
        throw MusubiV1Error.invalidValue("\(field) must be exact non-empty text.")
    }
}

private func musubiIsBidiControl(_ scalar: Unicode.Scalar) -> Bool {
    switch scalar.value {
    case 0x061c, 0x200e, 0x200f, 0x202a...0x202e, 0x2066...0x2069:
        return true
    default:
        return false
    }
}

private func musubiRequireName(_ value: String, field: String) throws {
    try musubiRequireExactText(value, field: field)
    guard value.utf8.count <= 255,
          value.unicodeScalars.allSatisfy({ scalar in
              !CharacterSet.whitespacesAndNewlines.contains(scalar)
                  && !musubiIsBidiControl(scalar)
                  && scalar != "@" && scalar != "#" && scalar != "$"
          }),
          value.precomposedStringWithCanonicalMapping == value else {
        throw MusubiV1Error.invalidValue("\(field) is not a canonical Iroha Name.")
    }
}

private func musubiRequireASCIILowerKebab(
    _ value: String,
    maximum: Int,
    field: String
) throws {
    try musubiRequireExactText(value, field: field)
    let bytes = Array(value.utf8)
    guard bytes.count <= maximum,
          bytes.first != Character("-").asciiValue,
          bytes.last != Character("-").asciiValue,
          !value.contains("--"),
          bytes.allSatisfy({ (0x61...0x7a).contains($0) || (0x30...0x39).contains($0) || $0 == 0x2d }) else {
        throw MusubiV1Error.invalidValue("\(field) must be lowercase ASCII kebab text.")
    }
}

private func musubiRequireChainIDV1(_ value: String, field: String) throws {
    let bytes = Array(value.utf8)
    let isAlphaNumeric: (UInt8) -> Bool = {
        (0x30...0x39).contains($0) || (0x41...0x5a).contains($0)
            || (0x61...0x7a).contains($0)
    }
    guard (1...128).contains(bytes.count),
          bytes.first.map(isAlphaNumeric) == true,
          bytes.last.map(isAlphaNumeric) == true,
          bytes.allSatisfy({
              isAlphaNumeric($0) || $0 == 0x2e || $0 == 0x5f || $0 == 0x3a || $0 == 0x2d
          }) else {
        throw MusubiV1Error.invalidValue(
            "\(field) must be 1-128 bytes of canonical ASCII chain-id text."
        )
    }
}

private func musubiNormalizedSearchTerms(_ query: String) throws -> Set<String> {
    try musubiRequireExactText(query, field: "Musubi search query")
    guard query.utf8.count <= 256 else {
        throw MusubiV1Error.invalidValue("Musubi search query exceeds 256 UTF-8 bytes.")
    }
    var terms = Set<String>()
    for componentSlice in query.split(whereSeparator: { $0.isWhitespace }) {
        let component = String(componentSlice)
        let asciiComponent = component.utf8.count <= 64
            && component.unicodeScalars.allSatisfy { scalar in
                (0x41...0x5a).contains(scalar.value)
                    || (0x61...0x7a).contains(scalar.value)
                    || (0x30...0x39).contains(scalar.value)
                    || scalar.value == 0x2d
            }
        if asciiComponent { terms.insert(component.lowercased()) }
        let words = component.split { character in
            !character.unicodeScalars.allSatisfy(CharacterSet.alphanumerics.contains)
        }
        for word in words {
            let normalized = word.lowercased()
            guard normalized.utf8.count <= 64 else {
                throw MusubiV1Error.invalidValue("Musubi search term exceeds 64 UTF-8 bytes.")
            }
            terms.insert(normalized)
            guard terms.count <= 16 else {
                throw MusubiV1Error.invalidValue(
                    "Musubi search query exceeds 16 normalized terms."
                )
            }
        }
    }
    guard !terms.isEmpty, terms.count <= 16 else {
        throw MusubiV1Error.invalidValue("Musubi search query has no bounded normalized terms.")
    }
    return terms
}

private func musubiDecodeSingleText(_ decoder: Decoder, field: String) throws -> String {
    var container = try decoder.unkeyedContainer()
    let value = try container.decode(String.self)
    guard container.isAtEnd else {
        throw MusubiV1Error.invalidValue("\(field) must contain one Norito newtype item.")
    }
    return value
}

private func musubiEncodeSingleText(_ value: String, to encoder: Encoder) throws {
    var container = encoder.unkeyedContainer()
    try container.encode(value)
}

/// Canonical human-facing namespace (`dataspace` or `domain.dataspace`).
public struct MusubiNamespaceV1: Codable, Hashable, Sendable {
    public let value: String

    public init(_ value: String) throws {
        try musubiRequireExactText(value, field: "Musubi namespace")
        let segments = value.split(separator: ".", omittingEmptySubsequences: false)
        guard value.utf8.count <= 255,
              !value.contains("/"), !value.contains("@"), !value.contains(":"),
              (1...2).contains(segments.count) else {
            throw MusubiV1Error.invalidValue(
                "Musubi namespace must be dataspace or domain.dataspace."
            )
        }
        for segment in segments {
            try musubiRequireName(String(segment), field: "Musubi namespace segment")
        }
        self.value = value
    }

    public init(from decoder: Decoder) throws {
        try self.init(musubiDecodeSingleText(decoder, field: "Musubi namespace"))
    }

    public func encode(to encoder: Encoder) throws {
        try musubiEncodeSingleText(value, to: encoder)
    }
}

/// Canonical lowercase ASCII kebab package name.
public struct MusubiPackageNameV1: Codable, Hashable, Sendable {
    public let value: String

    public init(_ value: String) throws {
        try musubiRequireASCIILowerKebab(value, maximum: 64, field: "Musubi package name")
        self.value = value
    }

    public init(from decoder: Decoder) throws {
        try self.init(musubiDecodeSingleText(decoder, field: "Musubi package name"))
    }

    public func encode(to encoder: Encoder) throws {
        try musubiEncodeSingleText(value, to: encoder)
    }
}

/// Canonical permanent global alias name.
public struct MusubiAliasNameV1: Codable, Hashable, Sendable {
    public let value: String

    public init(_ value: String) throws {
        try musubiRequireASCIILowerKebab(value, maximum: 32, field: "Musubi alias")
        self.value = value
    }

    public init(from decoder: Decoder) throws {
        try self.init(musubiDecodeSingleText(decoder, field: "Musubi alias"))
    }

    public func encode(to encoder: Encoder) throws {
        try musubiEncodeSingleText(value, to: encoder)
    }
}

/// Bounded canonical public reason attached to a registry transition.
public struct MusubiReasonV1: Codable, Hashable, Sendable {
    public let value: String

    public init(_ value: String) throws {
        try musubiRequireExactText(value, field: "Musubi reason")
        guard value.utf8.count <= 1_024 else {
            throw MusubiV1Error.invalidValue("Musubi reason exceeds 1024 UTF-8 bytes.")
        }
        self.value = value
    }

    public init(from decoder: Decoder) throws {
        try self.init(musubiDecodeSingleText(decoder, field: "Musubi reason"))
    }

    public func encode(to encoder: Encoder) throws {
        try musubiEncodeSingleText(value, to: encoder)
    }
}

/// Independent permissions granted to a package maintainer.
public struct MusubiMaintainerPermissionsV1: Codable, Hashable, Sendable {
    public let publish: Bool
    public let yank: Bool
    public let metadata: Bool
    public let archiveLocations: Bool

    public init(
        publish: Bool,
        yank: Bool,
        metadata: Bool,
        archiveLocations: Bool
    ) throws {
        guard publish || yank || metadata || archiveLocations else {
            throw MusubiV1Error.invalidValue(
                "Musubi maintainer permissions must grant at least one capability."
            )
        }
        self.publish = publish
        self.yank = yank
        self.metadata = metadata
        self.archiveLocations = archiveLocations
    }

    private enum CodingKeys: String, CodingKey {
        case publish, yank, metadata
        case archiveLocations = "archive_locations"
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(
            decoder, ["publish", "yank", "metadata", "archive_locations"]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            publish: container.decode(Bool.self, forKey: .publish),
            yank: container.decode(Bool.self, forKey: .yank),
            metadata: container.decode(Bool.self, forKey: .metadata),
            archiveLocations: container.decode(Bool.self, forKey: .archiveLocations)
        )
    }
}

/// Owner or independently permissioned maintainer package role.
public enum MusubiPackageRoleV1: Codable, Hashable, Sendable {
    case owner
    case maintainer(MusubiMaintainerPermissionsV1)

    private enum CodingKeys: String, CodingKey { case kind, value }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["kind", "value"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "Owner":
            guard try container.decodeNil(forKey: .value) else {
                throw MusubiV1Error.invalidValue("Owner role value must be null.")
            }
            self = .owner
        case "Maintainer":
            self = .maintainer(
                try container.decode(MusubiMaintainerPermissionsV1.self, forKey: .value)
            )
        default:
            throw MusubiV1Error.unsupportedVersion("Unsupported Musubi V1 package role.")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .owner:
            try container.encode("Owner", forKey: .kind)
            try container.encodeNil(forKey: .value)
        case .maintainer(let permissions):
            try container.encode("Maintainer", forKey: .kind)
            try container.encode(permissions, forKey: .value)
        }
    }
}

/// Structural dataspace-root or domain package scope.
public enum MusubiPackageScopeV1: Codable, Hashable, Sendable {
    case dataspaceRoot
    case domain(String)

    private enum CodingKeys: String, CodingKey { case kind, value }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["kind", "value"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "DataspaceRoot":
            guard try container.decodeNil(forKey: .value) else {
                throw MusubiV1Error.invalidValue("DataspaceRoot scope value must be null.")
            }
            self = .dataspaceRoot
        case "Domain":
            let value = try container.decode(String.self, forKey: .value)
            try musubiRequireName(value, field: "Musubi package scope domain")
            self = .domain(value)
        default:
            throw MusubiV1Error.unsupportedVersion("Unsupported Musubi V1 package scope.")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .dataspaceRoot:
            try container.encode("DataspaceRoot", forKey: .kind)
            try container.encodeNil(forKey: .value)
        case .domain(let value):
            try musubiRequireName(value, field: "Musubi package scope domain")
            try container.encode("Domain", forKey: .kind)
            try container.encode(value, forKey: .value)
        }
    }
}

/// Stable structural package identity stored in releases and locks.
public struct MusubiPackageIdV1: Codable, Hashable, Sendable {
    public let homeDataspace: UInt64
    public let scope: MusubiPackageScopeV1
    public let name: MusubiPackageNameV1

    public init(
        homeDataspace: UInt64,
        scope: MusubiPackageScopeV1,
        name: MusubiPackageNameV1
    ) throws {
        if case .domain(let domain) = scope {
            try musubiRequireName(domain, field: "Musubi package scope domain")
        }
        self.homeDataspace = homeDataspace
        self.scope = scope
        self.name = name
    }

    private enum CodingKeys: String, CodingKey {
        case homeDataspace = "home_dataspace"
        case scope, name
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["home_dataspace", "scope", "name"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        homeDataspace = try container.decode(UInt64.self, forKey: .homeDataspace)
        scope = try container.decode(MusubiPackageScopeV1.self, forKey: .scope)
        name = try container.decode(MusubiPackageNameV1.self, forKey: .name)
    }
}

/// Immutable public namespace binding used to authorize first publication.
public struct MusubiNamespaceBindingV1: Codable, Hashable, Sendable {
    public let namespace: MusubiNamespaceV1
    public let homeDataspace: UInt64
    public let scope: MusubiPackageScopeV1
    public let generation: UInt64

    public init(
        namespace: MusubiNamespaceV1,
        homeDataspace: UInt64,
        scope: MusubiPackageScopeV1,
        generation: UInt64
    ) throws {
        guard generation > 0 else {
            throw MusubiV1Error.invalidValue(
                "Musubi namespace binding generation must be non-zero."
            )
        }
        let segments = namespace.value.split(separator: ".", omittingEmptySubsequences: false)
        switch scope {
        case .dataspaceRoot where segments.count == 1:
            break
        case .domain(let domain) where segments.count == 2 && segments[0] == domain:
            break
        default:
            throw MusubiV1Error.invalidValue(
                "Musubi namespace binding text and scope disagree."
            )
        }
        self.namespace = namespace
        self.homeDataspace = homeDataspace
        self.scope = scope
        self.generation = generation
    }

    private enum CodingKeys: String, CodingKey {
        case namespace
        case homeDataspace = "home_dataspace"
        case scope, generation
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(
            decoder, ["namespace", "home_dataspace", "scope", "generation"]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            namespace: container.decode(MusubiNamespaceV1.self, forKey: .namespace),
            homeDataspace: container.decode(UInt64.self, forKey: .homeDataspace),
            scope: container.decode(MusubiPackageScopeV1.self, forKey: .scope),
            generation: container.decode(UInt64.self, forKey: .generation)
        )
    }
}

/// Public namespace/package selector.
public struct MusubiPackageSelectorV1: Codable, Hashable, Sendable {
    public let namespace: MusubiNamespaceV1
    public let name: MusubiPackageNameV1

    public init(namespace: MusubiNamespaceV1, name: MusubiPackageNameV1) {
        self.namespace = namespace
        self.name = name
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["namespace", "name"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        namespace = try container.decode(MusubiNamespaceV1.self, forKey: .namespace)
        name = try container.decode(MusubiPackageNameV1.self, forKey: .name)
    }

    private enum CodingKeys: String, CodingKey { case namespace, name }
}

/// One canonical SemVer prerelease identifier.
public enum MusubiPrereleaseIdentifierV1: Codable, Hashable, Sendable, Comparable {
    case numeric(UInt64)
    case alphaNumeric(String)

    private enum CodingKeys: String, CodingKey { case kind, value }

    public static func parse(_ value: String) throws -> Self {
        guard !value.isEmpty, value.utf8.count <= 64 else {
            throw MusubiV1Error.invalidValue("Musubi prerelease identifier is out of bounds.")
        }
        if value.utf8.allSatisfy({ (0x30...0x39).contains($0) }) {
            guard value.count == 1 || !value.hasPrefix("0"), let numeric = UInt64(value) else {
                throw MusubiV1Error.invalidValue(
                    "Musubi numeric prerelease identifier is noncanonical."
                )
            }
            return .numeric(numeric)
        }
        guard value.utf8.allSatisfy({ byte in
            (0x30...0x39).contains(byte) || (0x41...0x5a).contains(byte)
                || (0x61...0x7a).contains(byte) || byte == 0x2d
        }) else {
            throw MusubiV1Error.invalidValue(
                "Musubi prerelease identifier must be ASCII alphanumeric or hyphen."
            )
        }
        return .alphaNumeric(value)
    }

    public var canonicalText: String {
        switch self {
        case .numeric(let value): return String(value)
        case .alphaNumeric(let value): return value
        }
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["kind", "value"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "Numeric": self = .numeric(try container.decode(UInt64.self, forKey: .value))
        case "AlphaNumeric":
            let value = try container.decode(String.self, forKey: .value)
            guard case .alphaNumeric = try Self.parse(value) else {
                throw MusubiV1Error.invalidValue("Alphanumeric prerelease value is numeric.")
            }
            self = .alphaNumeric(value)
        default:
            throw MusubiV1Error.unsupportedVersion("Unsupported Musubi V1 prerelease tag.")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .numeric(let value):
            try container.encode("Numeric", forKey: .kind)
            try container.encode(value, forKey: .value)
        case .alphaNumeric(let value):
            guard case .alphaNumeric = try Self.parse(value) else {
                throw MusubiV1Error.invalidValue("Alphanumeric prerelease value is numeric.")
            }
            try container.encode("AlphaNumeric", forKey: .kind)
            try container.encode(value, forKey: .value)
        }
    }

    public static func < (lhs: Self, rhs: Self) -> Bool {
        switch (lhs, rhs) {
        case let (.numeric(left), .numeric(right)): return left < right
        case (.numeric, .alphaNumeric): return true
        case (.alphaNumeric, .numeric): return false
        case let (.alphaNumeric(left), .alphaNumeric(right)): return left < right
        }
    }
}

/// Structured canonical SemVer. Build metadata is deliberately unsupported.
public struct MusubiVersionV1: Codable, Hashable, Sendable, Comparable {
    public let major: UInt64
    public let minor: UInt64
    public let patch: UInt64
    public let prerelease: [MusubiPrereleaseIdentifierV1]

    public init(
        major: UInt64,
        minor: UInt64,
        patch: UInt64,
        prerelease: [MusubiPrereleaseIdentifierV1] = []
    ) throws {
        guard prerelease.count <= 16 else {
            throw MusubiV1Error.invalidValue("Musubi version has too many prerelease identifiers.")
        }
        for identifier in prerelease {
            if case .alphaNumeric(let value) = identifier {
                guard try MusubiPrereleaseIdentifierV1.parse(value) == identifier else {
                    throw MusubiV1Error.invalidValue(
                        "Musubi alphanumeric prerelease identifier is noncanonical."
                    )
                }
            }
        }
        self.major = major
        self.minor = minor
        self.patch = patch
        self.prerelease = prerelease
    }

    public static func parse(_ value: String) throws -> Self {
        try musubiRequireExactText(value, field: "Musubi version")
        guard !value.contains("+") else {
            throw MusubiV1Error.invalidValue("Musubi V1 versions reject build metadata.")
        }
        let split = value.split(separator: "-", maxSplits: 1, omittingEmptySubsequences: false)
        let core = split[0].split(separator: ".", omittingEmptySubsequences: false)
        guard core.count == 3 else {
            throw MusubiV1Error.invalidValue("Musubi version must use MAJOR.MINOR.PATCH.")
        }
        let numbers = try core.map { component -> UInt64 in
            let text = String(component)
            guard !text.isEmpty,
                  text.count == 1 || !text.hasPrefix("0"),
                  text.utf8.allSatisfy({ (0x30...0x39).contains($0) }),
                  let parsed = UInt64(text) else {
                throw MusubiV1Error.invalidValue("Musubi version component is noncanonical.")
            }
            return parsed
        }
        let prerelease: [MusubiPrereleaseIdentifierV1]
        if split.count == 1 {
            prerelease = []
        } else {
            guard !split[1].isEmpty else {
                throw MusubiV1Error.invalidValue("Musubi prerelease must not be empty.")
            }
            prerelease = try split[1]
                .split(separator: ".", omittingEmptySubsequences: false)
                .map { try MusubiPrereleaseIdentifierV1.parse(String($0)) }
        }
        return try Self(
            major: numbers[0], minor: numbers[1], patch: numbers[2], prerelease: prerelease
        )
    }

    public var canonicalText: String {
        var result = "\(major).\(minor).\(patch)"
        if !prerelease.isEmpty {
            result += "-" + prerelease.map(\.canonicalText).joined(separator: ".")
        }
        return result
    }

    private enum CodingKeys: String, CodingKey { case major, minor, patch, prerelease }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["major", "minor", "patch", "prerelease"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            major: container.decode(UInt64.self, forKey: .major),
            minor: container.decode(UInt64.self, forKey: .minor),
            patch: container.decode(UInt64.self, forKey: .patch),
            prerelease: container.decode([MusubiPrereleaseIdentifierV1].self, forKey: .prerelease)
        )
    }

    public static func < (lhs: Self, rhs: Self) -> Bool {
        if lhs.major != rhs.major { return lhs.major < rhs.major }
        if lhs.minor != rhs.minor { return lhs.minor < rhs.minor }
        if lhs.patch != rhs.patch { return lhs.patch < rhs.patch }
        if lhs.prerelease.isEmpty != rhs.prerelease.isEmpty {
            return !lhs.prerelease.isEmpty
        }
        for index in 0..<min(lhs.prerelease.count, rhs.prerelease.count) {
            if lhs.prerelease[index] != rhs.prerelease[index] {
                return lhs.prerelease[index] < rhs.prerelease[index]
            }
        }
        return lhs.prerelease.count < rhs.prerelease.count
    }
}

/// Comparator operator in the canonical requirement AST.
public enum MusubiComparatorOpV1: Int, Codable, Hashable, Sendable, Comparable {
    case greater
    case greaterOrEqual
    case less
    case lessOrEqual
    case equal

    private enum CodingKeys: String, CodingKey { case kind, value }

    fileprivate var wireName: String {
        switch self {
        case .greater: return "Greater"
        case .greaterOrEqual: return "GreaterOrEqual"
        case .less: return "Less"
        case .lessOrEqual: return "LessOrEqual"
        case .equal: return "Equal"
        }
    }

    fileprivate var token: String {
        switch self {
        case .greater: return ">"
        case .greaterOrEqual: return ">="
        case .less: return "<"
        case .lessOrEqual: return "<="
        case .equal: return "="
        }
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["kind", "value"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        guard try container.decodeNil(forKey: .value) else {
            throw MusubiV1Error.invalidValue("Musubi comparator tag value must be null.")
        }
        switch try container.decode(String.self, forKey: .kind) {
        case "Greater": self = .greater
        case "GreaterOrEqual": self = .greaterOrEqual
        case "Less": self = .less
        case "LessOrEqual": self = .lessOrEqual
        case "Equal": self = .equal
        default: throw MusubiV1Error.unsupportedVersion("Unsupported Musubi V1 comparator.")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(wireName, forKey: .kind)
        try container.encodeNil(forKey: .value)
    }

    public static func < (lhs: Self, rhs: Self) -> Bool { lhs.rawValue < rhs.rawValue }
}

/// One complete comparator node.
public struct MusubiVersionComparatorV1: Codable, Hashable, Sendable, Comparable {
    public let op: MusubiComparatorOpV1
    public let version: MusubiVersionV1

    public init(op: MusubiComparatorOpV1, version: MusubiVersionV1) {
        self.op = op
        self.version = version
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["op", "version"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        op = try container.decode(MusubiComparatorOpV1.self, forKey: .op)
        version = try container.decode(MusubiVersionV1.self, forKey: .version)
    }

    private enum CodingKeys: String, CodingKey { case op, version }

    public static func < (lhs: Self, rhs: Self) -> Bool {
        lhs.op != rhs.op ? lhs.op < rhs.op : lhs.version < rhs.version
    }
}

/// Canonical Cargo-style requirement AST used in published dependencies.
public enum MusubiVersionReqV1: Codable, Hashable, Sendable {
    case any
    case caret(MusubiVersionV1)
    case tilde(MusubiVersionV1)
    case majorWildcard(UInt64)
    case minorWildcard(major: UInt64, minor: UInt64)
    case exact(MusubiVersionV1)
    case comparators([MusubiVersionComparatorV1])

    private enum CodingKeys: String, CodingKey { case kind, value }
    private struct MinorWildcard: Codable {
        let major: UInt64
        let minor: UInt64
        init(major: UInt64, minor: UInt64) {
            self.major = major
            self.minor = minor
        }
        init(from decoder: Decoder) throws {
            try musubiRequireExactKeys(decoder, ["major", "minor"])
            let container = try decoder.container(keyedBy: CodingKeys.self)
            major = try container.decode(UInt64.self, forKey: .major)
            minor = try container.decode(UInt64.self, forKey: .minor)
        }
        private enum CodingKeys: String, CodingKey { case major, minor }
    }

    public static func parse(_ value: String) throws -> Self {
        try musubiRequireExactText(value, field: "Musubi version requirement")
        if value == "*" { return .any }
        if value.hasPrefix("="), !value.contains(",") {
            return .exact(try MusubiVersionV1.parse(String(value.dropFirst())))
        }
        if value.contains(",") || value.hasPrefix(">") || value.hasPrefix("<") {
            var comparators = try value.split(separator: ",", omittingEmptySubsequences: false)
                .map {
                    try parseComparator(
                        String($0).trimmingCharacters(
                            in: CharacterSet(charactersIn: " ")
                        )
                    )
                }
            comparators.sort()
            comparators = Array(Set(comparators)).sorted()
            guard !comparators.isEmpty, comparators.count <= 16 else {
                throw MusubiV1Error.invalidValue("Musubi comparator list is empty or oversized.")
            }
            let exacts = Set(comparators.filter { $0.op == .equal }.map(\.version))
            guard exacts.count <= 1 else {
                throw MusubiV1Error.invalidValue("Musubi comparator list has conflicting exacts.")
            }
            if comparators.count == 1, comparators[0].op == .equal {
                return .exact(comparators[0].version)
            }
            return .comparators(comparators)
        }
        if value.hasPrefix("^") {
            return .caret(try MusubiVersionV1.parse(String(value.dropFirst())))
        }
        if value.hasPrefix("~") {
            return .tilde(try MusubiVersionV1.parse(String(value.dropFirst())))
        }
        if value.hasSuffix(".*") {
            let components = value.dropLast(2).split(separator: ".", omittingEmptySubsequences: false)
            let parsed = try components.map { component -> UInt64 in
                let text = String(component)
                guard !text.isEmpty,
                      text.count == 1 || !text.hasPrefix("0"),
                      text.utf8.allSatisfy({ (0x30...0x39).contains($0) }),
                      let number = UInt64(text) else {
                    throw MusubiV1Error.invalidValue("Musubi wildcard is noncanonical.")
                }
                return number
            }
            if parsed.count == 1 { return .majorWildcard(parsed[0]) }
            if parsed.count == 2 { return .minorWildcard(major: parsed[0], minor: parsed[1]) }
            throw MusubiV1Error.invalidValue("Musubi wildcard must be MAJOR.* or MAJOR.MINOR.*.")
        }
        return .caret(try MusubiVersionV1.parse(value))
    }

    private static func parseComparator(_ value: String) throws -> MusubiVersionComparatorV1 {
        let pair: (MusubiComparatorOpV1, String)
        if value.hasPrefix(">=") { pair = (.greaterOrEqual, String(value.dropFirst(2))) }
        else if value.hasPrefix("<=") { pair = (.lessOrEqual, String(value.dropFirst(2))) }
        else if value.hasPrefix(">") { pair = (.greater, String(value.dropFirst())) }
        else if value.hasPrefix("<") { pair = (.less, String(value.dropFirst())) }
        else if value.hasPrefix("=") { pair = (.equal, String(value.dropFirst())) }
        else { throw MusubiV1Error.invalidValue("Musubi comparator has no supported operator.") }
        return MusubiVersionComparatorV1(op: pair.0, version: try MusubiVersionV1.parse(pair.1))
    }

    public var canonicalText: String {
        switch self {
        case .any: return "*"
        case .caret(let version): return "^\(version.canonicalText)"
        case .tilde(let version): return "~\(version.canonicalText)"
        case .majorWildcard(let major): return "\(major).*"
        case let .minorWildcard(major, minor): return "\(major).\(minor).*"
        case .exact(let version): return "=\(version.canonicalText)"
        case .comparators(let values):
            return values.map { $0.op.token + $0.version.canonicalText }.joined(separator: ",")
        }
    }

    /// Returns whether an exact version satisfies this requirement under Cargo prerelease rules.
    public func matches(_ candidate: MusubiVersionV1) -> Bool {
        musubiRequirementMatchesV1(self, version: candidate)
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["kind", "value"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "Any":
            guard try container.decodeNil(forKey: .value) else {
                throw MusubiV1Error.invalidValue("Musubi Any requirement value must be null.")
            }
            self = .any
        case "Caret": self = .caret(try container.decode(MusubiVersionV1.self, forKey: .value))
        case "Tilde": self = .tilde(try container.decode(MusubiVersionV1.self, forKey: .value))
        case "MajorWildcard": self = .majorWildcard(try container.decode(UInt64.self, forKey: .value))
        case "MinorWildcard":
            let value = try container.decode(MinorWildcard.self, forKey: .value)
            self = .minorWildcard(major: value.major, minor: value.minor)
        case "Exact": self = .exact(try container.decode(MusubiVersionV1.self, forKey: .value))
        case "Comparators":
            let values = try container.decode([MusubiVersionComparatorV1].self, forKey: .value)
            guard !values.isEmpty, values.count <= 16,
                  values == Array(Set(values)).sorted(),
                  !(values.count == 1 && values[0].op == .equal),
                  values.filter({ $0.op == .equal }).count <= 1 else {
                throw MusubiV1Error.invalidValue("Musubi comparator AST is noncanonical.")
            }
            self = .comparators(values)
        default:
            throw MusubiV1Error.unsupportedVersion("Unsupported Musubi V1 requirement tag.")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .any:
            try container.encode("Any", forKey: .kind)
            try container.encodeNil(forKey: .value)
        case .caret(let value):
            try container.encode("Caret", forKey: .kind); try container.encode(value, forKey: .value)
        case .tilde(let value):
            try container.encode("Tilde", forKey: .kind); try container.encode(value, forKey: .value)
        case .majorWildcard(let value):
            try container.encode("MajorWildcard", forKey: .kind); try container.encode(value, forKey: .value)
        case let .minorWildcard(major, minor):
            try container.encode("MinorWildcard", forKey: .kind)
            try container.encode(MinorWildcard(major: major, minor: minor), forKey: .value)
        case .exact(let value):
            try container.encode("Exact", forKey: .kind); try container.encode(value, forKey: .value)
        case .comparators(let value):
            guard !value.isEmpty, value.count <= 16,
                  value == Array(Set(value)).sorted(),
                  !(value.count == 1 && value[0].op == .equal),
                  value.filter({ $0.op == .equal }).count <= 1 else {
                throw MusubiV1Error.invalidValue("Musubi comparator AST is noncanonical.")
            }
            try container.encode("Comparators", forKey: .kind); try container.encode(value, forKey: .value)
        }
    }
}

/// Exact release identity.
public struct MusubiReleaseIdV1: Codable, Hashable, Sendable {
    public let package: MusubiPackageIdV1
    public let version: MusubiVersionV1

    public init(package: MusubiPackageIdV1, version: MusubiVersionV1) {
        self.package = package
        self.version = version
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["package", "version"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        package = try container.decode(MusubiPackageIdV1.self, forKey: .package)
        version = try container.decode(MusubiVersionV1.self, forKey: .version)
    }

    private enum CodingKeys: String, CodingKey { case package, version }
}

/// Canonical Norito JSON wrapper for a Musubi 32-byte digest newtype.
public struct MusubiDigest32V1: Codable, Hashable, Sendable {
    public let bytes: [UInt8]

    public init(bytes: [UInt8]) throws {
        guard bytes.count == 32 else {
            throw MusubiV1Error.invalidValue("Musubi digest must contain exactly 32 bytes.")
        }
        self.bytes = bytes
    }

    public init(from decoder: Decoder) throws {
        var outer = try decoder.unkeyedContainer()
        let bytes = try outer.decode([UInt8].self)
        guard outer.isAtEnd else {
            throw MusubiV1Error.invalidValue("Musubi digest must contain one Norito newtype item.")
        }
        try self.init(bytes: bytes)
    }

    public func encode(to encoder: Encoder) throws {
        guard bytes.count == 32 else {
            throw MusubiV1Error.invalidValue("Musubi digest must contain exactly 32 bytes.")
        }
        var outer = encoder.unkeyedContainer()
        try outer.encode(bytes)
    }
}

/// Enacted Parliament decision authorizing one delayed Musubi governance action.
public struct MusubiGovernanceDecisionV1: Codable, Hashable, Sendable {
    /// Exact enacted proposal fingerprint. Unlike `actionDigest`, this is a fixed byte array,
    /// not a Norito digest newtype.
    public let decisionID: [UInt8]
    /// Domain-separated digest of the exact Parliament action parameters.
    public let actionDigest: MusubiDigest32V1
    /// Finalized height at which Parliament enacted the decision.
    public let enactedAtHeight: UInt64
    /// First finalized height at which the delayed decision may execute.
    public let executeAfterHeight: UInt64

    public init(
        decisionID: [UInt8],
        actionDigest: MusubiDigest32V1,
        enactedAtHeight: UInt64,
        executeAfterHeight: UInt64
    ) throws {
        guard decisionID.count == 32,
              decisionID.contains(where: { $0 != 0 }) else {
            throw MusubiV1Error.invalidValue(
                "Musubi governance decision ID must be a non-zero 32-byte value."
            )
        }
        guard actionDigest.bytes.contains(where: { $0 != 0 }) else {
            throw MusubiV1Error.invalidValue(
                "Musubi governance action digest must be non-zero."
            )
        }
        guard enactedAtHeight > 0,
              executeAfterHeight > enactedAtHeight else {
            throw MusubiV1Error.invalidValue(
                "Musubi governance execution height must follow a non-zero enactment height."
            )
        }
        self.decisionID = decisionID
        self.actionDigest = actionDigest
        self.enactedAtHeight = enactedAtHeight
        self.executeAfterHeight = executeAfterHeight
    }

    private enum CodingKeys: String, CodingKey {
        case decisionID = "decision_id"
        case actionDigest = "action_digest"
        case enactedAtHeight = "enacted_at_height"
        case executeAfterHeight = "execute_after_height"
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(
            decoder,
            ["decision_id", "action_digest", "enacted_at_height", "execute_after_height"]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            decisionID: container.decode([UInt8].self, forKey: .decisionID),
            actionDigest: container.decode(MusubiDigest32V1.self, forKey: .actionDigest),
            enactedAtHeight: container.decode(UInt64.self, forKey: .enactedAtHeight),
            executeAfterHeight: container.decode(UInt64.self, forKey: .executeAfterHeight)
        )
    }
}

/// Finalized universal registry snapshot bound into page cursors.
public struct MusubiRegistrySnapshotV1: Codable, Hashable, Sendable {
    public let finalizedHeight: UInt64
    public let finalizedBlockHash: [UInt8]
    public let indexRevision: UInt64

    public init(finalizedHeight: UInt64, finalizedBlockHash: [UInt8], indexRevision: UInt64) throws {
        guard finalizedHeight > 0, indexRevision > 0,
              finalizedBlockHash.count == 32,
              finalizedBlockHash.contains(where: { $0 != 0 }) else {
            throw MusubiV1Error.invalidValue("Musubi snapshot anchors must be non-inert.")
        }
        self.finalizedHeight = finalizedHeight
        self.finalizedBlockHash = finalizedBlockHash
        self.indexRevision = indexRevision
    }

    private enum CodingKeys: String, CodingKey {
        case finalizedHeight = "finalized_height"
        case finalizedBlockHash = "finalized_block_hash"
        case indexRevision = "index_revision"
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(
            decoder, ["finalized_height", "finalized_block_hash", "index_revision"]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            finalizedHeight: container.decode(UInt64.self, forKey: .finalizedHeight),
            finalizedBlockHash: container.decode([UInt8].self, forKey: .finalizedBlockHash),
            indexRevision: container.decode(UInt64.self, forKey: .indexRevision)
        )
    }
}

/// Opaque finalized cursor whose key and query binding remain server-owned.
public struct MusubiFinalizedCursorV1: Codable, Hashable, Sendable {
    public let snapshot: MusubiRegistrySnapshotV1
    public let queryHash: MusubiDigest32V1
    public let lastKey: String
    public let caller: String?

    public init(
        snapshot: MusubiRegistrySnapshotV1,
        queryHash: MusubiDigest32V1,
        lastKey: String,
        caller: String?
    ) throws {
        try musubiRequireExactText(lastKey, field: "Musubi cursor last key")
        guard lastKey.utf8.count <= 512,
              queryHash.bytes.contains(where: { $0 != 0 }) else {
            throw MusubiV1Error.invalidValue("Musubi cursor key or query hash is invalid.")
        }
        if let caller { try musubiRequireExactText(caller, field: "Musubi cursor caller") }
        self.snapshot = snapshot
        self.queryHash = queryHash
        self.lastKey = lastKey
        self.caller = caller
    }

    private enum CodingKeys: String, CodingKey {
        case snapshot
        case queryHash = "query_hash"
        case lastKey = "last_key"
        case caller
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["snapshot", "query_hash", "last_key", "caller"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            snapshot: container.decode(MusubiRegistrySnapshotV1.self, forKey: .snapshot),
            queryHash: container.decode(MusubiDigest32V1.self, forKey: .queryHash),
            lastKey: container.decode(String.self, forKey: .lastKey),
            caller: container.decodeIfPresent(String.self, forKey: .caller)
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(snapshot, forKey: .snapshot)
        try container.encode(queryHash, forKey: .queryHash)
        try container.encode(lastKey, forKey: .lastKey)
        if let caller { try container.encode(caller, forKey: .caller) }
        else { try container.encodeNil(forKey: .caller) }
    }
}

/// Shared bounded page request.
public struct MusubiPageRequestV1: Codable, Hashable, Sendable {
    public let limit: UInt32
    public let cursor: MusubiFinalizedCursorV1?

    public init(limit: UInt32 = 50, cursor: MusubiFinalizedCursorV1? = nil) {
        self.limit = limit
        self.cursor = cursor
    }

    private enum CodingKeys: String, CodingKey { case limit, cursor }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["limit", "cursor"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        limit = try container.decode(UInt32.self, forKey: .limit)
        cursor = try container.decodeIfPresent(MusubiFinalizedCursorV1.self, forKey: .cursor)
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(limit, forKey: .limit)
        if let cursor { try container.encode(cursor, forKey: .cursor) }
        else { try container.encodeNil(forKey: .cursor) }
    }
}

/// Exact package query body.
public struct MusubiExactPackageQueryV1: Codable, Hashable, Sendable {
    public let package: MusubiPackageIdV1
    public init(package: MusubiPackageIdV1) { self.package = package }
    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["package"])
        package = try decoder.container(keyedBy: CodingKeys.self)
            .decode(MusubiPackageIdV1.self, forKey: .package)
    }
    private enum CodingKeys: String, CodingKey { case package }
}

/// Exact release query body.
public struct MusubiExactReleaseQueryV1: Codable, Hashable, Sendable {
    public let release: MusubiReleaseIdV1
    public init(release: MusubiReleaseIdV1) { self.release = release }
    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["release"])
        release = try decoder.container(keyedBy: CodingKeys.self)
            .decode(MusubiReleaseIdV1.self, forKey: .release)
    }
    private enum CodingKeys: String, CodingKey { case release }
}

/// Sparse resolver-index query body.
public struct MusubiResolverIndexQueryV1: Codable, Hashable, Sendable {
    public let package: MusubiPackageIdV1
    public let requirement: MusubiVersionReqV1?
    public let page: MusubiPageRequestV1

    public init(
        package: MusubiPackageIdV1,
        requirement: MusubiVersionReqV1? = nil,
        page: MusubiPageRequestV1 = .init()
    ) {
        self.package = package; self.requirement = requirement; self.page = page
    }

    private enum CodingKeys: String, CodingKey { case package, requirement, page }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["package", "requirement", "page"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        package = try container.decode(MusubiPackageIdV1.self, forKey: .package)
        requirement = try container.decodeIfPresent(MusubiVersionReqV1.self, forKey: .requirement)
        page = try container.decode(MusubiPageRequestV1.self, forKey: .page)
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(package, forKey: .package)
        if let requirement { try container.encode(requirement, forKey: .requirement) }
        else { try container.encodeNil(forKey: .requirement) }
        try container.encode(page, forKey: .page)
    }
}

/// Package-scoped page query used by versions and maintainers.
public struct MusubiPackagePageQueryV1: Codable, Hashable, Sendable {
    public let package: MusubiPackageIdV1
    public let page: MusubiPageRequestV1
    public init(package: MusubiPackageIdV1, page: MusubiPageRequestV1 = .init()) {
        self.package = package; self.page = page
    }
    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["package", "page"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        package = try container.decode(MusubiPackageIdV1.self, forKey: .package)
        page = try container.decode(MusubiPageRequestV1.self, forKey: .page)
    }
    private enum CodingKeys: String, CodingKey { case package, page }
}

/// Archive-location page query body.
public struct MusubiArchiveLocationQueryV1: Codable, Hashable, Sendable {
    public let archiveId: MusubiDigest32V1
    public let page: MusubiPageRequestV1
    public init(archiveId: MusubiDigest32V1, page: MusubiPageRequestV1 = .init()) {
        self.archiveId = archiveId; self.page = page
    }
    private enum CodingKeys: String, CodingKey { case archiveId = "archive_id", page }
    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["archive_id", "page"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        archiveId = try container.decode(MusubiDigest32V1.self, forKey: .archiveId)
        page = try container.decode(MusubiPageRequestV1.self, forKey: .page)
    }
}

/// Fresh-selection availability for one finalized archive projection.
public enum MusubiStorageAvailabilityV1: Codable, Hashable, Sendable {
    case selectable
    case belowQuorum
    case unavailable

    private enum CodingKeys: String, CodingKey { case kind, value }

    private var wireKind: String {
        switch self {
        case .selectable: return "Selectable"
        case .belowQuorum: return "BelowQuorum"
        case .unavailable: return "Unavailable"
        }
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["kind", "value"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        guard try container.decodeNil(forKey: .value) else {
            throw MusubiV1Error.invalidValue(
                "Musubi storage-availability tag value must be null."
            )
        }
        switch try container.decode(String.self, forKey: .kind) {
        case "Selectable": self = .selectable
        case "BelowQuorum": self = .belowQuorum
        case "Unavailable": self = .unavailable
        default:
            throw MusubiV1Error.unsupportedVersion(
                "Unsupported Musubi V1 storage availability."
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(wireKind, forKey: .kind)
        try container.encodeNil(forKey: .value)
    }
}

/// Finalized aggregate storage projection carried by retention decisions.
public struct MusubiArchiveAvailabilityV1: Codable, Hashable, Sendable {
    public let archiveId: MusubiDigest32V1
    public let availability: MusubiStorageAvailabilityV1
    public let healthyReplicas: UInt16
    public let activeLocations: UInt8
    public let finalizedHeight: UInt64
    public let finalizedBlockHash: [UInt8]
    public let indexRevision: UInt64

    public init(
        archiveId: MusubiDigest32V1,
        availability: MusubiStorageAvailabilityV1,
        healthyReplicas: UInt16,
        activeLocations: UInt8,
        finalizedHeight: UInt64,
        finalizedBlockHash: [UInt8],
        indexRevision: UInt64
    ) throws {
        let healthyCapacity = Int(activeLocations) * 64
        let expected: MusubiStorageAvailabilityV1
        if healthyReplicas >= 3 { expected = .selectable }
        else if activeLocations > 0 && healthyReplicas > 0 { expected = .belowQuorum }
        else { expected = .unavailable }
        guard archiveId.bytes.contains(where: { $0 != 0 }), activeLocations <= 4,
              Int(healthyReplicas) <= healthyCapacity,
              finalizedHeight > 0, indexRevision > 0,
              finalizedBlockHash.count == 32,
              finalizedBlockHash.contains(where: { $0 != 0 }),
              availability == expected else {
            throw MusubiV1Error.invalidValue(
                "Musubi archive availability record is invalid."
            )
        }
        self.archiveId = archiveId
        self.availability = availability
        self.healthyReplicas = healthyReplicas
        self.activeLocations = activeLocations
        self.finalizedHeight = finalizedHeight
        self.finalizedBlockHash = finalizedBlockHash
        self.indexRevision = indexRevision
    }

    private enum CodingKeys: String, CodingKey {
        case archiveId = "archive_id"
        case availability
        case healthyReplicas = "healthy_replicas"
        case activeLocations = "active_locations"
        case finalizedHeight = "finalized_height"
        case finalizedBlockHash = "finalized_block_hash"
        case indexRevision = "index_revision"
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(
            decoder,
            [
                "archive_id", "availability", "healthy_replicas", "active_locations",
                "finalized_height", "finalized_block_hash", "index_revision"
            ]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            archiveId: container.decode(MusubiDigest32V1.self, forKey: .archiveId),
            availability: container.decode(
                MusubiStorageAvailabilityV1.self,
                forKey: .availability
            ),
            healthyReplicas: container.decode(UInt16.self, forKey: .healthyReplicas),
            activeLocations: container.decode(UInt8.self, forKey: .activeLocations),
            finalizedHeight: container.decode(UInt64.self, forKey: .finalizedHeight),
            finalizedBlockHash: container.decode([UInt8].self, forKey: .finalizedBlockHash),
            indexRevision: container.decode(UInt64.self, forKey: .indexRevision)
        )
    }
}

/// Authoritative cache-retention classification for one exact archive.
public enum MusubiArchiveRetentionDispositionV1: Codable, Hashable, Sendable {
    case retainUnknown
    case retainReferenced
    case pruneUnreferenced
    case pruneGovernedTakedown

    private enum CodingKeys: String, CodingKey { case kind, value }

    public var mustRetain: Bool {
        self == .retainUnknown || self == .retainReferenced
    }

    private var wireKind: String {
        switch self {
        case .retainUnknown: return "RetainUnknown"
        case .retainReferenced: return "RetainReferenced"
        case .pruneUnreferenced: return "PruneUnreferenced"
        case .pruneGovernedTakedown: return "PruneGovernedTakedown"
        }
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["kind", "value"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        guard try container.decodeNil(forKey: .value) else {
            throw MusubiV1Error.invalidValue(
                "Musubi archive-retention disposition value must be null."
            )
        }
        switch try container.decode(String.self, forKey: .kind) {
        case "RetainUnknown": self = .retainUnknown
        case "RetainReferenced": self = .retainReferenced
        case "PruneUnreferenced": self = .pruneUnreferenced
        case "PruneGovernedTakedown": self = .pruneGovernedTakedown
        default:
            throw MusubiV1Error.unsupportedVersion(
                "Unsupported Musubi V1 archive-retention disposition."
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(wireKind, forKey: .kind)
        try container.encodeNil(forKey: .value)
    }
}

/// One exact finalized cache-retention decision.
public struct MusubiArchiveRetentionDecisionV1: Codable, Hashable, Sendable {
    public let archiveId: MusubiDigest32V1
    public let disposition: MusubiArchiveRetentionDispositionV1
    public let activeReleases: UInt16
    public let yankedReleases: UInt16
    public let takenDownReleases: UInt16
    public let storage: MusubiArchiveAvailabilityV1?

    public init(
        archiveId: MusubiDigest32V1,
        disposition: MusubiArchiveRetentionDispositionV1,
        activeReleases: UInt16,
        yankedReleases: UInt16,
        takenDownReleases: UInt16,
        storage: MusubiArchiveAvailabilityV1?
    ) throws {
        let referenced = Int(activeReleases) + Int(yankedReleases) + Int(takenDownReleases)
        let available = Int(activeReleases) + Int(yankedReleases)
        let canonical: Bool
        switch disposition {
        case .retainUnknown:
            canonical = referenced == 0 && storage == nil
        case .retainReferenced:
            canonical = available > 0 && storage != nil
        case .pruneUnreferenced:
            canonical = referenced == 0 && storage != nil
        case .pruneGovernedTakedown:
            canonical = available == 0 && takenDownReleases > 0 && storage != nil
        }
        guard archiveId.bytes.contains(where: { $0 != 0 }), referenced <= 1_024,
              storage == nil || storage?.archiveId == archiveId, canonical else {
            throw MusubiV1Error.invalidValue(
                "Musubi archive-retention decision is internally inconsistent."
            )
        }
        self.archiveId = archiveId
        self.disposition = disposition
        self.activeReleases = activeReleases
        self.yankedReleases = yankedReleases
        self.takenDownReleases = takenDownReleases
        self.storage = storage
    }

    public var mustRetain: Bool { disposition.mustRetain }

    private enum CodingKeys: String, CodingKey {
        case archiveId = "archive_id"
        case disposition
        case activeReleases = "active_releases"
        case yankedReleases = "yanked_releases"
        case takenDownReleases = "taken_down_releases"
        case storage
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(
            decoder,
            [
                "archive_id", "disposition", "active_releases", "yanked_releases",
                "taken_down_releases", "storage"
            ]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            archiveId: container.decode(MusubiDigest32V1.self, forKey: .archiveId),
            disposition: container.decode(
                MusubiArchiveRetentionDispositionV1.self,
                forKey: .disposition
            ),
            activeReleases: container.decode(UInt16.self, forKey: .activeReleases),
            yankedReleases: container.decode(UInt16.self, forKey: .yankedReleases),
            takenDownReleases: container.decode(UInt16.self, forKey: .takenDownReleases),
            storage: container.decodeIfPresent(
                MusubiArchiveAvailabilityV1.self,
                forKey: .storage
            )
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(archiveId, forKey: .archiveId)
        try container.encode(disposition, forKey: .disposition)
        try container.encode(activeReleases, forKey: .activeReleases)
        try container.encode(yankedReleases, forKey: .yankedReleases)
        try container.encode(takenDownReleases, forKey: .takenDownReleases)
        if let storage { try container.encode(storage, forKey: .storage) }
        else { try container.encodeNil(forKey: .storage) }
    }
}

/// Bounded, sorted exact archive identities for authoritative cache retention.
public struct MusubiArchiveRetentionQueryV1: Codable, Hashable, Sendable {
    public let archiveIds: [MusubiDigest32V1]
    public let expectedSnapshot: MusubiRegistrySnapshotV1?

    public init(
        archiveIds: [MusubiDigest32V1],
        expectedSnapshot: MusubiRegistrySnapshotV1? = nil
    ) throws {
        guard !archiveIds.isEmpty, archiveIds.count <= 100,
              archiveIds.allSatisfy({ $0.bytes.contains(where: { $0 != 0 }) }),
              zip(archiveIds, archiveIds.dropFirst()).allSatisfy({ pair in
                  musubiCompareUnsignedBytes(pair.0.bytes, pair.1.bytes) < 0
              }) else {
            throw MusubiV1Error.invalidValue(
                "Musubi archive-retention batch is empty, oversized, or noncanonical."
            )
        }
        self.archiveIds = archiveIds
        self.expectedSnapshot = expectedSnapshot
    }

    private enum CodingKeys: String, CodingKey {
        case archiveIds = "archive_ids"
        case expectedSnapshot = "expected_snapshot"
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["archive_ids", "expected_snapshot"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            archiveIds: container.decode([MusubiDigest32V1].self, forKey: .archiveIds),
            expectedSnapshot: container.decodeIfPresent(
                MusubiRegistrySnapshotV1.self,
                forKey: .expectedSnapshot
            )
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(archiveIds, forKey: .archiveIds)
        if let expectedSnapshot { try container.encode(expectedSnapshot, forKey: .expectedSnapshot) }
        else { try container.encodeNil(forKey: .expectedSnapshot) }
    }
}

/// Exact alias or alias-history query body.
public struct MusubiAliasQueryV1: Codable, Hashable, Sendable {
    public let alias: String
    public let page: MusubiPageRequestV1
    public init(alias: String, page: MusubiPageRequestV1 = .init()) throws {
        try musubiRequireASCIILowerKebab(alias, maximum: 32, field: "Musubi alias")
        self.alias = alias; self.page = page
    }
    private enum CodingKeys: String, CodingKey { case alias, page }
    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["alias", "page"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            alias: musubiDecodeNewtypeText(container, forKey: .alias, field: "Musubi alias"),
            page: container.decode(MusubiPageRequestV1.self, forKey: .page)
        )
    }
    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode([alias], forKey: .alias)
        try container.encode(page, forKey: .page)
    }
}

/// Deterministic public-directory prefix query body.
public struct MusubiOrderedPrefixQueryV1: Codable, Hashable, Sendable {
    public let prefix: String
    public let page: MusubiPageRequestV1
    public init(prefix: String, page: MusubiPageRequestV1 = .init()) throws {
        try musubiRequireExactText(prefix, field: "Musubi ordered prefix")
        guard prefix.utf8.count <= 512 else {
            throw MusubiV1Error.invalidValue("Musubi ordered prefix exceeds 512 bytes.")
        }
        self.prefix = prefix; self.page = page
    }
    private enum CodingKeys: String, CodingKey { case prefix, page }
    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["prefix", "page"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            prefix: musubiDecodeNewtypeText(container, forKey: .prefix, field: "Musubi prefix"),
            page: container.decode(MusubiPageRequestV1.self, forKey: .page)
        )
    }
    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode([prefix], forKey: .prefix)
        try container.encode(page, forKey: .page)
    }
}

/// Finalized anchor for the rebuildable package-search projection.
public struct MusubiSearchSnapshotV1: Codable, Hashable, Sendable {
    public let finalizedHeight: UInt64
    public let finalizedBlockHash: [UInt8]
    public let projectionRevision: UInt64

    public init(
        finalizedHeight: UInt64,
        finalizedBlockHash: [UInt8],
        projectionRevision: UInt64
    ) throws {
        guard finalizedHeight > 0, projectionRevision > 0,
              finalizedBlockHash.count == 32,
              finalizedBlockHash.contains(where: { $0 != 0 }) else {
            throw MusubiV1Error.invalidValue("Musubi search snapshot is invalid.")
        }
        self.finalizedHeight = finalizedHeight
        self.finalizedBlockHash = finalizedBlockHash
        self.projectionRevision = projectionRevision
    }

    private enum CodingKeys: String, CodingKey {
        case finalizedHeight = "finalized_height"
        case finalizedBlockHash = "finalized_block_hash"
        case projectionRevision = "projection_revision"
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(
            decoder, ["finalized_height", "finalized_block_hash", "projection_revision"]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            finalizedHeight: container.decode(UInt64.self, forKey: .finalizedHeight),
            finalizedBlockHash: container.decode([UInt8].self, forKey: .finalizedBlockHash),
            projectionRevision: container.decode(UInt64.self, forKey: .projectionRevision)
        )
    }
}

/// Search continuation bound to one exact query and projection snapshot.
public struct MusubiSearchCursorV1: Codable, Hashable, Sendable {
    public let snapshot: MusubiSearchSnapshotV1
    public let queryHash: MusubiDigest32V1
    public let lastPackage: MusubiPackageIdV1

    public init(
        snapshot: MusubiSearchSnapshotV1,
        queryHash: MusubiDigest32V1,
        lastPackage: MusubiPackageIdV1
    ) throws {
        guard queryHash.bytes.contains(where: { $0 != 0 }) else {
            throw MusubiV1Error.invalidValue("Musubi search cursor query hash is inert.")
        }
        self.snapshot = snapshot
        self.queryHash = queryHash
        self.lastPackage = lastPackage
    }

    private enum CodingKeys: String, CodingKey {
        case snapshot
        case queryHash = "query_hash"
        case lastPackage = "last_package"
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["snapshot", "query_hash", "last_package"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            snapshot: container.decode(MusubiSearchSnapshotV1.self, forKey: .snapshot),
            queryHash: container.decode(MusubiDigest32V1.self, forKey: .queryHash),
            lastPackage: container.decode(MusubiPackageIdV1.self, forKey: .lastPackage)
        )
    }
}

/// Bounded page controls for rich package discovery.
public struct MusubiSearchPageRequestV1: Codable, Hashable, Sendable {
    public let limit: UInt32
    public let cursor: MusubiSearchCursorV1?

    public init(limit: UInt32 = 50, cursor: MusubiSearchCursorV1? = nil) throws {
        guard limit <= 100 else {
            throw MusubiV1Error.invalidValue("Musubi search page limit exceeds 100.")
        }
        self.limit = limit
        self.cursor = cursor
    }

    private enum CodingKeys: String, CodingKey { case limit, cursor }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["limit", "cursor"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            limit: container.decode(UInt32.self, forKey: .limit),
            cursor: container.decodeIfPresent(MusubiSearchCursorV1.self, forKey: .cursor)
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(limit, forKey: .limit)
        if let cursor { try container.encode(cursor, forKey: .cursor) }
        else { try container.encodeNil(forKey: .cursor) }
    }
}

/// Bounded exact-token description and keyword search query.
public struct MusubiSearchQueryV1: Codable, Hashable, Sendable {
    public let query: String
    public let page: MusubiSearchPageRequestV1

    public init(query: String, page: MusubiSearchPageRequestV1? = nil) throws {
        _ = try musubiNormalizedSearchTerms(query)
        self.query = query
        self.page = try page ?? MusubiSearchPageRequestV1()
    }

    private enum CodingKeys: String, CodingKey { case query, page }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["query", "page"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            query: container.decode(String.self, forKey: .query),
            page: container.decode(MusubiSearchPageRequestV1.self, forKey: .page)
        )
    }
}

private func musubiDecodeNewtypeText<Key: CodingKey>(
    _ container: KeyedDecodingContainer<Key>,
    forKey key: Key,
    field: String
) throws -> String {
    let values = try container.decode([String].self, forKey: key)
    guard values.count == 1 else {
        throw MusubiV1Error.invalidValue("\(field) must contain one Norito newtype item.")
    }
    return values[0]
}

/// Exact integer-only JSON value used for SoraFS-owned fields retained by Musubi DTOs.
public indirect enum MusubiJSONValueV1: Codable, Hashable, Sendable {
    case string(String)
    case unsigned(UInt64)
    case bool(Bool)
    case array([MusubiJSONValueV1])
    case object([String: MusubiJSONValueV1])
    case null

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() { self = .null }
        else if let value = try? container.decode(String.self) { self = .string(value) }
        else if let value = try? container.decode(Bool.self) { self = .bool(value) }
        else if let value = try? container.decode(UInt64.self) { self = .unsigned(value) }
        else if let value = try? container.decode([MusubiJSONValueV1].self) { self = .array(value) }
        else if let value = try? container.decode([String: MusubiJSONValueV1].self) {
            self = .object(value)
        } else {
            throw MusubiV1Error.invalidValue(
                "Musubi V1 JSON permits only exact unsigned integers and bounded structural values."
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .string(let value): try container.encode(value)
        case .unsigned(let value): try container.encode(value)
        case .bool(let value): try container.encode(value)
        case .array(let value): try container.encode(value)
        case .object(let value): try container.encode(value)
        case .null: try container.encodeNil()
        }
    }
}

private func musubiRawObject(
    _ value: MusubiJSONValueV1,
    field: String,
    exactKeys: Set<String>? = nil
) throws -> [String: MusubiJSONValueV1] {
    guard case .object(let object) = value else {
        throw MusubiV1Error.invalidValue("\(field) must be an object.")
    }
    if let exactKeys, Set(object.keys) != exactKeys {
        throw MusubiV1Error.unknownFields(
            expected: exactKeys.sorted(), actual: object.keys.sorted()
        )
    }
    return object
}

private func musubiRawArray(_ value: MusubiJSONValueV1?, field: String) throws -> [MusubiJSONValueV1] {
    guard let value, case .array(let array) = value else {
        throw MusubiV1Error.invalidValue("\(field) must be an array.")
    }
    return array
}

private func musubiRawString(_ value: MusubiJSONValueV1?, field: String) throws -> String {
    guard let value, case .string(let string) = value else {
        throw MusubiV1Error.invalidValue("\(field) must be a string.")
    }
    return string
}

private func musubiRawUnsigned(_ value: MusubiJSONValueV1?, field: String) throws -> UInt64 {
    guard let value, case .unsigned(let number) = value else {
        throw MusubiV1Error.invalidValue("\(field) must be an unsigned integer.")
    }
    return number
}

private func musubiRawBool(_ value: MusubiJSONValueV1?, field: String) throws -> Bool {
    guard let value, case .bool(let bool) = value else {
        throw MusubiV1Error.invalidValue("\(field) must be a boolean.")
    }
    return bool
}

private func musubiDecodeRaw<T: Decodable>(
    _ value: MusubiJSONValueV1,
    as type: T.Type
) throws -> T {
    try JSONDecoder().decode(type, from: JSONEncoder().encode(value))
}

private func musubiValidateRawDigest(_ value: MusubiJSONValueV1?, field: String) throws {
    let outer = try musubiRawArray(value, field: field)
    guard outer.count == 1 else {
        throw MusubiV1Error.invalidValue("\(field) must contain one Norito newtype item.")
    }
    try musubiValidateRawFixedBytes(outer[0], field: "\(field)[0]")
}

private func musubiValidateRawFixedBytes(_ value: MusubiJSONValueV1?, field: String) throws {
    _ = try musubiRawBytes(value, field: field, count: 32)
}

private func musubiRawBytes(
    _ value: MusubiJSONValueV1?,
    field: String,
    count: Int
) throws -> [UInt8] {
    let bytes = try musubiRawArray(value, field: field)
    guard bytes.count == count else {
        throw MusubiV1Error.invalidValue("\(field) must contain exactly \(count) bytes.")
    }
    return try bytes.map { byte in
        guard case .unsigned(let value) = byte, value <= UInt8.max else {
            throw MusubiV1Error.invalidValue("\(field) contains a non-byte value.")
        }
        return UInt8(value)
    }
}

private func musubiCompareUnsignedBytes(_ left: [UInt8], _ right: [UInt8]) -> Int {
    for index in 0..<min(left.count, right.count) {
        if left[index] != right[index] { return left[index] < right[index] ? -1 : 1 }
    }
    if left.count == right.count { return 0 }
    return left.count < right.count ? -1 : 1
}

private func musubiRawNewtypeText(_ value: MusubiJSONValueV1?, field: String) throws -> String {
    let wrapper = try musubiRawArray(value, field: field)
    guard wrapper.count == 1 else {
        throw MusubiV1Error.invalidValue("\(field) must contain one Norito newtype item.")
    }
    return try musubiRawString(wrapper[0], field: "\(field)[0]")
}

private func musubiRawBytesValue(_ bytes: [UInt8]) -> MusubiJSONValueV1 {
    .array(bytes.map { .unsigned(UInt64($0)) })
}

private func musubiRawDigestValue(_ digest: MusubiDigest32V1) -> MusubiJSONValueV1 {
    .array([musubiRawBytesValue(digest.bytes)])
}

private func musubiRawNewtypeTextValue(_ value: String) -> MusubiJSONValueV1 {
    .array([.string(value)])
}

private func musubiValidateTaggedUnit(
    _ value: MusubiJSONValueV1?,
    field: String,
    allowed: Set<String>
) throws -> String {
    guard let value else { throw MusubiV1Error.invalidValue("\(field) is missing.") }
    let object = try musubiRawObject(value, field: field, exactKeys: ["kind", "value"])
    let kind = try musubiRawString(object["kind"], field: "\(field).kind")
    guard allowed.contains(kind), object["value"] == .null else {
        throw MusubiV1Error.unsupportedVersion("\(field) has an unsupported Musubi V1 tag.")
    }
    return kind
}

/// Package compare-and-set revisions.
public struct MusubiPackageRevisionsV1: Codable, Hashable, Sendable {
    public let governance: UInt64
    public let metadata: UInt64
    public let archiveLocations: UInt64

    public init(governance: UInt64, metadata: UInt64, archiveLocations: UInt64) {
        self.governance = governance
        self.metadata = metadata
        self.archiveLocations = archiveLocations
    }

    private enum CodingKeys: String, CodingKey {
        case governance, metadata
        case archiveLocations = "archive_locations"
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["governance", "metadata", "archive_locations"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        governance = try container.decode(UInt64.self, forKey: .governance)
        metadata = try container.decode(UInt64.self, forKey: .metadata)
        archiveLocations = try container.decode(UInt64.self, forKey: .archiveLocations)
    }
}

/// Exact structural package record.
public struct MusubiPackageRecordV1: Codable, Hashable, Sendable {
    public let package: MusubiPackageIdV1
    public let claimedNamespace: MusubiNamespaceV1
    public let claimedNamespaceBinding: MusubiDigest32V1
    public let owners: [String]
    public let memberAccounts: [String]
    public let claimedAtHeight: UInt64
    public let revisions: MusubiPackageRevisionsV1

    private enum CodingKeys: String, CodingKey {
        case package
        case claimedNamespace = "claimed_namespace"
        case claimedNamespaceBinding = "claimed_namespace_binding"
        case owners
        case memberAccounts = "member_accounts"
        case claimedAtHeight = "claimed_at_height"
        case revisions
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(
            decoder,
            [
                "package", "claimed_namespace", "claimed_namespace_binding", "owners",
                "member_accounts", "claimed_at_height", "revisions"
            ]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        package = try container.decode(MusubiPackageIdV1.self, forKey: .package)
        claimedNamespace = try container.decode(MusubiNamespaceV1.self, forKey: .claimedNamespace)
        claimedNamespaceBinding = try container.decode(
            MusubiDigest32V1.self, forKey: .claimedNamespaceBinding
        )
        owners = try container.decode([String].self, forKey: .owners)
        memberAccounts = try container.decode([String].self, forKey: .memberAccounts)
        claimedAtHeight = try container.decode(UInt64.self, forKey: .claimedAtHeight)
        revisions = try container.decode(MusubiPackageRevisionsV1.self, forKey: .revisions)
        guard !owners.isEmpty else {
            throw MusubiV1Error.invalidValue("Musubi package record must retain at least one owner.")
        }
        try (owners + memberAccounts).forEach {
            try musubiRequireExactText($0, field: "Musubi package account")
        }
    }
}

private func musubiValidateManifest(_ value: MusubiJSONValueV1) throws -> MusubiReleaseIdV1 {
    let object = try musubiRawObject(
        value,
        field: "manifest",
        exactKeys: [
            "release", "edition", "abi", "dependencies", "exports", "interface_digest",
            "metadata", "archive_id", "verification_lock_digest"
        ]
    )
    guard let releaseValue = object["release"] else {
        throw MusubiV1Error.invalidValue("manifest.release is missing.")
    }
    let release = try musubiDecodeRaw(releaseValue, as: MusubiReleaseIdV1.self)
    _ = try musubiValidateTaggedUnit(object["edition"], field: "manifest.edition", allowed: ["V1"])

    guard let abiValue = object["abi"] else {
        throw MusubiV1Error.invalidValue("manifest.abi is missing.")
    }
    let abi = try musubiRawObject(
        abiValue, field: "manifest.abi", exactKeys: ["abi_version", "abi_hash"]
    )
    guard try musubiRawUnsigned(abi["abi_version"], field: "manifest.abi.abi_version") == 1 else {
        throw MusubiV1Error.unsupportedVersion(
            "Musubi only supports IVM ABI V1; the response advertised another version."
        )
    }
    try musubiValidateRawFixedBytes(abi["abi_hash"], field: "manifest.abi.abi_hash")

    for (index, dependencyValue) in try musubiRawArray(
        object["dependencies"], field: "manifest.dependencies"
    ).enumerated() {
        let field = "manifest.dependencies[\(index)]"
        let dependency = try musubiRawObject(
            dependencyValue, field: field, exactKeys: ["alias", "package", "requirement"]
        )
        try musubiRequireName(
            musubiRawString(dependency["alias"], field: "\(field).alias"),
            field: "\(field).alias"
        )
        guard let package = dependency["package"], let requirement = dependency["requirement"] else {
            throw MusubiV1Error.invalidValue("\(field) is incomplete.")
        }
        _ = try musubiDecodeRaw(package, as: MusubiPackageIdV1.self)
        _ = try musubiDecodeRaw(requirement, as: MusubiVersionReqV1.self)
    }
    for export in try musubiRawArray(object["exports"], field: "manifest.exports") {
        try musubiRequireName(
            musubiRawString(export, field: "manifest.exports[]"),
            field: "Musubi export"
        )
    }
    try musubiValidateRawDigest(object["interface_digest"], field: "manifest.interface_digest")
    try musubiValidateRawDigest(object["archive_id"], field: "manifest.archive_id")
    try musubiValidateRawDigest(
        object["verification_lock_digest"], field: "manifest.verification_lock_digest"
    )

    guard let metadataValue = object["metadata"] else {
        throw MusubiV1Error.invalidValue("manifest.metadata is missing.")
    }
    let metadata = try musubiRawObject(
        metadataValue,
        field: "manifest.metadata",
        exactKeys: ["description", "readme", "license", "repository", "keywords"]
    )
    for key in ["description", "readme", "license", "repository"] {
        if metadata[key] != .null {
            _ = try musubiRawNewtypeText(metadata[key], field: "manifest.metadata.\(key)")
        }
    }
    for keyword in try musubiRawArray(metadata["keywords"], field: "manifest.metadata.keywords") {
        try musubiRequireASCIILowerKebab(
            musubiRawNewtypeText(keyword, field: "manifest.metadata.keyword"),
            maximum: 64,
            field: "Musubi keyword"
        )
    }
    return release
}

private func musubiValidateYank(
    _ value: MusubiJSONValueV1?,
    expectedRelease: MusubiReleaseIdV1,
    field: String
) throws {
    guard let value else { throw MusubiV1Error.invalidValue("\(field) is missing.") }
    let object = try musubiRawObject(
        value,
        field: field,
        exactKeys: ["release", "yanked", "reason", "changed_by", "changed_at_height", "revision"]
    )
    guard let releaseRaw = object["release"],
          try musubiDecodeRaw(releaseRaw, as: MusubiReleaseIdV1.self) == expectedRelease else {
        throw MusubiV1Error.invalidValue("\(field).release does not match the manifest.")
    }
    _ = try musubiRawBool(object["yanked"], field: "\(field).yanked")
    _ = try musubiRawNewtypeText(object["reason"], field: "\(field).reason")
    _ = try musubiRawString(object["changed_by"], field: "\(field).changed_by")
    guard try musubiRawUnsigned(object["changed_at_height"], field: "\(field).changed_at_height") > 0,
          try musubiRawUnsigned(object["revision"], field: "\(field).revision") > 0 else {
        throw MusubiV1Error.invalidValue("\(field) heights and revisions must be non-zero.")
    }
}

private func musubiValidateGovernance(_ value: MusubiJSONValueV1?, field: String) throws {
    guard let value else { throw MusubiV1Error.invalidValue("\(field) is missing.") }
    let object = try musubiRawObject(value, field: field, exactKeys: ["kind", "value"])
    let kind = try musubiRawString(object["kind"], field: "\(field).kind")
    switch kind {
    case "Available":
        guard object["value"] == .null else {
            throw MusubiV1Error.invalidValue("Available governance value must be null.")
        }
    case "TakenDown":
        guard let takedownValue = object["value"] else {
            throw MusubiV1Error.invalidValue("TakenDown governance value is missing.")
        }
        let takedown = try musubiRawObject(
            takedownValue,
            field: "\(field).value",
            exactKeys: ["action_digest", "reason", "applied_at_height"]
        )
        try musubiValidateRawDigest(takedown["action_digest"], field: "\(field).action_digest")
        _ = try musubiRawNewtypeText(takedown["reason"], field: "\(field).reason")
        guard try musubiRawUnsigned(
            takedown["applied_at_height"],
            field: "\(field).applied_at_height"
        ) > 0 else {
            throw MusubiV1Error.invalidValue("Governed takedown height must be non-zero.")
        }
    default:
        throw MusubiV1Error.unsupportedVersion("Unsupported Musubi V1 governance state.")
    }
}

/// Exact immutable release with strict validation of its mutable projections.
public struct MusubiReleaseRecordV1: Codable, Hashable, Sendable {
    public let release: MusubiReleaseIdV1
    public let releaseDigest: MusubiDigest32V1
    public let publishedBy: String
    public let publishedAtHeight: UInt64
    public let raw: [String: MusubiJSONValueV1]

    public init(from decoder: Decoder) throws {
        let keys: Set<String> = [
            "manifest", "release_digest", "published_by", "published_at_height", "yank",
            "artifact_governance", "revisions"
        ]
        try musubiRequireExactKeys(decoder, keys)
        let value = try decoder.singleValueContainer().decode(MusubiJSONValueV1.self)
        raw = try musubiRawObject(value, field: "release response", exactKeys: keys)
        guard let manifest = raw["manifest"], let digest = raw["release_digest"] else {
            throw MusubiV1Error.invalidValue("Musubi release response is incomplete.")
        }
        release = try musubiValidateManifest(manifest)
        releaseDigest = try musubiDecodeRaw(digest, as: MusubiDigest32V1.self)
        publishedBy = try musubiRawString(raw["published_by"], field: "published_by")
        publishedAtHeight = try musubiRawUnsigned(
            raw["published_at_height"], field: "published_at_height"
        )
        try musubiValidateYank(raw["yank"], expectedRelease: release, field: "yank")
        try musubiValidateGovernance(raw["artifact_governance"], field: "artifact_governance")
        guard let revisionsValue = raw["revisions"] else {
            throw MusubiV1Error.invalidValue("release revisions are missing.")
        }
        let revisions = try musubiRawObject(
            revisionsValue,
            field: "revisions",
            exactKeys: ["yank", "artifact_governance"]
        )
        guard try musubiRawUnsigned(revisions["yank"], field: "revisions.yank") > 0,
              try musubiRawUnsigned(
                  revisions["artifact_governance"], field: "revisions.artifact_governance"
              ) > 0 else {
            throw MusubiV1Error.invalidValue("Release revisions must be non-zero.")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(MusubiJSONValueV1.object(raw))
    }
}

/// Resolver-grade release row retained with exact integer JSON fields.
public struct MusubiResolverReleaseRowV1: Codable, Hashable, Sendable {
    public let release: MusubiReleaseIdV1
    public let indexRevision: UInt64
    public let raw: [String: MusubiJSONValueV1]

    public init(from decoder: Decoder) throws {
        let keys: Set<String> = [
            "release", "release_digest", "archive_id", "source_digest", "interface_digest",
            "abi", "dependencies", "selection", "index_revision"
        ]
        try musubiRequireExactKeys(decoder, keys)
        let value = try decoder.singleValueContainer().decode(MusubiJSONValueV1.self)
        raw = try musubiRawObject(value, field: "resolver row", exactKeys: keys)
        guard let releaseRaw = raw["release"] else {
            throw MusubiV1Error.invalidValue("resolver row release is missing.")
        }
        release = try musubiDecodeRaw(releaseRaw, as: MusubiReleaseIdV1.self)
        indexRevision = try musubiRawUnsigned(raw["index_revision"], field: "index_revision")
        guard indexRevision > 0 else {
            throw MusubiV1Error.invalidValue("Resolver index revision must be non-zero.")
        }
        for key in ["release_digest", "archive_id", "source_digest", "interface_digest"] {
            try musubiValidateRawDigest(raw[key], field: key)
        }
        guard let abiValue = raw["abi"] else {
            throw MusubiV1Error.invalidValue("resolver ABI is missing.")
        }
        let abi = try musubiRawObject(abiValue, field: "abi", exactKeys: ["abi_version", "abi_hash"])
        guard try musubiRawUnsigned(abi["abi_version"], field: "abi_version") == 1 else {
            throw MusubiV1Error.unsupportedVersion("Musubi resolver row is not IVM ABI V1.")
        }
        try musubiValidateRawFixedBytes(abi["abi_hash"], field: "abi_hash")
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(MusubiJSONValueV1.object(raw))
    }
}

/// Resolver page carrying exact chain/genesis identity for consumer lockfiles.
public struct MusubiResolverIndexPageV1: Codable, Hashable, Sendable {
    public let chainId: String
    public let genesisHash: [UInt8]
    public let items: [MusubiResolverReleaseRowV1]
    public let nextCursor: MusubiFinalizedCursorV1?
    public let snapshot: MusubiRegistrySnapshotV1

    private enum CodingKeys: String, CodingKey {
        case chainId = "chain_id"
        case genesisHash = "genesis_hash"
        case items
        case nextCursor = "next_cursor"
        case snapshot
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(
            decoder, ["chain_id", "genesis_hash", "items", "next_cursor", "snapshot"]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        chainId = try container.decode(String.self, forKey: .chainId)
        try musubiRequireExactText(chainId, field: "Musubi resolver chain ID")
        genesisHash = try container.decode([UInt8].self, forKey: .genesisHash)
        items = try container.decode([MusubiResolverReleaseRowV1].self, forKey: .items)
        nextCursor = try container.decodeIfPresent(MusubiFinalizedCursorV1.self, forKey: .nextCursor)
        snapshot = try container.decode(MusubiRegistrySnapshotV1.self, forKey: .snapshot)
        guard genesisHash.count == 32, genesisHash.contains(where: { $0 != 0 }),
              items.count <= 100,
              nextCursor == nil || nextCursor?.snapshot == snapshot else {
            throw MusubiV1Error.invalidValue(
                "Musubi resolver page has an invalid genesis hash, size, or cursor."
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        guard genesisHash.count == 32, genesisHash.contains(where: { $0 != 0 }) else {
            throw MusubiV1Error.invalidValue("Musubi genesis hash must contain 32 bytes.")
        }
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(chainId, forKey: .chainId)
        try container.encode(genesisHash, forKey: .genesisHash)
        try container.encode(items, forKey: .items)
        if let nextCursor { try container.encode(nextCursor, forKey: .nextCursor) }
        else { try container.encodeNil(forKey: .nextCursor) }
        try container.encode(snapshot, forKey: .snapshot)
    }
}

private func musubiPackageRoleKind(
    _ value: MusubiJSONValueV1?,
    field: String
) throws -> String {
    guard let value else {
        throw MusubiV1Error.invalidValue("\(field) is missing.")
    }
    let role = try musubiRawObject(value, field: field, exactKeys: ["kind", "value"])
    let kind = try musubiRawString(role["kind"], field: "\(field).kind")
    switch kind {
    case "Owner":
        guard role["value"] == .null else {
            throw MusubiV1Error.invalidValue("Owner role value must be null.")
        }
    case "Maintainer":
        guard let permissionsValue = role["value"] else {
            throw MusubiV1Error.invalidValue("Maintainer permissions are missing.")
        }
        let permissions = try musubiRawObject(
            permissionsValue,
            field: "\(field).value",
            exactKeys: ["publish", "yank", "metadata", "archive_locations"]
        )
        var grantsPermission = false
        for key in permissions.keys {
            grantsPermission = try musubiRawBool(
                permissions[key], field: "\(field).value.\(key)"
            ) || grantsPermission
        }
        guard grantsPermission else {
            throw MusubiV1Error.invalidValue("Maintainer role must grant a permission.")
        }
    default:
        throw MusubiV1Error.unsupportedVersion("Unsupported Musubi V1 member role.")
    }
    return kind
}

/// Accepted owner or maintainer record.
public struct MusubiPackageMemberV1: Codable, Hashable, Sendable {
    public let package: MusubiPackageIdV1
    public let account: String
    public let roleKind: String
    public let acceptedAtHeight: UInt64
    public let governanceRevision: UInt64
    public let raw: [String: MusubiJSONValueV1]

    public init(from decoder: Decoder) throws {
        let keys: Set<String> = [
            "package", "account", "role", "accepted_at_height", "governance_revision"
        ]
        try musubiRequireExactKeys(decoder, keys)
        let value = try decoder.singleValueContainer().decode(MusubiJSONValueV1.self)
        raw = try musubiRawObject(value, field: "package member", exactKeys: keys)
        guard let packageRaw = raw["package"], let roleRaw = raw["role"] else {
            throw MusubiV1Error.invalidValue("package member is incomplete.")
        }
        package = try musubiDecodeRaw(packageRaw, as: MusubiPackageIdV1.self)
        account = try musubiRawString(raw["account"], field: "member.account")
        roleKind = try musubiPackageRoleKind(roleRaw, field: "member.role")
        acceptedAtHeight = try musubiRawUnsigned(raw["accepted_at_height"], field: "accepted_at_height")
        governanceRevision = try musubiRawUnsigned(
            raw["governance_revision"], field: "governance_revision"
        )
        guard acceptedAtHeight > 0, governanceRevision > 0 else {
            throw MusubiV1Error.invalidValue("Member heights and revisions must be non-zero.")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(MusubiJSONValueV1.object(raw))
    }
}

/// Pending package-governance invitation that has not created authority.
public struct MusubiMaintainerInvitationV1: Codable, Hashable, Sendable {
    public let inviteId: MusubiDigest32V1
    public let package: MusubiPackageIdV1
    public let invitedBy: String
    public let invitedAccount: String
    public let roleKind: String
    public let expectedGovernanceRevision: UInt64
    public let expiresAtHeight: UInt64
    public let stateKind: String
    public let raw: [String: MusubiJSONValueV1]

    public init(from decoder: Decoder) throws {
        let keys: Set<String> = [
            "invite_id", "package", "invited_by", "invited_account", "role",
            "expected_governance_revision", "expires_at_height", "state"
        ]
        try musubiRequireExactKeys(decoder, keys)
        let value = try decoder.singleValueContainer().decode(MusubiJSONValueV1.self)
        raw = try musubiRawObject(value, field: "maintainer invitation", exactKeys: keys)
        guard let inviteIdRaw = raw["invite_id"],
              let packageRaw = raw["package"],
              let stateRaw = raw["state"] else {
            throw MusubiV1Error.invalidValue("Maintainer invitation is incomplete.")
        }
        inviteId = try musubiDecodeRaw(inviteIdRaw, as: MusubiDigest32V1.self)
        guard inviteId.bytes.contains(where: { $0 != 0 }) else {
            throw MusubiV1Error.invalidValue("Maintainer invite ID must not be inert.")
        }
        package = try musubiDecodeRaw(packageRaw, as: MusubiPackageIdV1.self)
        invitedBy = try musubiRawString(raw["invited_by"], field: "invitation.invited_by")
        invitedAccount = try musubiRawString(
            raw["invited_account"], field: "invitation.invited_account"
        )
        try musubiRequireExactText(invitedBy, field: "invitation.invited_by")
        try musubiRequireExactText(invitedAccount, field: "invitation.invited_account")
        roleKind = try musubiPackageRoleKind(raw["role"], field: "invitation.role")
        expectedGovernanceRevision = try musubiRawUnsigned(
            raw["expected_governance_revision"], field: "expected_governance_revision"
        )
        expiresAtHeight = try musubiRawUnsigned(
            raw["expires_at_height"], field: "expires_at_height"
        )
        let state = try musubiRawObject(
            stateRaw, field: "invitation.state", exactKeys: ["kind", "value"]
        )
        stateKind = try musubiRawString(state["kind"], field: "invitation.state.kind")
        guard stateKind == "Pending", state["value"] == .null,
              expectedGovernanceRevision > 0, expiresAtHeight > 0 else {
            throw MusubiV1Error.invalidValue(
                "Maintainer directory invitations must be pending with non-zero bounds."
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(MusubiJSONValueV1.object(raw))
    }
}

/// Accepted member or pending invitation returned by the maintainer directory.
public enum MusubiMaintainerDirectoryEntryV1: Codable, Hashable, Sendable {
    case accepted(MusubiPackageMemberV1)
    case pendingInvitation(MusubiMaintainerInvitationV1)

    private enum CodingKeys: String, CodingKey { case kind, value }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["kind", "value"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "Accepted":
            self = .accepted(try container.decode(MusubiPackageMemberV1.self, forKey: .value))
        case "PendingInvitation":
            self = .pendingInvitation(
                try container.decode(MusubiMaintainerInvitationV1.self, forKey: .value)
            )
        default:
            throw MusubiV1Error.unsupportedVersion(
                "Unsupported Musubi V1 maintainer-directory entry."
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        switch self {
        case .accepted(let member):
            try container.encode("Accepted", forKey: .kind)
            try container.encode(member, forKey: .value)
        case .pendingInvitation(let invitation):
            try container.encode("PendingInvitation", forKey: .kind)
            try container.encode(invitation, forKey: .value)
        }
    }
}

/// Exact SoraFS chunker profile bound into an archive commitment.
public struct MusubiChunkerProfileHandleV1: Codable, Hashable, Sendable {
    public let profileId: UInt32
    public let namespace: String
    public let name: String
    public let semver: String
    public let multihashCode: UInt64
    public let raw: [String: MusubiJSONValueV1]

    public init(
        profileId: UInt32,
        namespace: String,
        name: String,
        semver: String,
        multihashCode: UInt64
    ) throws {
        try musubiRequireExactText(namespace, field: "Chunker namespace")
        try musubiRequireExactText(name, field: "Chunker name")
        try musubiRequireExactText(semver, field: "Chunker SemVer")
        guard "\(namespace).\(name)@\(semver)".utf8.count <= 128 else {
            throw MusubiV1Error.invalidValue("Musubi chunker handle exceeds 128 UTF-8 bytes.")
        }
        self.profileId = profileId
        self.namespace = namespace
        self.name = name
        self.semver = semver
        self.multihashCode = multihashCode
        self.raw = [
            "profile_id": .unsigned(UInt64(profileId)),
            "namespace": .string(namespace),
            "name": .string(name),
            "semver": .string(semver),
            "multihash_code": .unsigned(multihashCode),
        ]
    }

    public init(from decoder: Decoder) throws {
        let keys: Set<String> = [
            "profile_id", "namespace", "name", "semver", "multihash_code"
        ]
        try musubiRequireExactKeys(decoder, keys)
        let value = try decoder.singleValueContainer().decode(MusubiJSONValueV1.self)
        raw = try musubiRawObject(value, field: "chunker profile", exactKeys: keys)
        let profile = try musubiRawUnsigned(raw["profile_id"], field: "chunker.profile_id")
        guard profile <= UInt32.max else {
            throw MusubiV1Error.invalidValue("Chunker profile ID must fit UInt32.")
        }
        profileId = UInt32(profile)
        namespace = try musubiRawString(raw["namespace"], field: "chunker.namespace")
        name = try musubiRawString(raw["name"], field: "chunker.name")
        semver = try musubiRawString(raw["semver"], field: "chunker.semver")
        multihashCode = try musubiRawUnsigned(
            raw["multihash_code"], field: "chunker.multihash_code"
        )
        try musubiRequireExactText(namespace, field: "Chunker namespace")
        try musubiRequireExactText(name, field: "Chunker name")
        try musubiRequireExactText(semver, field: "Chunker SemVer")
        guard "\(namespace).\(name)@\(semver)".utf8.count <= 128 else {
            throw MusubiV1Error.invalidValue("Musubi chunker handle exceeds 128 UTF-8 bytes.")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(MusubiJSONValueV1.object(raw))
    }
}

/// Complete immutable source-archive commitment returned by the registry.
public struct MusubiArchiveCommitmentV1: Codable, Hashable, Sendable {
    public let rootCid: [UInt8]
    public let chunker: MusubiChunkerProfileHandleV1
    public let chunkPlanDigest: MusubiDigest32V1
    public let porRoot: MusubiDigest32V1
    public let contentLength: UInt64
    public let carDigest: MusubiDigest32V1
    public let carSize: UInt64
    public let bundleDigest: MusubiDigest32V1
    public let sourceTreeDigest: MusubiDigest32V1
    public let descriptorDigest: MusubiDigest32V1
    public let fileCount: UInt32
    public let chunkCount: UInt32
    public let raw: [String: MusubiJSONValueV1]

    public init(
        rootCid: [UInt8],
        chunker: MusubiChunkerProfileHandleV1,
        chunkPlanDigest: MusubiDigest32V1,
        porRoot: MusubiDigest32V1,
        contentLength: UInt64,
        carDigest: MusubiDigest32V1,
        carSize: UInt64,
        bundleDigest: MusubiDigest32V1,
        sourceTreeDigest: MusubiDigest32V1,
        descriptorDigest: MusubiDigest32V1,
        fileCount: UInt32,
        chunkCount: UInt32
    ) throws {
        guard rootCid.count == 36,
              Array(rootCid.prefix(4)) == [1, 113, 31, 32],
              rootCid.dropFirst(4).contains(where: { $0 != 0 }),
              contentLength > 0, contentLength <= 64 << 20,
              carSize > 0, carSize <= 96 << 20,
              (1...4_096).contains(fileCount), (1...16_384).contains(chunkCount),
              [
                  chunkPlanDigest, porRoot, carDigest, bundleDigest, sourceTreeDigest,
                  descriptorDigest,
              ].allSatisfy({ $0.bytes.contains(where: { $0 != 0 }) }) else {
            throw MusubiV1Error.invalidValue("Musubi archive commitment is out of bounds.")
        }
        self.rootCid = rootCid
        self.chunker = chunker
        self.chunkPlanDigest = chunkPlanDigest
        self.porRoot = porRoot
        self.contentLength = contentLength
        self.carDigest = carDigest
        self.carSize = carSize
        self.bundleDigest = bundleDigest
        self.sourceTreeDigest = sourceTreeDigest
        self.descriptorDigest = descriptorDigest
        self.fileCount = fileCount
        self.chunkCount = chunkCount
        self.raw = [
            "root_cid": musubiRawBytesValue(rootCid),
            "chunker": .object(chunker.raw),
            "chunk_plan_digest": musubiRawDigestValue(chunkPlanDigest),
            "por_root": musubiRawDigestValue(porRoot),
            "content_length": .unsigned(contentLength),
            "car_digest": musubiRawDigestValue(carDigest),
            "car_size": .unsigned(carSize),
            "bundle_digest": musubiRawDigestValue(bundleDigest),
            "source_tree_digest": musubiRawDigestValue(sourceTreeDigest),
            "descriptor_digest": musubiRawDigestValue(descriptorDigest),
            "file_count": .unsigned(UInt64(fileCount)),
            "chunk_count": .unsigned(UInt64(chunkCount)),
        ]
    }

    public init(from decoder: Decoder) throws {
        let keys: Set<String> = [
            "root_cid", "chunker", "chunk_plan_digest", "por_root", "content_length",
            "car_digest", "car_size", "bundle_digest", "source_tree_digest",
            "descriptor_digest", "file_count", "chunk_count"
        ]
        try musubiRequireExactKeys(decoder, keys)
        let value = try decoder.singleValueContainer().decode(MusubiJSONValueV1.self)
        let rawValue = try musubiRawObject(value, field: "archive commitment", exactKeys: keys)
        raw = rawValue
        rootCid = try musubiRawBytes(
            rawValue["root_cid"], field: "commitment.root_cid", count: 36
        )
        guard Array(rootCid.prefix(4)) == [1, 113, 31, 32],
              rootCid.dropFirst(4).contains(where: { $0 != 0 }),
              let chunkerValue = rawValue["chunker"] else {
            throw MusubiV1Error.invalidValue(
                "Musubi root CID must use the canonical CIDv1/dag-cbor/BLAKE3-256 shape."
            )
        }
        chunker = try musubiDecodeRaw(chunkerValue, as: MusubiChunkerProfileHandleV1.self)
        func digest(_ key: String) throws -> MusubiDigest32V1 {
            guard let value = rawValue[key] else {
                throw MusubiV1Error.invalidValue("Archive commitment \(key) is missing.")
            }
            return try musubiDecodeRaw(value, as: MusubiDigest32V1.self)
        }
        chunkPlanDigest = try digest("chunk_plan_digest")
        porRoot = try digest("por_root")
        contentLength = try musubiRawUnsigned(
            rawValue["content_length"], field: "content_length"
        )
        carDigest = try digest("car_digest")
        carSize = try musubiRawUnsigned(rawValue["car_size"], field: "car_size")
        bundleDigest = try digest("bundle_digest")
        sourceTreeDigest = try digest("source_tree_digest")
        descriptorDigest = try digest("descriptor_digest")
        let files = try musubiRawUnsigned(rawValue["file_count"], field: "file_count")
        let chunks = try musubiRawUnsigned(rawValue["chunk_count"], field: "chunk_count")
        guard contentLength > 0, contentLength <= 64 << 20,
              carSize > 0, carSize <= 96 << 20,
              (1...4_096).contains(files), (1...16_384).contains(chunks),
              [
                  chunkPlanDigest, porRoot, carDigest, bundleDigest, sourceTreeDigest,
                  descriptorDigest
              ].allSatisfy({ $0.bytes.contains(where: { $0 != 0 }) }) else {
            throw MusubiV1Error.invalidValue("Musubi archive commitment is out of bounds.")
        }
        fileCount = UInt32(files)
        chunkCount = UInt32(chunks)
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(MusubiJSONValueV1.object(raw))
    }
}

/// Exact deployment and CAR-body binding signed by seed ingress.
public struct MusubiSeedIngressReceiptBindingV1: Codable, Hashable, Sendable {
    public let chainId: String
    public let genesisBlockHash: [UInt8]
    public let publisher: String
    public let ingressBroker: String
    public let seedProvider: String
    public let semanticReleaseManifestDigest: MusubiDigest32V1
    public let archiveId: MusubiDigest32V1
    public let carBodyDigest: MusubiDigest32V1
    public let carBodyLength: UInt64
    public let nonce: [UInt8]
    public let raw: [String: MusubiJSONValueV1]

    public init(
        chainId: String,
        genesisBlockHash: [UInt8],
        publisher: String,
        ingressBroker: String,
        seedProvider: String,
        semanticReleaseManifestDigest: MusubiDigest32V1,
        archiveId: MusubiDigest32V1,
        carBodyDigest: MusubiDigest32V1,
        carBodyLength: UInt64,
        nonce: [UInt8]
    ) throws {
        try musubiRequireChainIDV1(chainId, field: "Seed-ingress chain ID")
        _ = try CanonicalNorito.encodeCompactAccountId(publisher)
        _ = try CanonicalNorito.encodeCompactAccountId(ingressBroker)
        let providerBytes = Array(seedProvider.utf8)
        guard genesisBlockHash.count == 32,
              genesisBlockHash.contains(where: { $0 != 0 }),
              nonce.count == 32, nonce.contains(where: { $0 != 0 }),
              carBodyLength > 0, carBodyLength <= 96 << 20,
              [semanticReleaseManifestDigest, archiveId, carBodyDigest]
                  .allSatisfy({ $0.bytes.contains(where: { $0 != 0 }) }),
              providerBytes.count == 64,
              providerBytes.allSatisfy({
                  (0x30...0x39).contains($0) || (0x41...0x46).contains($0)
              }) else {
            throw MusubiV1Error.invalidValue("Musubi seed-ingress binding is invalid.")
        }
        self.chainId = chainId
        self.genesisBlockHash = genesisBlockHash
        self.publisher = publisher
        self.ingressBroker = ingressBroker
        self.seedProvider = seedProvider
        self.semanticReleaseManifestDigest = semanticReleaseManifestDigest
        self.archiveId = archiveId
        self.carBodyDigest = carBodyDigest
        self.carBodyLength = carBodyLength
        self.nonce = nonce
        self.raw = [
            "chain_id": .string(chainId),
            "genesis_block_hash": musubiRawBytesValue(genesisBlockHash),
            "publisher": .string(publisher),
            "ingress_broker": .string(ingressBroker),
            "seed_provider": musubiRawNewtypeTextValue(seedProvider),
            "semantic_release_manifest_digest": musubiRawDigestValue(
                semanticReleaseManifestDigest
            ),
            "archive_id": musubiRawDigestValue(archiveId),
            "car_body_digest": musubiRawDigestValue(carBodyDigest),
            "car_body_length": .unsigned(carBodyLength),
            "nonce": musubiRawBytesValue(nonce),
        ]
    }

    public init(from decoder: Decoder) throws {
        let keys: Set<String> = [
            "chain_id", "genesis_block_hash", "publisher", "ingress_broker", "seed_provider",
            "semantic_release_manifest_digest", "archive_id", "car_body_digest",
            "car_body_length", "nonce"
        ]
        try musubiRequireExactKeys(decoder, keys)
        let value = try decoder.singleValueContainer().decode(MusubiJSONValueV1.self)
        raw = try musubiRawObject(value, field: "seed-ingress binding", exactKeys: keys)
        chainId = try musubiRawString(raw["chain_id"], field: "binding.chain_id")
        genesisBlockHash = try musubiRawBytes(
            raw["genesis_block_hash"], field: "binding.genesis_block_hash", count: 32
        )
        publisher = try musubiRawString(raw["publisher"], field: "binding.publisher")
        ingressBroker = try musubiRawString(
            raw["ingress_broker"], field: "binding.ingress_broker"
        )
        seedProvider = try musubiRawNewtypeText(
            raw["seed_provider"], field: "binding.seed_provider"
        )
        guard let semantic = raw["semantic_release_manifest_digest"],
              let archive = raw["archive_id"], let carDigest = raw["car_body_digest"] else {
            throw MusubiV1Error.invalidValue("Seed-ingress digest binding is incomplete.")
        }
        semanticReleaseManifestDigest = try musubiDecodeRaw(
            semantic, as: MusubiDigest32V1.self
        )
        archiveId = try musubiDecodeRaw(archive, as: MusubiDigest32V1.self)
        carBodyDigest = try musubiDecodeRaw(carDigest, as: MusubiDigest32V1.self)
        carBodyLength = try musubiRawUnsigned(
            raw["car_body_length"], field: "binding.car_body_length"
        )
        nonce = try musubiRawBytes(raw["nonce"], field: "binding.nonce", count: 32)
        let providerBytes = Array(seedProvider.utf8)
        guard carBodyLength > 0, carBodyLength <= 96 << 20,
              genesisBlockHash.contains(where: { $0 != 0 }),
              nonce.contains(where: { $0 != 0 }),
              [semanticReleaseManifestDigest, archiveId, carBodyDigest]
                  .allSatisfy({ $0.bytes.contains(where: { $0 != 0 }) }),
              providerBytes.count == 64,
              providerBytes.allSatisfy({
                  (0x30...0x39).contains($0) || (0x41...0x46).contains($0)
              }) else {
            throw MusubiV1Error.invalidValue("Musubi seed-ingress binding is invalid.")
        }
        try musubiRequireChainIDV1(chainId, field: "Seed-ingress chain ID")
        _ = try CanonicalNorito.encodeCompactAccountId(publisher)
        _ = try CanonicalNorito.encodeCompactAccountId(ingressBroker)
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(MusubiJSONValueV1.object(raw))
    }
}

/// One controller approval over a first-release seed-ingress receipt.
public struct MusubiSeedIngressReceiptApprovalV1: Codable, Hashable, Sendable {
    public let publicKey: String
    public let signature: String
    public let raw: [String: MusubiJSONValueV1]

    public init(publicKey: String, signature: String) throws {
        let parsed = try MusubiControllerApprovalV1(
            publicKey: publicKey,
            signature: signature
        )
        self.publicKey = parsed.publicKey
        self.signature = parsed.signature
        self.raw = ["public_key": .string(publicKey), "signature": .string(signature)]
    }

    public init(from decoder: Decoder) throws {
        let keys: Set<String> = ["public_key", "signature"]
        try musubiRequireExactKeys(decoder, keys)
        let value = try decoder.singleValueContainer().decode(MusubiJSONValueV1.self)
        raw = try musubiRawObject(value, field: "seed-ingress approval", exactKeys: keys)
        publicKey = try musubiRawString(raw["public_key"], field: "approval.public_key")
        signature = try musubiRawString(raw["signature"], field: "approval.signature")
        _ = try MusubiControllerApprovalV1(publicKey: publicKey, signature: signature)
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(MusubiJSONValueV1.object(raw))
    }
}

/// Version-one signed seed-ingress receipt payload.
public struct MusubiSeedIngressReceiptPayloadV1: Codable, Hashable, Sendable {
    public let version: UInt8
    public let binding: MusubiSeedIngressReceiptBindingV1
    public let issuedAtMs: UInt64
    public let expiresAtMs: UInt64
    public let raw: [String: MusubiJSONValueV1]

    public init(
        version: UInt8 = 1,
        binding: MusubiSeedIngressReceiptBindingV1,
        issuedAtMs: UInt64,
        expiresAtMs: UInt64
    ) throws {
        guard version == 1, issuedAtMs > 0, expiresAtMs > issuedAtMs,
              expiresAtMs - issuedAtMs <= 86_400_000 else {
            throw MusubiV1Error.invalidValue("Seed-ingress receipt lifetime is invalid.")
        }
        self.version = version
        self.binding = binding
        self.issuedAtMs = issuedAtMs
        self.expiresAtMs = expiresAtMs
        self.raw = [
            "version": .unsigned(UInt64(version)),
            "binding": .object(binding.raw),
            "issued_at_ms": .unsigned(issuedAtMs),
            "expires_at_ms": .unsigned(expiresAtMs),
        ]
    }

    public init(from decoder: Decoder) throws {
        let keys: Set<String> = ["version", "binding", "issued_at_ms", "expires_at_ms"]
        try musubiRequireExactKeys(decoder, keys)
        let value = try decoder.singleValueContainer().decode(MusubiJSONValueV1.self)
        raw = try musubiRawObject(value, field: "seed-ingress payload", exactKeys: keys)
        let rawVersion = try musubiRawUnsigned(raw["version"], field: "receipt.version")
        guard rawVersion == 1, let bindingValue = raw["binding"] else {
            throw MusubiV1Error.unsupportedVersion("Seed-ingress receipt is not V1.")
        }
        version = UInt8(rawVersion)
        binding = try musubiDecodeRaw(bindingValue, as: MusubiSeedIngressReceiptBindingV1.self)
        issuedAtMs = try musubiRawUnsigned(raw["issued_at_ms"], field: "receipt.issued_at_ms")
        expiresAtMs = try musubiRawUnsigned(
            raw["expires_at_ms"], field: "receipt.expires_at_ms"
        )
        guard issuedAtMs > 0, expiresAtMs > issuedAtMs,
              expiresAtMs - issuedAtMs <= 86_400_000 else {
            throw MusubiV1Error.invalidValue("Seed-ingress receipt lifetime is invalid.")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(MusubiJSONValueV1.object(raw))
    }
}

/// Authenticated seed-ingress receipt retained by archive registration.
public struct MusubiSeedIngressReceiptV1: Codable, Hashable, Sendable {
    public let payload: MusubiSeedIngressReceiptPayloadV1
    public let approvals: [MusubiSeedIngressReceiptApprovalV1]
    public let raw: [String: MusubiJSONValueV1]

    public init(
        payload: MusubiSeedIngressReceiptPayloadV1,
        approvals: [MusubiSeedIngressReceiptApprovalV1]
    ) throws {
        guard !approvals.isEmpty, approvals.count <= 64 else {
            throw MusubiV1Error.invalidValue(
                "Seed-ingress receipt approvals must be bounded."
            )
        }
        let parsed = try approvals.map {
            try MusubiControllerApprovalV1(publicKey: $0.publicKey, signature: $0.signature)
        }
        guard zip(parsed, parsed.dropFirst()).allSatisfy({ $0.0 < $0.1 }) else {
            throw MusubiV1Error.invalidValue(
                "Seed-ingress receipt approvals must be sorted and distinct."
            )
        }
        self.payload = payload
        self.approvals = approvals
        self.raw = [
            "payload": .object(payload.raw),
            "approvals": .array(approvals.map { .object($0.raw) }),
        ]
    }

    public init(from decoder: Decoder) throws {
        let keys: Set<String> = ["payload", "approvals"]
        try musubiRequireExactKeys(decoder, keys)
        let value = try decoder.singleValueContainer().decode(MusubiJSONValueV1.self)
        raw = try musubiRawObject(value, field: "seed-ingress receipt", exactKeys: keys)
        guard let payloadValue = raw["payload"] else {
            throw MusubiV1Error.invalidValue("Seed-ingress receipt payload is missing.")
        }
        payload = try musubiDecodeRaw(payloadValue, as: MusubiSeedIngressReceiptPayloadV1.self)
        approvals = try musubiRawArray(raw["approvals"], field: "receipt.approvals").map {
            try musubiDecodeRaw($0, as: MusubiSeedIngressReceiptApprovalV1.self)
        }
        guard !approvals.isEmpty, approvals.count <= 64 else {
            throw MusubiV1Error.invalidValue(
                "Seed-ingress receipt approvals must be bounded."
            )
        }
        let parsed = try approvals.map {
            try MusubiControllerApprovalV1(publicKey: $0.publicKey, signature: $0.signature)
        }
        guard zip(parsed, parsed.dropFirst()).allSatisfy({ $0.0 < $0.1 }) else {
            throw MusubiV1Error.invalidValue(
                "Seed-ingress receipt approvals must be sorted and distinct."
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(MusubiJSONValueV1.object(raw))
    }
}

/// Authoritative immutable archive registration independent of renewable locations.
public struct MusubiArchiveRecordV1: Codable, Hashable, Sendable {
    public let archiveId: MusubiDigest32V1
    public let commitment: MusubiArchiveCommitmentV1
    public let stagingReceipt: MusubiSeedIngressReceiptV1
    public let registeredBy: String
    public let registeredAtHeight: UInt64
    public let locationRevision: UInt64
    public let locationIds: [MusubiDigest32V1]
    public let raw: [String: MusubiJSONValueV1]

    public init(from decoder: Decoder) throws {
        let keys: Set<String> = [
            "archive_id", "commitment", "staging_receipt", "registered_by",
            "registered_at_height", "location_revision", "location_ids"
        ]
        try musubiRequireExactKeys(decoder, keys)
        let value = try decoder.singleValueContainer().decode(MusubiJSONValueV1.self)
        raw = try musubiRawObject(value, field: "archive record", exactKeys: keys)
        guard let archive = raw["archive_id"], let commitmentValue = raw["commitment"],
              let receipt = raw["staging_receipt"] else {
            throw MusubiV1Error.invalidValue("Archive registration is incomplete.")
        }
        archiveId = try musubiDecodeRaw(archive, as: MusubiDigest32V1.self)
        commitment = try musubiDecodeRaw(commitmentValue, as: MusubiArchiveCommitmentV1.self)
        stagingReceipt = try musubiDecodeRaw(receipt, as: MusubiSeedIngressReceiptV1.self)
        registeredBy = try musubiRawString(raw["registered_by"], field: "archive.registered_by")
        registeredAtHeight = try musubiRawUnsigned(
            raw["registered_at_height"], field: "archive.registered_at_height"
        )
        locationRevision = try musubiRawUnsigned(
            raw["location_revision"], field: "archive.location_revision"
        )
        locationIds = try musubiRawArray(raw["location_ids"], field: "archive.location_ids").map {
            try musubiDecodeRaw($0, as: MusubiDigest32V1.self)
        }
        guard stagingReceipt.payload.binding.archiveId == archiveId,
              stagingReceipt.payload.binding.carBodyDigest == commitment.carDigest,
              stagingReceipt.payload.binding.carBodyLength == commitment.carSize,
              stagingReceipt.payload.binding.publisher == registeredBy,
              registeredAtHeight > 0, locationRevision > 0, locationIds.count <= 4,
              locationIds.allSatisfy({ $0.bytes.contains(where: { $0 != 0 }) }),
              zip(locationIds, locationIds.dropFirst()).allSatisfy({ pair in
                  musubiCompareUnsignedBytes(pair.0.bytes, pair.1.bytes) < 0
              }) else {
            throw MusubiV1Error.invalidValue("Archive registration binding is invalid.")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(MusubiJSONValueV1.object(raw))
    }
}

/// Renewable archive-location record with strict outer fields.
public struct MusubiArchiveLocationV1: Codable, Hashable, Sendable {
    public let locationId: MusubiDigest32V1
    public let archiveId: MusubiDigest32V1
    public let revision: UInt64
    public let stateKind: String
    public let raw: [String: MusubiJSONValueV1]

    public init(from decoder: Decoder) throws {
        let keys: Set<String> = [
            "location_id", "archive_id", "pin_manifest", "replication_order", "providers",
            "provider_attestations", "renew_after_epoch", "expires_at_epoch",
            "finalized_height", "revision", "state"
        ]
        try musubiRequireExactKeys(decoder, keys)
        let value = try decoder.singleValueContainer().decode(MusubiJSONValueV1.self)
        raw = try musubiRawObject(value, field: "archive location", exactKeys: keys)
        guard let locationRaw = raw["location_id"], let archiveRaw = raw["archive_id"] else {
            throw MusubiV1Error.invalidValue("archive location identity is missing.")
        }
        locationId = try musubiDecodeRaw(locationRaw, as: MusubiDigest32V1.self)
        archiveId = try musubiDecodeRaw(archiveRaw, as: MusubiDigest32V1.self)
        revision = try musubiRawUnsigned(raw["revision"], field: "location.revision")
        stateKind = try musubiValidateTaggedUnit(
            raw["state"], field: "location.state", allowed: ["Pending", "Healthy", "Degraded", "Retired"]
        )
        _ = try musubiRawArray(raw["providers"], field: "location.providers")
        _ = try musubiRawArray(raw["provider_attestations"], field: "location.provider_attestations")
        guard raw["pin_manifest"] != nil, raw["replication_order"] != nil, revision > 0 else {
            throw MusubiV1Error.invalidValue("archive location is incomplete.")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(MusubiJSONValueV1.object(raw))
    }
}

/// Archive-location page carrying deployment identity and the immutable commitment.
public struct MusubiArchiveLocationPageV1: Codable, Hashable, Sendable {
    public let chainId: String
    public let genesisHash: [UInt8]
    public let archive: MusubiArchiveRecordV1
    public let items: [MusubiArchiveLocationV1]
    public let nextCursor: MusubiFinalizedCursorV1?
    public let snapshot: MusubiRegistrySnapshotV1

    private enum CodingKeys: String, CodingKey {
        case chainId = "chain_id"
        case genesisHash = "genesis_hash"
        case archive, items
        case nextCursor = "next_cursor"
        case snapshot
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(
            decoder,
            ["chain_id", "genesis_hash", "archive", "items", "next_cursor", "snapshot"]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        chainId = try container.decode(String.self, forKey: .chainId)
        genesisHash = try container.decode([UInt8].self, forKey: .genesisHash)
        archive = try container.decode(MusubiArchiveRecordV1.self, forKey: .archive)
        items = try container.decode([MusubiArchiveLocationV1].self, forKey: .items)
        nextCursor = try container.decodeIfPresent(MusubiFinalizedCursorV1.self, forKey: .nextCursor)
        snapshot = try container.decode(MusubiRegistrySnapshotV1.self, forKey: .snapshot)
        try musubiRequireExactText(chainId, field: "Archive-location chain ID")
        guard genesisHash.count == 32, genesisHash.contains(where: { $0 != 0 }),
              archive.stagingReceipt.payload.binding.chainId == chainId,
              archive.stagingReceipt.payload.binding.genesisBlockHash == genesisHash,
              archive.registeredAtHeight <= snapshot.finalizedHeight,
              items.count <= 4,
              items.allSatisfy({ $0.archiveId == archive.archiveId }),
              nextCursor == nil || nextCursor?.snapshot == snapshot else {
            throw MusubiV1Error.invalidValue(
                "Archive-location page has an inconsistent deployment, archive, or cursor."
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(chainId, forKey: .chainId)
        try container.encode(genesisHash, forKey: .genesisHash)
        try container.encode(archive, forKey: .archive)
        try container.encode(items, forKey: .items)
        if let nextCursor { try container.encode(nextCursor, forKey: .nextCursor) }
        else { try container.encodeNil(forKey: .nextCursor) }
        try container.encode(snapshot, forKey: .snapshot)
    }
}

/// Exact finalized cache-retention decisions for one bounded request batch.
public struct MusubiArchiveRetentionPageV1: Codable, Hashable, Sendable {
    public let chainId: String
    public let genesisHash: [UInt8]
    public let items: [MusubiArchiveRetentionDecisionV1]
    public let snapshot: MusubiRegistrySnapshotV1

    private enum CodingKeys: String, CodingKey {
        case chainId = "chain_id"
        case genesisHash = "genesis_hash"
        case items, snapshot
    }

    public init(
        chainId: String,
        genesisHash: [UInt8],
        items: [MusubiArchiveRetentionDecisionV1],
        snapshot: MusubiRegistrySnapshotV1
    ) throws {
        try musubiRequireExactText(chainId, field: "Musubi archive-retention chain ID")
        let canonicalItems = zip(items, items.dropFirst()).allSatisfy { pair in
            musubiCompareUnsignedBytes(pair.0.archiveId.bytes, pair.1.archiveId.bytes) < 0
        }
        let anchoredItems = items.allSatisfy { decision in
            guard let storage = decision.storage else { return true }
            return storage.finalizedHeight <= snapshot.finalizedHeight
                && storage.indexRevision <= snapshot.indexRevision
                && (storage.finalizedHeight != snapshot.finalizedHeight
                    || storage.finalizedBlockHash == snapshot.finalizedBlockHash)
        }
        guard genesisHash.count == 32, genesisHash.contains(where: { $0 != 0 }),
              !items.isEmpty, items.count <= 100, canonicalItems, anchoredItems else {
            throw MusubiV1Error.invalidValue(
                "Musubi archive-retention page has an invalid deployment or item bound."
            )
        }
        self.chainId = chainId
        self.genesisHash = genesisHash
        self.items = items
        self.snapshot = snapshot
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["chain_id", "genesis_hash", "items", "snapshot"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            chainId: container.decode(String.self, forKey: .chainId),
            genesisHash: container.decode([UInt8].self, forKey: .genesisHash),
            items: container.decode(
                [MusubiArchiveRetentionDecisionV1].self,
                forKey: .items
            ),
            snapshot: container.decode(MusubiRegistrySnapshotV1.self, forKey: .snapshot)
        )
    }

    /// Enforces the exact request identity order and optional snapshot binding.
    public func requireMatches(_ request: MusubiArchiveRetentionQueryV1) throws {
        guard (request.expectedSnapshot == nil || request.expectedSnapshot == snapshot),
              items.map(\.archiveId) == request.archiveIds else {
            throw MusubiV1Error.invalidValue(
                "Musubi archive-retention response does not match the exact request."
            )
        }
    }
}

/// Permanent global alias record.
public struct MusubiAliasRecordV1: Codable, Hashable, Sendable {
    public let alias: String
    public let target: MusubiPackageIdV1
    public let registeredBy: String
    public let pricingRevision: UInt64
    public let paidXor: UInt64
    public let registeredAtHeight: UInt64
    public let historyRevision: UInt64

    private enum CodingKeys: String, CodingKey {
        case alias, target
        case registeredBy = "registered_by"
        case pricingRevision = "pricing_revision"
        case paidXor = "paid_xor"
        case registeredAtHeight = "registered_at_height"
        case historyRevision = "history_revision"
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(
            decoder,
            [
                "alias", "target", "registered_by", "pricing_revision", "paid_xor",
                "registered_at_height", "history_revision"
            ]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        alias = try musubiDecodeNewtypeText(container, forKey: .alias, field: "Musubi alias")
        try musubiRequireASCIILowerKebab(alias, maximum: 32, field: "Musubi alias")
        target = try container.decode(MusubiPackageIdV1.self, forKey: .target)
        registeredBy = try container.decode(String.self, forKey: .registeredBy)
        pricingRevision = try container.decode(UInt64.self, forKey: .pricingRevision)
        paidXor = try container.decode(UInt64.self, forKey: .paidXor)
        registeredAtHeight = try container.decode(UInt64.self, forKey: .registeredAtHeight)
        historyRevision = try container.decode(UInt64.self, forKey: .historyRevision)
        guard pricingRevision > 0, registeredAtHeight > 0, historyRevision > 0 else {
            throw MusubiV1Error.invalidValue("Alias heights and revisions must be non-zero.")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode([alias], forKey: .alias)
        try container.encode(target, forKey: .target)
        try container.encode(registeredBy, forKey: .registeredBy)
        try container.encode(pricingRevision, forKey: .pricingRevision)
        try container.encode(paidXor, forKey: .paidXor)
        try container.encode(registeredAtHeight, forKey: .registeredAtHeight)
        try container.encode(historyRevision, forKey: .historyRevision)
    }
}

/// Immutable alias history action.
public enum MusubiAliasHistoryActionV1: String, Codable, Hashable, Sendable {
    case registered = "Registered"
    case parliamentRetarget = "ParliamentRetarget"

    private enum CodingKeys: String, CodingKey { case kind, value }
    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["kind", "value"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        guard try container.decodeNil(forKey: .value),
              let action = Self(rawValue: try container.decode(String.self, forKey: .kind)) else {
            throw MusubiV1Error.unsupportedVersion("Unsupported Musubi V1 alias history action.")
        }
        self = action
    }
    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(rawValue, forKey: .kind)
        try container.encodeNil(forKey: .value)
    }
}

/// One immutable alias-history entry.
public struct MusubiAliasHistoryEntryV1: Codable, Hashable, Sendable {
    public let alias: String
    public let revision: UInt64
    public let action: MusubiAliasHistoryActionV1
    public let previousTarget: MusubiPackageIdV1?
    public let target: MusubiPackageIdV1
    public let governanceAction: MusubiDigest32V1?
    public let finalizedHeight: UInt64

    private enum CodingKeys: String, CodingKey {
        case alias, revision, action
        case previousTarget = "previous_target"
        case target
        case governanceAction = "governance_action"
        case finalizedHeight = "finalized_height"
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(
            decoder,
            [
                "alias", "revision", "action", "previous_target", "target",
                "governance_action", "finalized_height"
            ]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        alias = try musubiDecodeNewtypeText(container, forKey: .alias, field: "Musubi alias")
        try musubiRequireASCIILowerKebab(alias, maximum: 32, field: "Musubi alias")
        revision = try container.decode(UInt64.self, forKey: .revision)
        action = try container.decode(MusubiAliasHistoryActionV1.self, forKey: .action)
        previousTarget = try container.decodeIfPresent(MusubiPackageIdV1.self, forKey: .previousTarget)
        target = try container.decode(MusubiPackageIdV1.self, forKey: .target)
        governanceAction = try container.decodeIfPresent(MusubiDigest32V1.self, forKey: .governanceAction)
        finalizedHeight = try container.decode(UInt64.self, forKey: .finalizedHeight)
        guard revision > 0, finalizedHeight > 0 else {
            throw MusubiV1Error.invalidValue("Alias history height and revision must be non-zero.")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode([alias], forKey: .alias)
        try container.encode(revision, forKey: .revision)
        try container.encode(action, forKey: .action)
        if let previousTarget { try container.encode(previousTarget, forKey: .previousTarget) }
        else { try container.encodeNil(forKey: .previousTarget) }
        try container.encode(target, forKey: .target)
        if let governanceAction { try container.encode(governanceAction, forKey: .governanceAction) }
        else { try container.encodeNil(forKey: .governanceAction) }
        try container.encode(finalizedHeight, forKey: .finalizedHeight)
    }
}

/// Ordered public-directory package entry.
public struct MusubiOrderedPackageEntryV1: Codable, Hashable, Sendable {
    public let selector: MusubiPackageSelectorV1
    public let package: MusubiPackageIdV1
    public let latestSelectable: MusubiVersionV1?
    public let metadataRevision: UInt64
    public let indexRevision: UInt64

    private enum CodingKeys: String, CodingKey {
        case selector, package
        case latestSelectable = "latest_selectable"
        case metadataRevision = "metadata_revision"
        case indexRevision = "index_revision"
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(
            decoder,
            ["selector", "package", "latest_selectable", "metadata_revision", "index_revision"]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        selector = try container.decode(MusubiPackageSelectorV1.self, forKey: .selector)
        package = try container.decode(MusubiPackageIdV1.self, forKey: .package)
        latestSelectable = try container.decodeIfPresent(MusubiVersionV1.self, forKey: .latestSelectable)
        metadataRevision = try container.decode(UInt64.self, forKey: .metadataRevision)
        indexRevision = try container.decode(UInt64.self, forKey: .indexRevision)
        guard metadataRevision > 0, indexRevision > 0 else {
            throw MusubiV1Error.invalidValue("Directory revisions must be non-zero.")
        }
        guard selector.name == package.name else {
            throw MusubiV1Error.invalidValue(
                "Musubi ordered entry selector and package names disagree."
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(selector, forKey: .selector)
        try container.encode(package, forKey: .package)
        if let latestSelectable { try container.encode(latestSelectable, forKey: .latestSelectable) }
        else { try container.encodeNil(forKey: .latestSelectable) }
        try container.encode(metadataRevision, forKey: .metadataRevision)
        try container.encode(indexRevision, forKey: .indexRevision)
    }
}

/// Ordered-directory page carrying exact chain/genesis identity for lock creation.
public struct MusubiOrderedPrefixPageV1: Codable, Hashable, Sendable {
    public let chainId: String
    public let genesisHash: [UInt8]
    public let namespaceBinding: MusubiNamespaceBindingV1
    public let items: [MusubiOrderedPackageEntryV1]
    public let nextCursor: MusubiFinalizedCursorV1?
    public let snapshot: MusubiRegistrySnapshotV1

    private enum CodingKeys: String, CodingKey {
        case chainId = "chain_id"
        case genesisHash = "genesis_hash"
        case namespaceBinding = "namespace_binding"
        case items
        case nextCursor = "next_cursor"
        case snapshot
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(
            decoder,
            [
                "chain_id", "genesis_hash", "namespace_binding", "items", "next_cursor",
                "snapshot"
            ]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        chainId = try container.decode(String.self, forKey: .chainId)
        try musubiRequireExactText(chainId, field: "Musubi directory chain ID")
        genesisHash = try container.decode([UInt8].self, forKey: .genesisHash)
        namespaceBinding = try container.decode(
            MusubiNamespaceBindingV1.self, forKey: .namespaceBinding
        )
        items = try container.decode([MusubiOrderedPackageEntryV1].self, forKey: .items)
        nextCursor = try container.decodeIfPresent(MusubiFinalizedCursorV1.self, forKey: .nextCursor)
        snapshot = try container.decode(MusubiRegistrySnapshotV1.self, forKey: .snapshot)
        let ordered = zip(items, items.dropFirst()).allSatisfy { pair in
            let namespaceOrder = musubiCompareUnsignedBytes(
                Array(pair.0.selector.namespace.value.utf8),
                Array(pair.1.selector.namespace.value.utf8)
            )
            return namespaceOrder < 0 || namespaceOrder == 0
                && musubiCompareUnsignedBytes(
                    Array(pair.0.selector.name.value.utf8),
                    Array(pair.1.selector.name.value.utf8)
                ) < 0
        }
        guard genesisHash.count == 32, genesisHash.contains(where: { $0 != 0 }),
              items.count <= 100,
              items.allSatisfy({ item in
                  item.selector.namespace == namespaceBinding.namespace
                      && item.package.homeDataspace == namespaceBinding.homeDataspace
                      && item.package.scope == namespaceBinding.scope
              }),
              ordered,
              nextCursor == nil || nextCursor?.snapshot == snapshot else {
            throw MusubiV1Error.invalidValue(
                "Musubi ordered-prefix page has an invalid genesis hash, size, or cursor."
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        guard genesisHash.count == 32, genesisHash.contains(where: { $0 != 0 }) else {
            throw MusubiV1Error.invalidValue("Musubi genesis hash must contain 32 bytes.")
        }
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(chainId, forKey: .chainId)
        try container.encode(genesisHash, forKey: .genesisHash)
        try container.encode(namespaceBinding, forKey: .namespaceBinding)
        try container.encode(items, forKey: .items)
        if let nextCursor { try container.encode(nextCursor, forKey: .nextCursor) }
        else { try container.encodeNil(forKey: .nextCursor) }
        try container.encode(snapshot, forKey: .snapshot)
    }
}

private func musubiPackageIdLessThan(
    _ left: MusubiPackageIdV1,
    _ right: MusubiPackageIdV1
) -> Bool {
    if left.homeDataspace != right.homeDataspace {
        return left.homeDataspace < right.homeDataspace
    }
    switch (left.scope, right.scope) {
    case (.dataspaceRoot, .domain): return true
    case (.domain, .dataspaceRoot): return false
    case let (.domain(leftDomain), .domain(rightDomain)) where leftDomain != rightDomain:
        return musubiCompareUnsignedBytes(Array(leftDomain.utf8), Array(rightDomain.utf8)) < 0
    default: break
    }
    return musubiCompareUnsignedBytes(Array(left.name.value.utf8), Array(right.name.value.utf8)) < 0
}

private func musubiNamespaceMatchesScope(
    package: MusubiPackageIdV1,
    namespace: MusubiNamespaceV1
) -> Bool {
    let components = namespace.value.split(separator: ".", omittingEmptySubsequences: false)
    switch package.scope {
    case .dataspaceRoot: return components.count == 1
    case .domain(let domain):
        return components.count == 2 && components.first.map(String.init) == domain
    }
}

/// One exact-token package metadata search result.
public struct MusubiSearchHitV1: Codable, Hashable, Sendable {
    public let package: MusubiPackageIdV1
    public let claimedNamespace: MusubiNamespaceV1
    public let description: String?
    public let keywords: [String]
    public let metadataRevision: UInt64

    public init(
        package: MusubiPackageIdV1,
        claimedNamespace: MusubiNamespaceV1,
        description: String?,
        keywords: [String],
        metadataRevision: UInt64
    ) throws {
        if let description {
            try musubiRequireExactText(description, field: "Musubi search description")
            guard description.utf8.count <= 4_096 else {
                throw MusubiV1Error.invalidValue(
                    "Musubi search description exceeds 4096 UTF-8 bytes."
                )
            }
        }
        guard keywords.count <= 32,
              keywords == Array(Set(keywords)).sorted(),
              metadataRevision > 0,
              musubiNamespaceMatchesScope(package: package, namespace: claimedNamespace) else {
            throw MusubiV1Error.invalidValue("Musubi search hit is invalid.")
        }
        for keyword in keywords {
            try musubiRequireASCIILowerKebab(
                keyword, maximum: 64, field: "Musubi search keyword"
            )
        }
        self.package = package
        self.claimedNamespace = claimedNamespace
        self.description = description
        self.keywords = keywords
        self.metadataRevision = metadataRevision
    }

    private enum CodingKeys: String, CodingKey {
        case package
        case claimedNamespace = "claimed_namespace"
        case description, keywords
        case metadataRevision = "metadata_revision"
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(
            decoder,
            ["package", "claimed_namespace", "description", "keywords", "metadata_revision"]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let descriptionValues = try container.decodeIfPresent([String].self, forKey: .description)
        guard descriptionValues == nil || descriptionValues?.count == 1 else {
            throw MusubiV1Error.invalidValue(
                "Musubi search description must contain one Norito newtype item."
            )
        }
        let keywordValues = try container.decode([[String]].self, forKey: .keywords)
        guard keywordValues.allSatisfy({ $0.count == 1 }) else {
            throw MusubiV1Error.invalidValue(
                "Musubi search keyword must contain one Norito newtype item."
            )
        }
        try self.init(
            package: container.decode(MusubiPackageIdV1.self, forKey: .package),
            claimedNamespace: container.decode(
                MusubiNamespaceV1.self, forKey: .claimedNamespace
            ),
            description: descriptionValues?.first,
            keywords: keywordValues.compactMap(\.first),
            metadataRevision: container.decode(UInt64.self, forKey: .metadataRevision)
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(package, forKey: .package)
        try container.encode(claimedNamespace, forKey: .claimedNamespace)
        if let description { try container.encode([description], forKey: .description) }
        else { try container.encodeNil(forKey: .description) }
        try container.encode(keywords.map { [$0] }, forKey: .keywords)
        try container.encode(metadataRevision, forKey: .metadataRevision)
    }
}

/// Bounded page from the finalized-event package-search projection.
public struct MusubiSearchPageV1: Codable, Hashable, Sendable {
    public let items: [MusubiSearchHitV1]
    public let nextCursor: MusubiSearchCursorV1?
    public let snapshot: MusubiSearchSnapshotV1

    public init(
        items: [MusubiSearchHitV1],
        nextCursor: MusubiSearchCursorV1?,
        snapshot: MusubiSearchSnapshotV1
    ) throws {
        let ordered = zip(items, items.dropFirst()).allSatisfy {
            musubiPackageIdLessThan($0.0.package, $0.1.package)
        }
        guard items.count <= 100, ordered,
              nextCursor == nil || nextCursor?.snapshot == snapshot,
              nextCursor == nil || nextCursor?.lastPackage == items.last?.package else {
            throw MusubiV1Error.invalidValue("Musubi search page is invalid.")
        }
        self.items = items
        self.nextCursor = nextCursor
        self.snapshot = snapshot
    }

    private enum CodingKeys: String, CodingKey {
        case items
        case nextCursor = "next_cursor"
        case snapshot
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["items", "next_cursor", "snapshot"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            items: container.decode([MusubiSearchHitV1].self, forKey: .items),
            nextCursor: container.decodeIfPresent(MusubiSearchCursorV1.self, forKey: .nextCursor),
            snapshot: container.decode(MusubiSearchSnapshotV1.self, forKey: .snapshot)
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(items, forKey: .items)
        if let nextCursor { try container.encode(nextCursor, forKey: .nextCursor) }
        else { try container.encodeNil(forKey: .nextCursor) }
        try container.encode(snapshot, forKey: .snapshot)
    }
}

/// Typed bounded page shared by all finalized Musubi list responses.
public struct MusubiPageV1<Item: Codable & Hashable & Sendable>: Codable, Hashable, Sendable {
    public let items: [Item]
    public let nextCursor: MusubiFinalizedCursorV1?
    public let snapshot: MusubiRegistrySnapshotV1

    public init(
        items: [Item],
        nextCursor: MusubiFinalizedCursorV1?,
        snapshot: MusubiRegistrySnapshotV1
    ) throws {
        guard items.count <= 100, nextCursor == nil || nextCursor?.snapshot == snapshot else {
            throw MusubiV1Error.invalidValue("Musubi page is oversized or has a mismatched cursor.")
        }
        self.items = items
        self.nextCursor = nextCursor
        self.snapshot = snapshot
    }

    private enum CodingKeys: String, CodingKey {
        case items
        case nextCursor = "next_cursor"
        case snapshot
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["items", "next_cursor", "snapshot"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            items: container.decode([Item].self, forKey: .items),
            nextCursor: container.decodeIfPresent(MusubiFinalizedCursorV1.self, forKey: .nextCursor),
            snapshot: container.decode(MusubiRegistrySnapshotV1.self, forKey: .snapshot)
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(items, forKey: .items)
        if let nextCursor { try container.encode(nextCursor, forKey: .nextCursor) }
        else { try container.encodeNil(forKey: .nextCursor) }
        try container.encode(snapshot, forKey: .snapshot)
    }
}

// MARK: - Publication mutation values

private func musubiDecodeVarintV1(_ bytes: [UInt8], from start: Int) throws -> (UInt64, Int) {
    var value: UInt64 = 0
    var shift: UInt64 = 0
    var index = start
    while index < bytes.count, shift <= 63 {
        let byte = bytes[index]
        index += 1
        let payload = UInt64(byte & 0x7f)
        guard shift != 63 || payload <= 1 else {
            throw MusubiV1Error.invalidValue("Musubi public-key multihash varint overflows.")
        }
        value |= payload << shift
        if byte & 0x80 == 0 {
            guard index - start == 1 || payload != 0 else {
                throw MusubiV1Error.invalidValue(
                    "Musubi public-key multihash uses a noncanonical varint."
                )
            }
            return (value, index)
        }
        shift += 7
    }
    throw MusubiV1Error.invalidValue("Musubi public-key multihash is truncated.")
}

private func musubiSigningAlgorithmV1(_ code: UInt64) -> SigningAlgorithm? {
    switch code {
    case 0xed: return .ed25519
    case 0xe7: return .secp256k1
    case 0xea: return .blsNormal
    case 0xeb: return .blsSmall
    case 0xee: return .mlDsa
    case 0x1200: return .gost2012_256A
    case 0x1201: return .gost2012_256B
    case 0x1202: return .gost2012_256C
    case 0x1203: return .gost2012_512A
    case 0x1204: return .gost2012_512B
    case 0x1306: return .sm2
    default: return nil
    }
}

/// Canonical controller key and typed-signature bytes used by Musubi signed proofs.
public struct MusubiControllerApprovalV1: Hashable, Sendable, Comparable {
    public let publicKey: String
    public let signature: String
    let algorithm: SigningAlgorithm
    let publicKeyPayload: [UInt8]
    let signaturePayload: [UInt8]

    public init(publicKey: String, signature: String) throws {
        guard publicKey == publicKey.trimmingCharacters(in: .whitespacesAndNewlines),
              let encodedKey = Data(hexString: publicKey) else {
            throw MusubiV1Error.invalidValue("Musubi approval public key is not canonical hex.")
        }
        let keyBytes = [UInt8](encodedKey)
        let (code, codeEnd) = try musubiDecodeVarintV1(keyBytes, from: 0)
        let (length, payloadStart) = try musubiDecodeVarintV1(keyBytes, from: codeEnd)
        guard let algorithm = musubiSigningAlgorithmV1(code),
              length <= UInt64(Int.max),
              keyBytes.count - payloadStart == Int(length) else {
            throw MusubiV1Error.invalidValue("Musubi approval public key has an invalid multihash.")
        }
        let keyPayload = Data(keyBytes[payloadStart...])
        guard CanonicalNorito.publicKeyMultihash(
            algorithm: algorithm,
            payload: keyPayload
        ) == publicKey else {
            throw MusubiV1Error.invalidValue("Musubi approval public key is noncanonical.")
        }
        if algorithm == .ed25519, !Ed25519PublicKeyAdmission.isValidPublicKey(keyPayload) {
            throw MusubiV1Error.invalidValue("Musubi approval Ed25519 public key is invalid.")
        }

        guard signature == signature.uppercased(), !signature.hasPrefix("0X"),
              let signatureBytes = Data(hexString: signature),
              !signatureBytes.isEmpty, signatureBytes.count <= 16_384,
              signatureBytes.contains(where: { $0 != 0 }) else {
            throw MusubiV1Error.invalidValue("Musubi approval signature is not canonical hex.")
        }
        if algorithm == .ed25519,
           !Ed25519SignatureAdmission.isValidSignature(signatureBytes) {
            throw MusubiV1Error.invalidValue("Musubi approval Ed25519 signature is invalid.")
        }
        self.publicKey = publicKey
        self.signature = signature
        self.algorithm = algorithm
        self.publicKeyPayload = [UInt8](keyPayload)
        self.signaturePayload = [UInt8](signatureBytes)
    }

    public static func < (lhs: Self, rhs: Self) -> Bool {
        if lhs.algorithm.noritoDiscriminant != rhs.algorithm.noritoDiscriminant {
            return lhs.algorithm.noritoDiscriminant < rhs.algorithm.noritoDiscriminant
        }
        return lhs.publicKeyPayload.lexicographicallyPrecedes(rhs.publicKeyPayload)
    }
}

/// Governed identity of a provider-ingest completion signer policy.
public struct MusubiProviderIngestCompletionSignerPolicyV1: Hashable, Sendable {
    public let policyID: [UInt8]
    public let revision: UInt64
    public let predecessorDigest: [UInt8]?
    public let policyDigest: [UInt8]

    public init(
        policyID: [UInt8],
        revision: UInt64,
        predecessorDigest: [UInt8]?,
        policyDigest: [UInt8]
    ) throws {
        let predecessorIsCanonical = revision == 1
            ? predecessorDigest == nil
            : predecessorDigest?.count == 32
                && predecessorDigest?.contains(where: { $0 != 0 }) == true
        guard policyID.count == 32, policyID.contains(where: { $0 != 0 }),
              revision > 0, predecessorIsCanonical,
              policyDigest.count == 32, policyDigest.contains(where: { $0 != 0 }) else {
            throw MusubiV1Error.invalidValue(
                "Musubi provider completion signer policy is invalid."
            )
        }
        self.policyID = policyID
        self.revision = revision
        self.predecessorDigest = predecessorDigest
        self.policyDigest = policyDigest
    }
}

/// Chain-authoritative provider owner and governed completion signer policy.
public struct MusubiProviderIngestCompletionAuthorityV1: Hashable, Sendable {
    public let providerOwner: String
    public let signerPolicy: MusubiProviderIngestCompletionSignerPolicyV1

    public init(
        providerOwner: String,
        signerPolicy: MusubiProviderIngestCompletionSignerPolicyV1
    ) throws {
        _ = try CanonicalNorito.encodeCompactAccountId(providerOwner)
        self.providerOwner = providerOwner
        self.signerPolicy = signerPolicy
    }
}

/// Finalized committed-chain anchor carried by one provider completion.
public struct MusubiProviderIngestFinalizedAnchorV1: Hashable, Sendable {
    public let height: UInt64
    public let blockHash: [UInt8]

    public init(height: UInt64, blockHash: [UInt8]) throws {
        guard height > 0, blockHash.count == 32,
              blockHash.contains(where: { $0 != 0 }) else {
            throw MusubiV1Error.invalidValue("Musubi provider finalized anchor is invalid.")
        }
        self.height = height
        self.blockHash = blockHash
    }
}

/// Exact parsed-bundle and finalized-replication completion binding.
public struct MusubiProviderBundleVerificationBindingV1: Hashable, Sendable {
    public let chainID: String
    public let genesisBlockHash: [UInt8]
    public let providerID: MusubiDigest32V1
    public let completedBy: String
    public let completionAuthority: MusubiProviderIngestCompletionAuthorityV1
    public let replicationOrder: MusubiDigest32V1
    public let assignmentRevision: UInt64
    public let completionEpoch: UInt64
    public let finalizedAnchor: MusubiProviderIngestFinalizedAnchorV1
    public let archiveID: MusubiDigest32V1
    public let bundleDigest: MusubiDigest32V1
    public let descriptorDigest: MusubiDigest32V1
    public let semanticReleaseManifestDigest: MusubiDigest32V1
    public let verificationLockDigest: MusubiDigest32V1
    public let sourceTreeDigest: MusubiDigest32V1

    public init(
        chainID: String,
        genesisBlockHash: [UInt8],
        providerID: MusubiDigest32V1,
        completedBy: String,
        completionAuthority: MusubiProviderIngestCompletionAuthorityV1,
        replicationOrder: MusubiDigest32V1,
        assignmentRevision: UInt64,
        completionEpoch: UInt64,
        finalizedAnchor: MusubiProviderIngestFinalizedAnchorV1,
        archiveID: MusubiDigest32V1,
        bundleDigest: MusubiDigest32V1,
        descriptorDigest: MusubiDigest32V1,
        semanticReleaseManifestDigest: MusubiDigest32V1,
        verificationLockDigest: MusubiDigest32V1,
        sourceTreeDigest: MusubiDigest32V1
    ) throws {
        try musubiRequireChainIDV1(chainID, field: "Musubi provider chain ID")
        let completedByPayload = try CanonicalNorito.encodeCompactAccountId(completedBy)
        let providerOwnerPayload = try CanonicalNorito.encodeCompactAccountId(
            completionAuthority.providerOwner
        )
        guard genesisBlockHash.count == 32,
              genesisBlockHash.contains(where: { $0 != 0 }),
              completedByPayload == providerOwnerPayload,
              assignmentRevision > 0, completionEpoch > 0,
              [
                  providerID, replicationOrder, archiveID, bundleDigest,
                  descriptorDigest, semanticReleaseManifestDigest,
                  verificationLockDigest, sourceTreeDigest,
              ].allSatisfy({ $0.bytes.contains(where: { $0 != 0 }) }) else {
            throw MusubiV1Error.invalidValue(
                "Musubi provider bundle verification binding is invalid."
            )
        }
        self.chainID = chainID
        self.genesisBlockHash = genesisBlockHash
        self.providerID = providerID
        self.completedBy = completedBy
        self.completionAuthority = completionAuthority
        self.replicationOrder = replicationOrder
        self.assignmentRevision = assignmentRevision
        self.completionEpoch = completionEpoch
        self.finalizedAnchor = finalizedAnchor
        self.archiveID = archiveID
        self.bundleDigest = bundleDigest
        self.descriptorDigest = descriptorDigest
        self.semanticReleaseManifestDigest = semanticReleaseManifestDigest
        self.verificationLockDigest = verificationLockDigest
        self.sourceTreeDigest = sourceTreeDigest
    }
}

/// Version-one provider parsed-bundle statement.
public struct MusubiProviderBundleVerificationPayloadV1: Hashable, Sendable {
    public let version: UInt8
    public let binding: MusubiProviderBundleVerificationBindingV1

    public init(
        version: UInt8 = 1,
        binding: MusubiProviderBundleVerificationBindingV1
    ) throws {
        guard version == 1 else {
            throw MusubiV1Error.unsupportedVersion(
                "Musubi provider bundle verification payload must be V1."
            )
        }
        self.version = version
        self.binding = binding
    }
}

public typealias MusubiProviderBundleVerificationApprovalV1 = MusubiControllerApprovalV1

/// Signed provider proof that a canonical bundle was parsed before completion.
public struct MusubiProviderBundleVerificationAttestationV1: Hashable, Sendable {
    public let payload: MusubiProviderBundleVerificationPayloadV1
    public let approvals: [MusubiProviderBundleVerificationApprovalV1]

    public init(
        payload: MusubiProviderBundleVerificationPayloadV1,
        approvals: [MusubiProviderBundleVerificationApprovalV1]
    ) throws {
        guard !approvals.isEmpty, approvals.count <= 64,
              zip(approvals, approvals.dropFirst()).allSatisfy({ $0.0 < $0.1 }) else {
            throw MusubiV1Error.invalidValue(
                "Musubi provider bundle approvals must be bounded, sorted, and unique."
            )
        }
        self.payload = payload
        self.approvals = approvals
    }
}

/// Exact IVM ABI V1 binding embedded in a release or verification node.
public struct MusubiAbiBindingV1: Hashable, Sendable {
    public let abiVersion: UInt16
    public let abiHash: [UInt8]

    public init(abiVersion: UInt16 = 1, abiHash: [UInt8]) throws {
        guard abiVersion == 1, abiHash.count == 32,
              abiHash.contains(where: { $0 != 0 }) else {
            throw MusubiV1Error.invalidValue("Musubi ABI binding is invalid.")
        }
        self.abiVersion = abiVersion
        self.abiHash = abiHash
    }
}

/// Normal dependency requirement in a published release manifest.
public struct MusubiDependencyReqV1: Hashable, Sendable {
    public let alias: String
    public let package: MusubiPackageIdV1
    public let requirement: MusubiVersionReqV1

    public init(
        alias: String,
        package: MusubiPackageIdV1,
        requirement: MusubiVersionReqV1
    ) throws {
        try musubiRequireName(alias, field: "Musubi dependency alias")
        try musubiValidateVersionRequirementV1(requirement)
        self.alias = alias
        self.package = package
        self.requirement = requirement
    }
}

/// Dependency kind recorded in an exact verification graph.
public enum MusubiDependencyKindV1: UInt32, Hashable, Sendable {
    case normal = 0
    case development = 1
}

/// Parent-local exact edge in a publication proof.
public struct MusubiExactDependencyEdgeV1: Hashable, Sendable {
    public let alias: String
    public let kind: MusubiDependencyKindV1
    public let package: MusubiPackageIdV1
    public let requirement: MusubiVersionReqV1
    public let selected: MusubiReleaseIdV1

    public init(
        alias: String,
        kind: MusubiDependencyKindV1,
        package: MusubiPackageIdV1,
        requirement: MusubiVersionReqV1,
        selected: MusubiReleaseIdV1
    ) throws {
        try musubiRequireName(alias, field: "Musubi exact dependency alias")
        try musubiValidateVersionRequirementV1(requirement)
        guard selected.package == package,
              musubiRequirementMatchesV1(requirement, version: selected.version) else {
            throw MusubiV1Error.invalidValue(
                "Musubi exact dependency does not satisfy its package requirement."
            )
        }
        self.alias = alias
        self.kind = kind
        self.package = package
        self.requirement = requirement
        self.selected = selected
    }
}

/// Exact immutable dependency node used in publication verification.
public struct MusubiVerificationNodeV1: Hashable, Sendable {
    public let release: MusubiReleaseIdV1
    public let releaseDigest: MusubiDigest32V1
    public let archiveID: MusubiDigest32V1
    public let sourceDigest: MusubiDigest32V1
    public let interfaceDigest: MusubiDigest32V1
    public let abi: MusubiAbiBindingV1
    public let dependencies: [MusubiExactDependencyEdgeV1]

    public init(
        release: MusubiReleaseIdV1,
        releaseDigest: MusubiDigest32V1,
        archiveID: MusubiDigest32V1,
        sourceDigest: MusubiDigest32V1,
        interfaceDigest: MusubiDigest32V1,
        abi: MusubiAbiBindingV1,
        dependencies: [MusubiExactDependencyEdgeV1]
    ) throws {
        guard dependencies.count <= 256,
              zip(dependencies, dependencies.dropFirst()).allSatisfy({
                  musubiExactDependencyLessV1($0.0, $0.1)
              }),
              [releaseDigest, archiveID, sourceDigest, interfaceDigest]
                  .allSatisfy({ $0.bytes.contains(where: { $0 != 0 }) }) else {
            throw MusubiV1Error.invalidValue("Musubi verification node is invalid.")
        }
        self.release = release
        self.releaseDigest = releaseDigest
        self.archiveID = archiveID
        self.sourceDigest = sourceDigest
        self.interfaceDigest = interfaceDigest
        self.abi = abi
        self.dependencies = dependencies
    }
}

/// Normalized, secret-free exact verification lock packaged with a release.
public struct MusubiVerificationLockV1: Hashable, Sendable {
    public let schema: String
    public let version: UInt8
    public let root: MusubiReleaseIdV1
    public let rootDependencies: [MusubiExactDependencyEdgeV1]
    public let nodes: [MusubiVerificationNodeV1]

    public init(
        schema: String = "musubi-verification-lock",
        version: UInt8 = 1,
        root: MusubiReleaseIdV1,
        rootDependencies: [MusubiExactDependencyEdgeV1],
        nodes: [MusubiVerificationNodeV1]
    ) throws {
        guard schema == "musubi-verification-lock", version == 1,
              rootDependencies.count <= 256, nodes.count <= 1_024,
              rootDependencies.allSatisfy({ $0.kind == .normal }),
              zip(rootDependencies, rootDependencies.dropFirst()).allSatisfy({
                  musubiExactDependencyLessV1($0.0, $0.1)
              }),
              zip(nodes, nodes.dropFirst()).allSatisfy({
                  musubiReleaseLessV1($0.0.release, $0.1.release)
              }) else {
            throw MusubiV1Error.invalidValue("Musubi verification lock is invalid.")
        }
        let byRelease = Dictionary(grouping: nodes, by: \.release)
        guard byRelease.count == nodes.count,
              rootDependencies.allSatisfy({ byRelease[$0.selected]?.count == 1 }) else {
            throw MusubiV1Error.invalidValue(
                "Musubi verification lock has duplicate or missing nodes."
            )
        }
        var complete = Set<MusubiReleaseIdV1>()
        var visiting = Set<MusubiReleaseIdV1>()
        func visit(_ release: MusubiReleaseIdV1, depth: Int) throws {
            guard depth <= 64 else {
                throw MusubiV1Error.invalidValue("Musubi verification graph exceeds depth 64.")
            }
            if complete.contains(release) { return }
            guard visiting.insert(release).inserted,
                  let node = byRelease[release]?.first else {
                throw MusubiV1Error.invalidValue(
                    "Musubi verification graph contains a cycle or missing node."
                )
            }
            for edge in node.dependencies where edge.kind == .normal {
                try visit(edge.selected, depth: depth + 1)
            }
            visiting.remove(release)
            complete.insert(release)
        }
        for node in nodes { try visit(node.release, depth: 1) }
        self.schema = schema
        self.version = version
        self.root = root
        self.rootDependencies = rootDependencies
        self.nodes = nodes
    }
}

/// Bounded exact resolution proof supplied with publication.
public struct MusubiResolutionProofV1: Hashable, Sendable {
    public let snapshot: MusubiRegistrySnapshotV1
    public let lock: MusubiVerificationLockV1

    public init(snapshot: MusubiRegistrySnapshotV1, lock: MusubiVerificationLockV1) {
        self.snapshot = snapshot
        self.lock = lock
    }
}

/// Immutable release metadata and mutable package metadata projection.
public struct MusubiReleaseMetadataV1: Hashable, Sendable {
    public let description: String?
    public let readme: String?
    public let license: String?
    public let repository: String?
    public let keywords: [String]

    public init(
        description: String? = nil,
        readme: String? = nil,
        license: String? = nil,
        repository: String? = nil,
        keywords: [String] = []
    ) throws {
        if let description {
            try musubiRequireExactText(description, field: "Musubi description")
            guard description.utf8.count <= 4_096 else {
                throw MusubiV1Error.invalidValue("Musubi description exceeds 4096 bytes.")
            }
        }
        for (field, value) in [
            ("readme", readme), ("license", license), ("repository", repository),
        ] {
            if let value {
                try musubiRequireExactText(value, field: "Musubi \(field)")
                guard value.utf8.count <= 2_048 else {
                    throw MusubiV1Error.invalidValue("Musubi \(field) exceeds 2048 bytes.")
                }
            }
        }
        guard keywords.count <= 32 else {
            throw MusubiV1Error.invalidValue("Musubi metadata exceeds 32 keywords.")
        }
        for keyword in keywords {
            try musubiRequireASCIILowerKebab(
                keyword, maximum: 64, field: "Musubi keyword"
            )
        }
        guard zip(keywords, keywords.dropFirst()).allSatisfy({ $0.0 < $0.1 }) else {
            throw MusubiV1Error.invalidValue(
                "Musubi keywords must be strictly sorted and unique."
            )
        }
        self.description = description
        self.readme = readme
        self.license = license
        self.repository = repository
        self.keywords = keywords
    }
}

/// First-release Kotodama edition.
public enum MusubiKotodamaEditionV1: UInt32, Hashable, Sendable {
    case v1 = 0
}

/// Immutable registry release manifest.
public struct MusubiReleaseManifestV1: Hashable, Sendable {
    public let release: MusubiReleaseIdV1
    public let edition: MusubiKotodamaEditionV1
    public let abi: MusubiAbiBindingV1
    public let dependencies: [MusubiDependencyReqV1]
    public let exports: [String]
    public let interfaceDigest: MusubiDigest32V1
    public let metadata: MusubiReleaseMetadataV1
    public let archiveID: MusubiDigest32V1
    public let verificationLockDigest: MusubiDigest32V1

    public init(
        release: MusubiReleaseIdV1,
        edition: MusubiKotodamaEditionV1 = .v1,
        abi: MusubiAbiBindingV1,
        dependencies: [MusubiDependencyReqV1],
        exports: [String],
        interfaceDigest: MusubiDigest32V1,
        metadata: MusubiReleaseMetadataV1,
        archiveID: MusubiDigest32V1,
        verificationLockDigest: MusubiDigest32V1
    ) throws {
        guard dependencies.count <= 256, exports.count <= 1_024,
              zip(dependencies, dependencies.dropFirst()).allSatisfy({
                  musubiDependencyReqLessV1($0.0, $0.1)
              }),
              interfaceDigest.bytes.contains(where: { $0 != 0 }),
              archiveID.bytes.contains(where: { $0 != 0 }),
              verificationLockDigest.bytes.contains(where: { $0 != 0 }) else {
            throw MusubiV1Error.invalidValue("Musubi release manifest is invalid.")
        }
        for dependency in dependencies {
            guard dependency.package != release.package else {
                throw MusubiV1Error.invalidValue(
                    "Musubi release cannot depend on its own package."
                )
            }
        }
        for export in exports { try musubiRequireName(export, field: "Musubi export") }
        guard zip(exports, exports.dropFirst()).allSatisfy({
            musubiCompareStringV1($0.0, $0.1) < 0
        }) else {
            throw MusubiV1Error.invalidValue("Musubi exports must be sorted and unique.")
        }
        self.release = release
        self.edition = edition
        self.abi = abi
        self.dependencies = dependencies
        self.exports = exports
        self.interfaceDigest = interfaceDigest
        self.metadata = metadata
        self.archiveID = archiveID
        self.verificationLockDigest = verificationLockDigest
    }
}

/// Publication payload binding a release to its exact dependency proof.
public struct MusubiPublicationV1: Hashable, Sendable {
    public let manifest: MusubiReleaseManifestV1
    public let resolution: MusubiResolutionProofV1

    public init(manifest: MusubiReleaseManifestV1, resolution: MusubiResolutionProofV1) throws {
        guard resolution.lock.root == manifest.release,
              manifest.dependencies.count == resolution.lock.rootDependencies.count else {
            throw MusubiV1Error.invalidValue(
                "Musubi publication proof does not bind the release manifest."
            )
        }
        for (manifestDependency, exact) in zip(
            manifest.dependencies, resolution.lock.rootDependencies
        ) {
            guard exact.kind == .normal,
                  exact.alias == manifestDependency.alias,
                  exact.package == manifestDependency.package,
                  exact.requirement == manifestDependency.requirement else {
                throw MusubiV1Error.invalidValue(
                    "Musubi publication direct dependency proof is inconsistent."
                )
            }
        }
        self.manifest = manifest
        self.resolution = resolution
    }
}

/// Canonical generation-bound namespace delegation payload.
public struct MusubiNamespaceDelegationPayloadV1: Hashable, Sendable {
    public let version: UInt8
    public let namespaceBinding: MusubiDigest32V1
    public let ownerGeneration: UInt64
    public let owner: String
    public let delegate: String
    public let expiresAtHeight: UInt64

    public init(
        version: UInt8 = 1,
        namespaceBinding: MusubiDigest32V1,
        ownerGeneration: UInt64,
        owner: String,
        delegate: String,
        expiresAtHeight: UInt64
    ) throws {
        _ = try CanonicalNorito.encodeCompactAccountId(owner)
        _ = try CanonicalNorito.encodeCompactAccountId(delegate)
        guard version == 1, namespaceBinding.bytes.contains(where: { $0 != 0 }),
              ownerGeneration > 0, expiresAtHeight > 0 else {
            throw MusubiV1Error.invalidValue("Musubi namespace delegation payload is invalid.")
        }
        self.version = version
        self.namespaceBinding = namespaceBinding
        self.ownerGeneration = ownerGeneration
        self.owner = owner
        self.delegate = delegate
        self.expiresAtHeight = expiresAtHeight
    }
}

public typealias MusubiNamespaceDelegationApprovalV1 = MusubiControllerApprovalV1

/// Generation-bound authority to claim an absent package in one namespace.
public struct MusubiNamespaceDelegationV1: Hashable, Sendable {
    public let payload: MusubiNamespaceDelegationPayloadV1
    public let approvals: [MusubiNamespaceDelegationApprovalV1]

    public init(
        payload: MusubiNamespaceDelegationPayloadV1,
        approvals: [MusubiNamespaceDelegationApprovalV1]
    ) throws {
        guard !approvals.isEmpty, approvals.count <= 64,
              zip(approvals, approvals.dropFirst()).allSatisfy({ $0.0 < $0.1 }) else {
            throw MusubiV1Error.invalidValue(
                "Musubi namespace delegation approvals must be sorted and unique."
            )
        }
        self.payload = payload
        self.approvals = approvals
    }
}

/// Admission mode for new Musubi archives, releases, and aliases.
public enum MusubiRegistryAdmissionModeV1: UInt32, Hashable, Sendable {
    case closed = 0
    case allowlisted = 1
    case open = 2
}

/// Prospective permanent-alias price policy denominated in whole XOR.
public struct MusubiAliasPricingPolicyV1: Hashable, Sendable {
    public let revision: UInt64
    public let length1Xor: UInt64
    public let length2Xor: UInt64
    public let length3Xor: UInt64
    public let length4Xor: UInt64
    public let length5To32Xor: UInt64

    public init(
        revision: UInt64,
        length1Xor: UInt64,
        length2Xor: UInt64,
        length3Xor: UInt64,
        length4Xor: UInt64,
        length5To32Xor: UInt64
    ) throws {
        guard revision > 0,
              [length1Xor, length2Xor, length3Xor, length4Xor, length5To32Xor]
                  .allSatisfy({ $0 > 0 }) else {
            throw MusubiV1Error.invalidValue("Musubi alias pricing policy is invalid.")
        }
        self.revision = revision
        self.length1Xor = length1Xor
        self.length2Xor = length2Xor
        self.length3Xor = length3Xor
        self.length4Xor = length4Xor
        self.length5To32Xor = length5To32Xor
    }
}

/// Versioned first-release Musubi registry policy.
public struct MusubiRegistryPolicyV1: Hashable, Sendable {
    public let version: UInt8
    public let revision: UInt64
    public let mode: MusubiRegistryAdmissionModeV1
    public let allowlistedDataspaces: [UInt64]
    public let aliasPricing: MusubiAliasPricingPolicyV1

    public init(
        version: UInt8 = 1,
        revision: UInt64,
        mode: MusubiRegistryAdmissionModeV1,
        allowlistedDataspaces: [UInt64],
        aliasPricing: MusubiAliasPricingPolicyV1
    ) throws {
        guard version == 1, revision > 0,
              allowlistedDataspaces.count <= 1_024,
              zip(allowlistedDataspaces, allowlistedDataspaces.dropFirst())
                  .allSatisfy({ $0.0 < $0.1 }),
              mode == .allowlisted || allowlistedDataspaces.isEmpty else {
            throw MusubiV1Error.invalidValue(
                "Musubi registry policy is invalid or noncanonical."
            )
        }
        self.version = version
        self.revision = revision
        self.mode = mode
        self.allowlistedDataspaces = allowlistedDataspaces
        self.aliasPricing = aliasPricing
    }
}

private func musubiValidateVersionRequirementV1(_ requirement: MusubiVersionReqV1) throws {
    if case .comparators(let comparators) = requirement {
        guard !comparators.isEmpty, comparators.count <= 16,
              comparators == Array(Set(comparators)).sorted(),
              !(comparators.count == 1 && comparators[0].op == .equal),
              comparators.filter({ $0.op == .equal }).count <= 1 else {
            throw MusubiV1Error.invalidValue("Musubi comparator AST is noncanonical.")
        }
    }
}

private func musubiRequirementMatchesV1(
    _ requirement: MusubiVersionReqV1,
    version: MusubiVersionV1
) -> Bool {
    let prereleaseEligible: Bool = {
        guard !version.prerelease.isEmpty else { return true }
        func namesCore(_ candidate: MusubiVersionV1) -> Bool {
            !candidate.prerelease.isEmpty
                && candidate.major == version.major
                && candidate.minor == version.minor
                && candidate.patch == version.patch
        }
        switch requirement {
        case .caret(let base), .tilde(let base), .exact(let base): return namesCore(base)
        case .comparators(let values): return values.contains { namesCore($0.version) }
        default: return false
        }
    }()
    guard prereleaseEligible else { return false }
    switch requirement {
    case .any: return true
    case .exact(let expected): return version == expected
    case .majorWildcard(let major): return version.major == major
    case .minorWildcard(let major, let minor):
        return version.major == major && version.minor == minor
    case .comparators(let values):
        return values.allSatisfy { item in
            switch item.op {
            case .greater: return version > item.version
            case .greaterOrEqual: return version >= item.version
            case .less: return version < item.version
            case .lessOrEqual: return version <= item.version
            case .equal: return version == item.version
            }
        }
    case .caret(let base):
        guard version >= base else { return false }
        if base.major > 0 { return version.major == base.major }
        if base.minor > 0 { return version.major == 0 && version.minor == base.minor }
        return version.major == 0 && version.minor == 0 && version.patch == base.patch
    case .tilde(let base):
        return version >= base && version.major == base.major && version.minor == base.minor
    }
}

private func musubiCompareStringV1(_ left: String, _ right: String) -> Int {
    let leftBytes = Array(left.utf8)
    let rightBytes = Array(right.utf8)
    for (leftByte, rightByte) in zip(leftBytes, rightBytes) where leftByte != rightByte {
        return leftByte < rightByte ? -1 : 1
    }
    if leftBytes.count == rightBytes.count { return 0 }
    return leftBytes.count < rightBytes.count ? -1 : 1
}

private func musubiComparePackageV1(
    _ left: MusubiPackageIdV1,
    _ right: MusubiPackageIdV1
) -> Int {
    if left.homeDataspace != right.homeDataspace {
        return left.homeDataspace < right.homeDataspace ? -1 : 1
    }
    switch (left.scope, right.scope) {
    case (.dataspaceRoot, .domain): return -1
    case (.domain, .dataspaceRoot): return 1
    case (.dataspaceRoot, .dataspaceRoot): break
    case let (.domain(leftDomain), .domain(rightDomain)):
        let comparison = musubiCompareStringV1(leftDomain, rightDomain)
        if comparison != 0 { return comparison }
    }
    return musubiCompareStringV1(left.name.value, right.name.value)
}

private func musubiReleaseLessV1(
    _ left: MusubiReleaseIdV1,
    _ right: MusubiReleaseIdV1
) -> Bool {
    let packageComparison = musubiComparePackageV1(left.package, right.package)
    if packageComparison != 0 { return packageComparison < 0 }
    return left.version < right.version
}

private func musubiRequirementRankV1(_ value: MusubiVersionReqV1) -> Int {
    switch value {
    case .any: return 0
    case .caret: return 1
    case .tilde: return 2
    case .majorWildcard: return 3
    case .minorWildcard: return 4
    case .exact: return 5
    case .comparators: return 6
    }
}

private func musubiRequirementLessV1(
    _ left: MusubiVersionReqV1,
    _ right: MusubiVersionReqV1
) -> Bool {
    let leftRank = musubiRequirementRankV1(left)
    let rightRank = musubiRequirementRankV1(right)
    if leftRank != rightRank { return leftRank < rightRank }
    switch (left, right) {
    case (.any, .any): return false
    case let (.caret(left), .caret(right)),
         let (.tilde(left), .tilde(right)),
         let (.exact(left), .exact(right)):
        return left < right
    case let (.majorWildcard(left), .majorWildcard(right)):
        return left < right
    case let (.minorWildcard(leftMajor, leftMinor),
              .minorWildcard(rightMajor, rightMinor)):
        return leftMajor != rightMajor ? leftMajor < rightMajor : leftMinor < rightMinor
    case let (.comparators(left), .comparators(right)):
        for index in 0..<min(left.count, right.count) where left[index] != right[index] {
            return left[index] < right[index]
        }
        return left.count < right.count
    default:
        return false
    }
}

private func musubiDependencyReqLessV1(
    _ left: MusubiDependencyReqV1,
    _ right: MusubiDependencyReqV1
) -> Bool {
    let aliasComparison = musubiCompareStringV1(left.alias, right.alias)
    if aliasComparison != 0 { return aliasComparison < 0 }
    let packageComparison = musubiComparePackageV1(left.package, right.package)
    if packageComparison != 0 { return packageComparison < 0 }
    return musubiRequirementLessV1(left.requirement, right.requirement)
}

private func musubiExactDependencyLessV1(
    _ left: MusubiExactDependencyEdgeV1,
    _ right: MusubiExactDependencyEdgeV1
) -> Bool {
    let aliasComparison = musubiCompareStringV1(left.alias, right.alias)
    if aliasComparison != 0 { return aliasComparison < 0 }
    if left.kind != right.kind { return left.kind.rawValue < right.kind.rawValue }
    let packageComparison = musubiComparePackageV1(left.package, right.package)
    if packageComparison != 0 { return packageComparison < 0 }
    if left.requirement != right.requirement {
        return musubiRequirementLessV1(left.requirement, right.requirement)
    }
    return musubiReleaseLessV1(left.selected, right.selected)
}
