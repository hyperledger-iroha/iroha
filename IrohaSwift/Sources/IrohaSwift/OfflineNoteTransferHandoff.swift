import Foundation

public enum OfflineNoteTransferModality: String, CaseIterable, Sendable {
    case qrStreaming = "qr_streaming"
    case nfc = "nfc"
    case nearby = "nearby"
}

public enum OfflineNoteNfcAvailability: Equatable, Sendable {
    case supported
    case unavailable(String)

    public var isSupported: Bool {
        if case .supported = self { return true }
        return false
    }
}

public struct OfflineNoteTransferCapabilities: Equatable, Sendable {
    public let qrStreaming: Bool
    public let nfc: OfflineNoteNfcAvailability
    public let nearby: Bool

    public init(qrStreaming: Bool = true,
                nfc: OfflineNoteNfcAvailability,
                nearby: Bool = true) {
        self.qrStreaming = qrStreaming
        self.nfc = nfc
        self.nearby = nearby
    }

    public var supportedModalities: [OfflineNoteTransferModality] {
        var modalities: [OfflineNoteTransferModality] = []
        if qrStreaming { modalities.append(.qrStreaming) }
        if nfc.isSupported { modalities.append(.nfc) }
        if nearby { modalities.append(.nearby) }
        return modalities
    }

    public static func current(iosHceAllowed: Bool = false,
                               nearbyAvailable: Bool = true) -> OfflineNoteTransferCapabilities {
        #if os(iOS)
        let nfc: OfflineNoteNfcAvailability = iosHceAllowed
            ? .supported
            : .unavailable("iOS NFC payment-token transfer requires an allowed Core NFC HCE/CardSession use case and entitlement.")
        #else
        let nfc: OfflineNoteNfcAvailability = .unavailable("NFC payment-token transfer is only exposed on mobile platforms.")
        #endif
        return OfflineNoteTransferCapabilities(qrStreaming: true, nfc: nfc, nearby: nearbyAvailable)
    }
}

public struct OfflineNoteTransferPayload: Equatable, Sendable {
    public let modality: OfflineNoteTransferModality
    public let contentType: String
    public let payload: Data

    public init(modality: OfflineNoteTransferModality, contentType: String, payload: Data) {
        self.modality = modality
        self.contentType = contentType
        self.payload = payload
    }
}

public struct OfflineNoteTransferStreamResult: Equatable, Sendable {
    public let payload: Data?
    public let token: OfflineNotePaymentToken?
    public let receiveRequest: OfflineNoteReceiveRequest?
    public let receiptAck: OfflineNoteReceiptAck?
    public let receivedChunks: Int
    public let totalChunks: Int
    public let recoveredChunks: Int

    public var isComplete: Bool {
        token != nil || receiveRequest != nil || receiptAck != nil
    }

    public var progress: Double {
        guard totalChunks > 0 else { return 0 }
        return Double(receivedChunks) / Double(totalChunks)
    }
}

public enum OfflineNoteTextPayloadKind: Equatable, Sendable {
    case receiveRequest
    case paymentToken
    case receiptAck

    public var qrPayloadKind: OfflineQrPayloadKind {
        switch self {
        case .receiveRequest:
            return .offlineReceiveRequest
        case .paymentToken:
            return .offlinePaymentToken
        case .receiptAck:
            return .offlineReceiptAck
        }
    }

    public var nfcPayloadKind: OfflineNoteNfcPayloadKind {
        switch self {
        case .receiveRequest:
            return .receiveRequest
        case .paymentToken:
            return .paymentToken
        case .receiptAck:
            return .receiptAck
        }
    }

    public var nearbyMessageKind: OfflineNoteNearbyMessageKind {
        switch self {
        case .receiveRequest:
            return .receiveRequest
        case .paymentToken:
            return .payment
        case .receiptAck:
            return .receiptAck
        }
    }

    public var contentType: String {
        OfflineNoteTransferHandoff.textContentType(for: self)
    }
}

public enum OfflineNoteTransferTextPayloadCodecError: Error, LocalizedError, Equatable {
    case unknownPrefix
    case invalidPayload
    case kindMismatch(expected: OfflineNoteTextPayloadKind, actual: OfflineNoteTextPayloadKind)

    public var errorDescription: String? {
        switch self {
        case .unknownPrefix:
            return "Offline Note transfer text payload prefix is not recognized."
        case .invalidPayload:
            return "Offline Note transfer text payload is invalid."
        case let .kindMismatch(expected, actual):
            return "Offline Note transfer text payload kind mismatch: expected \(expected), got \(actual)."
        }
    }
}

public enum OfflineNoteTransferTextPayloadCodec {
    public static let receiveRequestPrefix = OfflineBearerV2TextCodec.receiveRequestTextPrefix
    public static let paymentTokenPrefix = OfflineBearerV2TextCodec.paymentTextPrefix
    public static let receiptAckPrefix = OfflineBearerV2TextCodec.ackTextPrefix

    public static func prefix(for kind: OfflineNoteTextPayloadKind) -> String {
        switch kind {
        case .receiveRequest:
            return receiveRequestPrefix
        case .paymentToken:
            return paymentTokenPrefix
        case .receiptAck:
            return receiptAckPrefix
        }
    }

    public static func encode(_ payload: Data, kind: OfflineNoteTextPayloadKind) throws -> String {
        guard !payload.isEmpty else {
            throw OfflineNoteTransferTextPayloadCodecError.invalidPayload
        }
        return prefix(for: kind) + base64UrlEncode(payload)
    }

    public static func encodeReceiveRequest(_ payload: OfflineNoteReceiveRequest) throws -> String {
        try OfflineNoteReceiveRequestCodec.encodeText(payload)
    }

    public static func encodeReceiveRequest(_ payload: OfflineReceiveRequestPayload) throws -> String {
        try OfflineNoteCompatibilityTextEncoding.encodeJsonText(payload, prefix: receiveRequestPrefix)
    }

    public static func decodeReceiveRequest(_ value: String) throws -> OfflineNoteReceiveRequest {
        try OfflineNoteReceiveRequestCodec.decodeText(value)
    }

    public static func decodeReceiveRequestPayload(_ value: String) throws -> OfflineReceiveRequestPayload {
        if let request = try? OfflineNoteReceiveRequestCodec.decodeText(value) {
            return OfflineReceiveRequestPayload(request: request)
        }
        return try OfflineNoteCompatibilityTextEncoding.decodeJsonText(
            OfflineReceiveRequestPayload.self,
            from: value,
            prefix: receiveRequestPrefix
        )
    }

    public static func encodePaymentToken(_ payload: OfflineNotePaymentToken) throws -> String {
        try OfflineNotePaymentTokenCodec.encodeText(payload)
    }

    public static func encodePaymentToken(_ payload: OfflinePaymentToken) throws -> String {
        try OfflineNoteCompatibilityTextEncoding.encodeJsonText(payload, prefix: paymentTokenPrefix)
    }

    public static func decodeNativePaymentToken(_ value: String) throws -> OfflineNotePaymentToken {
        try OfflineNotePaymentTokenCodec.decodeText(value)
    }

    public static func decodePaymentToken(_ value: String) throws -> OfflinePaymentToken {
        try OfflineNoteCompatibilityTextEncoding.decodeJsonText(
            OfflinePaymentToken.self,
            from: value,
            prefix: paymentTokenPrefix
        )
    }

    public static func encodeReceiptAck(_ payload: OfflineNoteReceiptAck) throws -> String {
        try OfflineNoteReceiptAckCodec.encodeText(payload)
    }

    public static func encodeReceiptAck(_ payload: OfflineReceiptAck) throws -> String {
        try OfflineNoteCompatibilityTextEncoding.encodeJsonText(payload, prefix: receiptAckPrefix)
    }

    public static func decodeNativeReceiptAck(_ value: String) throws -> OfflineNoteReceiptAck {
        try OfflineNoteReceiptAckCodec.decodeText(value)
    }

    public static func decodeReceiptAck(_ value: String) throws -> OfflineReceiptAck {
        if let ack = try? OfflineNoteReceiptAckCodec.decodeText(value) {
            let receiptAck = OfflineReceiptAck(ack: ack)
            try validateReceiptAckFields(receiptAck)
            return receiptAck
        }
        let receiptAck = try OfflineNoteCompatibilityTextEncoding.decodeJsonText(
            OfflineReceiptAck.self,
            from: value,
            prefix: receiptAckPrefix
        )
        try validateReceiptAckFields(receiptAck)
        return receiptAck
    }

    public static func decode(
        _ value: String,
        expectedKind: OfflineNoteTextPayloadKind? = nil
    ) throws -> (kind: OfflineNoteTextPayloadKind, payload: Data) {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        let kind: OfflineNoteTextPayloadKind
        let encoded: Substring
        if trimmed.hasPrefix(receiveRequestPrefix) {
            kind = .receiveRequest
            encoded = trimmed.dropFirst(receiveRequestPrefix.count)
        } else if trimmed.hasPrefix(paymentTokenPrefix) {
            kind = .paymentToken
            encoded = trimmed.dropFirst(paymentTokenPrefix.count)
        } else if trimmed.hasPrefix(receiptAckPrefix) {
            kind = .receiptAck
            encoded = trimmed.dropFirst(receiptAckPrefix.count)
        } else {
            throw OfflineNoteTransferTextPayloadCodecError.unknownPrefix
        }
        if let expectedKind, expectedKind != kind {
            throw OfflineNoteTransferTextPayloadCodecError.kindMismatch(expected: expectedKind, actual: kind)
        }
        guard let payload = base64UrlDecode(String(encoded)), !payload.isEmpty else {
            throw OfflineNoteTransferTextPayloadCodecError.invalidPayload
        }
        return (kind, payload)
    }

    static func validatePayloadContents(
        _ value: String,
        expectedKind: OfflineNoteTextPayloadKind? = nil
    ) throws {
        let decoded = try decode(value, expectedKind: expectedKind)
        switch decoded.kind {
        case .receiveRequest:
            try validateReceiveRequest(value)
        case .paymentToken:
            try validatePaymentToken(value)
        case .receiptAck:
            try validateReceiptAck(value)
        }
    }

    public static func payloadKind(for value: String) -> OfflineQrPayloadKind {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.hasPrefix(receiveRequestPrefix) {
            return OfflineNoteTextPayloadKind.receiveRequest.qrPayloadKind
        }
        if trimmed.hasPrefix(paymentTokenPrefix) {
            return OfflineNoteTextPayloadKind.paymentToken.qrPayloadKind
        }
        if trimmed.hasPrefix(receiptAckPrefix) {
            return OfflineNoteTextPayloadKind.receiptAck.qrPayloadKind
        }
        return .unspecified
    }

    private static func validateReceiveRequest(_ value: String) throws {
        _ = try OfflineBearerV2TextCodec.decodeReceiveRequestText(value)
    }

    private static func validatePaymentToken(_ value: String) throws {
        _ = try OfflineBearerV2TextCodec.decodePaymentText(value)
    }

    private static func validateReceiptAck(_ value: String) throws {
        _ = try OfflineBearerV2TextCodec.decodeAckText(value)
    }

    private static func validateReceiptAckFields(_ ack: OfflineReceiptAck) throws {
        guard !ack.tokenId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !ack.recipientAccountId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              ack.acceptedAtMs > 0,
              ack.chainId.map({ !$0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }) ?? true,
              ack.paymentRequestId.map({ !$0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }) ?? true else {
            throw OfflineNoteTransferTextPayloadCodecError.invalidPayload
        }
    }

    private static func base64UrlEncode(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .trimmingCharacters(in: CharacterSet(charactersIn: "="))
    }

    private static func base64UrlDecode(_ value: String) -> Data? {
        guard !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !value.contains("="),
              value.unicodeScalars.allSatisfy({ scalar in
                  let byte = scalar.value
                  return (65...90).contains(byte)
                      || (97...122).contains(byte)
                      || (48...57).contains(byte)
                      || byte == 45
                      || byte == 95
              }) else {
            return nil
        }
        var normalized = value
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        let padding = (4 - normalized.count % 4) % 4
        normalized.append(String(repeating: "=", count: padding))
        return Data(base64Encoded: normalized)
    }
}

public final class OfflineNoteTransferStreamReceiver {
    private let decoder = OfflineQrStreamDecoder()

    public init() {}

    public func ingestFrame(_ frameBytes: Data) throws -> OfflineNoteTransferStreamResult {
        let result = try decoder.ingest(frameBytes: frameBytes)
        let token: OfflineNotePaymentToken?
        let receiveRequest: OfflineNoteReceiveRequest?
        let receiptAck: OfflineNoteReceiptAck?
        if let payload = result.payload {
            switch result.payloadKind {
            case .offlinePaymentToken:
                token = try OfflineNotePaymentTokenCodec.decodeQrPayload(payload)
                receiveRequest = nil
                receiptAck = nil
            case .offlineReceiveRequest:
                token = nil
                receiveRequest = try OfflineNoteReceiveRequestCodec.decodeQrPayload(payload)
                receiptAck = nil
            case .offlineReceiptAck:
                token = nil
                receiveRequest = nil
                receiptAck = try OfflineNoteReceiptAckCodec.decodeQrPayload(payload)
            default:
                throw OfflineQrStreamError.invalidEnvelope("payload_kind mismatch")
            }
        } else {
            token = nil
            receiveRequest = nil
            receiptAck = nil
        }
        return OfflineNoteTransferStreamResult(
            payload: result.payload,
            token: token,
            receiveRequest: receiveRequest,
            receiptAck: receiptAck,
            receivedChunks: result.receivedChunks,
            totalChunks: result.totalChunks,
            recoveredChunks: result.recoveredChunks
        )
    }
}

public enum OfflineNoteTransferHandoff {
    public static let paymentTokenContentType = "application/vnd.iroha.offline.payment-token+norito"
    public static let receiveRequestContentType = "application/vnd.iroha.offline.receive-request+norito"
    public static let receiptAckContentType = "application/vnd.iroha.offline.receipt-ack+norito"
    public static let textPaymentTokenContentType = "text/vnd.iroha.offline.payment-token"
    public static let textReceiveRequestContentType = "text/vnd.iroha.offline.receive-request"
    public static let textReceiptAckContentType = "text/vnd.iroha.offline.receipt-ack"
    public static let nearbyServiceName = "iroha-pay"
    public static let nfcExternalType = "org.hyperledger.iroha:offline-payment"
    public static let defaultNfcAidHex = OfflineNoteNfcApduProtocol.aidHex
    public static let qrFrameCadenceMs = 500

    public static let qrStreamingOptions = OfflineQrStreamOptions(chunkSize: 180, parityGroup: 2)
    public static let nfcStreamingOptions = OfflineQrStreamOptions(
        chunkSize: OfflineNoteNfcApduProtocol.androidSafeChunkBytes - 20,
        parityGroup: 0
    )
    public static let nearbyStreamingOptions = OfflineQrStreamOptions(chunkSize: 4096, parityGroup: 0)

    public static func rawPaymentTokenBytes(for token: OfflineNotePaymentToken) throws -> Data {
        try OfflineNotePaymentTokenCodec.encodeNorito(token)
    }

    public static func paymentTokenPayload(
        for token: OfflineNotePaymentToken,
        modality: OfflineNoteTransferModality
    ) throws -> OfflineNoteTransferPayload {
        OfflineNoteTransferPayload(
            modality: modality,
            contentType: paymentTokenContentType,
            payload: try rawPaymentTokenBytes(for: token)
        )
    }

    public static func decodePaymentToken(from payload: OfflineNoteTransferPayload) throws -> OfflineNotePaymentToken {
        guard payload.contentType == paymentTokenContentType else {
            throw OfflineNoteNearbyError.invalidMessage
        }
        return try OfflineNotePaymentTokenCodec.decodeNorito(payload.payload)
    }

    public static func decodePaymentToken(fromRawPayload payload: Data) throws -> OfflineNotePaymentToken {
        try OfflineNotePaymentTokenCodec.decodeNorito(payload)
    }

    public static func rawReceiveRequestBytes(for request: OfflineNoteReceiveRequest) throws -> Data {
        try OfflineNoteReceiveRequestCodec.encodeNorito(request)
    }

    public static func rawReceiptAckBytes(for ack: OfflineNoteReceiptAck) throws -> Data {
        try OfflineNoteReceiptAckCodec.encodeNorito(ack)
    }

    public static func receiveRequestPayload(
        for request: OfflineNoteReceiveRequest,
        modality: OfflineNoteTransferModality
    ) throws -> OfflineNoteTransferPayload {
        OfflineNoteTransferPayload(
            modality: modality,
            contentType: receiveRequestContentType,
            payload: try rawReceiveRequestBytes(for: request)
        )
    }

    public static func decodeReceiveRequest(from payload: OfflineNoteTransferPayload) throws -> OfflineNoteReceiveRequest {
        guard payload.contentType == receiveRequestContentType else {
            throw OfflineNoteNearbyError.invalidMessage
        }
        return try OfflineNoteReceiveRequestCodec.decodeNorito(payload.payload)
    }

    public static func decodeReceiveRequest(fromRawPayload payload: Data) throws -> OfflineNoteReceiveRequest {
        try OfflineNoteReceiveRequestCodec.decodeNorito(payload)
    }

    public static func receiptAckPayload(
        for ack: OfflineNoteReceiptAck,
        modality: OfflineNoteTransferModality
    ) throws -> OfflineNoteTransferPayload {
        OfflineNoteTransferPayload(
            modality: modality,
            contentType: receiptAckContentType,
            payload: try rawReceiptAckBytes(for: ack)
        )
    }

    public static func decodeReceiptAck(from payload: OfflineNoteTransferPayload) throws -> OfflineNoteReceiptAck {
        guard payload.contentType == receiptAckContentType else {
            throw OfflineNoteNearbyError.invalidMessage
        }
        return try OfflineNoteReceiptAckCodec.decodeNorito(payload.payload)
    }

    public static func decodeReceiptAck(fromRawPayload payload: Data) throws -> OfflineNoteReceiptAck {
        try OfflineNoteReceiptAckCodec.decodeNorito(payload)
    }

    public static func qrStreamingFrameBytes(
        for token: OfflineNotePaymentToken,
        options: OfflineQrStreamOptions = qrStreamingOptions
    ) throws -> [Data] {
        try OfflineNotePaymentTokenCodec.encodeQrFrameBytes(token, options: options)
    }

    public static func qrStreamingFrameBytes(
        for request: OfflineNoteReceiveRequest,
        options: OfflineQrStreamOptions = qrStreamingOptions
    ) throws -> [Data] {
        try OfflineNoteReceiveRequestCodec.encodeQrFrameBytes(request, options: options)
    }

    public static func qrStreamingFrameBytes(
        for ack: OfflineNoteReceiptAck,
        options: OfflineQrStreamOptions = qrStreamingOptions
    ) throws -> [Data] {
        try OfflineNoteReceiptAckCodec.encodeQrFrameBytes(ack, options: options)
    }

    public static func nfcFrameBytes(
        for token: OfflineNotePaymentToken,
        options: OfflineQrStreamOptions = nfcStreamingOptions
    ) throws -> [Data] {
        try streamFrameBytes(for: token, options: options)
    }

    public static func nfcPaymentTokenWriteAPDUs(
        for token: OfflineNotePaymentToken,
        maxChunkLength: Int = OfflineNoteNfcApduProtocol.androidSafeChunkBytes
    ) throws -> [Data] {
        try OfflineNoteNfcApduProtocol.writePayloadAPDUs(
            kind: .paymentToken,
            payloadBytes: rawPaymentTokenBytes(for: token),
            maxChunkLength: maxChunkLength
        )
    }

    public static func nfcReceiptAckWriteAPDUs(
        for ack: OfflineNoteReceiptAck,
        maxChunkLength: Int = OfflineNoteNfcApduProtocol.androidSafeChunkBytes
    ) throws -> [Data] {
        try OfflineNoteNfcApduProtocol.writePayloadAPDUs(
            kind: .receiptAck,
            payloadBytes: rawReceiptAckBytes(for: ack),
            maxChunkLength: maxChunkLength
        )
    }

    public static func nearbyPayload(for token: OfflineNotePaymentToken) throws -> OfflineNoteTransferPayload {
        try paymentTokenPayload(for: token, modality: .nearby)
    }

    public static func nearbyPaymentEnvelopeBytes(for token: OfflineNotePaymentToken) throws -> Data {
        try OfflineNoteNearbyEnvelope(
            kind: .payment,
            payload: rawPaymentTokenBytes(for: token),
            contentType: paymentTokenContentType
        ).encoded()
    }

    public static func decodeNearbyPaymentToken(from envelopeBytes: Data) throws -> OfflineNotePaymentToken {
        try OfflineNoteNearbyEnvelope.decode(envelopeBytes).paymentToken()
    }

    public static func nearbyPayload(for ack: OfflineNoteReceiptAck) throws -> OfflineNoteTransferPayload {
        try receiptAckPayload(for: ack, modality: .nearby)
    }

    public static func nearbyReceiptAckEnvelopeBytes(for ack: OfflineNoteReceiptAck) throws -> Data {
        try OfflineNoteNearbyEnvelope(
            kind: .receiptAck,
            payload: rawReceiptAckBytes(for: ack),
            contentType: receiptAckContentType
        ).encoded()
    }

    public static func decodeNearbyReceiptAck(from envelopeBytes: Data) throws -> OfflineNoteReceiptAck {
        try OfflineNoteNearbyEnvelope.decode(envelopeBytes).receiptAck()
    }

    public static func textContentType(for kind: OfflineNoteTextPayloadKind) -> String {
        switch kind {
        case .receiveRequest:
            return textReceiveRequestContentType
        case .paymentToken:
            return textPaymentTokenContentType
        case .receiptAck:
            return textReceiptAckContentType
        }
    }

    public static func textPayloadKind(for contentType: String) -> OfflineNoteTextPayloadKind? {
        switch contentType {
        case textReceiveRequestContentType:
            return .receiveRequest
        case textPaymentTokenContentType:
            return .paymentToken
        case textReceiptAckContentType:
            return .receiptAck
        default:
            return nil
        }
    }

    public static func textPayloadKind(for nfcPayloadKind: OfflineNoteNfcPayloadKind) -> OfflineNoteTextPayloadKind {
        switch nfcPayloadKind {
        case .receiveRequest:
            return .receiveRequest
        case .paymentToken:
            return .paymentToken
        case .receiptAck:
            return .receiptAck
        }
    }

    public static func normalizeTextTransportPayload(
        _ value: String,
        expectedKind: OfflineNoteTextPayloadKind? = nil
    ) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        try OfflineNoteTransferTextPayloadCodec.validatePayloadContents(trimmed, expectedKind: expectedKind)
        return trimmed
    }

    public static func isValidTextTransportPayload(
        _ value: String,
        expectedKind: OfflineNoteTextPayloadKind? = nil
    ) -> Bool {
        (try? normalizeTextTransportPayload(value, expectedKind: expectedKind)) != nil
    }

    public static func nearbyTextEnvelopeBytes(
        payload: String,
        kind: OfflineNoteTextPayloadKind,
        pairingChallenge: OfflineNoteNearbyPairingChallenge? = nil
    ) throws -> Data {
        let normalized = try normalizeTextTransportPayload(payload, expectedKind: kind)
        return try OfflineNoteNearbyEnvelope(
            kind: kind.nearbyMessageKind,
            payload: Data(normalized.utf8),
            contentType: textContentType(for: kind),
            pairingChallenge: pairingChallenge
        ).encoded()
    }

    public static func decodeNearbyTextPayload(
        from envelopeBytes: Data,
        expectedKind: OfflineNoteTextPayloadKind? = nil
    ) throws -> (kind: OfflineNoteTextPayloadKind, payload: String, pairingChallenge: OfflineNoteNearbyPairingChallenge?) {
        let envelope = try OfflineNoteNearbyEnvelope.decode(envelopeBytes)
        guard let textKind = textPayloadKind(for: envelope.contentType) else {
            throw OfflineNoteNearbyError.invalidMessage
        }
        if let expectedKind, expectedKind != textKind {
            throw OfflineNoteNearbyError.invalidMessage
        }
        switch (envelope.kind, textKind) {
        case (.receiveRequest, .receiveRequest), (.payment, .paymentToken), (.receiptAck, .receiptAck):
            break
        default:
            throw OfflineNoteNearbyError.invalidMessage
        }
        guard let payload = String(data: envelope.payload, encoding: .utf8) else {
            throw OfflineNoteNearbyError.invalidMessage
        }
        return (
            textKind,
            try normalizeTextTransportPayload(payload, expectedKind: textKind),
            envelope.pairingChallenge
        )
    }

    public static func nearbyFrameBytes(
        for token: OfflineNotePaymentToken,
        options: OfflineQrStreamOptions = nearbyStreamingOptions
    ) throws -> [Data] {
        try streamFrameBytes(for: token, options: options)
    }

    private static func streamFrameBytes(
        for token: OfflineNotePaymentToken,
        options: OfflineQrStreamOptions
    ) throws -> [Data] {
        try OfflineQrStreamEncoder.encodeFrameBytes(
            payload: rawPaymentTokenBytes(for: token),
            payloadKind: .offlinePaymentToken,
            options: options
        )
    }
}
