import Foundation
import CryptoKit

/// Errors surfaced by offline receipt and bundle builders when inputs are invalid.
public enum OfflineReceiptBuilderError: Error, LocalizedError, Equatable {
    case invalidTxIdLength(actual: Int)
    case invalidTxIdHash
    case invalidReplayLogHeadHex
    case invalidBundleIdLength(actual: Int)
    case invalidBundleIdHash
    case invalidAccountId(field: String, value: String)
    case emptyInvoiceId
    case emptyReceiver
    case emptyDepositAccount
    case emptySender
    case emptyReceipts
    case duplicateInvoiceId(String)
    case invalidAssetId(String)
    case invalidAmount(String)
    case fractionalAmount(String)
    case amountScaleMismatch(value: String, expected: Int, actual: Int)
    case nonPositiveAmount(String)
    case amountExceedsPolicy(amount: String, max: String)
    case invalidReceiptTimestamp(String)
    case receiptReceiverMismatch
    case receiptSenderMismatch
    case receiptAssetMismatch
    case receiptCertificateMismatch
    case receiptOrderInvalid
    case mixedCounterScopes
    case balanceProofAssetMismatch
    case claimedDeltaMismatch(expected: String, actual: String)
    case invalidPlatformSnapshot(String)
    case invalidPlatformProof(String)
    case challengeHashMismatch
    case spendKeyMismatch(expected: String, actual: String)
    case invalidSpendPublicKey(String)
    case unsupportedSpendPublicKeyAlgorithm(String)
    case missingSenderSignature
    case invalidSenderSignature
    case signatureVerificationUnavailable(String)
    case invalidCommitmentLength(Int)
    case invalidResultingCommitmentLength(Int)
    case missingBalanceProof
    case invalidBalanceProofLength(expected: Int, actual: Int)
    case unsupportedBalanceProofVersion(UInt8)
    case aggregateOverflow
    case aggregateProofVersionUnsupported(UInt16)
    case aggregateProofRootMismatch
    case aggregateProofHashError(String)
    case aggregateProofRequestEncodingFailed(String)
    case aggregateProofGenerationUnavailable(String)
    case aggregateProofGenerationFailed(String)

    public var errorDescription: String? {
        switch self {
        case let .invalidTxIdLength(actual):
            return "Receipt txId must be 32 bytes (got \(actual))."
        case .invalidTxIdHash:
            return "Receipt txId must have its least significant bit set."
        case .invalidReplayLogHeadHex:
            return "Replay log head must be a 32-byte hash hex string."
        case let .invalidBundleIdLength(actual):
            return "Bundle id must be 32 bytes (got \(actual))."
        case .invalidBundleIdHash:
            return "Bundle id must have its least significant bit set."
        case let .invalidAccountId(field, value):
            return "Invalid \(field) account id: \(value)."
        case .emptyInvoiceId:
            return "Receipt invoice_id must be non-empty."
        case .emptyReceiver:
            return "Receiver account id must be non-empty."
        case .emptyDepositAccount:
            return "Deposit account id must be non-empty."
        case .emptySender:
            return "Sender account id must be non-empty."
        case .emptyReceipts:
            return "Offline transfer bundle must include at least one receipt."
        case let .duplicateInvoiceId(value):
            return "Offline transfer bundle contains duplicate invoice_id \(value)."
        case let .invalidAssetId(value):
            return "Invalid asset id: \(value)."
        case let .invalidAmount(value):
            return "Invalid numeric amount: \(value)."
        case let .fractionalAmount(value):
            return "Offline amounts must use scale 0: \(value)."
        case let .amountScaleMismatch(value, expected, actual):
            return "Offline amounts must use scale \(expected) (got \(actual)): \(value)."
        case let .nonPositiveAmount(value):
            return "Receipt amount must be positive: \(value)."
        case let .amountExceedsPolicy(amount, max):
            return "Receipt amount \(amount) exceeds policy max_tx_value \(max)."
        case let .invalidReceiptTimestamp(reason):
            return "Receipt issued_at_ms is invalid: \(reason)."
        case .receiptReceiverMismatch:
            return "Receipt receiver must match the bundle receiver."
        case .receiptSenderMismatch:
            return "Receipt sender must match the certificate controller."
        case .receiptAssetMismatch:
            return "Receipt asset must match the allowance asset."
        case .receiptCertificateMismatch:
            return "Receipts must share the same sender certificate."
        case .receiptOrderInvalid:
            return "Receipts must be ordered by (counter, tx_id)."
        case .mixedCounterScopes:
            return "Receipts must share a single counter scope."
        case .balanceProofAssetMismatch:
            return "Balance proof asset must match the allowance asset."
        case let .claimedDeltaMismatch(expected, actual):
            return "Balance proof claimed_delta \(actual) does not match receipt sum \(expected)."
        case let .invalidPlatformSnapshot(reason):
            return "Invalid platform snapshot: \(reason)."
        case let .invalidPlatformProof(reason):
            return "Invalid platform proof: \(reason)."
        case .challengeHashMismatch:
            return "Platform proof challenge hash does not match the receipt payload."
        case let .spendKeyMismatch(expected, actual):
            return "Spend key mismatch: expected \(expected), got \(actual)."
        case let .invalidSpendPublicKey(reason):
            return "Spend public key is invalid: \(reason)."
        case let .unsupportedSpendPublicKeyAlgorithm(reason):
            return "Spend public key algorithm is unsupported: \(reason)."
        case .missingSenderSignature:
            return "Receipt sender_signature must be provided."
        case .invalidSenderSignature:
            return "Receipt sender_signature does not match the spend public key."
        case let .signatureVerificationUnavailable(reason):
            return "Receipt signature verification is unavailable: \(reason)."
        case let .invalidCommitmentLength(length):
            return "Commitment must be 32 bytes (got \(length))."
        case let .invalidResultingCommitmentLength(length):
            return "Resulting commitment must be 32 bytes (got \(length))."
        case .missingBalanceProof:
            return "Balance proof zk_proof must be provided."
        case let .invalidBalanceProofLength(expected, actual):
            return "Balance proof must be \(expected) bytes (got \(actual))."
        case let .unsupportedBalanceProofVersion(version):
            return "Balance proof version \(version) is not supported."
        case .aggregateOverflow:
            return "Aggregate amount exceeds numeric bounds."
        case let .aggregateProofVersionUnsupported(version):
            return "Aggregate proof version \(version) is not supported."
        case .aggregateProofRootMismatch:
            return "Aggregate proof receipts_root does not match transfer receipts."
        case let .aggregateProofHashError(reason):
            return "Aggregate proof receipts_root could not be computed: \(reason)."
        case let .aggregateProofRequestEncodingFailed(reason):
            return "Aggregate proof request could not be encoded: \(reason)."
        case let .aggregateProofGenerationUnavailable(reason):
            return "Aggregate proof generation is unavailable: \(reason)."
        case let .aggregateProofGenerationFailed(reason):
            return "Aggregate proof generation failed: \(reason)."
        }
    }
}

/// Records offline receipts into the journal and optional audit log.
public final class OfflineReceiptRecorder {
    public let journal: OfflineJournal
    public let auditLogger: OfflineAuditLogger?

    public init(journal: OfflineJournal, auditLogger: OfflineAuditLogger? = nil) {
        self.journal = journal
        self.auditLogger = auditLogger
    }

    @discardableResult
    public func appendPending(receipt: OfflineSpendReceipt, timestampMs: UInt64? = nil) throws -> OfflineJournalEntry {
        let entry = try journal.appendPending(receipt: receipt, timestampMs: timestampMs)
        recordAudit(receipt: receipt, timestampMs: entry.recordedAtMs)
        return entry
    }

    public func markCommitted(receipt: OfflineSpendReceipt, timestampMs: UInt64? = nil) throws {
        try journal.markCommitted(receipt: receipt, timestampMs: timestampMs)
    }

    public func recordAudit(receipt: OfflineSpendReceipt, timestampMs: UInt64? = nil) {
        guard let auditLogger else { return }
        let recordedAt = timestampMs ?? OfflineReceiptRecorder.currentTimestampMs()
        let entry = OfflineAuditEntry(
            txId: receipt.txId.hexUppercased(),
            senderId: receipt.from,
            receiverId: receipt.to,
            assetId: receipt.assetId,
            amount: receipt.amount,
            timestampMs: recordedAt
        )
        auditLogger.record(entry: entry)
    }

    private static func currentTimestampMs() -> UInt64 {
        UInt64(Date().timeIntervalSince1970 * 1_000)
    }
}

/// Helpers for building and validating offline receipts and settlement bundles.
public enum OfflineReceiptBuilder {
    public static func generateReceiptId() -> Data {
        IrohaHash.hash(Data(UUID().uuidString.utf8))
    }

    public static func generateReceiptId(seed: Data) -> Data {
        IrohaHash.hash(seed)
    }

    public static func generateBundleId() -> Data {
        IrohaHash.hash(Data(UUID().uuidString.utf8))
    }

    public static func generateBundleId(seed: Data) -> Data {
        IrohaHash.hash(seed)
    }

    /// Builds and signs a receipt using the spend key from the sender certificate.
    /// The `chainId` is hashed into the platform challenge to prevent cross-chain replays.
    @available(macOS 10.15, iOS 13.0, *)
    public static func buildSignedReceipt(
        txId: Data? = nil,
        txIdSeed: Data? = nil,
        chainId: String,
        receiverAccountId: String,
        amount: String,
        invoiceId: String,
        platformProof: OfflinePlatformProof,
        platformSnapshot: OfflinePlatformTokenSnapshot? = nil,
        senderCertificate: OfflineWalletCertificate,
        signingKey: SigningKey,
        recorder: OfflineReceiptRecorder? = nil,
        timestampMs: UInt64? = nil,
        issuedAtMs: UInt64? = nil
    ) throws -> OfflineSpendReceipt {
        let finalTxId: Data
        if let txId {
            finalTxId = txId
        } else if let seed = txIdSeed {
            finalTxId = generateReceiptId(seed: seed)
        } else {
            finalTxId = generateReceiptId()
        }
        let issuedAt = issuedAtMs ?? timestampMs ?? UInt64(Date().timeIntervalSince1970 * 1000.0)
        try validateAccountId(receiverAccountId, field: "receiver")
        try validateSpendKey(signingKey: signingKey, certificate: senderCertificate)
        let receipt = OfflineSpendReceipt(
            txId: finalTxId,
            from: senderCertificate.controller,
            to: receiverAccountId,
            assetId: senderCertificate.allowance.assetId,
            amount: amount,
            issuedAtMs: issuedAt,
            invoiceId: invoiceId,
            platformProof: platformProof,
            platformSnapshot: platformSnapshot,
            senderCertificate: senderCertificate,
            senderSignature: Data()
        )
        let signed = try receipt.signed(with: signingKey)
        try validateReceipt(signed, chainId: chainId)
        if let recorder {
            try recorder.appendPending(receipt: signed, timestampMs: timestampMs ?? issuedAt)
        }
        return signed
    }

    /// Aggregates receipt amounts into the claimed delta string used by balance proofs.
    /// Receipt amounts must match the allowance scale; mismatches are rejected.
    public static func aggregateAmount(receipts: [OfflineSpendReceipt]) throws -> String {
        guard !receipts.isEmpty else {
            throw OfflineReceiptBuilderError.emptyReceipts
        }
        let expectedScale = try expectedScale(for: receipts[0].senderCertificate)
        var total = OfflineDecimal.zero(scale: expectedScale)
        for receipt in receipts {
            let value = try parseAmount(receipt.amount, expectedScale: expectedScale)
            guard !value.isNegative, !value.isZero else {
                throw OfflineReceiptBuilderError.nonPositiveAmount(receipt.amount)
            }
            total = try total.adding(value)
        }
        let rendered = total.render()
        do {
            _ = try OfflineNorito.encodeNumeric(rendered)
        } catch let error as OfflineNoritoError {
            if case .numericOverflow = error {
                throw OfflineReceiptBuilderError.aggregateOverflow
            }
            throw OfflineReceiptBuilderError.invalidAmount(rendered)
        }
        return rendered
    }

    /// Computes the Poseidon receipts_root used by aggregate proof envelopes.
    public static func computeReceiptsRoot(receipts: [OfflineSpendReceipt]) throws -> OfflinePoseidonDigest {
        let bytes = try OfflineReceiptPoseidon.receiptsRoot(receipts: receipts)
        return OfflinePoseidonDigest(bytes: bytes)
    }

    private static let replayChainDomain = Data("iroha.offline.fastpq.replay.chain.v1".utf8)

    /// Computes the replay log tail by hashing the head with each txId in receipt order.
    public static func computeReplayLogTail(headHex: String, txIds: [Data]) throws -> String {
        let trimmed = headHex.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw OfflineReceiptBuilderError.invalidReplayLogHeadHex
        }
        var hex = trimmed
        if hex.hasPrefix("0x") || hex.hasPrefix("0X") {
            hex = String(hex.dropFirst(2))
        }
        guard let head = Data(hexString: hex),
              head.count == 32,
              let last = head.last,
              (last & 1) == 1 else {
            throw OfflineReceiptBuilderError.invalidReplayLogHeadHex
        }
        var current = head
        for txId in txIds {
            try validateTxId(txId)
            var writer = Data()
            writer.reserveCapacity(replayChainDomain.count + current.count + txId.count)
            writer.append(replayChainDomain)
            writer.append(current)
            writer.append(txId)
            current = IrohaHash.hash(writer)
        }
        return current.hexUppercased()
    }

    /// Builds an aggregate proof envelope with the receipts_root derived from the receipts.
    public static func buildAggregateProofEnvelope(
        receipts: [OfflineSpendReceipt],
        proofSum: Data? = nil,
        proofCounter: Data? = nil,
        proofReplay: Data? = nil,
        metadata: [String: ToriiJSONValue] = [:],
        version: UInt16 = 1
    ) throws -> OfflineAggregateProofEnvelope {
        guard version == 1 else {
            throw OfflineReceiptBuilderError.aggregateProofVersionUnsupported(version)
        }
        let root = try computeReceiptsRoot(receipts: receipts)
        return OfflineAggregateProofEnvelope(
            version: version,
            receiptsRoot: root,
            proofSum: proofSum,
            proofCounter: proofCounter,
            proofReplay: proofReplay,
            metadata: metadata
        )
    }

    /// Generates aggregate proof bytes from FASTPQ witness payloads returned by Torii.
    public static func generateAggregateProofs(
        sumRequest: ToriiJSONValue? = nil,
        counterRequest: ToriiJSONValue? = nil,
        replayRequest: ToriiJSONValue? = nil
    ) throws -> (sum: Data?, counter: Data?, replay: Data?) {
        let sum = try generateAggregateProof(
            request: sumRequest,
            bridgeCall: { try NoritoNativeBridge.shared.offlineFastpqProofSum(requestJson: $0) }
        )
        let counter = try generateAggregateProof(
            request: counterRequest,
            bridgeCall: { try NoritoNativeBridge.shared.offlineFastpqProofCounter(requestJson: $0) }
        )
        let replay = try generateAggregateProof(
            request: replayRequest,
            bridgeCall: { try NoritoNativeBridge.shared.offlineFastpqProofReplay(requestJson: $0) }
        )
        return (sum: sum, counter: counter, replay: replay)
    }

    private static func generateAggregateProof(
        request: ToriiJSONValue?,
        bridgeCall: (Data) throws -> Data?
    ) throws -> Data? {
        guard let request else {
            return nil
        }
        let json: Data
        do {
            json = try request.encodedData()
        } catch {
            throw OfflineReceiptBuilderError.aggregateProofRequestEncodingFailed(error.localizedDescription)
        }
        do {
            if let proof = try bridgeCall(json) {
                return proof
            }
        } catch {
            throw OfflineReceiptBuilderError.aggregateProofGenerationFailed(error.localizedDescription)
        }
        let message = NoritoNativeBridge.bridgeUnavailableMessage(
            "FASTPQ proof generation requires the native bridge."
        )
        throw OfflineReceiptBuilderError.aggregateProofGenerationUnavailable(message)
    }

    /// Constructs a balance proof with a claimed delta derived from the receipts.
    public static func buildBalanceProof(
        initialCommitment: OfflineAllowanceCommitment,
        resultingCommitment: Data,
        receipts: [OfflineSpendReceipt],
        zkProof: Data? = nil
    ) throws -> OfflineBalanceProof {
        try validateCommitmentLength(initialCommitment.commitment)
        try validateResultingCommitmentLength(resultingCommitment)
        let claimedDelta = try aggregateAmount(receipts: receipts)
        return OfflineBalanceProof(
            initialCommitment: initialCommitment,
            resultingCommitment: resultingCommitment,
            claimedDelta: claimedDelta,
            zkProof: zkProof
        )
    }

    /// Builds an offline transfer bundle and validates receipt invariants.
    public static func buildTransfer(
        bundleId: Data? = nil,
        bundleIdSeed: Data? = nil,
        chainId: String,
        receiver: String,
        depositAccount: String,
        receipts: [OfflineSpendReceipt],
        balanceProof: OfflineBalanceProof,
        aggregateProof: OfflineAggregateProofEnvelope? = nil,
        attachments: OfflineProofAttachmentList? = nil,
        platformSnapshot: OfflinePlatformTokenSnapshot? = nil,
        sortReceipts: Bool = true
    ) throws -> OfflineToOnlineTransfer {
        let finalBundleId: Data
        if let bundleId {
            finalBundleId = bundleId
        } else if let seed = bundleIdSeed {
            finalBundleId = generateBundleId(seed: seed)
        } else {
            finalBundleId = generateBundleId()
        }
        let orderedReceipts = sortReceipts ? receipts.sorted(by: receiptSort) : receipts
        let transfer = OfflineToOnlineTransfer(
            bundleId: finalBundleId,
            receiver: receiver,
            depositAccount: depositAccount,
            receipts: orderedReceipts,
            balanceProof: balanceProof,
            aggregateProof: aggregateProof,
            attachments: attachments,
            platformSnapshot: platformSnapshot
        )
        try validateTransfer(transfer, chainId: chainId)
        return transfer
    }

    /// Validates a receipt for offline submission.
    public static func validateReceipt(_ receipt: OfflineSpendReceipt, chainId: String) throws {
        try validateTxId(receipt.txId)
        if receipt.invoiceId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            throw OfflineReceiptBuilderError.emptyInvoiceId
        }
        try validateAccountId(receipt.from, field: "sender")
        try validateAccountId(receipt.to, field: "receiver")
        do {
            _ = try OfflineNorito.encodeAssetId(receipt.assetId)
        } catch {
            throw OfflineReceiptBuilderError.invalidAssetId(receipt.assetId)
        }
        let expectedScale = try expectedScale(for: receipt.senderCertificate)
        let amount = try parseAmount(receipt.amount, expectedScale: expectedScale)
        guard !amount.isNegative, !amount.isZero else {
            throw OfflineReceiptBuilderError.nonPositiveAmount(receipt.amount)
        }
        try validateReceiptTimestamp(receipt)
        if receipt.from != receipt.senderCertificate.controller {
            throw OfflineReceiptBuilderError.receiptSenderMismatch
        }
        if receipt.assetId != receipt.senderCertificate.allowance.assetId {
            throw OfflineReceiptBuilderError.receiptAssetMismatch
        }
        let policyMax = try parseAmount(receipt.senderCertificate.policy.maxTxValue,
                                        expectedScale: expectedScale)
        if amount.compare(to: policyMax) == .orderedDescending {
            throw OfflineReceiptBuilderError.amountExceedsPolicy(amount: receipt.amount,
                                                                max: receipt.senderCertificate.policy.maxTxValue)
        }
        guard !receipt.senderSignature.isEmpty else {
            throw OfflineReceiptBuilderError.missingSenderSignature
        }
        try verifyReceiptSignature(receipt)
        try validateSnapshot(receipt.platformSnapshot, metadata: receipt.senderCertificate.metadata)
        try validatePlatformProof(receipt, chainId: chainId)
    }

    private static func validateReceiptTimestamp(_ receipt: OfflineSpendReceipt) throws {
        let issuedAtMs = receipt.issuedAtMs
        if issuedAtMs == 0 {
            throw OfflineReceiptBuilderError.invalidReceiptTimestamp("issued_at_ms must be > 0")
        }
        if issuedAtMs < receipt.senderCertificate.issuedAtMs {
            throw OfflineReceiptBuilderError.invalidReceiptTimestamp(
                "issued_at_ms predates certificate issuance"
            )
        }
        let expiryBound = min(receipt.senderCertificate.expiresAtMs,
                              receipt.senderCertificate.policy.expiresAtMs)
        if issuedAtMs > expiryBound {
            throw OfflineReceiptBuilderError.invalidReceiptTimestamp(
                "issued_at_ms exceeds certificate/policy expiry"
            )
        }
    }

    /// Validates a transfer bundle before submission.
    public static func validateTransfer(_ transfer: OfflineToOnlineTransfer, chainId: String) throws {
        try validateBundleId(transfer.bundleId)
        try validateAccountId(transfer.receiver, field: "receiver")
        try validateAccountId(transfer.depositAccount, field: "depositAccount")
        guard !transfer.receipts.isEmpty else {
            throw OfflineReceiptBuilderError.emptyReceipts
        }
        guard receiptsAreCanonical(transfer.receipts) else {
            throw OfflineReceiptBuilderError.receiptOrderInvalid
        }
        let first = transfer.receipts[0]
        try validateSnapshot(transfer.platformSnapshot, metadata: first.senderCertificate.metadata)
        let expectedScope = try counterScope(for: first)
        let expectedCertificateId = try first.senderCertificate.certificateId()
        let expectedAssetId = first.senderCertificate.allowance.assetId
        let expectedScale = try expectedScale(for: first.senderCertificate)
        let policyMax = try parseAmount(first.senderCertificate.policy.maxTxValue,
                                        expectedScale: expectedScale)
        let expectedSum = try aggregateAmount(receipts: transfer.receipts)
        let claimed = try parseAmount(transfer.balanceProof.claimedDelta,
                                      expectedScale: expectedScale)

        try validateCommitmentLength(transfer.balanceProof.initialCommitment.commitment)
        try validateResultingCommitmentLength(transfer.balanceProof.resultingCommitment)
        guard let proof = transfer.balanceProof.zkProof, !proof.isEmpty else {
            throw OfflineReceiptBuilderError.missingBalanceProof
        }
        if proof.count != OfflineBalanceProofBuilder.proofLength {
            throw OfflineReceiptBuilderError.invalidBalanceProofLength(
                expected: OfflineBalanceProofBuilder.proofLength,
                actual: proof.count
            )
        }
        if proof.first != 1 {
            throw OfflineReceiptBuilderError.unsupportedBalanceProofVersion(proof.first ?? 0)
        }
        if transfer.balanceProof.initialCommitment.assetId != expectedAssetId {
            throw OfflineReceiptBuilderError.balanceProofAssetMismatch
        }
        _ = try parseAmount(transfer.balanceProof.initialCommitment.amount,
                            expectedScale: expectedScale)
        guard !claimed.isNegative, !claimed.isZero else {
            throw OfflineReceiptBuilderError.nonPositiveAmount(transfer.balanceProof.claimedDelta)
        }
        if !claimed.equals(expected: expectedSum) {
            throw OfflineReceiptBuilderError.claimedDeltaMismatch(expected: expectedSum,
                                                                 actual: transfer.balanceProof.claimedDelta)
        }

        var invoiceIds = Set<String>()
        for receipt in transfer.receipts {
            try validateReceipt(receipt, chainId: chainId)
            if !invoiceIds.insert(receipt.invoiceId).inserted {
                throw OfflineReceiptBuilderError.duplicateInvoiceId(receipt.invoiceId)
            }
            if receipt.to != transfer.receiver {
                throw OfflineReceiptBuilderError.receiptReceiverMismatch
            }
            if receipt.from != first.senderCertificate.controller {
                throw OfflineReceiptBuilderError.receiptSenderMismatch
            }
            if receipt.assetId != expectedAssetId {
                throw OfflineReceiptBuilderError.receiptAssetMismatch
            }
            let certId = try receipt.senderCertificate.certificateId()
            if certId != expectedCertificateId {
                throw OfflineReceiptBuilderError.receiptCertificateMismatch
            }
            let amount = try parseAmount(receipt.amount, expectedScale: expectedScale)
            guard !amount.isNegative, !amount.isZero else {
                throw OfflineReceiptBuilderError.nonPositiveAmount(receipt.amount)
            }
            if amount.compare(to: policyMax) == .orderedDescending {
                throw OfflineReceiptBuilderError.amountExceedsPolicy(amount: receipt.amount,
                                                                    max: first.senderCertificate.policy.maxTxValue)
            }
            let scope = try counterScope(for: receipt)
            if scope != expectedScope {
                throw OfflineReceiptBuilderError.mixedCounterScopes
            }
        }
        try validateAggregateProof(transfer)
    }

    // MARK: - Internal validation helpers

    private static func validateTxId(_ txId: Data) throws {
        guard txId.count == 32 else {
            throw OfflineReceiptBuilderError.invalidTxIdLength(actual: txId.count)
        }
        guard let last = txId.last, (last & 1) == 1 else {
            throw OfflineReceiptBuilderError.invalidTxIdHash
        }
    }

    private static func validateAccountId(_ value: String, field: String) throws {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            switch field {
            case "receiver":
                throw OfflineReceiptBuilderError.emptyReceiver
            case "depositAccount":
                throw OfflineReceiptBuilderError.emptyDepositAccount
            case "sender":
                throw OfflineReceiptBuilderError.emptySender
            default:
                throw OfflineReceiptBuilderError.invalidAccountId(field: field, value: value)
            }
        }
        if trimmed != value {
            throw OfflineReceiptBuilderError.invalidAccountId(field: field, value: trimmed)
        }
        if trimmed.rangeOfCharacter(from: .whitespacesAndNewlines) != nil {
            throw OfflineReceiptBuilderError.invalidAccountId(field: field, value: trimmed)
        }
        if trimmed.contains("#") || trimmed.contains("$") {
            throw OfflineReceiptBuilderError.invalidAccountId(field: field, value: trimmed)
        }
        let components = trimmed.split(separator: "@", omittingEmptySubsequences: false)
        guard components.count == 2 else {
            throw OfflineReceiptBuilderError.invalidAccountId(field: field, value: trimmed)
        }
        guard !components[0].isEmpty, !components[1].isEmpty else {
            throw OfflineReceiptBuilderError.invalidAccountId(field: field, value: trimmed)
        }
    }

    private static func validateBundleId(_ bundleId: Data) throws {
        guard bundleId.count == 32 else {
            throw OfflineReceiptBuilderError.invalidBundleIdLength(actual: bundleId.count)
        }
        guard let last = bundleId.last, (last & 1) == 1 else {
            throw OfflineReceiptBuilderError.invalidBundleIdHash
        }
    }

    @available(macOS 10.15, iOS 13.0, *)
    private static func validateSpendKey(signingKey: SigningKey,
                                         certificate: OfflineWalletCertificate) throws {
        let spendKey = try parseSpendPublicKey(certificate.spendPublicKey)
        let publicKey = try signingKey.publicKey()
        let expected = encodeSpendPublicKeyString(algorithm: signingKey.algorithm, publicKey: publicKey)
        let actual = encodeSpendPublicKeyString(algorithm: spendKey.algorithm, publicKey: spendKey.publicKey)
        guard signingKey.algorithm == spendKey.algorithm else {
            throw OfflineReceiptBuilderError.spendKeyMismatch(expected: expected, actual: actual)
        }
        if publicKey != spendKey.publicKey {
            throw OfflineReceiptBuilderError.spendKeyMismatch(expected: expected, actual: actual)
        }
    }

    private static func canonicalSpendKey(_ value: String) -> String {
        if let parsed = try? parseSpendPublicKey(value) {
            return encodeSpendPublicKeyString(algorithm: parsed.algorithm, publicKey: parsed.publicKey)
        }
        return value.trimmingCharacters(in: .whitespacesAndNewlines).uppercased()
    }

    private struct SpendPublicKey {
        let algorithm: SigningAlgorithm
        let publicKey: Data
    }

    private static func parseSpendPublicKey(_ value: String) throws -> SpendPublicKey {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw OfflineReceiptBuilderError.invalidSpendPublicKey("empty value")
        }
        let rawHex: String
        var prefixedAlgorithm: SigningAlgorithm?
        if let separator = trimmed.firstIndex(of: ":") {
            let prefix = String(trimmed[..<separator])
            guard let parsed = parseAlgorithmPrefix(prefix) else {
                throw OfflineReceiptBuilderError.invalidSpendPublicKey("unknown algorithm prefix \(prefix)")
            }
            prefixedAlgorithm = parsed
            rawHex = String(trimmed[trimmed.index(after: separator)...])
        } else {
            rawHex = trimmed
        }
        guard !rawHex.isEmpty, let bytes = Data(hexString: rawHex) else {
            throw OfflineReceiptBuilderError.invalidSpendPublicKey("invalid hex payload")
        }
        let decoded = try decodePublicKeyMultihash(bytes)
        if let prefixedAlgorithm, decoded.algorithm != prefixedAlgorithm {
            throw OfflineReceiptBuilderError.invalidSpendPublicKey("algorithm prefix does not match multihash")
        }
        return decoded
    }

    private static func parseAlgorithmPrefix(_ value: String) -> SigningAlgorithm? {
        switch value.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() {
        case "ed25519":
            return .ed25519
        case "secp256k1":
            return .secp256k1
        case "ml-dsa", "mldsa":
            return .mlDsa
        case "sm2":
            return .sm2
        default:
            return nil
        }
    }

    private static func decodePublicKeyMultihash(_ bytes: Data) throws -> SpendPublicKey {
        let raw = [UInt8](bytes)
        let (functionCode, functionEnd) = try decodeVarint(raw, startIndex: 0)
        let (length, lengthEnd) = try decodeVarint(raw, startIndex: functionEnd)
        guard lengthEnd <= raw.count else {
            throw OfflineReceiptBuilderError.invalidSpendPublicKey("digest size not found")
        }
        let payload = Data(raw[lengthEnd...])
        guard payload.count == Int(length) else {
            throw OfflineReceiptBuilderError.invalidSpendPublicKey("digest size not equal to actual length")
        }
        let algorithm = try signingAlgorithm(multihashCode: functionCode)
        return SpendPublicKey(algorithm: algorithm, publicKey: payload)
    }

    private static func decodeVarint(_ bytes: [UInt8], startIndex: Int) throws -> (UInt64, Int) {
        var value: UInt64 = 0
        var shift: UInt64 = 0
        var index = startIndex
        while index < bytes.count {
            let byte = bytes[index]
            let chunk = UInt64(byte & 0x7F)
            if shift >= 64 {
                throw OfflineReceiptBuilderError.invalidSpendPublicKey("varint overflow")
            }
            value |= chunk << shift
            index += 1
            if (byte & 0x80) == 0 {
                return (value, index)
            }
            shift += 7
        }
        throw OfflineReceiptBuilderError.invalidSpendPublicKey("varint truncated")
    }

    private static func signingAlgorithm(multihashCode: UInt64) throws -> SigningAlgorithm {
        switch multihashCode {
        case 0xed:
            return .ed25519
        case 0xe7:
            return .secp256k1
        case 0xee:
            return .mlDsa
        case 0x1306:
            return .sm2
        default:
            let reason = String(format: "multihash code 0x%X", multihashCode)
            throw OfflineReceiptBuilderError.unsupportedSpendPublicKeyAlgorithm(reason)
        }
    }

    private static func encodeSpendPublicKeyString(algorithm: SigningAlgorithm, publicKey: Data) -> String {
        let functionCode = multihashFunctionCode(for: algorithm)
        var bytes = [UInt8]()
        bytes.append(contentsOf: encodeVarint(functionCode))
        bytes.append(contentsOf: encodeVarint(UInt64(publicKey.count)))
        bytes.append(contentsOf: publicKey)
        return Data(bytes).hexUppercased()
    }

    private static func multihashFunctionCode(for algorithm: SigningAlgorithm) -> UInt64 {
        switch algorithm {
        case .ed25519:
            return 0xed
        case .secp256k1:
            return 0xe7
        case .mlDsa:
            return 0xee
        case .sm2:
            return 0x1306
        }
    }

    private static func encodeVarint(_ value: UInt64) -> [UInt8] {
        var value = value
        var out: [UInt8] = []
        while true {
            var byte = UInt8(value & 0x7F)
            value >>= 7
            if value != 0 {
                byte |= 0x80
            }
            out.append(byte)
            if value == 0 {
                break
            }
        }
        return out
    }

    private static func verifyReceiptSignature(_ receipt: OfflineSpendReceipt) throws {
        let spendKey = try parseSpendPublicKey(receipt.senderCertificate.spendPublicKey)
        let payload = try receipt.signingBytes()
        let signature = receipt.senderSignature
        switch spendKey.algorithm {
        case .ed25519:
            guard #available(macOS 10.15, iOS 13.0, *) else {
                throw OfflineReceiptBuilderError.signatureVerificationUnavailable("ed25519 requires iOS 13/macOS 10.15")
            }
            let publicKey: Curve25519.Signing.PublicKey
            do {
                publicKey = try Curve25519.Signing.PublicKey(rawRepresentation: spendKey.publicKey)
            } catch {
                throw OfflineReceiptBuilderError.invalidSpendPublicKey("invalid ed25519 public key")
            }
            guard publicKey.isValidSignature(signature, for: payload) else {
                throw OfflineReceiptBuilderError.invalidSenderSignature
            }
        case .secp256k1:
            let verified = NoritoNativeBridge.shared.verifyDetached(
                algorithm: .secp256k1,
                publicKey: spendKey.publicKey,
                message: payload,
                signature: signature
            ) ?? NoritoNativeBridge.shared.secp256k1Verify(
                publicKey: spendKey.publicKey,
                message: payload,
                signature: signature
            )
            guard let verified else {
                throw OfflineReceiptBuilderError.signatureVerificationUnavailable("secp256k1 support is unavailable")
            }
            guard verified else {
                throw OfflineReceiptBuilderError.invalidSenderSignature
            }
        case .sm2:
            let distid = Sm2Keypair.defaultDistid()
            let verified = NoritoNativeBridge.shared.verifyDetached(
                algorithm: .sm2,
                publicKey: spendKey.publicKey,
                message: payload,
                signature: signature
            ) ?? NoritoNativeBridge.shared.sm2Verify(
                distid: distid,
                publicKey: spendKey.publicKey,
                message: payload,
                signature: signature
            )
            guard let verified else {
                throw OfflineReceiptBuilderError.signatureVerificationUnavailable("SM2 support is unavailable")
            }
            guard verified else {
                throw OfflineReceiptBuilderError.invalidSenderSignature
            }
        case .mlDsa:
            if let verified = NoritoNativeBridge.shared.verifyDetached(
                algorithm: .mlDsa,
                publicKey: spendKey.publicKey,
                message: payload,
                signature: signature
            ) {
                guard verified else {
                    throw OfflineReceiptBuilderError.invalidSenderSignature
                }
                return
            }
            guard let suite = inferMlDsaSuite(publicKey: spendKey.publicKey, signature: signature) else {
                throw OfflineReceiptBuilderError.signatureVerificationUnavailable("ML-DSA suite could not be inferred")
            }
            guard let verified = NoritoNativeBridge.shared.mldsaVerify(
                suiteId: suite.rawValue,
                publicKey: spendKey.publicKey,
                message: payload,
                signature: signature
            ) else {
                throw OfflineReceiptBuilderError.signatureVerificationUnavailable("ML-DSA support is unavailable")
            }
            guard verified else {
                throw OfflineReceiptBuilderError.invalidSenderSignature
            }
        }
    }

    private static func inferMlDsaSuite(publicKey: Data, signature: Data) -> MlDsaSuite? {
        for suite in MlDsaSuite.allCases {
            guard let params = NoritoNativeBridge.shared.mldsaParameters(suiteId: suite.rawValue) else {
                continue
            }
            if publicKey.count == params.publicKeyLength, signature.count == params.signatureLength {
                return suite
            }
        }
        return nil
    }

    private static func validateSnapshot(_ snapshot: OfflinePlatformTokenSnapshot?,
                                         metadata: [String: ToriiJSONValue]?) throws {
        guard let snapshot else { return }
        if snapshot.policy.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            throw OfflineReceiptBuilderError.invalidPlatformSnapshot("policy must be non-empty")
        }
        guard Data(base64Encoded: snapshot.attestationJwsB64) != nil else {
            throw OfflineReceiptBuilderError.invalidPlatformSnapshot("attestation_jws_b64 is not valid base64")
        }
        guard let policyValue = normalizedMetadataString(metadata?["android.integrity.policy"]) else {
            throw OfflineReceiptBuilderError.invalidPlatformSnapshot(
                "platform token snapshot cannot be used without android integrity metadata"
            )
        }
        guard let metadataPolicy = normalizeIntegrityPolicy(policyValue, allowNonTokenPolicies: true) else {
            throw OfflineReceiptBuilderError.invalidPlatformSnapshot(
                "platform token snapshot contains an unknown policy"
            )
        }
        guard let snapshotPolicy = normalizeIntegrityPolicy(snapshot.policy, allowNonTokenPolicies: false) else {
            throw OfflineReceiptBuilderError.invalidPlatformSnapshot(
                "platform token snapshot contains an unknown policy"
            )
        }
        guard snapshotPolicy == metadataPolicy else {
            throw OfflineReceiptBuilderError.invalidPlatformSnapshot(
                "platform token snapshot policy does not match allowance metadata"
            )
        }
    }

    private static func validatePlatformProof(_ receipt: OfflineSpendReceipt,
                                              chainId: String) throws {
        if chainId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            throw OfflineReceiptBuilderError.invalidPlatformProof("chainId must be non-empty")
        }
        switch receipt.platformProof {
        case .appleAppAttest(let proof):
            _ = try canonicalAppAttestKeyId(proof.keyId)
            try validateHash(proof.challengeHash, context: "apple_app_attest.challenge_hash")
            let expected = try receipt.challengeHash(chainId: chainId)
            guard proof.challengeHash == expected else {
                throw OfflineReceiptBuilderError.challengeHashMismatch
            }
        case .provisioned(let proof):
            guard let challenge = proof.challengeHashData else {
                throw OfflineReceiptBuilderError.invalidPlatformProof("provisioned challenge hash is missing")
            }
            try validateHash(challenge, context: "provisioned.challenge_hash")
            let expected = try receipt.challengeHash(chainId: chainId)
            guard challenge == expected else {
                throw OfflineReceiptBuilderError.challengeHashMismatch
            }
        case .androidMarkerKey(let proof):
            let derivedSeries = try markerSeries(for: proof.markerPublicKey)
            guard proof.series == derivedSeries else {
                throw OfflineReceiptBuilderError.invalidPlatformProof(
                    "marker series does not match marker_public_key"
                )
            }
            if let signature = proof.markerSignature, signature.count != 64 {
                throw OfflineReceiptBuilderError.invalidPlatformProof(
                    "marker_signature must be 64 bytes"
                )
            }
        }
    }

    private static func validateHash(_ bytes: Data, context: String) throws {
        guard bytes.count == 32 else {
            throw OfflineReceiptBuilderError.invalidPlatformProof("\(context) must be 32 bytes")
        }
        guard let last = bytes.last, (last & 1) == 1 else {
            throw OfflineReceiptBuilderError.invalidPlatformProof("\(context) must have LSB set")
        }
    }

    private static func validateCommitmentLength(_ commitment: Data) throws {
        guard commitment.count == 32 else {
            throw OfflineReceiptBuilderError.invalidCommitmentLength(commitment.count)
        }
    }

    private static func validateResultingCommitmentLength(_ commitment: Data) throws {
        guard commitment.count == 32 else {
            throw OfflineReceiptBuilderError.invalidResultingCommitmentLength(commitment.count)
        }
    }

    private static func validateAggregateProof(_ transfer: OfflineToOnlineTransfer) throws {
        guard let envelope = transfer.aggregateProof else { return }
        guard envelope.version == 1 else {
            throw OfflineReceiptBuilderError.aggregateProofVersionUnsupported(envelope.version)
        }
        do {
            let root = try OfflineReceiptPoseidon.receiptsRoot(receipts: transfer.receipts)
            guard root == envelope.receiptsRoot.bytes else {
                throw OfflineReceiptBuilderError.aggregateProofRootMismatch
            }
        } catch let error as OfflineReceiptBuilderError {
            throw error
        } catch {
            throw OfflineReceiptBuilderError.aggregateProofHashError(String(describing: error))
        }
    }

    private static func expectedScale(for certificate: OfflineWalletCertificate) throws -> Int {
        let allowance = try parseAmount(certificate.allowance.amount)
        let expected = allowance.scale
        _ = try parseAmount(certificate.policy.maxBalance, expectedScale: expected)
        _ = try parseAmount(certificate.policy.maxTxValue, expectedScale: expected)
        return expected
    }

    private static func parseAmount(_ value: String, expectedScale: Int? = nil) throws -> OfflineDecimal {
        let parsed: OfflineDecimal
        do {
            parsed = try OfflineDecimal.parse(value)
        } catch {
            throw OfflineReceiptBuilderError.invalidAmount(value)
        }
        if let expectedScale, parsed.scale != expectedScale {
            if expectedScale == 0 {
                throw OfflineReceiptBuilderError.fractionalAmount(value)
            }
            throw OfflineReceiptBuilderError.amountScaleMismatch(value: value,
                                                                 expected: expectedScale,
                                                                 actual: parsed.scale)
        }
        do {
            _ = try OfflineNorito.encodeNumeric(value)
        } catch {
            throw OfflineReceiptBuilderError.invalidAmount(value)
        }
        return parsed
    }

    private static func receiptSort(_ lhs: OfflineSpendReceipt, _ rhs: OfflineSpendReceipt) -> Bool {
        if lhs.platformProof.counter != rhs.platformProof.counter {
            return lhs.platformProof.counter < rhs.platformProof.counter
        }
        return lexicographicLess(lhs.txId, rhs.txId)
    }

    private enum CounterScopeKind: Equatable {
        case apple
        case androidMarker
        case androidProvisioned
    }

    private struct CounterScope: Equatable {
        let kind: CounterScopeKind
        let scope: String
    }

    private static let markerSeriesPrefix = "marker::"
    private static let provisionedSeriesPrefix = "provisioned::"

    private static func canonicalAppAttestKeyId(_ value: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw OfflineReceiptBuilderError.invalidPlatformProof("apple_app_attest.key_id must be non-empty")
        }
        guard trimmed == value else {
            throw OfflineReceiptBuilderError.invalidPlatformProof(
                "apple_app_attest.key_id must not contain whitespace"
            )
        }
        guard let decoded = Data(base64Encoded: value) else {
            throw OfflineReceiptBuilderError.invalidPlatformProof(
                "apple_app_attest.key_id must use standard base64 encoding"
            )
        }
        let canonical = decoded.base64EncodedString()
        guard canonical == value else {
            throw OfflineReceiptBuilderError.invalidPlatformProof(
                "apple_app_attest.key_id must use canonical base64 encoding"
            )
        }
        return canonical
    }

    private static func markerSeries(for markerPublicKey: Data) throws -> String {
        guard markerPublicKey.count == 65, markerPublicKey.first == 0x04 else {
            throw OfflineReceiptBuilderError.invalidPlatformProof(
                "marker_public_key must be a 65-byte SEC1 P-256 key"
            )
        }
        let hash = IrohaHash.hash(markerPublicKey)
        return markerSeriesPrefix + hash.hexLowercased()
    }

    private static func counterScope(for receipt: OfflineSpendReceipt) throws -> CounterScope {
        switch receipt.platformProof {
        case .appleAppAttest(let proof):
            let keyId = try canonicalAppAttestKeyId(proof.keyId)
            return CounterScope(kind: .apple, scope: keyId)
        case .androidMarkerKey(let proof):
            let series = try markerSeries(for: proof.markerPublicKey)
            guard proof.series == series else {
                throw OfflineReceiptBuilderError.invalidPlatformProof(
                    "marker series does not match marker_public_key"
                )
            }
            return CounterScope(kind: .androidMarker, scope: series)
        case .provisioned(let proof):
            let scope = try provisionedCounterScope(for: proof)
            return CounterScope(kind: .androidProvisioned, scope: scope)
        }
    }

    private static func provisionedCounterScope(for proof: AndroidProvisionedProof) throws -> String {
        let schema = proof.manifestSchema.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !schema.isEmpty else {
            throw OfflineReceiptBuilderError.invalidPlatformProof(
                "provisioned manifest_schema must be non-empty"
            )
        }
        let deviceId = try provisionedDeviceId(proof.deviceManifest)
        return provisionedSeriesPrefix + "\(schema)::\(deviceId)"
    }

    private static func provisionedDeviceId(_ manifest: [String: ToriiJSONValue]) throws -> String {
        guard let raw = normalizedMetadataString(manifest["android.provisioned.device_id"]) else {
            throw OfflineReceiptBuilderError.invalidPlatformProof(
                "android.provisioned.device_id is missing"
            )
        }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw OfflineReceiptBuilderError.invalidPlatformProof(
                "android.provisioned.device_id must be non-empty"
            )
        }
        return trimmed
    }

    private static func receiptsAreCanonical(_ receipts: [OfflineSpendReceipt]) -> Bool {
        guard receipts.count > 1 else { return true }
        for idx in 1..<receipts.count {
            if receiptSort(receipts[idx], receipts[idx - 1]) {
                return false
            }
        }
        return true
    }

    private static func lexicographicLess(_ lhs: Data, _ rhs: Data) -> Bool {
        let count = min(lhs.count, rhs.count)
        for idx in 0..<count {
            let left = lhs[lhs.index(lhs.startIndex, offsetBy: idx)]
            let right = rhs[rhs.index(rhs.startIndex, offsetBy: idx)]
            if left == right { continue }
            return left < right
        }
        return lhs.count < rhs.count
    }

    private static func normalizedMetadataString(_ value: ToriiJSONValue?) -> String? {
        guard let value else { return nil }
        switch value {
        case .string(let string):
            let trimmed = string.trimmingCharacters(in: .whitespacesAndNewlines)
            return trimmed.isEmpty ? nil : trimmed
        case .number(let number):
            guard number.isFinite else { return nil }
            return String(number)
        case .bool(let flag):
            return flag ? "true" : "false"
        default:
            return nil
        }
    }

    private static func normalizeIntegrityPolicy(_ value: String,
                                                 allowNonTokenPolicies: Bool) -> String? {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        let normalized = trimmed.lowercased().replacingOccurrences(of: "-", with: "_")
        switch normalized {
        case "play_integrity", "hms_safety_detect":
            return normalized
        case "marker_key", "provisioned":
            return allowNonTokenPolicies ? normalized : nil
        default:
            return nil
        }
    }
}

public extension OfflinePlatformProof {
    var counter: UInt64 {
        switch self {
        case .appleAppAttest(let proof):
            return proof.counter
        case .androidMarkerKey(let proof):
            return proof.counter
        case .provisioned(let proof):
            return proof.counter
        }
    }
}

private struct OfflineDecimal: Equatable {
    let digits: [UInt8]
    let scale: Int
    let isNegative: Bool

    var isZero: Bool {
        digits.allSatisfy { $0 == 0 }
    }

    static func zero(scale: Int) -> OfflineDecimal {
        OfflineDecimal(digits: [0], scale: scale, isNegative: false)
    }

    func compare(to other: OfflineDecimal) -> ComparisonResult {
        let targetScale = max(scale, other.scale)
        let lhs = scaledDigits(targetScale)
        let rhs = other.scaledDigits(targetScale)
        let lhsTrim = Self.trimLeadingZeros(lhs)
        let rhsTrim = Self.trimLeadingZeros(rhs)
        if lhsTrim.count != rhsTrim.count {
            return lhsTrim.count < rhsTrim.count ? .orderedAscending : .orderedDescending
        }
        for (a, b) in zip(lhsTrim, rhsTrim) {
            if a != b {
                return a < b ? .orderedAscending : .orderedDescending
            }
        }
        return .orderedSame
    }

    func equals(expected: String) -> Bool {
        (try? OfflineDecimal.parse(expected)) == self
    }

    func adding(_ other: OfflineDecimal) throws -> OfflineDecimal {
        guard !isNegative, !other.isNegative else {
            throw OfflineReceiptBuilderError.invalidAmount(render())
        }
        let targetScale = max(scale, other.scale)
        let lhs = scaledDigits(targetScale)
        let rhs = other.scaledDigits(targetScale)
        let sum = Self.addDigits(lhs, rhs)
        return OfflineDecimal(digits: sum, scale: targetScale, isNegative: false)
    }

    func render() -> String {
        let digitsStr = digits.map(String.init).joined()
        if scale == 0 {
            return digitsStr
        }
        if digitsStr.count <= scale {
            let zeros = String(repeating: "0", count: scale - digitsStr.count)
            return "0.\(zeros)\(digitsStr)"
        }
        let splitIndex = digitsStr.index(digitsStr.endIndex, offsetBy: -scale)
        let intPart = digitsStr[..<splitIndex]
        let fracPart = digitsStr[splitIndex...]
        return "\(intPart).\(fracPart)"
    }

    static func parse(_ raw: String) throws -> OfflineDecimal {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw OfflineReceiptBuilderError.invalidAmount(raw)
        }
        var index = trimmed.startIndex
        var negative = false
        if trimmed[index] == "-" {
            negative = true
            index = trimmed.index(after: index)
        } else if trimmed[index] == "+" {
            index = trimmed.index(after: index)
        }

        var digits: [UInt8] = []
        var scale = 0
        var seenDot = false
        while index < trimmed.endIndex {
            let ch = trimmed[index]
            if ch == "." {
                if seenDot {
                    throw OfflineReceiptBuilderError.invalidAmount(raw)
                }
                seenDot = true
                index = trimmed.index(after: index)
                continue
            }
            guard let value = ch.wholeNumberValue else {
                throw OfflineReceiptBuilderError.invalidAmount(raw)
            }
            digits.append(UInt8(value))
            if seenDot {
                scale += 1
            }
            index = trimmed.index(after: index)
        }

        guard !digits.isEmpty else {
            throw OfflineReceiptBuilderError.invalidAmount(raw)
        }
        if scale > 28 {
            throw OfflineReceiptBuilderError.invalidAmount(raw)
        }
        let trimmedDigits = trimLeadingZeros(digits)
        let isZero = trimmedDigits.count == 1 && trimmedDigits[0] == 0
        return OfflineDecimal(digits: trimmedDigits, scale: scale, isNegative: negative && !isZero)
    }

    private func scaledDigits(_ targetScale: Int) -> [UInt8] {
        guard targetScale >= scale else { return digits }
        let diff = targetScale - scale
        guard diff > 0 else { return digits }
        return digits + Array(repeating: 0, count: diff)
    }

    private static func addDigits(_ lhs: [UInt8], _ rhs: [UInt8]) -> [UInt8] {
        var result: [UInt8] = []
        var i = lhs.count - 1
        var j = rhs.count - 1
        var carry = 0
        while i >= 0 || j >= 0 || carry > 0 {
            let left = i >= 0 ? Int(lhs[i]) : 0
            let right = j >= 0 ? Int(rhs[j]) : 0
            let sum = left + right + carry
            result.append(UInt8(sum % 10))
            carry = sum / 10
            if i > 0 { i -= 1 } else { i = -1 }
            if j > 0 { j -= 1 } else { j = -1 }
        }
        return trimLeadingZeros(Array(result.reversed()))
    }

    private static func trimLeadingZeros(_ digits: [UInt8]) -> [UInt8] {
        var index = 0
        while index < digits.count - 1 && digits[index] == 0 {
            index += 1
        }
        return Array(digits[index...])
    }
}

private enum OfflineReceiptPoseidon {
    private static let fieldModulus: UInt64 = 0xffff_ffff_0000_0001
    private static let packedLimbBytes = 7
    private static let domainTag = Data("iroha.offline.receipt.merkle.v1".utf8)
    private static let accountIdTypeName = "iroha_data_model::account::model::AccountId"
    private static let platformProofTypeName = "iroha_data_model::offline::model::OfflinePlatformProof"

    private static let roundConstants: [[UInt64]] = [
        [0x0355_defb_099b_ed96, 0xfcc5_5fb9_6813_55e6, 0x9313_68d7_25f8_720a],
        [0x3b87_52f8_1e4e_100a, 0x53b8_64ee_53de_1e7c, 0xfccb_110a_3a83_9800],
        [0xfff0_e990_12e3_2773, 0xfca2_8ab2_898e_ff49, 0xb02b_1c85_4e99_b411],
        [0x9c45_a0b2_aded_dcdd, 0xab6b_d8d6_b495_9cfa, 0x32eb_07eb_92be_cca5],
        [0x0269_8cda_7aa8_674a, 0xa691_4454_ffbb_c85a, 0x3304_02af_7562_de69],
        [0x6b9d_eb68_9731_670a, 0xb6ac_ba18_2405_2a6e, 0x3ad1_81a6_aaed_99ec],
        [0x871f_2534_5b28_b8e9, 0xcd88_d358_315e_eec1, 0xe051_da57_7c86_be1d],
        [0x153a_d181_7c0c_125a, 0x9746_1b44_2e8a_be8c, 0x8659_1af5_1639_f5a9],
        [0xb5bb_c2a4_45ce_1d3c, 0x3cf8_7df3_7950_3557, 0x69c8_7ef4_b926_f914],
        [0x941b_ba1e_4ef7_e9f5, 0x2f9b_29b5_8901_fe31, 0x49c9_5be0_66d4_e00b],
        [0xcdb4_fa57_1ed1_6b7e, 0x8152_6ee1_686c_1130, 0x9afe_7622_fd18_7b9d],
        [0x1c6d_3d32_f566_6a4a, 0xbc71_4a50_9ed0_c9e2, 0xdbd4_4f43_3d9b_b989],
        [0x07f9_a90b_7e43_d95c, 0xbf08_30bc_8e11_be74, 0x97f8_2d76_58d4_24af],
        [0x232d_735b_d912_e717, 0x24c5_847c_a749_1e7a, 0x050e_45f9_ad01_8df7],
        [0x3d87_5e31_3e44_5623, 0xf0e7_65b8_e635_0e3a, 0x24e1_f73a_acb2_a5d8],
        [0xa260_8cac_4edd_cbf6, 0xbb84_373a_1401_2579, 0x153f_f026_357d_db2e],
        [0x31c5_1990_051b_2fce, 0xc432_6a6d_f134_2061, 0x110a_b731_271e_081d],
        [0xd7d4_ed98_3f73_8f7d, 0x04c2_243a_9d2d_2dd8, 0xec84_bd9f_09f4_a4ee],
        [0x459c_6a91_e831_0f7d, 0xbd54_48db_7b31_2560, 0x5540_5bc1_d484_1791],
        [0x9b29_aa29_d32f_f2ef, 0x19b4_882d_7d91_817d, 0xff96_eeff_0d35_d395],
        [0x725b_ee8d_2e75_2eba, 0xb778_a21c_6431_0114, 0x9991_cf61_41e2_878a],
        [0x5baf_8b23_f6d3_0359, 0x17d0_b954_a0c2_4e6d, 0x3050_c407_8053_b0d5],
        [0xf9f5_2a30_90ef_a474, 0xa598_d529_15f9_9806, 0xbf48_572b_618f_bd61],
        [0xa34b_fa5c_0b92_ec3e, 0xab11_1573_6057_c63c, 0x2f7d_4168_4601_4aaa],
        [0xe19f_2142_92d3_a546, 0xb48c_ed43_6104_2203, 0xb64f_6bfc_1cfc_bd92],
        [0xe0f4_8eec_d39b_7cbf, 0xc438_4583_c0d6_450d, 0x7460_cad3_3f43_6aca],
        [0xa69f_9eff_5caf_7223, 0x82ac_d34e_811e_340a, 0xecb3_d78f_9881_bedf],
        [0x9767_60db_1c85_079a, 0xa0d5_2712_8f52_31ca, 0xa882_3b80_503c_432b],
        [0xc41a_fca1_9eb7_8258, 0x33b3_0793_338e_26ee, 0x3369_d80a_26f2_6384],
        [0x4f52_37b4_3fe7_c081, 0x3e2e_28d3_7379_26d1, 0x9e9f_d944_9eaf_c854],
        [0xf6db_f8e5_0187_050f, 0x0883_a737_2f01_a7d2, 0xe85c_8cbe_6053_fa56],
        [0x43e6_6dea_535d_ce9d, 0xff74_45db_745c_243b, 0xde5d_71d4_d770_9458],
        [0xb14f_0f45_451d_99f9, 0xfcdf_81f9_7120_6fc3, 0x2508_429e_d0bb_abfa],
        [0x4081_15a6_f42a_57e8, 0xfcde_3356_9c03_693d, 0x8392_6cc6_0624_1abd],
        [0xfe61_9141_9d8e_0a84, 0x358d_7136_2d23_4258, 0x3bfb_29c6_4195_c104],
        [0x0ed6_b527_fe03_4ad7, 0xcd19_98fe_841a_698a, 0x6ab8_90eb_bfe7_72a9],
        [0x099a_748e_d541_5721, 0x254b_906a_5a91_bc8a, 0x4d51_30a3_0f0c_bb72],
        [0xd093_2ea3_fa64_d467, 0xbe72_249d_94ae_a218, 0x0621_5750_fd6a_29c5],
        [0xd790_3148_4db8_9846, 0xdd89_cad9_59ba_24a4, 0xdc04_6dfa_4487_1254],
        [0xb36f_522b_f4af_2ef6, 0x9279_6eae_3863_2589, 0x9796_6381_78a0_1e39],
        [0xbe2c_6a4f_4312_1eff, 0x0c20_942e_a145_116b, 0xb6d2_1c3a_f9d7_af98],
        [0x953a_f8c7_9c3e_074e, 0xc039_1530_65b4_a5c5, 0x9f51_afaf_415b_8a55],
        [0x6198_a829_2756_9707, 0xa093_66e5_bb6a_66c7, 0x68e7_10be_9451_ff59],
        [0x9226_cca1_e498_8ddd, 0x27e1_5510_153c_0224, 0x8d6a_3e1f_c7c1_97f2],
        [0xfaa9_29e2_5257_bc60, 0x6b37_c307_ac47_aa83, 0x907b_20a3_5cfe_4fc2],
        [0x5035_0676_383d_204e, 0x446c_05dd_253b_59e8, 0x0f31_889e_23c5_d5ea],
        [0x38ec_3e1c_2e66_7720, 0x026f_df27_b554_ff10, 0x1ca8_2d95_c6fa_fd39],
        [0xddd0_bc05_6342_e191, 0x2d31_5fb4_4b04_ac30, 0xfd4b_193b_ae11_b94b],
        [0x086d_f800_b348_9093, 0xf30f_ae76_04b9_87e2, 0x10a1_9793_fe69_ebe8],
        [0x0c77_141b_7edf_0449, 0x5ea8_7577_9328_deb1, 0x0b67_4c79_767e_421f],
        [0x4a5a_726d_82f3_258e, 0x09e8_c7a8_be47_b1de, 0xfffa_2ac0_8527_6013],
        [0xc091_3932_4aa9_d4f9, 0xb51a_2320_5924_fd17, 0xdcc8_8949_49e0_2da4],
        [0x2274_8352_9222_ad6e, 0x6533_7b21_01df_c3d8, 0xe19d_fd7c_78aa_acc7],
        [0xf1b9_8df3_40a1_6ca6, 0x18d5_4312_3d27_950d, 0xc0c1_6b9b_6007_e396],
        [0xb90b_c3c0_d060_5c8c, 0xde10_8ac1_d6c0_bdca, 0x5ee8_196b_6114_6783],
        [0x2aab_a85c_6bbc_f12f, 0x02ff_605b_7384_3154, 0x2c37_fc5e_4dfe_bb1e],
        [0xcb1c_a352_a46e_4d33, 0x187d_0f76_fc0f_b4a3, 0xaead_cdfe_0a66_b1f1],
        [0xc125_32bf_b67e_3436, 0x4b40_3b0b_f220_033e, 0xe0de_c101_5d69_cd0b],
        [0x5d3e_b71f_9ef3_0c4a, 0xf82d_9fbc_df53_2aeb, 0xa362_551d_86be_bd87],
        [0x6823_e258_0285_1126, 0x189c_e62b_7765_0805, 0xefc2_61ff_a36b_f041],
        [0x4788_6351_1cba_f173, 0x4f4b_d042_c56b_7936, 0x5b4c_f8cc_8584_ca9a],
        [0xcc94_4e5f_7606_3e0c, 0xd29a_0b00_2c78_3ca7, 0x2f59_efce_5cde_182b],
        [0x93a0_767d_9186_685c, 0xee25_01a7_61ec_f4e5, 0xe751_4fa4_8b14_5686],
        [0x5e0f_189e_8c61_d97c, 0xebe9_bd9b_ef0f_5443, 0xd6cf_e2a7_6189_672a],
        [0x38ff_8f40_252c_1ab7, 0x3cf9_c4c3_804d_8166, 0x512f_1e3a_c8d0_ffe5],
    ]

    private static let mds: [[UInt64]] = [
        [0x9825_13a2_3d22_b592, 0xa311_5db8_cf1d_9c90, 0x46ba_684b_9eee_84b7],
        [0xbe3d_ce25_491d_b768, 0xfb0a_6f73_1943_519f, 0xfce5_bd95_3cde_1896],
        [0xe624_719c_41eb_1a09, 0xd222_1b0f_1aa2_ebc4, 0x1ab5_e60d_03ad_44bc],
    ]

    static func receiptsRoot(receipts: [OfflineSpendReceipt]) throws -> Data {
        guard !receipts.isEmpty else {
            return Data(repeating: 0, count: 32)
        }
        if let native = try? receiptsRootNative(receipts: receipts) {
            return native
        }
        var leaves: [LeafEntry] = []
        leaves.reserveCapacity(receipts.count)
        for receipt in receipts {
            let digest = try leafDigest(receipt)
            leaves.append(LeafEntry(counter: receipt.platformProof.counter,
                                    txId: receipt.txId,
                                    digest: digest))
        }
        leaves.sort { lhs, rhs in
            if lhs.counter != rhs.counter {
                return lhs.counter < rhs.counter
            }
            return lexicographicLess(lhs.txId, rhs.txId)
        }
        var level = leaves.map { $0.digest }
        while level.count > 1 {
            if level.count % 2 == 1 {
                level.append(0)
            }
            var next: [UInt64] = []
            next.reserveCapacity(level.count / 2)
            var index = 0
            while index < level.count {
                let left = level[index]
                let right = level[index + 1]
                next.append(hashNode(left, right))
                index += 2
            }
            level = next
        }
        return digestBytes(from: level[0])
    }

    private static func receiptsRootNative(receipts: [OfflineSpendReceipt]) throws -> Data? {
        let values = try receipts.map { try $0.toriiJSON() }
        let payload = ToriiJSONValue.array(values)
        let data = try payload.encodedData()
        return try NoritoNativeBridge.shared.offlineReceiptsRoot(receiptsJson: data)
    }

    private struct LeafEntry {
        let counter: UInt64
        let txId: Data
        let digest: UInt64
    }

    private static func leafDigest(_ receipt: OfflineSpendReceipt) throws -> UInt64 {
        let txField = reduceBytes(receipt.txId)
        let (mantissaField, scaleField) = try numericFields(receipt.amount)
        let counterField = reduceU64(receipt.platformProof.counter)
        let receiverPayload = OfflineNorito.encodeString(receipt.to)
        let receiverBytes = OfflineNorito.wrap(typeName: accountIdTypeName, payload: receiverPayload)
        let receiverHash = IrohaHash.hash(receiverBytes)
        let receiverField = reduceBytes(receiverHash)
        let invoiceHash = IrohaHash.hash(Data(receipt.invoiceId.utf8))
        let invoiceField = reduceBytes(invoiceHash)
        let proofPayload = try receipt.platformProof.noritoPayload()
        let proofBytes = OfflineNorito.wrap(typeName: platformProofTypeName, payload: proofPayload)
        let proofHash = IrohaHash.hash(proofBytes)
        let proofField = reduceBytes(proofHash)
        var sponge = PoseidonSponge()
        absorbTag(&sponge, tag: domainTag)
        sponge.absorb(txField)
        sponge.absorb(mantissaField)
        sponge.absorb(scaleField)
        sponge.absorb(counterField)
        sponge.absorb(receiverField)
        sponge.absorb(invoiceField)
        sponge.absorb(proofField)
        return sponge.squeeze()
    }

    private static func numericFields(_ value: String) throws -> (UInt64, UInt64) {
        let decimal = try OfflineDecimal.parse(value)
        if decimal.isNegative {
            throw OfflineReceiptBuilderError.invalidAmount(value)
        }
        var acc: UInt64 = 0
        for digit in decimal.digits {
            acc = add(mul(acc, 10), UInt64(digit))
        }
        let scaleField = reduceU64(UInt64(decimal.scale))
        return (acc, scaleField)
    }

    private static func digestBytes(from field: UInt64) -> Data {
        var bytes = Data(repeating: 0, count: 32)
        let fieldBytes = field.bigEndian
        withUnsafeBytes(of: fieldBytes) { buffer in
            bytes.replaceSubrange(24..<32, with: buffer)
        }
        return bytes
    }

    private static func reduceBytes(_ bytes: Data) -> UInt64 {
        var acc: UInt64 = 0
        for byte in bytes {
            acc = add(mul(acc, 256), UInt64(byte))
        }
        return acc
    }

    private static func reduceU64(_ value: UInt64) -> UInt64 {
        if value >= fieldModulus {
            return value - fieldModulus
        }
        return value
    }

    private static func hashNode(_ left: UInt64, _ right: UInt64) -> UInt64 {
        var sponge = PoseidonSponge()
        absorbTag(&sponge, tag: domainTag)
        sponge.absorb(left)
        sponge.absorb(right)
        return sponge.squeeze()
    }

    private static func absorbTag(_ sponge: inout PoseidonSponge, tag: Data) {
        sponge.absorb(UInt64(tag.count) % fieldModulus)
        for limb in packBytes(tag) {
            sponge.absorb(limb)
        }
    }

    private static func packBytes(_ bytes: Data) -> [UInt64] {
        guard !bytes.isEmpty else { return [] }
        var limbs: [UInt64] = []
        limbs.reserveCapacity((bytes.count + packedLimbBytes - 1) / packedLimbBytes)
        var offset = 0
        while offset < bytes.count {
            let remaining = bytes.count - offset
            let take = min(remaining, packedLimbBytes)
            var limb: UInt64 = 0
            for idx in 0..<take {
                limb |= UInt64(bytes[offset + idx]) << (8 * idx)
            }
            limbs.append(limb)
            offset += take
        }
        return limbs
    }

    private struct PoseidonSponge {
        private var state: [UInt64] = [0, 0, 0]
        private var rateIndex = 0
        private var finalised = false

        mutating func absorb(_ element: UInt64) {
            precondition(!finalised, "cannot absorb into a finalised sponge")
            state[rateIndex] = add(state[rateIndex], element)
            rateIndex += 1
            if rateIndex == 2 {
                permute(&state)
                rateIndex = 0
            }
        }

        mutating func squeeze() -> UInt64 {
            ensureFinalised()
            return state[0]
        }

        private mutating func ensureFinalised() {
            guard !finalised else { return }
            absorb(1)
            while rateIndex != 0 {
                absorb(0)
            }
            finalised = true
        }
    }

    private static func permute(_ state: inout [UInt64]) {
        var round = 0
        for _ in 0..<4 {
            fullRound(&state, roundConstants[round])
            round += 1
        }
        for _ in 0..<57 {
            partialRound(&state, roundConstants[round])
            round += 1
        }
        for _ in 0..<4 {
            fullRound(&state, roundConstants[round])
            round += 1
        }
    }

    private static func fullRound(_ state: inout [UInt64], _ constants: [UInt64]) {
        for idx in 0..<state.count {
            state[idx] = pow5(add(state[idx], constants[idx]))
        }
        applyMds(&state)
    }

    private static func partialRound(_ state: inout [UInt64], _ constants: [UInt64]) {
        for idx in 0..<state.count {
            state[idx] = add(state[idx], constants[idx])
        }
        state[0] = pow5(state[0])
        applyMds(&state)
    }

    private static func applyMds(_ state: inout [UInt64]) {
        var next = [UInt64](repeating: 0, count: 3)
        for row in 0..<3 {
            var acc: UInt64 = 0
            for col in 0..<3 {
                acc = add(acc, mul(mds[row][col], state[col]))
            }
            next[row] = acc
        }
        state = next
    }

    private static func pow5(_ value: UInt64) -> UInt64 {
        let square = mul(value, value)
        let fourth = mul(square, square)
        return mul(fourth, value)
    }

    private static func add(_ a: UInt64, _ b: UInt64) -> UInt64 {
        let (sum, overflow) = a.addingReportingOverflow(b)
        var result = sum
        if overflow {
            result &-= fieldModulus
        }
        if result >= fieldModulus {
            result &-= fieldModulus
        }
        return result
    }

    private static func mul(_ a: UInt64, _ b: UInt64) -> UInt64 {
        let product = a.multipliedFullWidth(by: b)
        return reduceWide(lo: product.low, hi: product.high)
    }

    private static func reduceWide(lo: UInt64, hi: UInt64) -> UInt64 {
        let hiLo = hi & 0xffff_ffff
        let hiHi = hi >> 32
        var acc = SignedInt128(lo)
        acc.add(hiLo << 32)
        acc.subtract(hiLo)
        acc.subtract(hiHi)
        let modulus = SignedInt128(fieldModulus)
        if acc < .zero {
            acc.add(fieldModulus)
            if acc < .zero {
                acc.add(fieldModulus)
            }
        }
        if acc >= modulus {
            acc.subtract(fieldModulus)
            if acc >= modulus {
                acc.subtract(fieldModulus)
            }
        }
        return acc.low
    }

    private static func lexicographicLess(_ lhs: Data, _ rhs: Data) -> Bool {
        let count = min(lhs.count, rhs.count)
        for idx in 0..<count {
            let left = lhs[lhs.index(lhs.startIndex, offsetBy: idx)]
            let right = rhs[rhs.index(rhs.startIndex, offsetBy: idx)]
            if left == right { continue }
            return left < right
        }
        return lhs.count < rhs.count
    }
}

private struct SignedInt128: Comparable {
    static let zero = SignedInt128(UInt64(0))

    var high: Int64
    var low: UInt64

    init(_ value: UInt64) {
        high = 0
        low = value
    }

    init(_ value: Int64) {
        high = value < 0 ? -1 : 0
        low = UInt64(bitPattern: value)
    }

    mutating func add(_ value: UInt64) {
        let (newLow, overflow) = low.addingReportingOverflow(value)
        low = newLow
        if overflow {
            high &+= 1
        }
    }

    mutating func subtract(_ value: UInt64) {
        let (newLow, overflow) = low.subtractingReportingOverflow(value)
        low = newLow
        if overflow {
            high &-= 1
        }
    }

    static func < (lhs: SignedInt128, rhs: SignedInt128) -> Bool {
        if lhs.high != rhs.high {
            return lhs.high < rhs.high
        }
        return lhs.low < rhs.low
    }
}
