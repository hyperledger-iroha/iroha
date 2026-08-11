import Foundation

enum ToriiConnectJSON {
    static func normalizedString(_ value: ToriiJSONValue?) -> String? {
        guard let value else { return nil }
        switch value {
        case .string(let string):
            let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
            return trimmed.isEmpty ? nil : trimmed
        case .number(let number):
            guard number.isFinite else { return nil }
            if number.rounded(.towardZero) == number {
                guard let integer = Int(exactly: number) else { return nil }
                return String(integer)
            }
            return String(number)
        case .bool(let bool):
            return bool ? "true" : "false"
        default:
            return nil
        }
    }

    static func optionalString(_ record: [String: ToriiJSONValue],
                               key: String) -> String? {
        normalizedString(record[key])
    }

    static func requireString(_ record: [String: ToriiJSONValue],
                              key: String,
                              field: String) throws -> String {
        if let value = optionalString(record, key: key) {
            return value
        }
        throw ToriiClientError.invalidPayload("\(field) field was missing or empty")
    }

    static func requireExactString(_ record: [String: ToriiJSONValue],
                                   key: String,
                                   field: String) throws -> String {
        guard case .string(let value) = record[key],
              !value.isEmpty,
              value.trimmingCharacters(in: .whitespacesAndNewlines) == value else {
            throw ToriiClientError.invalidPayload(
                "\(field) must be an exact non-empty JSON string"
            )
        }
        return value
    }

    static func optionalBool(_ record: [String: ToriiJSONValue],
                             key: String) -> Bool? {
        guard let value = record[key] else { return nil }
        switch value {
        case .bool(let bool):
            return bool
        case .string(let string):
            let lowercased = string.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
            if lowercased == "true" { return true }
            if lowercased == "false" { return false }
        default:
            break
        }
        return nil
    }

    static func optionalUInt64(_ record: [String: ToriiJSONValue],
                               key: String) -> UInt64? {
        guard let value = record[key] else { return nil }
        return value.normalizedUInt64
    }

    static func requireUInt64(_ record: [String: ToriiJSONValue],
                              key: String,
                              field: String) throws -> UInt64 {
        if let value = optionalUInt64(record, key: key) {
            return value
        }
        throw ToriiClientError.invalidPayload("\(field) field was missing or invalid")
    }

    static func requireUInt64(_ record: [String: ToriiJSONValue],
                              key: String,
                              field: String,
                              allowZero: Bool) throws -> UInt64 {
        let value = try requireUInt64(record, key: key, field: field)
        if !allowZero && value == 0 {
            throw ToriiClientError.invalidPayload("\(field) must be greater than zero")
        }
        return value
    }

    static func optionalInt(_ record: [String: ToriiJSONValue],
                            key: String) -> Int? {
        guard let value = record[key] else { return nil }
        guard let parsed = value.normalizedInt64 else { return nil }
        guard parsed >= Int64(Int.min), parsed <= Int64(Int.max) else {
            return nil
        }
        return Int(parsed)
    }

    static func objectsArray(_ record: [String: ToriiJSONValue],
                             key: String,
                             field: String) throws -> [[String: ToriiJSONValue]] {
        guard let value = record[key] else { return [] }
        guard case .array(let array) = value else {
            throw ToriiClientError.invalidPayload("\(field) must be an array")
        }
        return try array.map { value in
            guard case .object(let object) = value else {
                throw ToriiClientError.invalidPayload("\(field) entries must be objects")
            }
            return object
        }
    }

    static func requireObject(_ record: [String: ToriiJSONValue],
                              key: String,
                              field: String) throws -> [String: ToriiJSONValue] {
        guard let value = record[key] else {
            throw ToriiClientError.invalidPayload("\(field) field was missing or invalid")
        }
        if case .object(let object) = value {
            return object
        }
        throw ToriiClientError.invalidPayload("\(field) must be an object")
    }

    static func optionalObject(_ record: [String: ToriiJSONValue],
                               key: String) -> [String: ToriiJSONValue]? {
        guard let value = record[key] else { return nil }
        if case .object(let object) = value {
            return object
        }
        return nil
    }

    static func stringArray(_ value: ToriiJSONValue?,
                            field: String) throws -> [String] {
        guard let value else { return [] }
        guard case .array(let values) = value else {
            throw ToriiClientError.invalidPayload("\(field) must be an array")
        }
        var result: [String] = []
        for item in values {
            guard case .string(let raw) = item else {
                throw ToriiClientError.invalidPayload("\(field) entries must be strings")
            }
            let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else {
                throw ToriiClientError.invalidPayload("\(field) entries must not be empty")
            }
            result.append(trimmed)
        }
        return result
    }

    static func mergeExtra(record: [String: ToriiJSONValue],
                           knownKeys: Set<String>) -> [String: ToriiJSONValue] {
        var extra: [String: ToriiJSONValue] = [:]
        for (key, value) in record where !knownKeys.contains(key) {
            extra[key] = value
        }
        return extra
    }

    static func encodePayload(_ payload: [String: ToriiJSONValue]) throws -> Data {
        try ToriiJSONValue.object(payload).encodedData()
    }

    static func trimmedNonEmpty(_ value: String,
                                field: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw ToriiClientError.invalidPayload("\(field) must be a non-empty string")
        }
        return trimmed
    }
}
