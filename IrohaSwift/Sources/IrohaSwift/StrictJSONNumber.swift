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
