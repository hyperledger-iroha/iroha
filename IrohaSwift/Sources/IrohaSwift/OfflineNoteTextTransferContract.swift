import Foundation

public enum OfflineNoteTextTransferContractError: Error, Equatable {
    case emptyPayload
    case invalidCharacters
    case invalidBase64URLBody
    case payloadTooLarge(maxBytes: Int)
}

public enum OfflineNoteTextTransferContract {
    public static let maxDeviceToDevicePayloadBytes = OfflineNoteNfcApduProtocol.maxIncomingPayloadBytes
    private static let boundaryWhitespace = CharacterSet(charactersIn: " \t\r\n")

    public static func trimmingBoundaryWhitespace(_ payload: String) -> String {
        payload.trimmingCharacters(in: boundaryWhitespace)
    }

    public static func hasOnlyTextTransportCharacters(_ payload: String) -> Bool {
        guard !payload.isEmpty else { return false }
        return payload.unicodeScalars.allSatisfy(isTextTransportScalarAllowed)
    }

    public static func hasBase64URLTextBody(_ payload: String, prefixes: [String]) -> Bool {
        (try? requireBase64URLTextBody(payload, prefixes: prefixes)) != nil
    }

    @discardableResult
    public static func requireBase64URLTextBody(
        _ payload: String,
        prefixes: [String]
    ) throws -> (prefix: String, body: String) {
        guard let prefix = prefixes.first(where: { payload.hasPrefix($0) }) else {
            throw OfflineNoteTextTransferContractError.invalidBase64URLBody
        }
        let body = String(payload.dropFirst(prefix.count))
        guard !body.isEmpty,
              body.unicodeScalars.allSatisfy(isBase64URLScalarAllowed) else {
            throw OfflineNoteTextTransferContractError.invalidBase64URLBody
        }
        return (prefix, body)
    }

    public static func normalizeTextTransportEnvelope(_ payload: String) throws -> String {
        let trimmed = trimmingBoundaryWhitespace(payload)
        guard !trimmed.isEmpty else {
            throw OfflineNoteTextTransferContractError.emptyPayload
        }
        guard hasOnlyTextTransportCharacters(trimmed) else {
            throw OfflineNoteTextTransferContractError.invalidCharacters
        }
        return trimmed
    }

    public static func requireDeviceToDevicePayloadByteCount(_ byteCount: Int) throws {
        guard byteCount > 0, byteCount <= maxDeviceToDevicePayloadBytes else {
            throw OfflineNoteTextTransferContractError.payloadTooLarge(maxBytes: maxDeviceToDevicePayloadBytes)
        }
    }

    public static func base64URLEncodedString(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .trimmingCharacters(in: CharacterSet(charactersIn: "="))
    }

    public static func base64URLDecodedData(_ value: String) -> Data? {
        guard !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !value.contains("="),
              value.unicodeScalars.allSatisfy(isBase64URLScalarAllowed) else {
            return nil
        }
        var normalized = value
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        let padding = (4 - normalized.count % 4) % 4
        normalized.append(String(repeating: "=", count: padding))
        return Data(base64Encoded: normalized)
    }

    private static func isBase64URLScalarAllowed(_ scalar: Unicode.Scalar) -> Bool {
        let value = scalar.value
        return (value >= 65 && value <= 90)
            || (value >= 97 && value <= 122)
            || (value >= 48 && value <= 57)
            || value == 45
            || value == 95
    }

    private static func isTextTransportScalarAllowed(_ scalar: Unicode.Scalar) -> Bool {
        scalar.value >= 33 && scalar.value <= 126
    }
}

public enum OfflineNativePaymentTextPayloadCodec {
    public static let compactMarker = "ios-compact-v1:"

    public static func encodePaymentToken(
        _ token: OfflineNotePaymentToken,
        maxRawBytes: Int? = nil
    ) throws -> String {
        let raw = try OfflineNoteTransferTextPayloadCodec.encodePaymentToken(token)
        guard let maxRawBytes else {
            return raw
        }
        if raw.utf8.count <= maxRawBytes {
            return raw
        }
        let compactToken = OfflineNotePaymentToken(
            chainId: token.chainId,
            paymentRequestId: token.paymentRequestId,
            tokenNonce: token.tokenNonce,
            tokenId: token.tokenId,
            audit: token.audit,
            bearerAuditTrail: bearerAuditTrailWithoutTerminalAudit(token),
            createdAtMs: token.createdAtMs
        )
        let compactPayload = try OfflineNotePaymentTokenCodec.encodeNorito(compactToken)
        return OfflineNoteTransferTextPayloadCodec.paymentTokenPrefix
            + compactMarker
            + OfflineNoteTextTransferContract.base64URLEncodedString(compactPayload)
    }

    public static func decodePaymentToken(_ value: String) throws -> OfflineNotePaymentToken {
        let trimmed = try OfflineNoteTextTransferContract.normalizeTextTransportEnvelope(value)
        let prefix = OfflineNoteTransferTextPayloadCodec.paymentTokenPrefix
        guard trimmed.hasPrefix(prefix) else {
            return try OfflineNotePaymentTokenCodec.decodeText(trimmed)
        }
        let body = trimmed.dropFirst(prefix.count)
        guard body.hasPrefix(compactMarker) else {
            return try OfflineNotePaymentTokenCodec.decodeText(trimmed)
        }
        let encoded = String(body.dropFirst(compactMarker.count))
        guard let payload = OfflineNoteTextTransferContract.base64URLDecodedData(encoded) else {
            throw OfflineNoteTransferTextPayloadCodecError.invalidPayload
        }
        let compactToken = try OfflineNotePaymentTokenCodec.decodeNorito(payload)
        return OfflineNotePaymentToken(
            chainId: compactToken.chainId,
            paymentRequestId: compactToken.paymentRequestId,
            tokenNonce: compactToken.tokenNonce,
            tokenId: compactToken.tokenId,
            audit: compactToken.audit,
            bearerAuditTrail: bearerAuditTrail(
                forInputAudits: compactToken.bearerAuditTrail,
                appending: compactToken.audit
            ),
            createdAtMs: compactToken.createdAtMs
        )
    }

    public static func isCompactPaymentToken(_ value: String) -> Bool {
        let trimmed = OfflineNoteTextTransferContract.trimmingBoundaryWhitespace(value)
        return trimmed.hasPrefix(OfflineNoteTransferTextPayloadCodec.paymentTokenPrefix + compactMarker)
    }

    public static func bearerAuditTrail(
        forInputAudits inputAudits: [OfflineNoteAuditBundle],
        appending terminalAudit: OfflineNoteAuditBundle
    ) -> [OfflineNoteAuditBundle] {
        let terminalTokenId = terminalAudit.tokenId.map { String(format: "%02x", $0) }.joined()
        var seen: Set<String> = []
        var result: [OfflineNoteAuditBundle] = []
        for audit in inputAudits {
            let tokenId = audit.tokenId.map { String(format: "%02x", $0) }.joined()
            guard tokenId != terminalTokenId, seen.insert(tokenId).inserted else {
                continue
            }
            result.append(audit)
        }
        result.append(terminalAudit)
        return result
    }

    private static func bearerAuditTrailWithoutTerminalAudit(
        _ token: OfflineNotePaymentToken
    ) -> [OfflineNoteAuditBundle] {
        var trail = token.bearerAuditTrail
        if trail.last == token.audit {
            trail.removeLast()
        }
        return trail
    }
}
