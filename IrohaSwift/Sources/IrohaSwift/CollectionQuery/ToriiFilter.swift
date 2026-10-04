import Foundation

// Torii collection-query filters (`specs/torii/collection_queries.md`).
//
// One filter tree has two spellings: the canonical text form (`description`),
// e.g. `owned_by = "alice" and quantity >= "10.5"`, and the canonical JSON form
// (`jsonData()`), e.g. `{"op":"and","args":[...]}`. Both match the Rust
// reference `iroha_torii_shared::list_query` byte for byte.

// MARK: - Field paths

/// A dotted field path such as `owned_by`, `quantity` or `metadata.tier`.
///
/// `rawValue` is the dotted spelling with unquoted segments
/// (`metadata.display-name`); it is the form used by the JSON filter form and
/// by `select`. `description` is the canonical text spelling, which quotes a
/// segment in backticks when it is not an identifier or, in the first
/// segment, when it is a keyword: ``metadata.`display-name` ``, `` `and` ``.
public struct ToriiFieldPath: Hashable, Sendable, CustomStringConvertible, ExpressibleByStringLiteral {
    /// Maximum UTF-8 length of one field path.
    public static let maximumBytes = 256

    /// Dotted spelling with unquoted segments.
    public let rawValue: String

    public init(_ rawValue: String) {
        self.rawValue = rawValue
    }

    public init(stringLiteral value: String) {
        self.init(value)
    }

    /// Join unquoted segments with `.`.
    public init(segments: [String]) {
        self.init(segments.joined(separator: "."))
    }

    /// The path addressing one metadata entry, `metadata.<key>`.
    public static func metadata(_ key: String) -> ToriiFieldPath {
        ToriiFieldPath("metadata.\(key)")
    }

    /// The dot-separated segments of the path.
    public var segments: [Substring] {
        rawValue.split(separator: ".", omittingEmptySubsequences: false)
    }

    /// Canonical text spelling with backtick-quoted segments where needed.
    public var description: String {
        var out = ""
        for (index, segment) in segments.enumerated() {
            if index > 0 {
                out.append(".")
            }
            if Self.isBareSegment(segment, first: index == 0) {
                out.append(contentsOf: segment)
            } else {
                out.append("`")
                out.append(contentsOf: segment)
                out.append("`")
            }
        }
        return out
    }

    /// Words that cannot start a bare field path in the text grammar.
    static let keywords: Set<String> = ["and", "or", "not", "in", "is", "null", "true", "false", "exists"]

    static func isBareSegment(_ segment: Substring, first: Bool) -> Bool {
        guard let head = segment.utf8.first,
              isASCIILetter(head) || head == UInt8(ascii: "_"),
              segment.utf8.dropFirst().allSatisfy({ isASCIILetter($0) || isASCIIDigit($0) || $0 == UInt8(ascii: "_") })
        else {
            return false
        }
        return !(first && keywords.contains(segment.lowercased()))
    }

    /// Check the path syntax: non-empty segments, bounded length, no
    /// whitespace or control characters. Whether a collection exposes the
    /// field is decided by Torii.
    func validate(parameter: String) throws {
        func invalid(_ reason: String) -> ToriiListQueryError {
            ToriiListQueryError(parameter: parameter, message: "invalid field `\(rawValue)`: \(reason)")
        }
        guard !rawValue.isEmpty else {
            throw invalid("field paths must not be empty")
        }
        guard rawValue.utf8.count <= Self.maximumBytes else {
            throw invalid("field paths must not exceed \(Self.maximumBytes) bytes")
        }
        guard !rawValue.unicodeScalars.contains(where: {
            $0.properties.isWhitespace || $0.properties.generalCategory == .control
        }) else {
            throw invalid("field paths must not contain whitespace or control characters")
        }
        guard !segments.contains(where: \.isEmpty) else {
            throw invalid("field path segments must not be empty")
        }
    }
}

@inline(__always)
private func isASCIILetter(_ byte: UInt8) -> Bool {
    (UInt8(ascii: "a")...UInt8(ascii: "z")).contains(byte) || (UInt8(ascii: "A")...UInt8(ascii: "Z")).contains(byte)
}

@inline(__always)
private func isASCIIDigit(_ byte: UInt8) -> Bool {
    (UInt8(ascii: "0")...UInt8(ascii: "9")).contains(byte)
}

// MARK: - Literals

/// A filter literal.
///
/// Integers that fit `u64`/`i64` are JSON numbers. Decimals and wider
/// integers are exact decimal strings (`10.5` and `"10.5"` are the same
/// literal), so no value ever passes through an IEEE-754 double. Arrays and
/// objects are allowed only when comparing `metadata.*` entries.
public struct ToriiFilterValue: Hashable, Sendable, CustomStringConvertible {
    enum Storage: Hashable, Sendable {
        case null
        case bool(Bool)
        /// Canonical base-10 digits of an integer that fits `u64` or `i64`.
        case integer(String)
        case string(String)
        case array([ToriiFilterValue])
        case object([String: ToriiFilterValue])
    }

    let storage: Storage

    init(storage: Storage) {
        self.storage = storage
    }

    public static let null = ToriiFilterValue(storage: .null)

    public static func bool(_ value: Bool) -> ToriiFilterValue {
        ToriiFilterValue(storage: .bool(value))
    }

    public static func string(_ value: String) -> ToriiFilterValue {
        ToriiFilterValue(storage: .string(value))
    }

    /// An integer literal; values outside `u64`/`i64` become exact decimal strings.
    public static func integer<T: BinaryInteger>(_ value: T) -> ToriiFilterValue {
        if let unsigned = UInt64(exactly: value) {
            return ToriiFilterValue(storage: .integer(String(unsigned)))
        }
        if let signed = Int64(exactly: value) {
            return ToriiFilterValue(storage: .integer(String(signed)))
        }
        return ToriiFilterValue(storage: .string(String(value)))
    }

    /// An exact decimal literal such as `"10.5"` or `"-0.25"`.
    ///
    /// - Throws: `ToriiListQueryError` (`invalid_filter`) when `text` is not
    ///   `-?(0|[1-9][0-9]*)(\.[0-9]+)?`.
    public static func decimal(_ text: String) throws -> ToriiFilterValue {
        guard isCanonicalDecimalText(text) else {
            throw ToriiListQueryError(
                parameter: "filter",
                message: "`\(text)` is not an exact decimal; write digits with an optional fraction, e.g. `10.5`"
            )
        }
        return numberLiteral(canonicalText: text)
    }

    public static func array(_ values: [ToriiFilterValue]) -> ToriiFilterValue {
        ToriiFilterValue(storage: .array(values))
    }

    public static func object(_ members: [String: ToriiFilterValue]) -> ToriiFilterValue {
        ToriiFilterValue(storage: .object(members))
    }

    /// Apply the text-grammar number rule to canonical decimal text: integers
    /// that fit `u64`/`i64` are numbers, everything else is a decimal string.
    static func numberLiteral(canonicalText text: String) -> ToriiFilterValue {
        if !text.contains(".") {
            if let unsigned = UInt64(text) {
                return ToriiFilterValue(storage: .integer(String(unsigned)))
            }
            if let signed = Int64(text) {
                return ToriiFilterValue(storage: .integer(String(signed)))
            }
        }
        return ToriiFilterValue(storage: .string(text))
    }

    /// Whether `text` is a canonical decimal: `-?(0|[1-9][0-9]*)(\.[0-9]+)?`.
    static func isCanonicalDecimalText(_ text: String) -> Bool {
        var bytes = Array(text.utf8)[...]
        if bytes.first == UInt8(ascii: "-") {
            bytes = bytes.dropFirst()
        }
        let integer: ArraySlice<UInt8>
        let fraction: ArraySlice<UInt8>?
        if let dot = bytes.firstIndex(of: UInt8(ascii: ".")) {
            integer = bytes[bytes.startIndex..<dot]
            fraction = bytes[(dot + 1)...]
        } else {
            integer = bytes
            fraction = nil
        }
        let integerOK: Bool
        if integer.elementsEqual([UInt8(ascii: "0")]) {
            integerOK = true
        } else if let head = integer.first, (UInt8(ascii: "1")...UInt8(ascii: "9")).contains(head) {
            integerOK = integer.dropFirst().allSatisfy(isASCIIDigit)
        } else {
            integerOK = false
        }
        let fractionOK = fraction.map { !$0.isEmpty && $0.allSatisfy(isASCIIDigit) } ?? true
        return integerOK && fractionOK
    }

    var isString: Bool {
        if case .string = storage { return true }
        return false
    }

    var isBool: Bool {
        if case .bool = storage { return true }
        return false
    }

    var isStructured: Bool {
        switch storage {
        case .array, .object: return true
        default: return false
        }
    }

    /// Whether the literal can take part in a numeric range comparison.
    var isNumeric: Bool {
        switch storage {
        case .integer: return true
        case let .string(text): return Self.isCanonicalDecimalText(text)
        default: return false
        }
    }

    /// The literal as it appears in the text and JSON forms.
    public var description: String {
        var out = ""
        ToriiCanonicalJSON.write(self, into: &out)
        return out
    }
}

extension ToriiFilterValue: ExpressibleByStringLiteral, ExpressibleByIntegerLiteral, ExpressibleByBooleanLiteral,
    ExpressibleByNilLiteral, ExpressibleByArrayLiteral, ExpressibleByDictionaryLiteral {
    public init(stringLiteral value: String) {
        self = .string(value)
    }

    public init(integerLiteral value: Int64) {
        self = .integer(value)
    }

    public init(booleanLiteral value: Bool) {
        self = .bool(value)
    }

    public init(nilLiteral: ()) {
        self = .null
    }

    public init(arrayLiteral elements: ToriiFilterValue...) {
        self = .array(elements)
    }

    public init(dictionaryLiteral elements: (String, ToriiFilterValue)...) {
        var members: [String: ToriiFilterValue] = [:]
        for (key, value) in elements {
            members[key] = value
        }
        self = .object(members)
    }
}

extension ToriiFilterValue: Codable {
    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let lexemes = decoder.userInfo[exactJSONNumberLexemesUserInfoKey] as? [String: String],
           let lexeme = lexemes[exactJSONNumberCodingPathKey(decoder.codingPath)],
           lexeme.contains(where: { $0 == "." || $0 == "e" || $0 == "E" }) {
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "`\(lexeme)` is not a filter literal; write decimals as exact decimal strings such as \"1.5\""
            )
        }
        if container.decodeNil() {
            self = .null
        } else if let value = try? container.decode(Bool.self) {
            self = .bool(value)
        } else if let value = try? container.decode(UInt64.self) {
            self = .integer(value)
        } else if let value = try? container.decode(Int64.self) {
            self = .integer(value)
        } else if let value = try? container.decode(String.self) {
            self = .string(value)
        } else if let value = try? container.decode([ToriiFilterValue].self) {
            self = .array(value)
        } else if let value = try? container.decode([String: ToriiFilterValue].self) {
            self = .object(value)
        } else {
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "filter literals are strings, integers that fit u64/i64, booleans, null, arrays or objects; write decimals as exact decimal strings such as \"10.5\""
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch storage {
        case .null:
            try container.encodeNil()
        case let .bool(value):
            try container.encode(value)
        case let .integer(digits):
            if let unsigned = UInt64(digits) {
                try container.encode(unsigned)
            } else if let signed = Int64(digits) {
                try container.encode(signed)
            } else {
                throw EncodingError.invalidValue(
                    digits,
                    .init(codingPath: encoder.codingPath, debugDescription: "integer literal exceeds u64/i64")
                )
            }
        case let .string(value):
            try container.encode(value)
        case let .array(values):
            try container.encode(values)
        case let .object(members):
            try container.encode(members)
        }
    }
}

/// Swift values usable as filter literals.
///
/// Integers become numbers when they fit `u64`/`i64` and exact decimal strings
/// otherwise; `KotodamaDecimal`/`KotodamaQuantity` follow the same rule as the
/// text grammar. `Double` and `Float` deliberately do not conform: write
/// decimals with an exact type.
public protocol ToriiFilterLiteral: Sendable {
    /// The literal stored in the filter tree.
    var toriiFilterValue: ToriiFilterValue { get }
}

extension ToriiFilterValue: ToriiFilterLiteral {
    public var toriiFilterValue: ToriiFilterValue { self }
}

extension String: ToriiFilterLiteral {
    public var toriiFilterValue: ToriiFilterValue { .string(self) }
}

extension Bool: ToriiFilterLiteral {
    public var toriiFilterValue: ToriiFilterValue { .bool(self) }
}

extension Int: ToriiFilterLiteral {
    public var toriiFilterValue: ToriiFilterValue { .integer(self) }
}

extension Int8: ToriiFilterLiteral {
    public var toriiFilterValue: ToriiFilterValue { .integer(self) }
}

extension Int16: ToriiFilterLiteral {
    public var toriiFilterValue: ToriiFilterValue { .integer(self) }
}

extension Int32: ToriiFilterLiteral {
    public var toriiFilterValue: ToriiFilterValue { .integer(self) }
}

extension Int64: ToriiFilterLiteral {
    public var toriiFilterValue: ToriiFilterValue { .integer(self) }
}

extension UInt: ToriiFilterLiteral {
    public var toriiFilterValue: ToriiFilterValue { .integer(self) }
}

extension UInt8: ToriiFilterLiteral {
    public var toriiFilterValue: ToriiFilterValue { .integer(self) }
}

extension UInt16: ToriiFilterLiteral {
    public var toriiFilterValue: ToriiFilterValue { .integer(self) }
}

extension UInt32: ToriiFilterLiteral {
    public var toriiFilterValue: ToriiFilterValue { .integer(self) }
}

extension UInt64: ToriiFilterLiteral {
    public var toriiFilterValue: ToriiFilterValue { .integer(self) }
}

extension KotodamaInt: ToriiFilterLiteral {
    public var toriiFilterValue: ToriiFilterValue {
        ToriiFilterValue.numberLiteral(canonicalText: canonicalString)
    }
}

extension KotodamaDecimal: ToriiFilterLiteral {
    public var toriiFilterValue: ToriiFilterValue {
        ToriiFilterValue.numberLiteral(canonicalText: canonicalString)
    }
}

extension KotodamaQuantity: ToriiFilterLiteral {
    public var toriiFilterValue: ToriiFilterValue {
        ToriiFilterValue.numberLiteral(canonicalText: canonicalString)
    }
}

extension Optional: ToriiFilterLiteral where Wrapped: ToriiFilterLiteral {
    public var toriiFilterValue: ToriiFilterValue {
        map(\.toriiFilterValue) ?? .null
    }
}

// MARK: - Filter tree

/// A collection-query filter.
///
/// Build filters with `ToriiField` operators or methods, combine them with
/// `&&`, `||` and `!` (or `ToriiFilter.all`/`any`), and pass them to
/// `ToriiListQuery`. `description` is the canonical text form and
/// `jsonData()` the canonical JSON form; Torii parses both into the same tree.
///
/// ```swift
/// let filter = ToriiField("owned_by") == "alice"
///     && ToriiField("quantity") >= try KotodamaQuantity("10.5")
///     && !ToriiField.metadata("frozen").exists
/// filter.description // owned_by = "alice" and quantity >= "10.5" and not exists(metadata.frozen)
/// ```
public indirect enum ToriiFilter: Hashable, Sendable, CustomStringConvertible {
    /// All operands match (`a and b`).
    case and([ToriiFilter])
    /// At least one operand matches (`a or b`).
    case or([ToriiFilter])
    /// The operand does not match (`not a`).
    case not(ToriiFilter)
    /// `field = value`
    case eq(ToriiFieldPath, ToriiFilterValue)
    /// `field != value`; also matches rows where the field is absent.
    case ne(ToriiFieldPath, ToriiFilterValue)
    /// `field < value`
    case lt(ToriiFieldPath, ToriiFilterValue)
    /// `field <= value`
    case lte(ToriiFieldPath, ToriiFilterValue)
    /// `field > value`
    case gt(ToriiFieldPath, ToriiFilterValue)
    /// `field >= value`
    case gte(ToriiFieldPath, ToriiFilterValue)
    /// `field in [a, b]`
    case `in`(ToriiFieldPath, [ToriiFilterValue])
    /// `field not in [a, b]`; also matches rows where the field is absent.
    case notIn(ToriiFieldPath, [ToriiFilterValue])
    /// `exists(field)`: the field is present.
    case exists(ToriiFieldPath)
    /// `field is null`: the field is absent or null.
    case isNull(ToriiFieldPath)

    /// Maximum nesting depth (a single leaf has depth 0).
    public static let maximumDepth = 10
    /// Maximum operator nodes in one filter.
    public static let maximumNodes = 1_024
    /// Maximum literals in one `in`/`not in` list.
    public static let maximumMembershipValues = 1_024
    /// Maximum membership literals across one filter.
    public static let maximumTotalMembershipValues = 4_096
    /// Maximum UTF-8 length of a text filter.
    public static let maximumTextBytes = 32 * 1_024

    /// Operator name used by the JSON form.
    public var operatorName: String {
        switch self {
        case .and: return "and"
        case .or: return "or"
        case .not: return "not"
        case .eq: return "eq"
        case .ne: return "ne"
        case .lt: return "lt"
        case .lte: return "lte"
        case .gt: return "gt"
        case .gte: return "gte"
        case .in: return "in"
        case .notIn: return "nin"
        case .exists: return "exists"
        case .isNull: return "is_null"
        }
    }

    /// `self and other`, flattening chains of `and`.
    public func and(_ other: ToriiFilter) -> ToriiFilter {
        switch (self, other) {
        case let (.and(left), .and(right)): return .and(left + right)
        case let (.and(left), right): return .and(left + [right])
        case let (left, .and(right)): return .and([left] + right)
        case let (left, right): return .and([left, right])
        }
    }

    /// `self or other`, flattening chains of `or`.
    public func or(_ other: ToriiFilter) -> ToriiFilter {
        switch (self, other) {
        case let (.or(left), .or(right)): return .or(left + right)
        case let (.or(left), right): return .or(left + [right])
        case let (left, .or(right)): return .or([left] + right)
        case let (left, right): return .or([left, right])
        }
    }

    /// `not self`
    public var negated: ToriiFilter {
        .not(self)
    }

    /// Conjunction of `filters`, or `nil` when there are none.
    public static func all(_ filters: [ToriiFilter]) -> ToriiFilter? {
        guard let first = filters.first else { return nil }
        return filters.dropFirst().reduce(first) { $0.and($1) }
    }

    /// Disjunction of `filters`, or `nil` when there are none.
    public static func any(_ filters: [ToriiFilter]) -> ToriiFilter? {
        guard let first = filters.first else { return nil }
        return filters.dropFirst().reduce(first) { $0.or($1) }
    }

    /// Conjunction of the filters listed in the builder, or `nil` when none.
    public static func all(@ToriiFilterBuilder _ filters: () -> [ToriiFilter]) -> ToriiFilter? {
        all(filters())
    }

    /// Disjunction of the filters listed in the builder, or `nil` when none.
    public static func any(@ToriiFilterBuilder _ filters: () -> [ToriiFilter]) -> ToriiFilter? {
        any(filters())
    }

    public static func && (lhs: ToriiFilter, rhs: ToriiFilter) -> ToriiFilter {
        lhs.and(rhs)
    }

    public static func || (lhs: ToriiFilter, rhs: ToriiFilter) -> ToriiFilter {
        lhs.or(rhs)
    }

    public static prefix func ! (filter: ToriiFilter) -> ToriiFilter {
        filter.negated
    }

    // MARK: Canonical text form

    /// Canonical text form, identical to Torii's rendering of the same tree.
    ///
    /// Object and array literals (allowed only against `metadata.<key>`) exist
    /// only in the JSON form: for such filters this rendering is diagnostic,
    /// and they are sent as JSON (`POST /query`), never as text.
    public var description: String {
        var out = ""
        writeText(into: &out, parent: .root)
        return out
    }

    /// Whether the filter compares a field with an object or array literal,
    /// which only the JSON form can carry.
    public var hasStructuredLiterals: Bool {
        switch self {
        case let .and(operands), let .or(operands):
            return operands.contains { $0.hasStructuredLiterals }
        case let .not(inner):
            return inner.hasStructuredLiterals
        case let .eq(_, value), let .ne(_, value), let .lt(_, value),
             let .lte(_, value), let .gt(_, value), let .gte(_, value):
            return value.isStructured
        case let .in(_, values), let .notIn(_, values):
            return values.contains(where: \.isStructured)
        case .exists, .isNull:
            return false
        }
    }

    /// The canonical text form for transports that carry only text (`GET`
    /// parameters and event streams).
    ///
    /// - Throws: `ToriiListQueryError` with code `invalid_filter` (for
    ///   `parameter`) when the filter has object or array literals.
    func transmittableText(parameter: String) throws -> String {
        guard !hasStructuredLiterals else {
            throw ToriiListQueryError(
                parameter: parameter,
                message: "object and array literals exist only in the JSON form; send this filter with POST /query"
            )
        }
        return description
    }

    private enum TextParent {
        case root
        case or
        case and
        case not
    }

    private func writeText(into out: inout String, parent: TextParent) {
        switch self {
        case let .and(operands):
            let parenthesize = parent == .and || parent == .not
            Self.writeJunction(operands, keyword: "and", me: .and, parenthesize: parenthesize, into: &out)
        case let .or(operands):
            Self.writeJunction(operands, keyword: "or", me: .or, parenthesize: parent != .root, into: &out)
        case let .not(inner):
            if case let .isNull(field) = inner {
                out += "\(field) is not null"
                return
            }
            out += "not "
            inner.writeText(into: &out, parent: .not)
        case let .eq(field, value): Self.writeComparison(field, "=", value, into: &out)
        case let .ne(field, value): Self.writeComparison(field, "!=", value, into: &out)
        case let .lt(field, value): Self.writeComparison(field, "<", value, into: &out)
        case let .lte(field, value): Self.writeComparison(field, "<=", value, into: &out)
        case let .gt(field, value): Self.writeComparison(field, ">", value, into: &out)
        case let .gte(field, value): Self.writeComparison(field, ">=", value, into: &out)
        case let .in(field, values): Self.writeList(field, "in", values, into: &out)
        case let .notIn(field, values): Self.writeList(field, "not in", values, into: &out)
        case let .exists(field): out += "exists(\(field))"
        case let .isNull(field): out += "\(field) is null"
        }
    }

    private static func writeJunction(_ operands: [ToriiFilter],
                                      keyword: String,
                                      me: TextParent,
                                      parenthesize: Bool,
                                      into out: inout String) {
        if parenthesize {
            out += "("
        }
        for (index, operand) in operands.enumerated() {
            if index > 0 {
                out += " \(keyword) "
            }
            operand.writeText(into: &out, parent: me)
        }
        if parenthesize {
            out += ")"
        }
    }

    private static func writeComparison(_ field: ToriiFieldPath,
                                        _ op: String,
                                        _ value: ToriiFilterValue,
                                        into out: inout String) {
        out += "\(field) \(op) "
        ToriiCanonicalJSON.write(value, into: &out)
    }

    private static func writeList(_ field: ToriiFieldPath,
                                  _ op: String,
                                  _ values: [ToriiFilterValue],
                                  into out: inout String) {
        out += "\(field) \(op) ["
        for (index, value) in values.enumerated() {
            if index > 0 {
                out += ", "
            }
            ToriiCanonicalJSON.write(value, into: &out)
        }
        out += "]"
    }

    // MARK: Event-stream subset

    /// Fields an event-stream filter may use.
    static let eventStreamFields: Set<String> = [
        "tx_status", "tx_hash", "tx_block_height", "tx_lane_id", "tx_dataspace_id",
        "block_status", "block_height", "proof_backend", "proof_call_hash", "proof_envelope_hash",
    ]

    /// Check the subset `/v1/events/sse` accepts: the event fields, `=` and
    /// `in`, combined with `and`/`or`; `not` only over `tx_status = …` or
    /// `block_status = …`; `tx_block_height is null`; and the status names.
    ///
    /// - Throws: `ToriiListQueryError` with code `invalid_filter`.
    func validateForEventStream() throws {
        func failure(_ message: String) -> ToriiListQueryError {
            ToriiListQueryError(parameter: "filter", message: message)
        }
        func checkField(_ field: ToriiFieldPath) throws {
            guard Self.eventStreamFields.contains(field.rawValue) else {
                throw failure(
                    "event streams cannot filter on `\(field.rawValue)`; use one of: "
                        + Self.eventStreamFields.sorted().joined(separator: ", ")
                )
            }
        }
        func checkValue(_ field: ToriiFieldPath, _ value: ToriiFilterValue) throws {
            let names: [String]
            switch field.rawValue {
            case "tx_status": names = ["Queued", "Expired", "Approved", "Rejected"]
            case "block_status": names = ["Created", "Approved", "Rejected", "Committed", "Applied"]
            default: return
            }
            guard case let .string(name) = value.storage, names.contains(name) else {
                throw failure("`\(field.rawValue)` is one of: \(names.joined(separator: ", "))")
            }
        }
        func walk(_ node: ToriiFilter) throws {
            switch node {
            case let .and(operands), let .or(operands):
                try operands.forEach(walk)
            case let .eq(field, value):
                try checkField(field)
                try checkValue(field, value)
            case let .in(field, values):
                try checkField(field)
                try values.forEach { try checkValue(field, $0) }
            case let .not(inner):
                guard case let .eq(field, value) = inner, ["tx_status", "block_status"].contains(field.rawValue) else {
                    throw failure("event streams accept `not` only over `tx_status = …` or `block_status = …`")
                }
                try checkValue(field, value)
            case let .isNull(field):
                guard field.rawValue == "tx_block_height" else {
                    throw failure("event streams accept `is null` only for `tx_block_height`")
                }
            case .ne, .lt, .lte, .gt, .gte, .notIn, .exists:
                throw failure("event streams accept only `=` and `in`, combined with `and` and `or`")
            }
        }
        try walk(self)
    }

    // MARK: Canonical JSON form

    /// Canonical JSON form, `{"args":[...],"op":"<operator>"}`: compact, with
    /// members in byte order exactly as Torii's reference serializer writes it.
    public func jsonData() -> Data {
        Data(jsonString.utf8)
    }

    /// Canonical JSON form as text.
    public var jsonString: String {
        var out = ""
        writeJSON(into: &out)
        return out
    }

    func writeJSON(into out: inout String) {
        out += "{\"args\":["
        switch self {
        case let .and(operands), let .or(operands):
            for (index, operand) in operands.enumerated() {
                if index > 0 {
                    out += ","
                }
                operand.writeJSON(into: &out)
            }
        case let .not(inner):
            inner.writeJSON(into: &out)
        case let .eq(field, value), let .ne(field, value), let .lt(field, value),
             let .lte(field, value), let .gt(field, value), let .gte(field, value):
            ToriiCanonicalJSON.writeString(field.rawValue, into: &out)
            out += ","
            ToriiCanonicalJSON.write(value, into: &out)
        case let .in(field, values), let .notIn(field, values):
            ToriiCanonicalJSON.writeString(field.rawValue, into: &out)
            out += ","
            ToriiCanonicalJSON.write(.array(values), into: &out)
        case let .exists(field), let .isNull(field):
            ToriiCanonicalJSON.writeString(field.rawValue, into: &out)
        }
        out += "],\"op\":"
        ToriiCanonicalJSON.writeString(operatorName, into: &out)
        out += "}"
    }

    // MARK: Validation

    /// Check the structural limits and operand shapes Torii enforces, without
    /// contacting a server.
    ///
    /// - Throws: `ToriiListQueryError` with code `invalid_filter`.
    public func validate() throws {
        var budget = ValidationBudget(parameter: "filter")
        try validate(depth: 0, budget: &budget)
    }

    func validate(parameter: String) throws {
        var budget = ValidationBudget(parameter: parameter)
        try validate(depth: 0, budget: &budget)
    }

    private struct ValidationBudget {
        let parameter: String
        var nodes = 0
        var membershipValues = 0

        func failure(_ message: String) -> ToriiListQueryError {
            ToriiListQueryError(parameter: parameter, message: message)
        }

        mutating func enter(depth: Int) throws {
            guard depth <= ToriiFilter.maximumDepth else {
                throw failure("filter exceeds the nesting depth limit of \(ToriiFilter.maximumDepth)")
            }
            nodes += 1
            guard nodes <= ToriiFilter.maximumNodes else {
                throw failure("filter exceeds the node count limit of \(ToriiFilter.maximumNodes)")
            }
        }

        mutating func membership(_ field: ToriiFieldPath, _ values: [ToriiFilterValue]) throws {
            func invalidOperand(_ reason: String) -> ToriiListQueryError {
                failure("invalid operand for `\(field.rawValue)`: \(reason)")
            }
            guard !values.isEmpty else {
                throw invalidOperand("membership lists must not be empty")
            }
            guard values.count <= ToriiFilter.maximumMembershipValues else {
                throw failure("filter exceeds the membership list size limit of \(ToriiFilter.maximumMembershipValues)")
            }
            membershipValues += values.count
            guard membershipValues <= ToriiFilter.maximumTotalMembershipValues else {
                throw failure("filter exceeds the total membership values limit of \(ToriiFilter.maximumTotalMembershipValues)")
            }
            guard Set(values).count == values.count else {
                throw invalidOperand("membership list values must be unique")
            }
            let homogeneous = values.allSatisfy(\.isString)
                || values.allSatisfy(\.isNumeric)
                || values.allSatisfy(\.isBool)
            guard homogeneous || field.rawValue.hasPrefix("metadata.") else {
                throw invalidOperand("membership list values must all be strings, numbers or booleans")
            }
        }
    }

    private func validate(depth: Int, budget: inout ValidationBudget) throws {
        try budget.enter(depth: depth)
        switch self {
        case let .and(operands), let .or(operands):
            guard !operands.isEmpty else {
                throw budget.failure("`\(operatorName)` needs at least one operand")
            }
            for operand in operands {
                try operand.validate(depth: depth + 1, budget: &budget)
            }
        case let .not(inner):
            try inner.validate(depth: depth + 1, budget: &budget)
        case let .eq(field, value), let .ne(field, value):
            try field.validate(parameter: budget.parameter)
            if value.isStructured && !field.rawValue.hasPrefix("metadata.") {
                throw budget.failure(
                    "invalid operand for `\(field.rawValue)`: comparison literals must be strings, numbers, booleans or null"
                )
            }
        case let .lt(field, value), let .lte(field, value), let .gt(field, value), let .gte(field, value):
            try field.validate(parameter: budget.parameter)
            guard value.isNumeric || value.isString else {
                throw budget.failure(
                    "invalid operand for `\(field.rawValue)`: range comparisons need a number, decimal or string literal"
                )
            }
        case let .in(field, values), let .notIn(field, values):
            try field.validate(parameter: budget.parameter)
            try budget.membership(field, values)
        case let .exists(field), let .isNull(field):
            try field.validate(parameter: budget.parameter)
        }
    }
}

extension ToriiFilter: Codable {
    private struct NodeKey: CodingKey {
        let stringValue: String
        var intValue: Int? { nil }

        init(_ stringValue: String) {
            self.stringValue = stringValue
        }

        init?(stringValue: String) {
            self.stringValue = stringValue
        }

        init?(intValue: Int) {
            nil
        }

        static let op = NodeKey("op")
        static let args = NodeKey("args")
    }

    /// Decode the JSON form `{"op": ..., "args": [...]}` with Torii's literal
    /// rules: duplicate members are rejected and numbers written with a
    /// fraction or exponent are refused (decimals are exact decimal strings).
    public init(jsonData: Data) throws {
        let decoder = JSONDecoder()
        decoder.userInfo[exactJSONNumberLexemesUserInfoKey] = try ExactJSONNumberLexemeScanner.scan(jsonData)
        self = try decoder.decode(ToriiFilter.self, from: jsonData)
    }

    /// Decode the JSON form `{"op": ..., "args": [...]}`. A plain `JSONDecoder`
    /// cannot see number spellings; prefer `init(jsonData:)`, which also refuses
    /// integral numbers written as `1.0` or `1e2`.
    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: NodeKey.self)
        if let unknown = container.allKeys.first(where: { $0.stringValue != "op" && $0.stringValue != "args" }) {
            throw DecodingError.dataCorruptedError(
                forKey: unknown,
                in: container,
                debugDescription: "unknown member `\(unknown.stringValue)`; a filter node has only `op` and `args`"
            )
        }
        let op = try container.decode(String.self, forKey: .op)
        func malformed(_ message: String) -> DecodingError {
            DecodingError.dataCorruptedError(forKey: NodeKey.args, in: container, debugDescription: message)
        }
        switch op {
        case "and", "or":
            let operands = try container.decode([ToriiFilter].self, forKey: .args)
            guard !operands.isEmpty else {
                throw malformed("`\(op)` needs at least one operand")
            }
            self = op == "and" ? .and(operands) : .or(operands)
        case "not":
            let operands = try container.decode([ToriiFilter].self, forKey: .args)
            guard operands.count == 1 else {
                throw malformed("`not` takes an array with exactly one filter node")
            }
            self = .not(operands[0])
        case "eq", "ne", "lt", "lte", "gt", "gte", "in", "nin":
            var args = try container.nestedUnkeyedContainer(forKey: .args)
            guard args.count == 2 else {
                throw malformed("`\(op)` takes [\"field\", value]")
            }
            let field = try ToriiFieldPath(args.decode(String.self))
            switch op {
            case "in", "nin":
                let values = try args.decode([ToriiFilterValue].self)
                self = op == "in" ? .in(field, values) : .notIn(field, values)
            default:
                let value = try args.decode(ToriiFilterValue.self)
                switch op {
                case "eq": self = .eq(field, value)
                case "ne": self = .ne(field, value)
                case "lt": self = .lt(field, value)
                case "lte": self = .lte(field, value)
                case "gt": self = .gt(field, value)
                default: self = .gte(field, value)
                }
            }
        case "exists", "is_null":
            let fields = try container.decode([String].self, forKey: .args)
            guard fields.count == 1 else {
                throw malformed("`\(op)` takes [\"field\"]")
            }
            let field = ToriiFieldPath(fields[0])
            self = op == "exists" ? .exists(field) : .isNull(field)
        default:
            throw DecodingError.dataCorruptedError(
                forKey: NodeKey.op,
                in: container,
                debugDescription: "unknown operator `\(op)`; expected one of: and, or, not, eq, ne, lt, lte, gt, gte, in, nin, exists, is_null"
            )
        }
    }

    /// Encode the JSON form. Use `jsonData()` for the canonical byte spelling.
    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: NodeKey.self)
        try container.encode(operatorName, forKey: .op)
        var args = container.nestedUnkeyedContainer(forKey: .args)
        switch self {
        case let .and(operands), let .or(operands):
            for operand in operands {
                try args.encode(operand)
            }
        case let .not(inner):
            try args.encode(inner)
        case let .eq(field, value), let .ne(field, value), let .lt(field, value),
             let .lte(field, value), let .gt(field, value), let .gte(field, value):
            try args.encode(field.rawValue)
            try args.encode(value)
        case let .in(field, values), let .notIn(field, values):
            try args.encode(field.rawValue)
            try args.encode(values)
        case let .exists(field), let .isNull(field):
            try args.encode(field.rawValue)
        }
    }
}

/// Collects filters for `ToriiFilter.all { ... }` and `ToriiFilter.any { ... }`.
@resultBuilder
public enum ToriiFilterBuilder {
    public static func buildExpression(_ filter: ToriiFilter) -> [ToriiFilter] {
        [filter]
    }

    public static func buildExpression(_ filter: ToriiFilter?) -> [ToriiFilter] {
        filter.map { [$0] } ?? []
    }

    public static func buildBlock(_ components: [ToriiFilter]...) -> [ToriiFilter] {
        components.flatMap { $0 }
    }

    public static func buildOptional(_ component: [ToriiFilter]?) -> [ToriiFilter] {
        component ?? []
    }

    public static func buildEither(first component: [ToriiFilter]) -> [ToriiFilter] {
        component
    }

    public static func buildEither(second component: [ToriiFilter]) -> [ToriiFilter] {
        component
    }

    public static func buildArray(_ components: [[ToriiFilter]]) -> [ToriiFilter] {
        components.flatMap { $0 }
    }
}

// MARK: - Builder

/// A field awaiting an operator. Reusable: every operator returns a new filter.
///
/// ```swift
/// let quantity = ToriiField("quantity")
/// let filter = quantity > 1 && quantity < 5          // quantity > 1 and quantity < 5
/// let status = ToriiField("status").in(["active", "paused"])
/// let sort = quantity.descending                      // -quantity
/// ```
public struct ToriiField: Sendable, CustomStringConvertible {
    public let path: ToriiFieldPath

    public init(_ path: ToriiFieldPath) {
        self.path = path
    }

    public init(_ rawPath: String) {
        self.path = ToriiFieldPath(rawPath)
    }

    /// The field addressing one metadata entry, `metadata.<key>`.
    public static func metadata(_ key: String) -> ToriiField {
        ToriiField(ToriiFieldPath.metadata(key))
    }

    public var description: String {
        path.description
    }

    /// `field = value`
    public func eq<Value: ToriiFilterLiteral>(_ value: Value) -> ToriiFilter {
        .eq(path, value.toriiFilterValue)
    }

    /// `field != value` (also matches rows where the field is absent).
    public func ne<Value: ToriiFilterLiteral>(_ value: Value) -> ToriiFilter {
        .ne(path, value.toriiFilterValue)
    }

    /// `field < value`
    public func lt<Value: ToriiFilterLiteral>(_ value: Value) -> ToriiFilter {
        .lt(path, value.toriiFilterValue)
    }

    /// `field <= value`
    public func lte<Value: ToriiFilterLiteral>(_ value: Value) -> ToriiFilter {
        .lte(path, value.toriiFilterValue)
    }

    /// `field > value`
    public func gt<Value: ToriiFilterLiteral>(_ value: Value) -> ToriiFilter {
        .gt(path, value.toriiFilterValue)
    }

    /// `field >= value`
    public func gte<Value: ToriiFilterLiteral>(_ value: Value) -> ToriiFilter {
        .gte(path, value.toriiFilterValue)
    }

    /// `field in [values...]`
    public func `in`<Values: Sequence>(_ values: Values) -> ToriiFilter where Values.Element: ToriiFilterLiteral {
        .in(path, values.map(\.toriiFilterValue))
    }

    /// `field not in [values...]` (also matches rows where the field is absent).
    public func notIn<Values: Sequence>(_ values: Values) -> ToriiFilter where Values.Element: ToriiFilterLiteral {
        .notIn(path, values.map(\.toriiFilterValue))
    }

    /// `exists(field)`
    public var exists: ToriiFilter {
        .exists(path)
    }

    /// `field is null` (absent or null).
    public var isNull: ToriiFilter {
        .isNull(path)
    }

    /// `field is not null`
    public var isNotNull: ToriiFilter {
        .not(.isNull(path))
    }

    /// Ascending sort key on this field.
    public var ascending: ToriiSortKey {
        .ascending(path)
    }

    /// Descending sort key on this field.
    public var descending: ToriiSortKey {
        .descending(path)
    }

    public static func == <Value: ToriiFilterLiteral>(lhs: ToriiField, rhs: Value) -> ToriiFilter {
        lhs.eq(rhs)
    }

    public static func != <Value: ToriiFilterLiteral>(lhs: ToriiField, rhs: Value) -> ToriiFilter {
        lhs.ne(rhs)
    }

    public static func < <Value: ToriiFilterLiteral>(lhs: ToriiField, rhs: Value) -> ToriiFilter {
        lhs.lt(rhs)
    }

    public static func <= <Value: ToriiFilterLiteral>(lhs: ToriiField, rhs: Value) -> ToriiFilter {
        lhs.lte(rhs)
    }

    public static func > <Value: ToriiFilterLiteral>(lhs: ToriiField, rhs: Value) -> ToriiFilter {
        lhs.gt(rhs)
    }

    public static func >= <Value: ToriiFilterLiteral>(lhs: ToriiField, rhs: Value) -> ToriiFilter {
        lhs.gte(rhs)
    }
}

// MARK: - Canonical JSON writer

/// Deterministic compact JSON spelling shared by the text and JSON forms:
/// strings use JSON escapes (`\"`, `\\`, `\n`, `\r`, `\t`, `\b`, `\f`,
/// `\u00xx` for other control characters, everything else verbatim), object
/// members are ordered by UTF-8 bytes, and numbers are exact digits.
enum ToriiCanonicalJSON {
    static func writeString(_ value: String, into out: inout String) {
        out.append("\"")
        for scalar in value.unicodeScalars {
            switch scalar.value {
            case 0x22: out.append("\\\"")
            case 0x5C: out.append("\\\\")
            case 0x0A: out.append("\\n")
            case 0x0D: out.append("\\r")
            case 0x09: out.append("\\t")
            case 0x08: out.append("\\b")
            case 0x0C: out.append("\\f")
            case 0x00..<0x20:
                let hex = Array("0123456789abcdef")
                out.append("\\u00")
                out.append(hex[Int(scalar.value >> 4)])
                out.append(hex[Int(scalar.value & 0x0F)])
            default:
                out.unicodeScalars.append(scalar)
            }
        }
        out.append("\"")
    }

    static func write(_ value: ToriiFilterValue, into out: inout String) {
        switch value.storage {
        case .null:
            out.append("null")
        case let .bool(flag):
            out.append(flag ? "true" : "false")
        case let .integer(digits):
            out.append(digits)
        case let .string(text):
            writeString(text, into: &out)
        case let .array(values):
            out.append("[")
            for (index, element) in values.enumerated() {
                if index > 0 {
                    out.append(",")
                }
                write(element, into: &out)
            }
            out.append("]")
        case let .object(members):
            out.append("{")
            let keys = members.keys.sorted { $0.utf8.lexicographicallyPrecedes($1.utf8) }
            for (index, key) in keys.enumerated() {
                if index > 0 {
                    out.append(",")
                }
                writeString(key, into: &out)
                out.append(":")
                write(members[key] ?? .null, into: &out)
            }
            out.append("}")
        }
    }
}
