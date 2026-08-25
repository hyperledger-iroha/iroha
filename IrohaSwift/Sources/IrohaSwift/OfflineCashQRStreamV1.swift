import Foundation

/// Options for the SDK-owned Offline Cash V1 IQR1 stream.
public struct OfflineCashQRStreamOptionsV1: Sendable {
    public let compressionPolicy: IrohaPeerWireCompressionPolicyV1

    public init(
        compressionPolicy: IrohaPeerWireCompressionPolicyV1 = .peerOptimized
    ) {
        self.compressionPolicy = compressionPolicy
    }

    public static let standard = OfflineCashQRStreamOptionsV1()
}

/// Offline Cash V1 QR framing over the shared, hardened IPM1/IQR1 transport.
public enum OfflineCashQRStreamCodecV1 {
    public static let nativeTextSchemaVersion: UInt16 = 0x0100

    /// Frames the exact UTF-8 `kgm2:` text. Context-bound semantic validation
    /// remains the responsibility of `OfflineCashPeerAdapterV1` on completion.
    public static func encodePeerText(
        _ peerText: String,
        kind: IrohaPeerWireKindV1,
        options: OfflineCashQRStreamOptionsV1 = .standard
    ) throws -> [String] {
        let message = try IrohaPeerWireMessageV1(
            profile: .offlineCashV1,
            kind: kind,
            schemaVersion: nativeTextSchemaVersion,
            canonicalPayload: Data(peerText.utf8),
            compressionPolicy: options.compressionPolicy
        )
        if let complete = try IrohaPeerQRCodecV1.staticCompleteTextCandidate(for: message) {
            return [complete]
        }
        return try IrohaPeerQRCodecV1.animatedFrameTexts(for: message)
    }

    public static func encodePaymentRequest(
        _ request: OfflineCashPaymentRequestV1,
        options: OfflineCashQRStreamOptionsV1 = .standard
    ) throws -> [String] {
        try encodePeerText(
            OfflineCashPeerAdapterV1().encodePaymentRequest(request),
            kind: .receiveRequest,
            options: options
        )
    }

    public static func encodePayment(
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
        options: OfflineCashQRStreamOptionsV1 = .standard
    ) throws -> [String] {
        try encodePeerText(
            OfflineCashPeerAdapterV1().encodePayment(request: request, payment: payment),
            kind: .payment,
            options: options
        )
    }

    public static func encodeAcknowledgement(
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
        acknowledgement: OfflineCashAcknowledgementV1,
        options: OfflineCashQRStreamOptionsV1 = .standard
    ) throws -> [String] {
        try encodePeerText(
            OfflineCashPeerAdapterV1().encodeAcknowledgement(
                request: request,
                payment: payment,
                acknowledgement: acknowledgement
            ),
            kind: .acknowledgement,
            options: options
        )
    }

    public static func decodeFrameText(_ frameText: String) throws -> IrohaPeerQRFrameV1 {
        let frame = try IrohaPeerQRCodecV1.decodeFrame(frameText)
        guard frame.profile == .offlineCashV1 else {
            throw IrohaPeerQRErrorV1.unexpectedProfile(
                expected: .offlineCashV1,
                actual: frame.profile
            )
        }
        return frame
    }

    public static func completedPeerText(_ message: IrohaPeerWireMessageV1) throws -> String {
        guard message.profile == .offlineCashV1 else {
            throw IrohaPeerWireMessageErrorV1.unexpectedProfile(
                expected: .offlineCashV1,
                actual: message.profile
            )
        }
        guard message.schemaVersion == nativeTextSchemaVersion else {
            throw IrohaPeerWireMessageErrorV1.schemaVersionMismatch(
                profile: .offlineCashV1,
                expected: nativeTextSchemaVersion,
                actual: message.schemaVersion
            )
        }
        guard let text = String(data: message.canonicalPayload, encoding: .utf8),
              Data(text.utf8) == message.canonicalPayload else {
            throw IrohaPeerWireMessageErrorV1.invalidCanonicalPayload(
                profile: .offlineCashV1,
                kind: message.kind
            )
        }
        return text
    }
}

/// One bounded scan update and, on completion, the exact canonical `kgm2:` text.
public struct OfflineCashQRStreamProgressV1: Equatable, Sendable {
    public let completedPeerText: String?
    public let kind: IrohaPeerWireKindV1
    public let streamID: Data
    public let receivedDataFrames: Int
    public let totalDataFrames: Int
    public let recoveredDataFrames: Int
    public let isDuplicate: Bool

    public var isComplete: Bool { completedPeerText != nil }
    public var fractionComplete: Double {
        if isComplete { return 1 }
        guard totalDataFrames > 0 else { return 0 }
        return min(1, Double(receivedDataFrames) / Double(totalDataFrames))
    }
}

/// Stateful, reorder-tolerant Offline Cash V1 animated QR decoder.
public final class OfflineCashQRStreamDecoderV1: @unchecked Sendable {
    private let decoder: IrohaPeerQRScanSessionV1

    public init(
        expectedKind: IrohaPeerWireKindV1? = nil,
        wireLimits: IrohaPeerWireLimitsV1 = .peerV1,
        scanLimits: IrohaPeerQRScanLimitsV1 = .standard,
        clock: @escaping @Sendable () -> TimeInterval = {
            ProcessInfo.processInfo.systemUptime
        }
    ) {
        decoder = IrohaPeerQRScanSessionV1(
            expectedProfile: .offlineCashV1,
            expectedKind: expectedKind,
            expectedSchemaVersion: OfflineCashQRStreamCodecV1.nativeTextSchemaVersion,
            wireLimits: wireLimits,
            scanLimits: scanLimits,
            clock: clock
        )
    }

    public var activeStreamCount: Int { decoder.activeStreamCount }

    public func reset() { decoder.reset() }

    /// Quarantines a stream after context-bound native adapter validation fails.
    public func quarantine(streamID: Data) throws {
        try decoder.quarantine(streamID: streamID)
    }

    public func quarantine(streamID: Data, atUptime uptime: TimeInterval) throws {
        try decoder.quarantine(streamID: streamID, atUptime: uptime)
    }

    public func ingest(_ frameText: String) throws -> OfflineCashQRStreamProgressV1 {
        let frame = try OfflineCashQRStreamCodecV1.decodeFrameText(frameText)
        return try progress(
            from: decoder.ingest(frameText),
            kind: frame.payloadKind
        )
    }

    public func ingest(
        _ frameText: String,
        atUptime uptime: TimeInterval
    ) throws -> OfflineCashQRStreamProgressV1 {
        let frame = try OfflineCashQRStreamCodecV1.decodeFrameText(frameText)
        return try progress(
            from: decoder.ingest(frameText, atUptime: uptime),
            kind: frame.payloadKind
        )
    }

    private func progress(
        from event: IrohaPeerQRScanEventV1,
        kind: IrohaPeerWireKindV1
    ) throws -> OfflineCashQRStreamProgressV1 {
        switch event {
        case .accepted(let value):
            return OfflineCashQRStreamProgressV1(
                completedPeerText: nil,
                kind: kind,
                streamID: value.streamID,
                receivedDataFrames: value.receivedDataShards,
                totalDataFrames: value.totalDataShards,
                recoveredDataFrames: value.recoveredDataShards,
                isDuplicate: false
            )
        case .duplicate(let value):
            return OfflineCashQRStreamProgressV1(
                completedPeerText: nil,
                kind: kind,
                streamID: value.streamID,
                receivedDataFrames: value.receivedDataShards,
                totalDataFrames: value.totalDataShards,
                recoveredDataFrames: value.recoveredDataShards,
                isDuplicate: true
            )
        case .completed(let completion):
            return OfflineCashQRStreamProgressV1(
                completedPeerText: try OfflineCashQRStreamCodecV1.completedPeerText(
                    completion.message
                ),
                kind: completion.message.kind,
                streamID: completion.progress.streamID,
                receivedDataFrames: completion.progress.receivedDataShards,
                totalDataFrames: completion.progress.totalDataShards,
                recoveredDataFrames: completion.progress.recoveredDataShards,
                isDuplicate: false
            )
        }
    }
}
