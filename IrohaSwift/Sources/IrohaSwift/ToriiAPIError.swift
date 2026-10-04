import Foundation

/// A Torii error response: the HTTP status plus the standard error envelope
/// `{"code": "...", "message": "...", "details": {...}}`.
///
/// Every non-success Torii response surfaces as `ToriiClientError.api(_:)`.
/// Collection-query rejections carry codes such as `invalid_filter` and
/// `details.field` naming the control at fault:
///
/// ```swift
/// do {
///     _ = try await torii.assetDefinitions.page(query)
/// } catch let ToriiClientError.api(error) where error.code == "invalid_filter" {
///     print(error.message, error.details?.hint ?? "")
/// }
/// ```
///
/// `code` and `details` come from the response body; `rejectCode` comes from
/// Torii's `X-Iroha-Reject-Code` header. Only the header has reject-code
/// provenance: a proxy can synthesize an arbitrary JSON body.
public struct ToriiAPIError: Error, Equatable, Sendable, CustomStringConvertible, LocalizedError {
    /// Structured `details` of the error envelope.
    public struct Details: Equatable, Sendable {
        /// The control or input at fault, e.g. `filter`.
        public let field: String?
        /// What would have been accepted, e.g. the fields a collection exposes.
        public let expected: String?
        /// What was received, e.g. the rejected field name.
        public let actual: String?
        /// A suggested fix.
        public let hint: String?
        /// The complete `details` object as received.
        public let raw: [String: ToriiJSONValue]

        public init(raw: [String: ToriiJSONValue]) {
            self.raw = raw
            field = Self.text(raw["field"])
            expected = Self.text(raw["expected"])
            actual = Self.text(raw["actual"])
            hint = Self.text(raw["hint"])
        }

        private static func text(_ value: ToriiJSONValue?) -> String? {
            switch value {
            case let .string(text)?:
                return text
            case let .array(values)?:
                let texts = values.compactMap { value -> String? in
                    if case let .string(text) = value { return text }
                    return nil
                }
                return texts.count == values.count ? texts.joined(separator: ", ") : nil
            default:
                return nil
            }
        }
    }

    /// HTTP status code.
    public let status: Int
    /// Stable error code from the envelope, e.g. `invalid_filter`; `nil` when
    /// the body was not a Torii error envelope.
    public let code: String?
    /// Human-readable message: the envelope `message`, or a description of
    /// the response when the body was not an envelope.
    public let message: String
    /// Structured envelope `details`, when present.
    public let details: Details?
    /// Torii's `X-Iroha-Reject-Code` response header, when present.
    public let rejectCode: String?

    public init(status: Int, code: String? = nil, message: String, details: Details? = nil, rejectCode: String? = nil) {
        self.status = status
        self.code = code
        self.message = message
        self.details = details
        self.rejectCode = rejectCode
    }

    public var description: String {
        var text = "Torii responded with HTTP status \(status)"
        if let code {
            text += " (\(code)): \(message)"
        } else {
            text += " (\(message))"
        }
        if let rejectCode {
            text += ". Reject code: \(rejectCode)"
        }
        return text + "."
    }

    public var errorDescription: String? {
        description
    }
}

extension ToriiAPIError {
    private struct Envelope: Decodable {
        let code: String
        let message: String
        let details: [String: ToriiJSONValue]?
    }

    /// Longest envelope message retained, in characters.
    static let maximumMessageLength = 4_096

    /// Parse a response into an error, preferring the standard envelope and
    /// falling back to `fallbackMessage` for other bodies.
    static func parse(status: Int,
                      body: Data?,
                      rejectCode: String?,
                      fallbackMessage: () -> String) -> ToriiAPIError {
        if let body, !body.isEmpty,
           let envelope = try? JSONDecoder().decode(Envelope.self, from: body),
           !envelope.code.isEmpty {
            let message = envelope.message.count > maximumMessageLength
                ? String(envelope.message.prefix(maximumMessageLength)) + "..."
                : envelope.message
            return ToriiAPIError(
                status: status,
                code: envelope.code,
                message: message,
                details: envelope.details.map(Details.init(raw:)),
                rejectCode: rejectCode
            )
        }
        return ToriiAPIError(status: status, message: fallbackMessage(), rejectCode: rejectCode)
    }

    /// The same error with every occurrence of `sensitiveValue` replaced.
    func redacting(_ sensitiveValue: String?) -> ToriiAPIError {
        guard let sensitiveValue, !sensitiveValue.isEmpty else {
            return self
        }
        func redact(_ text: String) -> String {
            text.replacingOccurrences(of: sensitiveValue, with: "<redacted>")
        }
        func redactJSON(_ value: ToriiJSONValue) -> ToriiJSONValue {
            switch value {
            case let .string(text): return .string(redact(text))
            case let .array(values): return .array(values.map(redactJSON))
            case let .object(members):
                return .object(members.reduce(into: [:]) { redacted, member in
                    redacted[redact(member.key)] = redactJSON(member.value)
                })
            default: return value
            }
        }
        return ToriiAPIError(
            status: status,
            code: code.map(redact),
            message: redact(message),
            details: details.map { details in
                guard case let .object(members) = redactJSON(.object(details.raw)) else {
                    return details
                }
                return Details(raw: members)
            },
            rejectCode: rejectCode.map(redact)
        )
    }
}
