import Foundation

extension ToriiJSONValue {
    public var normalizedString: String? {
        switch self {
        case .string(let string):
            let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
            return trimmed.isEmpty ? nil : trimmed
        case .number(let number):
            guard number.isFinite else {
                return nil
            }
            if number.rounded(.towardZero) == number {
                guard let value = Int(exactly: number) else {
                    return nil
                }
                return String(value)
            }
            return String(number)
        case .bool(let value):
            return value ? "true" : "false"
        case .null:
            return nil
        case .array, .object:
            return nil
        }
    }

    public var numberValue: Double? {
        switch self {
        case .number(let number):
            return number.isFinite ? number : nil
        case .string(let string):
            return Double(string.trimmingCharacters(in: .whitespacesAndNewlines))
        default:
            return nil
        }
    }

    public var normalizedUInt64: UInt64? {
        switch self {
        case .number(let number):
            guard number.isFinite, let value = UInt64(exactly: number) else {
                return nil
            }
            return value
        case .string(let string):
            return UInt64(string.trimmingCharacters(in: .whitespacesAndNewlines))
        default:
            return nil
        }
    }

    public var normalizedBytes: Data? {
        switch self {
        case .array(let items):
            var bytes = Data(capacity: items.count)
            for item in items {
                guard case let .number(number) = item,
                      number.isFinite,
                      number.rounded(.towardZero) == number,
                      number >= 0,
                      number <= 255
                else {
                    return nil
                }
                bytes.append(UInt8(number))
            }
            return bytes
        case .string(let string):
            let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.isEmpty {
                return nil
            }
            if let base64 = Data(base64Encoded: trimmed) {
                return base64
            }
            let cleaned = trimmed.lowercased().hasPrefix("0x") ? String(trimmed.dropFirst(2)) : trimmed
            guard !cleaned.isEmpty else {
                return nil
            }
            return Data(hexString: cleaned)
        default:
            return nil
        }
    }

    public var normalizedInt64: Int64? {
        switch self {
        case .number(let number):
            guard number.isFinite, let value = Int64(exactly: number) else {
                return nil
            }
            return value
        case .string(let string):
            return Int64(string.trimmingCharacters(in: .whitespacesAndNewlines))
        default:
            return nil
        }
    }

    public var normalizedInt: Int? {
        guard let value = normalizedInt64,
              value >= Int64(Int.min), value <= Int64(Int.max) else { return nil }
        return Int(value)
    }

    public var normalizedBool: Bool? {
        switch self {
        case .bool(let value):
            return value
        case .string(let string):
            switch string.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() {
            case "true", "1": return true
            case "false", "0": return false
            default: return nil
            }
        case .number(let number):
            if number == 1 { return true }
            if number == 0 { return false }
            return nil
        default:
            return nil
        }
    }

    public var normalizedStringArray: [String] {
        switch self {
        case .array(let values):
            return values.compactMap(\.normalizedString)
        case .string(let string):
            let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
            return trimmed.isEmpty ? [] : [trimmed]
        default:
            return []
        }
    }
}
