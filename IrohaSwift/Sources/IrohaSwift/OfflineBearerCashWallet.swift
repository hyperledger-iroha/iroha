import Foundation

public typealias OfflineBearerCashWallet = OfflineNoteWallet
public typealias OfflineBearerCashNote = OfflineNoteWalletNote
public typealias OfflineBearerCashReceiveRequestV1 = OfflineNoteReceiveRequest
public typealias OfflineBearerCashPaymentTokenV1 = OfflineNotePaymentToken
public typealias OfflineBearerCashAckV1 = OfflineNoteReceiptAck

public enum OfflineBearerCashTransport: String, Sendable {
    case staticQr = "static_qr"
    case streamingQr = "streaming_qr"
    case framedByteTransport = "framed_byte_transport"
}

public struct OfflineBearerCashPolicyV1: Equatable, Sendable {
    // TODO: Enforce custody and lineage limits when note audit payloads carry those counters.
    public let maxCustodyHops: Int
    public let maxLineageSteps: Int
    public let maxSingleQrPayloadBytes: Int
    public let maxStreamPayloadBytes: Int
    public let androidKeyPoolTarget: Int
    public let androidKeyPoolReplenishBelow: Int
    public let androidKeyPoolCap: Int

    public static let `default` = OfflineBearerCashPolicyV1()

    public init(maxCustodyHops: Int = 5,
                maxLineageSteps: Int = 32,
                maxSingleQrPayloadBytes: Int = 2_048,
                maxStreamPayloadBytes: Int = 12_288,
                androidKeyPoolTarget: Int = 20,
                androidKeyPoolReplenishBelow: Int = 8,
                androidKeyPoolCap: Int = 40) {
        precondition(maxCustodyHops > 0, "maxCustodyHops must be positive")
        precondition(maxLineageSteps > 0, "maxLineageSteps must be positive")
        precondition(maxSingleQrPayloadBytes > 0, "maxSingleQrPayloadBytes must be positive")
        precondition(maxStreamPayloadBytes >= maxSingleQrPayloadBytes, "stream payload limit must cover static QR")
        precondition(androidKeyPoolReplenishBelow > 0, "androidKeyPoolReplenishBelow must be positive")
        precondition(androidKeyPoolTarget >= androidKeyPoolReplenishBelow, "android key pool target must cover replenish threshold")
        precondition(androidKeyPoolCap >= androidKeyPoolTarget, "android key pool cap must cover target")
        self.maxCustodyHops = maxCustodyHops
        self.maxLineageSteps = maxLineageSteps
        self.maxSingleQrPayloadBytes = maxSingleQrPayloadBytes
        self.maxStreamPayloadBytes = maxStreamPayloadBytes
        self.androidKeyPoolTarget = androidKeyPoolTarget
        self.androidKeyPoolReplenishBelow = androidKeyPoolReplenishBelow
        self.androidKeyPoolCap = androidKeyPoolCap
    }

    public func recommendedTransport(payloadByteCount: Int) -> OfflineBearerCashTransport {
        precondition(payloadByteCount > 0, "payloadByteCount must be positive")
        if payloadByteCount <= maxSingleQrPayloadBytes {
            return .staticQr
        }
        if payloadByteCount <= maxStreamPayloadBytes {
            return .streamingQr
        }
        return .framedByteTransport
    }
}

public enum OfflineBearerCashPayloadKindV1: Equatable, Sendable {
    case receiveRequest
    case payment
    case ack
}

public enum OfflineBearerCashTextCodec {
    public static let receiveRequestTextPrefix = OfflineNoteReceiveRequestCodec.textPrefix
    public static let paymentTextPrefix = OfflineNotePaymentTokenCodec.textPrefix
    public static let ackTextPrefix = OfflineNoteReceiptAckCodec.textPrefix

    public static func encodeReceiveRequestText(_ request: OfflineBearerCashReceiveRequestV1) throws -> String {
        try OfflineNoteReceiveRequestCodec.encodeText(request)
    }

    public static func decodeReceiveRequestText(_ text: String) throws -> OfflineBearerCashReceiveRequestV1 {
        try OfflineNoteReceiveRequestCodec.decodeText(text)
    }

    public static func encodePaymentText(_ token: OfflineBearerCashPaymentTokenV1) throws -> String {
        try OfflineNotePaymentTokenCodec.encodeText(token)
    }

    public static func decodePaymentText(_ text: String) throws -> OfflineBearerCashPaymentTokenV1 {
        try OfflineNotePaymentTokenCodec.decodeText(text)
    }

    public static func encodeAckText(_ ack: OfflineBearerCashAckV1) throws -> String {
        try OfflineNoteReceiptAckCodec.encodeText(ack)
    }

    public static func decodeAckText(_ text: String) throws -> OfflineBearerCashAckV1 {
        try OfflineNoteReceiptAckCodec.decodeText(text)
    }

    public static func payloadKind(_ text: String) -> OfflineBearerCashPayloadKindV1? {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.hasPrefix(receiveRequestTextPrefix) {
            return .receiveRequest
        }
        if trimmed.hasPrefix(paymentTextPrefix) {
            return .payment
        }
        if trimmed.hasPrefix(ackTextPrefix) {
            return .ack
        }
        return nil
    }
}
