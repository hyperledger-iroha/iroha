import Foundation

public struct NoritoJSON: Equatable, Sendable {
    public enum EncodingError: Swift.Error, LocalizedError {
        case emptyPayload
        case invalidJSON

        public var errorDescription: String? {
            switch self {
            case .emptyPayload:
                return "JSON payload must not be empty."
            case .invalidJSON:
                return "Provided data is not valid JSON."
            }
        }
    }

    public let data: Data

    public init(data: Data) throws {
        guard !data.isEmpty else {
            throw EncodingError.emptyPayload
        }
        try NoritoJSON.validate(data: data)
        self.data = data
    }

    public init<T: Encodable>(_ value: T, encoder: JSONEncoder = NoritoJSON.makeEncoder()) throws {
        if #available(iOS 11.0, macOS 10.13, *) {
            encoder.outputFormatting.insert(.sortedKeys)
        }
        let encoded = try encoder.encode(value)
        try self.init(data: encoded)
    }

    public static func fromJSONObject(_ object: Any,
                                      options: JSONSerialization.WritingOptions = [.sortedKeys]) throws -> NoritoJSON {
        let encoded = try JSONSerialization.data(withJSONObject: object, options: options)
        return try NoritoJSON(data: encoded)
    }

    private static func validate(data: Data) throws {
        do {
            _ = try JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])
        } catch {
            throw EncodingError.invalidJSON
        }
    }

    public static func makeEncoder() -> JSONEncoder {
        let encoder = JSONEncoder()
        if #available(iOS 11.0, macOS 10.13, *) {
            encoder.outputFormatting.insert(.sortedKeys)
        }
        return encoder
    }
}
