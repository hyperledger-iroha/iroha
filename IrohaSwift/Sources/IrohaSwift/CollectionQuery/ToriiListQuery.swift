import Foundation

// MARK: - Errors

/// A collection-query control rejected before or by Torii.
///
/// `code` is the stable Torii error code for the control (`invalid_filter`,
/// `invalid_sort`, `invalid_select`, `invalid_aggregate`, `invalid_limit`,
/// `invalid_cursor`, `invalid_include_total` or `invalid_query`), so a
/// locally rejected query and a server rejection are handled the same way.
public struct ToriiListQueryError: Error, Hashable, Sendable, CustomStringConvertible, LocalizedError {
    /// The control at fault (`filter`, `sort`, `select`, `aggregate`,
    /// `limit`, `cursor`, `include_total`) or `query` for the request as a whole.
    public let parameter: String
    /// What is wrong, including a fix where one is obvious.
    public let message: String

    public init(parameter: String, message: String) {
        self.parameter = parameter
        self.message = message
    }

    /// Stable Torii error code for this control.
    public var code: String {
        Self.code(forParameter: parameter)
    }

    /// The Torii error code reported for `parameter`.
    public static func code(forParameter parameter: String) -> String {
        switch parameter {
        case "filter": return "invalid_filter"
        case "sort": return "invalid_sort"
        case "select": return "invalid_select"
        case "aggregate": return "invalid_aggregate"
        case "limit": return "invalid_limit"
        case "cursor": return "invalid_cursor"
        case "include_total": return "invalid_include_total"
        default: return "invalid_query"
        }
    }

    public var description: String {
        "invalid `\(parameter)`: \(message)"
    }

    public var errorDescription: String? {
        description
    }
}

// MARK: - Filter clause

/// The rows to keep: a built `ToriiFilter` or a text filter passed to Torii verbatim.
public enum ToriiFilterClause: Hashable, Sendable, CustomStringConvertible {
    /// A filter built with `ToriiField`/`ToriiFilter`; sent in the JSON form.
    case expression(ToriiFilter)
    /// Text in the filter grammar, e.g. `owned_by = "alice" and quantity > 1`.
    case text(String)

    /// The text form: the canonical rendering of a built filter, or the text as given.
    public var description: String {
        switch self {
        case let .expression(filter): return filter.description
        case let .text(text): return text
        }
    }

    /// The text sent as a `GET` parameter or event-stream filter.
    ///
    /// - Throws: `ToriiListQueryError` when a built filter has object or array
    ///   literals, which exist only in the JSON form.
    func transmittableText(parameter: String) throws -> String {
        switch self {
        case let .expression(filter): return try filter.transmittableText(parameter: parameter)
        case let .text(text): return text
        }
    }

    func validate(parameter: String) throws {
        switch self {
        case let .expression(filter):
            try filter.validate(parameter: parameter)
        case let .text(text):
            guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
                throw ToriiListQueryError(parameter: parameter, message: "expected a filter expression")
            }
            guard text.utf8.count <= ToriiFilter.maximumTextBytes else {
                throw ToriiListQueryError(
                    parameter: parameter,
                    message: "filters must not exceed \(ToriiFilter.maximumTextBytes) bytes"
                )
            }
        }
    }

    func writeJSON(into out: inout String) {
        switch self {
        case let .expression(filter): filter.writeJSON(into: &out)
        case let .text(text): ToriiCanonicalJSON.writeString(text, into: &out)
        }
    }
}

// MARK: - Sort keys

/// One sort key: `field` sorts ascending, `-field` descending.
public struct ToriiSortKey: Hashable, Sendable, CustomStringConvertible {
    public enum Order: Hashable, Sendable {
        case ascending
        case descending
    }

    /// Maximum keys in one sort specification.
    public static let maximumKeys = 8

    public var field: ToriiFieldPath
    public var order: Order

    public init(_ field: ToriiFieldPath, order: Order = .ascending) {
        self.field = field
        self.order = order
    }

    public static func ascending(_ field: ToriiFieldPath) -> ToriiSortKey {
        ToriiSortKey(field, order: .ascending)
    }

    public static func descending(_ field: ToriiFieldPath) -> ToriiSortKey {
        ToriiSortKey(field, order: .descending)
    }

    /// Parse one key in the text grammar, e.g. `-quantity` or ``metadata.`ui-order` ``.
    ///
    /// - Throws: `ToriiListQueryError` with code `invalid_sort`.
    public init(parsing text: String) throws {
        let keys = try Self.parseList(text)
        guard keys.count == 1 else {
            throw ToriiListQueryError(
                parameter: "sort",
                message: "expected exactly one sort key; pass each key as its own array element"
            )
        }
        self = keys[0]
    }

    /// Parse a comma-separated specification such as `-quantity,id`.
    ///
    /// - Throws: `ToriiListQueryError` with code `invalid_sort` for malformed,
    ///   empty or duplicate keys; the message names the column.
    public static func parseList(_ text: String) throws -> [ToriiSortKey] {
        var parser = ToriiSortSpecificationParser(text)
        return try parser.parse()
    }

    /// Canonical spelling, e.g. `-quantity` or ``metadata.`ui-order` ``.
    public var description: String {
        (order == .descending ? "-" : "") + field.description
    }
}

/// Lexer/parser for sort specifications, mirroring the Torii grammar and its messages.
private struct ToriiSortSpecificationParser {
    private enum Token: Equatable {
        case word(String)
        case quoted(String)
        case minus
        case dot
        case comma
        case end

        var describe: String {
            switch self {
            case let .word(word): return "`\(word)`"
            case let .quoted(segment): return "`\(segment)`"
            case .minus: return "`-`"
            case .dot: return "`.`"
            case .comma: return "`,`"
            case .end: return "the end of the input"
            }
        }
    }

    private let text: String
    private let bytes: [UInt8]
    private var tokens: [(token: Token, start: Int)] = []
    private var position = 0

    init(_ text: String) {
        self.text = text
        self.bytes = Array(text.utf8)
    }

    private func failure(at offset: Int, _ message: String) -> ToriiListQueryError {
        let clamped = min(max(offset, 0), bytes.count)
        let before = bytes[..<clamped]
        let line = before.filter { $0 == UInt8(ascii: "\n") }.count + 1
        let lineStart = before.lastIndex(of: UInt8(ascii: "\n")).map { $0 + 1 } ?? 0
        let column = String(decoding: bytes[lineStart..<clamped], as: UTF8.self).count + 1
        let location = text.contains("\n") ? "line \(line), column \(column)" : "column \(column)"
        return ToriiListQueryError(parameter: "sort", message: "\(message) (\(location))")
    }

    private mutating func lex() throws {
        var index = 0
        while true {
            while index < bytes.count, [UInt8(ascii: " "), UInt8(ascii: "\t"), UInt8(ascii: "\r"), UInt8(ascii: "\n")].contains(bytes[index]) {
                index += 1
            }
            let start = index
            guard index < bytes.count else {
                tokens.append((.end, start))
                return
            }
            let byte = bytes[index]
            switch byte {
            case UInt8(ascii: "-"):
                tokens.append((.minus, start))
                index += 1
            case UInt8(ascii: "."):
                tokens.append((.dot, start))
                index += 1
            case UInt8(ascii: ","):
                tokens.append((.comma, start))
                index += 1
            case UInt8(ascii: ":"):
                throw failure(at: start, "unexpected `:`; write `field` for ascending and `-field` for descending order")
            case UInt8(ascii: "`"):
                guard let close = bytes[(index + 1)...].firstIndex(of: UInt8(ascii: "`")) else {
                    throw failure(at: start, "unterminated backtick-quoted field name")
                }
                let segment = String(decoding: bytes[(index + 1)..<close], as: UTF8.self)
                guard !segment.isEmpty else {
                    throw failure(at: start, "backtick-quoted field names must not be empty")
                }
                guard !segment.contains(".") else {
                    throw failure(at: start, "a backtick-quoted segment must not contain `.`; quote each segment separately")
                }
                tokens.append((.quoted(segment), start))
                index = close + 1
            default:
                guard Self.isWordStart(byte) else {
                    let character = String(decoding: bytes[start...], as: UTF8.self).first.map(String.init) ?? "?"
                    throw failure(at: start, "unexpected character `\(character)`")
                }
                var end = index + 1
                while end < bytes.count, Self.isWordContinue(bytes[end]) {
                    end += 1
                }
                if end + 1 < bytes.count, bytes[end] == UInt8(ascii: "-"), Self.isASCIILetter(bytes[end + 1]) {
                    var wordEnd = end
                    while wordEnd < bytes.count, Self.isWordContinue(bytes[wordEnd]) || bytes[wordEnd] == UInt8(ascii: "-") {
                        wordEnd += 1
                    }
                    let word = String(decoding: bytes[start..<wordEnd], as: UTF8.self)
                    throw failure(at: start, "wrap field names containing `-` in backticks, e.g. `\(word)`")
                }
                tokens.append((.word(String(decoding: bytes[start..<end], as: UTF8.self)), start))
                index = end
            }
        }
    }

    private static func isASCIILetter(_ byte: UInt8) -> Bool {
        (UInt8(ascii: "a")...UInt8(ascii: "z")).contains(byte) || (UInt8(ascii: "A")...UInt8(ascii: "Z")).contains(byte)
    }

    private static func isWordStart(_ byte: UInt8) -> Bool {
        isASCIILetter(byte) || byte == UInt8(ascii: "_")
    }

    private static func isWordContinue(_ byte: UInt8) -> Bool {
        isWordStart(byte) || (UInt8(ascii: "0")...UInt8(ascii: "9")).contains(byte)
    }

    private mutating func advance() -> (token: Token, start: Int) {
        let current = tokens[position]
        if position < tokens.count - 1 {
            position += 1
        }
        return current
    }

    private func peek() -> (token: Token, start: Int) {
        tokens[position]
    }

    private mutating func path() throws -> ToriiFieldPath {
        let first = advance()
        var path: String
        switch first.token {
        case let .word(word):
            if ToriiFieldPath.keywords.contains(word.lowercased()) {
                throw failure(
                    at: first.start,
                    "expected a field name, found the keyword `\(word.lowercased())`; quote a field with this name as `\(word)` in backticks"
                )
            }
            path = word
        case let .quoted(segment):
            path = segment
        case let other:
            throw failure(at: first.start, "expected a field name, found \(other.describe)")
        }
        while peek().token == .dot {
            _ = advance()
            let segment = advance()
            switch segment.token {
            case let .word(word), let .quoted(word):
                path += ".\(word)"
            case let other:
                throw failure(at: segment.start, "expected a field name after `.`, found \(other.describe)")
            }
        }
        let field = ToriiFieldPath(path)
        do {
            try field.validate(parameter: "sort")
        } catch let error as ToriiListQueryError {
            throw failure(at: first.start, error.message)
        }
        return field
    }

    mutating func parse() throws -> [ToriiSortKey] {
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw failure(at: 0, "expected at least one sort key")
        }
        try lex()
        var keys: [ToriiSortKey] = []
        while true {
            let token = peek()
            let order: ToriiSortKey.Order
            if token.token == .minus {
                _ = advance()
                order = .descending
            } else {
                order = .ascending
            }
            let field = try path()
            if keys.contains(where: { $0.field == field }) {
                throw failure(at: token.start, "sort key `\(field)` appears more than once")
            }
            keys.append(ToriiSortKey(field, order: order))
            if keys.count > ToriiSortKey.maximumKeys {
                throw failure(at: token.start, "sort specifications accept at most \(ToriiSortKey.maximumKeys) keys")
            }
            let next = advance()
            switch next.token {
            case .end:
                return keys
            case .comma:
                continue
            case let .word(word) where ["asc", "desc"].contains(word.lowercased()):
                throw failure(at: next.start, "write `field` for ascending and `-field` for descending order")
            case let other:
                throw failure(at: next.start, "expected `,` between sort keys, found \(other.describe)")
            }
        }
    }
}

// MARK: - Aggregates

/// Grouped metrics computed after filtering (`POST /query` only).
///
/// Aggregates are computed where the rows live: Torii rejects a read whose
/// visible rows span several dataspace routes with `invalid_aggregate`; page
/// through the rows without `aggregate` instead. History collections never
/// accept aggregates.
///
/// ```swift
/// let aggregate = ToriiAggregate(
///     groupBy: ["asset"],
///     metrics: [.count(as: "holders"), .sum("quantity", as: "supply")],
///     having: ToriiField("holders") >= 10
/// )
/// ```
public struct ToriiAggregate: Hashable, Sendable {
    public enum Function: String, Hashable, Sendable, CaseIterable {
        case count
        case sum
        case min
        case max
        case avg
        case distinctCount = "distinct_count"
    }

    /// One metric computed per group.
    public struct Metric: Hashable, Sendable {
        /// Output column name, also usable in `having` and `sort`.
        public var alias: String
        public var function: Function
        /// Field consumed by the function (absent for `count`).
        public var field: ToriiFieldPath?

        public init(alias: String, function: Function, field: ToriiFieldPath? = nil) {
            self.alias = alias
            self.function = function
            self.field = field
        }

        public static func count(as alias: String) -> Metric {
            Metric(alias: alias, function: .count)
        }

        public static func sum(_ field: ToriiFieldPath, as alias: String) -> Metric {
            Metric(alias: alias, function: .sum, field: field)
        }

        public static func min(_ field: ToriiFieldPath, as alias: String) -> Metric {
            Metric(alias: alias, function: .min, field: field)
        }

        public static func max(_ field: ToriiFieldPath, as alias: String) -> Metric {
            Metric(alias: alias, function: .max, field: field)
        }

        public static func avg(_ field: ToriiFieldPath, as alias: String) -> Metric {
            Metric(alias: alias, function: .avg, field: field)
        }

        public static func distinctCount(_ field: ToriiFieldPath, as alias: String) -> Metric {
            Metric(alias: alias, function: .distinctCount, field: field)
        }
    }

    /// Grouping dimensions.
    public var groupBy: [ToriiFieldPath]
    /// Metrics computed per group; at least one.
    public var metrics: [Metric]
    /// Filter over group fields and metric aliases.
    public var having: ToriiFilterClause?

    public init(groupBy: [ToriiFieldPath] = [], metrics: [Metric], having: ToriiFilter? = nil) {
        self.groupBy = groupBy
        self.metrics = metrics
        self.having = having.map(ToriiFilterClause.expression)
    }

    public init(groupBy: [ToriiFieldPath] = [], metrics: [Metric], havingText: String) {
        self.groupBy = groupBy
        self.metrics = metrics
        self.having = .text(havingText)
    }

    func validate() throws {
        guard !metrics.isEmpty else {
            throw ToriiListQueryError(parameter: "aggregate", message: "`metrics` must list at least one metric")
        }
        if let having {
            do {
                try having.validate(parameter: "aggregate")
            } catch let error as ToriiListQueryError {
                throw ToriiListQueryError(parameter: "aggregate", message: "having: \(error.message)")
            }
        }
    }

    func writeJSON(into out: inout String) {
        out += "{\"group_by\":["
        for (index, field) in groupBy.enumerated() {
            if index > 0 {
                out += ","
            }
            ToriiCanonicalJSON.writeString(field.rawValue, into: &out)
        }
        out += "]"
        if let having {
            out += ",\"having\":"
            having.writeJSON(into: &out)
        }
        out += ",\"metrics\":["
        for (index, metric) in metrics.enumerated() {
            if index > 0 {
                out += ","
            }
            out += "{\"alias\":"
            ToriiCanonicalJSON.writeString(metric.alias, into: &out)
            if let field = metric.field {
                out += ",\"field\":"
                ToriiCanonicalJSON.writeString(field.rawValue, into: &out)
            }
            out += ",\"fn\":"
            ToriiCanonicalJSON.writeString(metric.function.rawValue, into: &out)
            out += "}"
        }
        out += "]}"
    }
}

// MARK: - List query

/// Filter, ordering, projection and page controls for one collection read
/// (`GET /v1/<collection>` or `POST /v1/<collection>/query`).
///
/// ```swift
/// let query = ToriiListQuery(
///     filter: ToriiField("owned_by") == "alice" && ToriiField("quantity") > 1,
///     sort: [.descending("quantity"), .ascending("id")],
///     limit: 25
/// )
/// ```
///
/// Validation mirrors Torii, so most mistakes surface locally as a
/// `ToriiListQueryError` carrying the same `code` Torii would return.
public struct ToriiListQuery: Hashable, Sendable {
    /// Maximum fields in one projection.
    public static let maximumSelectFields = 64
    /// Maximum encoded length of a continuation cursor.
    public static let maximumCursorBytes = 4_096

    /// Rows to keep; all rows when `nil`.
    public var filter: ToriiFilterClause?
    /// Ordering; the collection's default order applies when empty.
    public var sort: [ToriiSortKey]
    /// Fields to return per item; full rows when `nil`.
    public var select: [ToriiFieldPath]?
    /// Grouped metrics instead of rows (`POST` only; excludes `select`).
    public var aggregate: ToriiAggregate?
    /// Rows per page; the server default applies when `nil`.
    public var limit: Int?
    /// Continuation token from a previous page's `nextCursor`.
    public var cursor: String?
    /// Whether to compute the exact number of matching rows (costs a full scan).
    public var includeTotal: Bool

    public init(filter: ToriiFilter? = nil,
                sort: [ToriiSortKey] = [],
                select: [ToriiFieldPath]? = nil,
                aggregate: ToriiAggregate? = nil,
                limit: Int? = nil,
                cursor: String? = nil,
                includeTotal: Bool = false) {
        self.filter = filter.map(ToriiFilterClause.expression)
        self.sort = sort
        self.select = select
        self.aggregate = aggregate
        self.limit = limit
        self.cursor = cursor
        self.includeTotal = includeTotal
    }

    /// A query with a text filter passed to Torii as-is.
    public init(filterText: String,
                sort: [ToriiSortKey] = [],
                select: [ToriiFieldPath]? = nil,
                aggregate: ToriiAggregate? = nil,
                limit: Int? = nil,
                cursor: String? = nil,
                includeTotal: Bool = false) {
        self.init(sort: sort, select: select, aggregate: aggregate, limit: limit, cursor: cursor, includeTotal: includeTotal)
        self.filter = .text(filterText)
    }

    /// This query with `filter` as its filter.
    public func filtered(by filter: ToriiFilter) -> ToriiListQuery {
        var copy = self
        copy.filter = .expression(filter)
        return copy
    }

    /// This query ordered by `keys`.
    public func sorted(by keys: ToriiSortKey...) -> ToriiListQuery {
        var copy = self
        copy.sort = keys
        return copy
    }

    /// This query returning only `fields` per item.
    public func selecting(_ fields: ToriiFieldPath...) -> ToriiListQuery {
        var copy = self
        copy.select = fields
        return copy
    }

    /// This query with `limit` rows per page.
    public func limited(to limit: Int) -> ToriiListQuery {
        var copy = self
        copy.limit = limit
        return copy
    }

    /// This query continuing after the page that returned `cursor`.
    public func after(_ cursor: String) -> ToriiListQuery {
        var copy = self
        copy.cursor = cursor
        return copy
    }

    /// This query asking for the exact match count.
    public func includingTotal(_ includeTotal: Bool = true) -> ToriiListQuery {
        var copy = self
        copy.includeTotal = includeTotal
        return copy
    }

    /// The same query positioned after `page`, or `nil` on the last page.
    public func next<Item>(after page: ToriiPage<Item>) -> ToriiListQuery? {
        page.nextCursor.map(after)
    }

    /// Check every control without contacting a server.
    ///
    /// - Throws: `ToriiListQueryError` for the first invalid control.
    public func validate() throws {
        try filter?.validate(parameter: "filter")
        guard sort.count <= ToriiSortKey.maximumKeys else {
            throw ToriiListQueryError(parameter: "sort", message: "at most \(ToriiSortKey.maximumKeys) sort keys are allowed")
        }
        for (index, key) in sort.enumerated() {
            try key.field.validate(parameter: "sort")
            if sort[..<index].contains(where: { $0.field == key.field }) {
                throw ToriiListQueryError(parameter: "sort", message: "sort key `\(key.field)` appears more than once")
            }
        }
        if let select {
            guard !select.isEmpty else {
                throw ToriiListQueryError(parameter: "select", message: "`select` must list at least one field")
            }
            guard select.count <= Self.maximumSelectFields else {
                throw ToriiListQueryError(
                    parameter: "select",
                    message: "at most \(Self.maximumSelectFields) fields can be selected"
                )
            }
            for (index, field) in select.enumerated() {
                try field.validate(parameter: "select")
                if select[..<index].contains(field) {
                    throw ToriiListQueryError(parameter: "select", message: "field `\(field)` is selected more than once")
                }
            }
            if aggregate != nil {
                throw ToriiListQueryError(
                    parameter: "select",
                    message: "`select` and `aggregate` cannot be combined; aggregates define their own columns"
                )
            }
        }
        try aggregate?.validate()
        if let limit {
            guard limit >= 1 else {
                throw ToriiListQueryError(parameter: "limit", message: "`limit` must be at least 1")
            }
            guard limit <= Int(UInt32.max) else {
                throw ToriiListQueryError(parameter: "limit", message: "`limit` is too large")
            }
        }
        if let cursor {
            let valid = !cursor.isEmpty
                && cursor.utf8.count <= Self.maximumCursorBytes
                && cursor.utf8.allSatisfy { byte in
                    (UInt8(ascii: "a")...UInt8(ascii: "z")).contains(byte)
                        || (UInt8(ascii: "A")...UInt8(ascii: "Z")).contains(byte)
                        || (UInt8(ascii: "0")...UInt8(ascii: "9")).contains(byte)
                        || byte == UInt8(ascii: "-")
                        || byte == UInt8(ascii: "_")
                }
            guard valid else {
                throw ToriiListQueryError(
                    parameter: "cursor",
                    message: "`cursor` must be a `next_cursor` value returned by a previous page"
                )
            }
        }
    }

    /// Canonical JSON body for `POST /v1/<collection>/query`: compact, absent
    /// controls omitted, and members in byte order (`aggregate`, `cursor`,
    /// `filter`, `include_total`, `limit`, `select`, `sort`) exactly as Torii's
    /// reference serializer writes it. Member order carries no meaning.
    ///
    /// - Throws: `ToriiListQueryError` when a control is invalid.
    public func requestBody() throws -> Data {
        try validate()
        var out = "{"
        var needsSeparator = false
        func member(_ name: String) {
            if needsSeparator {
                out += ","
            }
            needsSeparator = true
            ToriiCanonicalJSON.writeString(name, into: &out)
            out += ":"
        }
        if let aggregate {
            member("aggregate")
            aggregate.writeJSON(into: &out)
        }
        if let cursor {
            member("cursor")
            ToriiCanonicalJSON.writeString(cursor, into: &out)
        }
        if let filter {
            member("filter")
            filter.writeJSON(into: &out)
        }
        if includeTotal {
            member("include_total")
            out += "true"
        }
        if let limit {
            member("limit")
            out += String(limit)
        }
        if let select {
            member("select")
            out += "["
            for (index, field) in select.enumerated() {
                if index > 0 {
                    out += ","
                }
                ToriiCanonicalJSON.writeString(field.rawValue, into: &out)
            }
            out += "]"
        }
        if !sort.isEmpty {
            member("sort")
            out += "["
            for (index, key) in sort.enumerated() {
                if index > 0 {
                    out += ","
                }
                ToriiCanonicalJSON.writeString(key.description, into: &out)
            }
            out += "]"
        }
        out += "}"
        return Data(out.utf8)
    }

    /// `GET` parameters in canonical order (not yet percent-encoded).
    ///
    /// - Throws: `ToriiListQueryError` when a control is invalid, the query
    ///   has an aggregate, or the filter has object or array literals; only
    ///   `POST /query` accepts those.
    public func queryItems() throws -> [URLQueryItem] {
        try validate()
        guard aggregate == nil else {
            throw ToriiListQueryError(parameter: "aggregate", message: "aggregates are only available through POST /query")
        }
        var items: [URLQueryItem] = []
        if let filter {
            items.append(URLQueryItem(name: "filter", value: try filter.transmittableText(parameter: "filter")))
        }
        if !sort.isEmpty {
            items.append(URLQueryItem(name: "sort", value: sort.map(\.description).joined(separator: ",")))
        }
        if let select {
            items.append(URLQueryItem(name: "select", value: select.map(\.rawValue).joined(separator: ",")))
        }
        if let limit {
            items.append(URLQueryItem(name: "limit", value: String(limit)))
        }
        if let cursor {
            items.append(URLQueryItem(name: "cursor", value: cursor))
        }
        if includeTotal {
            items.append(URLQueryItem(name: "include_total", value: "true"))
        }
        return items
    }
}

// MARK: - Query-string encoding

enum ToriiQueryStringEncoding {
    /// RFC 3986 unreserved characters; everything else is percent-encoded so
    /// `+`, `&`, `=` and `#` in values survive form decoding.
    private static let unreserved: CharacterSet = {
        var allowed = CharacterSet()
        allowed.insert(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")
        return allowed
    }()

    static func encodeComponent(_ value: String) -> String {
        value.addingPercentEncoding(withAllowedCharacters: unreserved) ?? value
    }

    /// Percent-encoded query string for ordered parameters.
    static func encode(_ items: [URLQueryItem]) -> String {
        items.map { item in
            "\(encodeComponent(item.name))=\(encodeComponent(item.value ?? ""))"
        }.joined(separator: "&")
    }
}
