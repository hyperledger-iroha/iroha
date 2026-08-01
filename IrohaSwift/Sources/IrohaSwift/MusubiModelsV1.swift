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
          value.unicodeScalars.allSatisfy({ !CharacterSet.controlCharacters.contains($0) }) else {
        throw MusubiV1Error.invalidValue("\(field) must be exact non-empty text.")
    }
}

private func musubiRequireName(_ value: String, field: String) throws {
    try musubiRequireExactText(value, field: field)
    guard value.utf8.count <= 255,
          value.unicodeScalars.allSatisfy({ scalar in
              !CharacterSet.whitespacesAndNewlines.contains(scalar)
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
    ) {
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
                .map { try parseComparator(String($0).trimmingCharacters(in: .whitespaces)) }
            comparators.sort()
            comparators = Array(Set(comparators)).sorted()
            guard !comparators.isEmpty, comparators.count <= 16 else {
                throw MusubiV1Error.invalidValue("Musubi comparator list is empty or oversized.")
            }
            let exacts = Set(comparators.filter { $0.op == .equal }.map(\.version))
            guard exacts.count <= 1 else {
                throw MusubiV1Error.invalidValue("Musubi comparator list has conflicting exacts.")
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
                  values == Array(Set(values)).sorted() else {
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
            guard !value.isEmpty, value.count <= 16, value == Array(Set(value)).sorted() else {
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

/// Finalized universal registry snapshot bound into page cursors.
public struct MusubiRegistrySnapshotV1: Codable, Hashable, Sendable {
    public let finalizedHeight: UInt64
    public let finalizedBlockHash: [UInt8]
    public let indexRevision: UInt64

    public init(finalizedHeight: UInt64, finalizedBlockHash: [UInt8], indexRevision: UInt64) throws {
        guard finalizedBlockHash.count == 32 else {
            throw MusubiV1Error.invalidValue("Finalized block hash must contain 32 bytes.")
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
        guard lastKey.utf8.count <= 512 else {
            throw MusubiV1Error.invalidValue("Musubi cursor last key exceeds 512 bytes.")
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
    let bytes = try musubiRawArray(value, field: field)
    guard bytes.count == 32 else {
        throw MusubiV1Error.invalidValue("\(field) must contain exactly 32 bytes.")
    }
    for byte in bytes {
        guard case .unsigned(let value) = byte, value <= UInt8.max else {
            throw MusubiV1Error.invalidValue("\(field) contains a non-byte value.")
        }
    }
}

private func musubiRawNewtypeText(_ value: MusubiJSONValueV1?, field: String) throws -> String {
    let wrapper = try musubiRawArray(value, field: field)
    guard wrapper.count == 1 else {
        throw MusubiV1Error.invalidValue("\(field) must contain one Norito newtype item.")
    }
    return try musubiRawString(wrapper[0], field: "\(field)[0]")
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
            exactKeys: ["action_digest", "reason", "enacted_at_height"]
        )
        try musubiValidateRawDigest(takedown["action_digest"], field: "\(field).action_digest")
        _ = try musubiRawNewtypeText(takedown["reason"], field: "\(field).reason")
        guard try musubiRawUnsigned(takedown["enacted_at_height"], field: field) > 0 else {
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
        guard genesisHash.count == 32,
              items.count <= 100,
              nextCursor == nil || nextCursor?.snapshot == snapshot else {
            throw MusubiV1Error.invalidValue(
                "Musubi resolver page has an invalid genesis hash, size, or cursor."
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        guard genesisHash.count == 32 else {
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
        let role = try musubiRawObject(roleRaw, field: "member.role", exactKeys: ["kind", "value"])
        roleKind = try musubiRawString(role["kind"], field: "member.role.kind")
        switch roleKind {
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
                field: "member.role.value",
                exactKeys: ["publish", "yank", "metadata", "archive_locations"]
            )
            for key in permissions.keys { _ = try musubiRawBool(permissions[key], field: key) }
        default:
            throw MusubiV1Error.unsupportedVersion("Unsupported Musubi V1 member role.")
        }
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
