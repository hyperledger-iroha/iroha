import Foundation

public enum OfflineNoteV2TransferModality: String, CaseIterable, Sendable {
    case qrStreaming = "qr_streaming"
    case nfc = "nfc"
    case nearby = "nearby"
}

public enum OfflineNoteV2NfcAvailability: Equatable, Sendable {
    case supported
    case unavailable(String)

    public var isSupported: Bool {
        if case .supported = self { return true }
        return false
    }
}

public struct OfflineNoteV2TransferCapabilities: Equatable, Sendable {
    public let qrStreaming: Bool
    public let nfc: OfflineNoteV2NfcAvailability
    public let nearby: Bool

    public init(qrStreaming: Bool = true,
                nfc: OfflineNoteV2NfcAvailability,
                nearby: Bool = true) {
        self.qrStreaming = qrStreaming
        self.nfc = nfc
        self.nearby = nearby
    }

    public var supportedModalities: [OfflineNoteV2TransferModality] {
        var modalities: [OfflineNoteV2TransferModality] = []
        if qrStreaming { modalities.append(.qrStreaming) }
        if nfc.isSupported { modalities.append(.nfc) }
        if nearby { modalities.append(.nearby) }
        return modalities
    }

    public static func current(iosHceAllowed: Bool = false,
                               nearbyAvailable: Bool = true) -> OfflineNoteV2TransferCapabilities {
        #if os(iOS)
        let nfc: OfflineNoteV2NfcAvailability = iosHceAllowed
            ? .supported
            : .unavailable("iOS NFC payment-token transfer requires an allowed Core NFC HCE/CardSession use case and entitlement.")
        #else
        let nfc: OfflineNoteV2NfcAvailability = .unavailable("NFC payment-token transfer is only exposed on mobile platforms.")
        #endif
        return OfflineNoteV2TransferCapabilities(qrStreaming: true, nfc: nfc, nearby: nearbyAvailable)
    }
}

public struct OfflineNoteV2TransferPayload: Equatable, Sendable {
    public let modality: OfflineNoteV2TransferModality
    public let contentType: String
    public let payload: Data

    public init(modality: OfflineNoteV2TransferModality, contentType: String, payload: Data) {
        self.modality = modality
        self.contentType = contentType
        self.payload = payload
    }
}

public struct OfflineNoteV2TransferStreamResult: Equatable, Sendable {
    public let payload: Data?
    public let token: OfflineNoteV2PaymentToken?
    public let receiveRequest: OfflineNoteV2ReceiveRequest?
    public let receiptAck: OfflineNoteV2ReceiptAck?
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

public enum OfflineNoteV2TextPayloadKind: Equatable, Sendable {
    case receiveChallenge
    case paymentToken
    case receiptAck

    public static let receiveRequest: OfflineNoteV2TextPayloadKind = .receiveChallenge

    public var qrPayloadKind: OfflineQrPayloadKind {
        switch self {
        case .receiveChallenge:
            return .offlineReceiveRequestV2
        case .paymentToken:
            return .offlinePaymentTokenV2
        case .receiptAck:
            return .offlineReceiptAckV2
        }
    }

    public var nfcPayloadKind: OfflineNoteV2NfcPayloadKind {
        switch self {
        case .receiveChallenge:
            return .receiveChallenge
        case .paymentToken:
            return .paymentToken
        case .receiptAck:
            return .receiptAck
        }
    }

    public var nearbyMessageKind: OfflineNoteV2NearbyMessageKind {
        switch self {
        case .receiveChallenge:
            return .challenge
        case .paymentToken:
            return .payment
        case .receiptAck:
            return .receiptAck
        }
    }

    public var contentType: String {
        OfflineNoteV2TransferHandoff.textContentType(for: self)
    }
}

public enum OfflineNoteV2TransferTextPayloadCodecError: Error, LocalizedError, Equatable {
    case unknownPrefix
    case invalidPayload
    case kindMismatch(expected: OfflineNoteV2TextPayloadKind, actual: OfflineNoteV2TextPayloadKind)

    public var errorDescription: String? {
        switch self {
        case .unknownPrefix:
            return "Offline Note V2 transfer text payload prefix is not recognized."
        case .invalidPayload:
            return "Offline Note V2 transfer text payload is invalid."
        case let .kindMismatch(expected, actual):
            return "Offline Note V2 transfer text payload kind mismatch: expected \(expected), got \(actual)."
        }
    }
}

public enum OfflineNoteV2TransferTextPayloadCodec {
    public static let receiveChallengePrefix = "wallet-offline-challenge-v2:"
    public static let receiveRequestPrefix = OfflineNoteV2ReceiveRequestCodec.textPrefix
    public static let paymentTokenPrefix = "wallet-offline-payment-v2:"
    public static let receiptAckPrefix = "wallet-offline-ack-v2:"

    public static func prefix(for kind: OfflineNoteV2TextPayloadKind) -> String {
        switch kind {
        case .receiveChallenge:
            return receiveRequestPrefix
        case .paymentToken:
            return paymentTokenPrefix
        case .receiptAck:
            return receiptAckPrefix
        }
    }

    public static func encode(_ payload: Data, kind: OfflineNoteV2TextPayloadKind) throws -> String {
        guard !payload.isEmpty else {
            throw OfflineNoteV2TransferTextPayloadCodecError.invalidPayload
        }
        return prefix(for: kind) + base64UrlEncode(payload)
    }

    public static func encodeReceiveRequest(_ payload: OfflineNoteV2ReceiveRequest) throws -> String {
        try OfflineNoteV2ReceiveRequestCodec.encodeText(payload)
    }

    public static func encodeReceiveChallenge(_ payload: OfflineReceiveChallengeV2) throws -> String {
        try OfflineNoteV2CompatibilityTextEncoding.encodeJsonText(payload, prefix: receiveChallengePrefix)
    }

    public static func decodeReceiveRequest(_ value: String) throws -> OfflineNoteV2ReceiveRequest {
        try OfflineNoteV2ReceiveRequestCodec.decodeText(value)
    }

    public static func decodeReceiveChallenge(_ value: String) throws -> OfflineReceiveChallengeV2 {
        if let request = try? OfflineNoteV2ReceiveRequestCodec.decodeText(value) {
            return OfflineReceiveChallengeV2(request: request)
        }
        if let challenge = try? OfflineNoteV2CompatibilityTextEncoding.decodeJsonText(
            OfflineReceiveChallengeV2.self,
            from: value,
            prefix: receiveChallengePrefix
        ) {
            return challenge
        }
        return try OfflineNoteV2CompatibilityTextEncoding.decodeJsonText(
            OfflineReceiveChallengeV2.self,
            from: value,
            prefix: receiveRequestPrefix
        )
    }

    public static func encodePaymentToken(_ payload: OfflineNoteV2PaymentToken) throws -> String {
        try OfflineNoteV2PaymentTokenCodec.encodeText(payload)
    }

    public static func encodePaymentToken(_ payload: OfflinePaymentTokenV2) throws -> String {
        try OfflineNoteV2CompatibilityTextEncoding.encodeJsonText(payload, prefix: paymentTokenPrefix)
    }

    public static func decodeNativePaymentToken(_ value: String) throws -> OfflineNoteV2PaymentToken {
        try OfflineNoteV2PaymentTokenCodec.decodeText(value)
    }

    public static func decodePaymentToken(_ value: String) throws -> OfflinePaymentTokenV2 {
        try OfflineNoteV2CompatibilityTextEncoding.decodeJsonText(
            OfflinePaymentTokenV2.self,
            from: value,
            prefix: paymentTokenPrefix
        )
    }

    public static func encodeReceiptAck(_ payload: OfflineNoteV2ReceiptAck) throws -> String {
        try OfflineNoteV2ReceiptAckCodec.encodeText(payload)
    }

    public static func encodeReceiptAck(_ payload: OfflineReceiptAckV2) throws -> String {
        try OfflineNoteV2CompatibilityTextEncoding.encodeJsonText(payload, prefix: receiptAckPrefix)
    }

    public static func decodeNativeReceiptAck(_ value: String) throws -> OfflineNoteV2ReceiptAck {
        try OfflineNoteV2ReceiptAckCodec.decodeText(value)
    }

    public static func decodeReceiptAck(_ value: String) throws -> OfflineReceiptAckV2 {
        if let ack = try? OfflineNoteV2ReceiptAckCodec.decodeText(value) {
            return OfflineReceiptAckV2(ack: ack)
        }
        return try OfflineNoteV2CompatibilityTextEncoding.decodeJsonText(
            OfflineReceiptAckV2.self,
            from: value,
            prefix: receiptAckPrefix
        )
    }

    public static func decode(
        _ value: String,
        expectedKind: OfflineNoteV2TextPayloadKind? = nil
    ) throws -> (kind: OfflineNoteV2TextPayloadKind, payload: Data) {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        let kind: OfflineNoteV2TextPayloadKind
        let encoded: Substring
        if trimmed.hasPrefix(receiveRequestPrefix) {
            kind = .receiveChallenge
            encoded = trimmed.dropFirst(receiveRequestPrefix.count)
        } else if trimmed.hasPrefix(receiveChallengePrefix) {
            kind = .receiveChallenge
            encoded = trimmed.dropFirst(receiveChallengePrefix.count)
        } else if trimmed.hasPrefix(paymentTokenPrefix) {
            kind = .paymentToken
            encoded = trimmed.dropFirst(paymentTokenPrefix.count)
        } else if trimmed.hasPrefix(receiptAckPrefix) {
            kind = .receiptAck
            encoded = trimmed.dropFirst(receiptAckPrefix.count)
        } else {
            throw OfflineNoteV2TransferTextPayloadCodecError.unknownPrefix
        }
        if let expectedKind, expectedKind != kind {
            throw OfflineNoteV2TransferTextPayloadCodecError.kindMismatch(expected: expectedKind, actual: kind)
        }
        guard let payload = base64UrlDecode(String(encoded)), !payload.isEmpty else {
            throw OfflineNoteV2TransferTextPayloadCodecError.invalidPayload
        }
        return (kind, payload)
    }

    public static func payloadKind(for value: String) -> OfflineQrPayloadKind {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.hasPrefix(receiveRequestPrefix) || trimmed.hasPrefix(receiveChallengePrefix) {
            return OfflineNoteV2TextPayloadKind.receiveChallenge.qrPayloadKind
        }
        if trimmed.hasPrefix(paymentTokenPrefix) {
            return OfflineNoteV2TextPayloadKind.paymentToken.qrPayloadKind
        }
        if trimmed.hasPrefix(receiptAckPrefix) {
            return OfflineNoteV2TextPayloadKind.receiptAck.qrPayloadKind
        }
        return .unspecified
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

public final class OfflineNoteV2TransferStreamReceiver {
    private let decoder = OfflineQrStreamDecoder()

    public init() {}

    public func ingestFrame(_ frameBytes: Data) throws -> OfflineNoteV2TransferStreamResult {
        let result = try decoder.ingest(frameBytes: frameBytes)
        let token: OfflineNoteV2PaymentToken?
        let receiveRequest: OfflineNoteV2ReceiveRequest?
        let receiptAck: OfflineNoteV2ReceiptAck?
        if let payload = result.payload {
            switch result.payloadKind {
            case .offlinePaymentTokenV2:
                token = try OfflineNoteV2PaymentTokenCodec.decodeQrPayload(payload)
                receiveRequest = nil
                receiptAck = nil
            case .offlineReceiveRequestV2:
                token = nil
                receiveRequest = try OfflineNoteV2ReceiveRequestCodec.decodeQrPayload(payload)
                receiptAck = nil
            case .offlineReceiptAckV2:
                token = nil
                receiveRequest = nil
                receiptAck = try OfflineNoteV2ReceiptAckCodec.decodeQrPayload(payload)
            default:
                throw OfflineQrStreamError.invalidEnvelope("payload_kind mismatch")
            }
        } else {
            token = nil
            receiveRequest = nil
            receiptAck = nil
        }
        return OfflineNoteV2TransferStreamResult(
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

public enum OfflineNoteV2TransferHandoff {
    public static let paymentTokenContentType = "application/vnd.iroha.offline.payment-token-v2+norito"
    public static let receiveRequestContentType = "application/vnd.iroha.offline.receive-request-v2+norito"
    public static let receiptAckContentType = "application/vnd.iroha.offline.receipt-ack-v2+norito"
    public static let textPaymentTokenContentType = "text/vnd.iroha.offline.payment-token-v2"
    public static let textReceiveRequestContentType = "text/vnd.iroha.offline.receive-request-v2"
    public static let textReceiptAckContentType = "text/vnd.iroha.offline.receipt-ack-v2"
    public static let nearbyServiceName = "iroha-pay-v2"
    public static let nfcExternalType = "org.hyperledger.iroha:offline-payment-v2"
    public static let defaultNfcAidHex = OfflineNoteV2NfcApduProtocol.aidHex
    public static let qrFrameCadenceMs = 500

    public static let qrStreamingOptions = OfflineQrStreamOptions(chunkSize: 180, parityGroup: 2)
    public static let nfcStreamingOptions = OfflineQrStreamOptions(
        chunkSize: OfflineNoteV2NfcApduProtocol.androidSafeChunkBytes - 20,
        parityGroup: 0
    )
    public static let nearbyStreamingOptions = OfflineQrStreamOptions(chunkSize: 4096, parityGroup: 0)

    public static func rawPaymentTokenBytes(for token: OfflineNoteV2PaymentToken) throws -> Data {
        try OfflineNoteV2PaymentTokenCodec.encodeNorito(token)
    }

    public static func paymentTokenPayload(
        for token: OfflineNoteV2PaymentToken,
        modality: OfflineNoteV2TransferModality
    ) throws -> OfflineNoteV2TransferPayload {
        OfflineNoteV2TransferPayload(
            modality: modality,
            contentType: paymentTokenContentType,
            payload: try rawPaymentTokenBytes(for: token)
        )
    }

    public static func decodePaymentToken(from payload: OfflineNoteV2TransferPayload) throws -> OfflineNoteV2PaymentToken {
        guard payload.contentType == paymentTokenContentType else {
            throw OfflineNoteV2NearbyError.invalidMessage
        }
        return try OfflineNoteV2PaymentTokenCodec.decodeNorito(payload.payload)
    }

    public static func decodePaymentToken(fromRawPayload payload: Data) throws -> OfflineNoteV2PaymentToken {
        try OfflineNoteV2PaymentTokenCodec.decodeNorito(payload)
    }

    public static func rawReceiveRequestBytes(for request: OfflineNoteV2ReceiveRequest) throws -> Data {
        try OfflineNoteV2ReceiveRequestCodec.encodeNorito(request)
    }

    public static func rawReceiptAckBytes(for ack: OfflineNoteV2ReceiptAck) throws -> Data {
        try OfflineNoteV2ReceiptAckCodec.encodeNorito(ack)
    }

    public static func receiveRequestPayload(
        for request: OfflineNoteV2ReceiveRequest,
        modality: OfflineNoteV2TransferModality
    ) throws -> OfflineNoteV2TransferPayload {
        OfflineNoteV2TransferPayload(
            modality: modality,
            contentType: receiveRequestContentType,
            payload: try rawReceiveRequestBytes(for: request)
        )
    }

    public static func decodeReceiveRequest(from payload: OfflineNoteV2TransferPayload) throws -> OfflineNoteV2ReceiveRequest {
        guard payload.contentType == receiveRequestContentType else {
            throw OfflineNoteV2NearbyError.invalidMessage
        }
        return try OfflineNoteV2ReceiveRequestCodec.decodeNorito(payload.payload)
    }

    public static func decodeReceiveRequest(fromRawPayload payload: Data) throws -> OfflineNoteV2ReceiveRequest {
        try OfflineNoteV2ReceiveRequestCodec.decodeNorito(payload)
    }

    public static func receiptAckPayload(
        for ack: OfflineNoteV2ReceiptAck,
        modality: OfflineNoteV2TransferModality
    ) throws -> OfflineNoteV2TransferPayload {
        OfflineNoteV2TransferPayload(
            modality: modality,
            contentType: receiptAckContentType,
            payload: try rawReceiptAckBytes(for: ack)
        )
    }

    public static func decodeReceiptAck(from payload: OfflineNoteV2TransferPayload) throws -> OfflineNoteV2ReceiptAck {
        guard payload.contentType == receiptAckContentType else {
            throw OfflineNoteV2NearbyError.invalidMessage
        }
        return try OfflineNoteV2ReceiptAckCodec.decodeNorito(payload.payload)
    }

    public static func decodeReceiptAck(fromRawPayload payload: Data) throws -> OfflineNoteV2ReceiptAck {
        try OfflineNoteV2ReceiptAckCodec.decodeNorito(payload)
    }

    public static func qrStreamingFrameBytes(
        for token: OfflineNoteV2PaymentToken,
        options: OfflineQrStreamOptions = qrStreamingOptions
    ) throws -> [Data] {
        try OfflineNoteV2PaymentTokenCodec.encodeQrFrameBytes(token, options: options)
    }

    public static func qrStreamingFrameBytes(
        for request: OfflineNoteV2ReceiveRequest,
        options: OfflineQrStreamOptions = qrStreamingOptions
    ) throws -> [Data] {
        try OfflineNoteV2ReceiveRequestCodec.encodeQrFrameBytes(request, options: options)
    }

    public static func qrStreamingFrameBytes(
        for ack: OfflineNoteV2ReceiptAck,
        options: OfflineQrStreamOptions = qrStreamingOptions
    ) throws -> [Data] {
        try OfflineNoteV2ReceiptAckCodec.encodeQrFrameBytes(ack, options: options)
    }

    public static func nfcFrameBytes(
        for token: OfflineNoteV2PaymentToken,
        options: OfflineQrStreamOptions = nfcStreamingOptions
    ) throws -> [Data] {
        try streamFrameBytes(for: token, options: options)
    }

    public static func nfcPaymentTokenWriteAPDUs(
        for token: OfflineNoteV2PaymentToken,
        maxChunkLength: Int = OfflineNoteV2NfcApduProtocol.androidSafeChunkBytes
    ) throws -> [Data] {
        try OfflineNoteV2NfcApduProtocol.writePayloadAPDUs(
            kind: .paymentToken,
            payloadBytes: rawPaymentTokenBytes(for: token),
            maxChunkLength: maxChunkLength
        )
    }

    public static func nfcReceiptAckWriteAPDUs(
        for ack: OfflineNoteV2ReceiptAck,
        maxChunkLength: Int = OfflineNoteV2NfcApduProtocol.androidSafeChunkBytes
    ) throws -> [Data] {
        try OfflineNoteV2NfcApduProtocol.writePayloadAPDUs(
            kind: .receiptAck,
            payloadBytes: rawReceiptAckBytes(for: ack),
            maxChunkLength: maxChunkLength
        )
    }

    public static func nearbyPayload(for token: OfflineNoteV2PaymentToken) throws -> OfflineNoteV2TransferPayload {
        try paymentTokenPayload(for: token, modality: .nearby)
    }

    public static func nearbyPaymentEnvelopeBytes(for token: OfflineNoteV2PaymentToken) throws -> Data {
        try OfflineNoteV2NearbyEnvelope(
            kind: .payment,
            payload: rawPaymentTokenBytes(for: token),
            contentType: paymentTokenContentType
        ).encoded()
    }

    public static func decodeNearbyPaymentToken(from envelopeBytes: Data) throws -> OfflineNoteV2PaymentToken {
        try OfflineNoteV2NearbyEnvelope.decode(envelopeBytes).paymentToken()
    }

    public static func nearbyPayload(for ack: OfflineNoteV2ReceiptAck) throws -> OfflineNoteV2TransferPayload {
        try receiptAckPayload(for: ack, modality: .nearby)
    }

    public static func nearbyReceiptAckEnvelopeBytes(for ack: OfflineNoteV2ReceiptAck) throws -> Data {
        try OfflineNoteV2NearbyEnvelope(
            kind: .receiptAck,
            payload: rawReceiptAckBytes(for: ack),
            contentType: receiptAckContentType
        ).encoded()
    }

    public static func decodeNearbyReceiptAck(from envelopeBytes: Data) throws -> OfflineNoteV2ReceiptAck {
        try OfflineNoteV2NearbyEnvelope.decode(envelopeBytes).receiptAck()
    }

    public static func textContentType(for kind: OfflineNoteV2TextPayloadKind) -> String {
        switch kind {
        case .receiveChallenge:
            return textReceiveRequestContentType
        case .paymentToken:
            return textPaymentTokenContentType
        case .receiptAck:
            return textReceiptAckContentType
        }
    }

    public static func textPayloadKind(for contentType: String) -> OfflineNoteV2TextPayloadKind? {
        switch contentType {
        case textReceiveRequestContentType:
            return .receiveChallenge
        case textPaymentTokenContentType:
            return .paymentToken
        case textReceiptAckContentType:
            return .receiptAck
        default:
            return nil
        }
    }

    public static func textPayloadKind(for nfcPayloadKind: OfflineNoteV2NfcPayloadKind) -> OfflineNoteV2TextPayloadKind {
        switch nfcPayloadKind {
        case .receiveChallenge:
            return .receiveChallenge
        case .paymentToken:
            return .paymentToken
        case .receiptAck:
            return .receiptAck
        }
    }

    public static func normalizeTextTransportPayload(
        _ value: String,
        expectedKind: OfflineNoteV2TextPayloadKind? = nil
    ) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        _ = try OfflineNoteV2TransferTextPayloadCodec.decode(trimmed, expectedKind: expectedKind)
        return trimmed
    }

    public static func isValidTextTransportPayload(
        _ value: String,
        expectedKind: OfflineNoteV2TextPayloadKind? = nil
    ) -> Bool {
        (try? normalizeTextTransportPayload(value, expectedKind: expectedKind)) != nil
    }

    public static func nearbyTextEnvelopeBytes(
        payload: String,
        kind: OfflineNoteV2TextPayloadKind,
        pairingChallenge: OfflineNoteV2NearbyPairingChallenge? = nil
    ) throws -> Data {
        let normalized = try normalizeTextTransportPayload(payload, expectedKind: kind)
        return try OfflineNoteV2NearbyEnvelope(
            kind: kind.nearbyMessageKind,
            payload: Data(normalized.utf8),
            contentType: textContentType(for: kind),
            pairingChallenge: pairingChallenge
        ).encoded()
    }

    public static func decodeNearbyTextPayload(
        from envelopeBytes: Data,
        expectedKind: OfflineNoteV2TextPayloadKind? = nil
    ) throws -> (kind: OfflineNoteV2TextPayloadKind, payload: String, pairingChallenge: OfflineNoteV2NearbyPairingChallenge?) {
        let envelope = try OfflineNoteV2NearbyEnvelope.decode(envelopeBytes)
        guard let textKind = textPayloadKind(for: envelope.contentType) else {
            throw OfflineNoteV2NearbyError.invalidMessage
        }
        if let expectedKind, expectedKind != textKind {
            throw OfflineNoteV2NearbyError.invalidMessage
        }
        switch (envelope.kind, textKind) {
        case (.challenge, .receiveChallenge), (.payment, .paymentToken), (.receiptAck, .receiptAck):
            break
        default:
            throw OfflineNoteV2NearbyError.invalidMessage
        }
        guard let payload = String(data: envelope.payload, encoding: .utf8) else {
            throw OfflineNoteV2NearbyError.invalidMessage
        }
        return (
            textKind,
            try normalizeTextTransportPayload(payload, expectedKind: textKind),
            envelope.pairingChallenge
        )
    }

    public static func nearbyFrameBytes(
        for token: OfflineNoteV2PaymentToken,
        options: OfflineQrStreamOptions = nearbyStreamingOptions
    ) throws -> [Data] {
        try streamFrameBytes(for: token, options: options)
    }

    private static func streamFrameBytes(
        for token: OfflineNoteV2PaymentToken,
        options: OfflineQrStreamOptions
    ) throws -> [Data] {
        try OfflineQrStreamEncoder.encodeFrameBytes(
            payload: rawPaymentTokenBytes(for: token),
            payloadKind: .offlinePaymentTokenV2,
            options: options
        )
    }
}
