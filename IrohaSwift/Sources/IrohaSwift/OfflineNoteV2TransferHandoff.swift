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
    public let receivedChunks: Int
    public let totalChunks: Int
    public let recoveredChunks: Int

    public var isComplete: Bool {
        token != nil
    }

    public var progress: Double {
        guard totalChunks > 0 else { return 0 }
        return Double(receivedChunks) / Double(totalChunks)
    }
}

public final class OfflineNoteV2TransferStreamReceiver {
    private let decoder = OfflineQrStreamDecoder()

    public init() {}

    public func ingestFrame(_ frameBytes: Data) throws -> OfflineNoteV2TransferStreamResult {
        let result = try decoder.ingest(frameBytes: frameBytes)
        let token: OfflineNoteV2PaymentToken?
        if let payload = result.payload {
            guard result.payloadKind == .offlinePaymentTokenV2 else {
                throw OfflineQrStreamError.invalidEnvelope("payload_kind mismatch")
            }
            token = try OfflineNoteV2PaymentTokenCodec.decodeQrPayload(payload)
        } else {
            token = nil
        }
        return OfflineNoteV2TransferStreamResult(
            payload: result.payload,
            token: token,
            receivedChunks: result.receivedChunks,
            totalChunks: result.totalChunks,
            recoveredChunks: result.recoveredChunks
        )
    }
}

public enum OfflineNoteV2TransferHandoff {
    public static let paymentTokenContentType = "application/vnd.iroha.offline.payment-token-v2+norito"
    public static let receiveChallengeContentType = "application/vnd.iroha.offline.receive-challenge-v1+octet-stream"
    public static let receiptAckContentType = "application/vnd.iroha.offline.receipt-ack-v1+octet-stream"
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

    public static func qrStreamingFrameBytes(
        for token: OfflineNoteV2PaymentToken,
        options: OfflineQrStreamOptions = qrStreamingOptions
    ) throws -> [Data] {
        try OfflineNoteV2PaymentTokenCodec.encodeQrFrameBytes(token, options: options)
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
