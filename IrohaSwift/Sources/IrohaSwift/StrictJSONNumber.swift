import Foundation

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
            guard rounded >= 0, rounded <= Double(UInt64.max) else { return nil }
            return UInt64(rounded)
        }
        let signed = number.int64Value
        guard signed >= 0 else { return nil }
        return UInt64(signed)
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
            guard rounded >= Double(Int.min), rounded <= Double(Int.max) else { return nil }
            return Int(rounded)
        }
        let signed = number.int64Value
        guard signed >= Int64(Int.min), signed <= Int64(Int.max) else { return nil }
        return Int(signed)
    }

    static func uint16(from value: Any?) -> UInt16? {
        guard let parsed = uint64(from: value), parsed <= UInt64(UInt16.max) else { return nil }
        return UInt16(parsed)
    }

    private static func trimmedString(from value: Any?) -> String? {
        guard let string = value as? String else { return nil }
        let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}
