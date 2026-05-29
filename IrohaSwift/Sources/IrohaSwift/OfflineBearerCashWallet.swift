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

public struct OfflineBearerCashAuditTrailMetricsV1: Equatable, Sendable {
    public let custodyHops: Int
    public let lineageSteps: Int

    public init(custodyHops: Int, lineageSteps: Int) {
        self.custodyHops = custodyHops
        self.lineageSteps = lineageSteps
    }
}

public enum OfflineBearerCashPolicyError: Error, Equatable {
    case terminalAuditMismatch
    case duplicateTokenId(String)
    case duplicateInputNullifier(String)
    case duplicateOutputCommitment(String)
    case uncommittedOutputClaim(String)
    case outOfOrderInputClaim(String)
    case maxCustodyHopsExceeded(actual: Int, max: Int)
    case maxLineageStepsExceeded(actual: Int, max: Int)
}

public struct OfflineBearerCashPolicyV1: Equatable, Sendable {
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

    public func auditTrailMetrics(
        _ audits: [OfflineNoteAuditBundle],
        terminalAudit: OfflineNoteAuditBundle? = nil
    ) throws -> OfflineBearerCashAuditTrailMetricsV1 {
        if let terminalAudit, audits.last != terminalAudit {
            throw OfflineBearerCashPolicyError.terminalAuditMismatch
        }
        guard !audits.isEmpty else {
            return OfflineBearerCashAuditTrailMetricsV1(custodyHops: 0, lineageSteps: 0)
        }

        var tokenIds = Set<String>()
        var nullifiers = Set<String>()
        var outputProducerIndex: [String: Int] = [:]
        for (index, audit) in audits.enumerated() {
            let tokenId = audit.tokenId.hexLowercased()
            guard tokenIds.insert(tokenId).inserted else {
                throw OfflineBearerCashPolicyError.duplicateTokenId(tokenId)
            }
            for nullifier in audit.inputNullifiers {
                let key = nullifier.hexLowercased()
                guard nullifiers.insert(key).inserted else {
                    throw OfflineBearerCashPolicyError.duplicateInputNullifier(key)
                }
            }
            let committed = Set(audit.outputCommitments.map { $0.hexLowercased() })
            for claim in audit.outputClaims {
                let key = claim.noteCommitment.hexLowercased()
                guard committed.contains(key) else {
                    throw OfflineBearerCashPolicyError.uncommittedOutputClaim(key)
                }
            }
            for output in audit.outputCommitments {
                let key = output.hexLowercased()
                guard outputProducerIndex[key] == nil else {
                    throw OfflineBearerCashPolicyError.duplicateOutputCommitment(key)
                }
                outputProducerIndex[key] = index
            }
        }

        var depths: [Int] = []
        depths.reserveCapacity(audits.count)
        var maxDepth = 0
        for (index, audit) in audits.enumerated() {
            var parentDepth = 0
            for claim in audit.inputClaims {
                let key = claim.noteCommitment.hexLowercased()
                guard let producerIndex = outputProducerIndex[key] else {
                    continue
                }
                guard producerIndex < index else {
                    throw OfflineBearerCashPolicyError.outOfOrderInputClaim(key)
                }
                parentDepth = max(parentDepth, depths[producerIndex])
            }
            let depth = parentDepth + 1
            depths.append(depth)
            maxDepth = max(maxDepth, depth)
        }

        return OfflineBearerCashAuditTrailMetricsV1(
            custodyHops: maxDepth,
            lineageSteps: audits.count
        )
    }

    @discardableResult
    public func validateAuditTrail(
        _ audits: [OfflineNoteAuditBundle],
        terminalAudit: OfflineNoteAuditBundle? = nil
    ) throws -> OfflineBearerCashAuditTrailMetricsV1 {
        let metrics = try auditTrailMetrics(audits, terminalAudit: terminalAudit)
        guard metrics.custodyHops <= maxCustodyHops else {
            throw OfflineBearerCashPolicyError.maxCustodyHopsExceeded(
                actual: metrics.custodyHops,
                max: maxCustodyHops
            )
        }
        guard metrics.lineageSteps <= maxLineageSteps else {
            throw OfflineBearerCashPolicyError.maxLineageStepsExceeded(
                actual: metrics.lineageSteps,
                max: maxLineageSteps
            )
        }
        return metrics
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
