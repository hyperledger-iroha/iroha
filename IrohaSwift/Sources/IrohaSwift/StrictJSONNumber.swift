import Foundation

let exactJSONNumberLexemesUserInfoKey = CodingUserInfoKey(
    rawValue: "org.hyperledger.iroha.exact-json-number-lexemes"
)!

struct SccpUInt128: Equatable, Sendable {
    static let maximumDecimal = "340282366920938463463374607431768211455"
    static let maximumSafeJSONIntegerDecimal = "9007199254740991"

    let decimalString: String

    static func parse(_ value: String) -> Self? {
        let bytes = Array(value.utf8)
        let maximumBytes = Array(maximumDecimal.utf8)
        guard !bytes.isEmpty,
              bytes.allSatisfy({ (0x30...0x39).contains($0) }),
              bytes.count == 1 || bytes[0] != 0x30,
              bytes.count < maximumBytes.count
                  || bytes.count == maximumBytes.count
                      && !maximumBytes.lexicographicallyPrecedes(bytes) else {
            return nil
        }
        return Self(decimalString: value)
    }

    var exceedsMaximumSafeJSONInteger: Bool {
        let bytes = Array(decimalString.utf8)
        let maximumSafeBytes = Array(Self.maximumSafeJSONIntegerDecimal.utf8)
        return bytes.count > maximumSafeBytes.count
            || bytes.count == maximumSafeBytes.count
                && maximumSafeBytes.lexicographicallyPrecedes(bytes)
    }
}

func exactJSONNumberCodingPathKey(_ path: [CodingKey]) -> String {
    path.map { key in
        if let index = key.intValue {
            return "i:\(index)"
        }
        return "k:\(key.stringValue.utf8.count):\(key.stringValue)"
    }.joined(separator: "/")
}

func legacyExactJSONIntegerCodingPathKey(_ path: [CodingKey]) -> String {
    path.map { key in
        if let index = key.intValue {
            return "i:\(index)"
        }
        return "k:\(key.stringValue)"
    }.joined(separator: "/")
}

enum ExactJSONNumberLexemeScanner {
    enum ScanError: Swift.Error {
        case invalidJSON
    }

    static func scan(_ data: Data) throws -> [String: String] {
        try StrictJSONDuplicateKeyRejector.rejectDuplicateObjectKeys(in: data)
        guard let text = String(data: data, encoding: .utf8), Data(text.utf8) == data else {
            throw ScanError.invalidJSON
        }
        var parser = Parser(Array(data))
        return try parser.parse()
    }

    private enum PathComponent {
        case key(String)
        case index(Int)
    }

    private struct Parser {
        private static let maximumNestingDepth = 128

        private let bytes: [UInt8]
        private var index = 0
        private var lexemes: [String: String] = [:]

        init(_ bytes: [UInt8]) {
            self.bytes = bytes
        }

        mutating func parse() throws -> [String: String] {
            try parseValue(path: [], depth: 0)
            skipWhitespace()
            guard index == bytes.count else { throw ScanError.invalidJSON }
            return lexemes
        }

        private mutating func parseValue(path: [PathComponent], depth: Int) throws {
            guard depth <= Self.maximumNestingDepth else { throw ScanError.invalidJSON }
            skipWhitespace()
            guard let byte = peek() else { throw ScanError.invalidJSON }
            switch byte {
            case 0x7b:
                try parseObject(path: path, depth: depth)
            case 0x5b:
                try parseArray(path: path, depth: depth)
            case 0x22:
                _ = try parseString()
            case 0x2d, 0x30...0x39:
                let lexeme = try parseNumber()
                lexemes[pathKey(path)] = lexeme
            case 0x74:
                try consume("true")
            case 0x66:
                try consume("false")
            case 0x6e:
                try consume("null")
            default:
                throw ScanError.invalidJSON
            }
        }

        private mutating func parseObject(path: [PathComponent], depth: Int) throws {
            try consume("{")
            skipWhitespace()
            if consumeIf("}") { return }
            while true {
                skipWhitespace()
                let key = try parseString()
                skipWhitespace()
                try consume(":")
                try parseValue(path: path + [.key(key)], depth: depth + 1)
                skipWhitespace()
                if consumeIf("}") { return }
                try consume(",")
            }
        }

        private mutating func parseArray(path: [PathComponent], depth: Int) throws {
            try consume("[")
            skipWhitespace()
            if consumeIf("]") { return }
            var elementIndex = 0
            while true {
                try parseValue(path: path + [.index(elementIndex)], depth: depth + 1)
                elementIndex += 1
                skipWhitespace()
                if consumeIf("]") { return }
                try consume(",")
            }
        }

        private mutating func parseString() throws -> String {
            let start = index
            try consume("\"")
            while let byte = peek() {
                switch byte {
                case 0x22:
                    index += 1
                    let encoded = Data(bytes[start..<index])
                    guard let decoded = try? JSONDecoder().decode(String.self, from: encoded) else {
                        throw ScanError.invalidJSON
                    }
                    return decoded
                case 0x5c:
                    index += 1
                    guard index < bytes.count else { throw ScanError.invalidJSON }
                    index += 1
                default:
                    index += 1
                }
            }
            throw ScanError.invalidJSON
        }

        private mutating func parseNumber() throws -> String {
            let start = index
            _ = consumeIf("-")
            guard let first = peek(), Self.isDigit(first) else { throw ScanError.invalidJSON }
            if first == 0x30 {
                index += 1
            } else {
                while let byte = peek(), Self.isDigit(byte) { index += 1 }
            }
            if consumeIf(".") {
                guard let digit = peek(), Self.isDigit(digit) else { throw ScanError.invalidJSON }
                while let byte = peek(), Self.isDigit(byte) { index += 1 }
            }
            if let byte = peek(), byte == 0x65 || byte == 0x45 {
                index += 1
                if let sign = peek(), sign == 0x2b || sign == 0x2d { index += 1 }
                guard let digit = peek(), Self.isDigit(digit) else { throw ScanError.invalidJSON }
                while let digit = peek(), Self.isDigit(digit) { index += 1 }
            }
            return String(decoding: bytes[start..<index], as: UTF8.self)
        }

        private func pathKey(_ path: [PathComponent]) -> String {
            path.map { component in
                switch component {
                case .key(let key):
                    return "k:\(key.utf8.count):\(key)"
                case .index(let index):
                    return "i:\(index)"
                }
            }.joined(separator: "/")
        }

        private mutating func skipWhitespace() {
            while let byte = peek(), byte == 0x20 || byte == 0x09 || byte == 0x0a || byte == 0x0d {
                index += 1
            }
        }

        private mutating func consume(_ literal: String) throws {
            guard consumeIf(literal) else { throw ScanError.invalidJSON }
        }

        private mutating func consumeIf(_ literal: String) -> Bool {
            let literalBytes = Array(literal.utf8)
            guard index + literalBytes.count <= bytes.count,
                  bytes[index..<(index + literalBytes.count)].elementsEqual(literalBytes) else {
                return false
            }
            index += literalBytes.count
            return true
        }

        private func peek() -> UInt8? {
            index < bytes.count ? bytes[index] : nil
        }

        private static func isDigit(_ byte: UInt8) -> Bool {
            (0x30...0x39).contains(byte)
        }
    }
}

enum ToriiJSONExactEncoder {
    static func encode(_ value: ToriiJSONValue, prettyPrinted: Bool) throws -> Data {
        var output = ""
        try append(value, to: &output, depth: 0, prettyPrinted: prettyPrinted)
        return Data(output.utf8)
    }

    private static func append(
        _ value: ToriiJSONValue,
        to output: inout String,
        depth: Int,
        prettyPrinted: Bool
    ) throws {
        guard depth <= 128 else {
            throw EncodingError.invalidValue(
                value,
                .init(codingPath: [], debugDescription: "JSON nesting exceeds 128 levels")
            )
        }
        switch value {
        case .null:
            output.append("null")
        case .bool(let flag):
            output.append(flag ? "true" : "false")
        case .number(let number):
            output.append(try encodedPrimitive(number))
        case .integer(let integer):
            guard SccpUInt128.parse(integer) != nil else {
                throw EncodingError.invalidValue(
                    integer,
                    .init(
                        codingPath: [],
                        debugDescription: "integer must be a canonical UInt128 decimal"
                    )
                )
            }
            output.append(integer)
        case .string(let string):
            output.append(try encodedPrimitive(string))
        case .array(let values):
            output.append("[")
            for (index, item) in values.enumerated() {
                if index != 0 { output.append(",") }
                if prettyPrinted {
                    output.append("\n")
                    output.append(indentation(depth + 1))
                }
                try append(item, to: &output, depth: depth + 1, prettyPrinted: prettyPrinted)
            }
            if prettyPrinted, !values.isEmpty {
                output.append("\n")
                output.append(indentation(depth))
            }
            output.append("]")
        case .object(let object):
            output.append("{")
            let keys = object.keys.sorted(by: unicodeScalarLexicographicallyPrecedes)
            for (index, key) in keys.enumerated() {
                if index != 0 { output.append(",") }
                if prettyPrinted {
                    output.append("\n")
                    output.append(indentation(depth + 1))
                }
                output.append(try encodedPrimitive(key))
                output.append(prettyPrinted ? ": " : ":")
                if let item = object[key] {
                    try append(item, to: &output, depth: depth + 1, prettyPrinted: prettyPrinted)
                }
            }
            if prettyPrinted, !keys.isEmpty {
                output.append("\n")
                output.append(indentation(depth))
            }
            output.append("}")
        }
    }

    private static func encodedPrimitive<T: Encodable>(_ value: T) throws -> String {
        let data = try JSONEncoder().encode(value)
        guard let encoded = String(data: data, encoding: .utf8) else {
            throw EncodingError.invalidValue(
                value,
                .init(codingPath: [], debugDescription: "failed to encode JSON primitive")
            )
        }
        return encoded
    }

    private static func indentation(_ depth: Int) -> String {
        String(repeating: " ", count: depth * 2)
    }

    private static func unicodeScalarLexicographicallyPrecedes(
        _ left: String,
        _ right: String
    ) -> Bool {
        var leftScalars = left.unicodeScalars.makeIterator()
        var rightScalars = right.unicodeScalars.makeIterator()
        while true {
            switch (leftScalars.next(), rightScalars.next()) {
            case let (left?, right?):
                if left.value != right.value { return left.value < right.value }
            case (nil, nil):
                return false
            case (nil, _?):
                return true
            case (_?, nil):
                return false
            }
        }
    }
}

enum StrictJSONNumber {
    static func uint64(from value: Any?) -> UInt64? {
        if let string = trimmedString(from: value) {
            return UInt64(string)
        }
        guard let number = value as? NSNumber else { return nil }
        guard CFGetTypeID(number) != CFBooleanGetTypeID() else { return nil }
        if CFNumberIsFloatType(number) {
            let doubleValue = number.doubleValue
            guard doubleValue.isFinite else { return nil }
            let rounded = doubleValue.rounded(.towardZero)
            guard rounded == doubleValue else { return nil }
            return UInt64(exactly: rounded)
        }
        return UInt64(number.stringValue)
    }

    static func int(from value: Any?) -> Int? {
        if let string = trimmedString(from: value) {
            return Int(string)
        }
        guard let number = value as? NSNumber else { return nil }
        guard CFGetTypeID(number) != CFBooleanGetTypeID() else { return nil }
        if CFNumberIsFloatType(number) {
            let doubleValue = number.doubleValue
            guard doubleValue.isFinite else { return nil }
            let rounded = doubleValue.rounded(.towardZero)
            guard rounded == doubleValue else { return nil }
            return Int(exactly: rounded)
        }
        return Int(number.stringValue)
    }

    static func uint16(from value: Any?) -> UInt16? {
        guard let parsed = uint64(from: value), parsed <= UInt64(UInt16.max) else { return nil }
        return UInt16(parsed)
    }

    static func saturatingNanoseconds(from seconds: TimeInterval) -> UInt64 {
        guard !seconds.isNaN, seconds > 0 else { return 0 }
        guard seconds.isFinite else { return UInt64.max }
        let nanoseconds = seconds * 1_000_000_000
        guard nanoseconds.isFinite, nanoseconds < Double(UInt64.max) else {
            return UInt64.max
        }
        return UInt64(exactly: nanoseconds.rounded(.towardZero)) ?? UInt64.max
    }

    private static func trimmedString(from value: Any?) -> String? {
        guard let string = value as? String else { return nil }
        let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}
