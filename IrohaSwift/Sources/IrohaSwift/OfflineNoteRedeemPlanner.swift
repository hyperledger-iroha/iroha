import Foundation

public enum OfflineNoteRedeemPlannerError: Error, LocalizedError, Equatable {
    case accountMismatch
    case assetAccountMismatch
    case commitmentMismatch(expected: String, actual: String)
    case invalidAmount(String)
    case amountNotPositive(String)
    case insufficientAmount(requested: String, available: String)
    case exactRedeemRequired
    case emptyBearerAuditTrail
    case outputMismatch
    case invalidField(String)

    public var errorDescription: String? {
        switch self {
        case .accountMismatch:
            return "Offline note account does not match its key certificate."
        case .assetAccountMismatch:
            return "Offline note asset owner does not match the note account."
        case let .commitmentMismatch(expected, actual):
            return "Offline note commitment mismatch: expected \(expected), got \(actual)."
        case let .invalidAmount(amount):
            return "Offline note amount is invalid: \(amount)."
        case let .amountNotPositive(amount):
            return "Offline note amount must be positive: \(amount)."
        case let .insufficientAmount(requested, available):
            return "Offline note amount \(requested) exceeds available note amount \(available)."
        case .exactRedeemRequired:
            return "Offline note partial redeem amount equals the source note amount; use exact redeem."
        case .emptyBearerAuditTrail:
            return "Offline bearer note is missing the audit trail required for defunding."
        case .outputMismatch:
            return "Offline note split output does not match the redemption source."
        case let .invalidField(field):
            return "Offline note redeem planner field \(field) is invalid."
        }
    }
}

public struct OfflineNoteOwnedInput: Equatable, Sendable {
    public let chainId: String
    public let accountId: String
    public let assetId: String
    public let amount: String
    public let keyCertificate: OfflineNoteKeyCertificate
    public let noteCommitment: Data
    public let noteSecret: Data
    public let origin: OfflineNoteCommitmentOrigin?
    public let bearerAuditTrail: [OfflineNoteAuditBundle]

    public init(chainId: String,
                accountId: String,
                assetId: String,
                amount: String,
                keyCertificate: OfflineNoteKeyCertificate,
                noteCommitment: Data,
                noteSecret: Data,
                origin: OfflineNoteCommitmentOrigin? = nil,
                bearerAuditTrail: [OfflineNoteAuditBundle] = []) throws {
        let checkedChainId = try Self.exactNonEmptyField(chainId, field: "chain_id")
        let checkedAccountId = try Self.exactNonEmptyField(accountId, field: "account_id")
        guard checkedAccountId == keyCertificate.accountId else {
            throw OfflineNoteRedeemPlannerError.accountMismatch
        }
        let normalizedAssetId = try OfflineNorito.canonicalAssetIdLiteral(assetId)
        if let assetAccountId = Self.assetAccountId(normalizedAssetId),
           assetAccountId != checkedAccountId {
            throw OfflineNoteRedeemPlannerError.assetAccountMismatch
        }
        let normalizedAmount: String
        do {
            let parsed = try OfflineNorito.parseCanonicalNumeric(amount)
            guard parsed.compared(to: try OfflineNorito.parseCanonicalNumeric("0")) == .orderedDescending else {
                throw OfflineNoteRedeemPlannerError.amountNotPositive(amount)
            }
            normalizedAmount = parsed.canonicalString
        } catch let error as OfflineNoteRedeemPlannerError {
            throw error
        } catch {
            throw OfflineNoteRedeemPlannerError.invalidAmount(amount)
        }
        try OfflineNoteValidation.validateHash(noteCommitment, field: "note_commitment")
        try OfflineNoteValidation.validateRandomBytes(noteSecret, field: "note_secret")
        self.chainId = checkedChainId
        self.accountId = checkedAccountId
        self.assetId = normalizedAssetId
        self.amount = normalizedAmount
        self.keyCertificate = keyCertificate
        self.noteCommitment = noteCommitment
        self.noteSecret = noteSecret
        self.origin = origin
        self.bearerAuditTrail = Self.normalizedAuditTrail(bearerAuditTrail)
        _ = try issuedClaim()
        if let origin {
            let expected = try OfflineNoteCommitmentPreimage(
                chainId: checkedChainId,
                ownerKeyCertificatePayloadHash: keyCertificate.payloadHash(),
                assetId: normalizedAssetId,
                amount: normalizedAmount,
                noteSecret: noteSecret,
                origin: origin
            ).deriveNoteCommitment()
            guard expected == noteCommitment else {
                throw OfflineNoteRedeemPlannerError.commitmentMismatch(
                    expected: expected.hexLowercased(),
                    actual: noteCommitment.hexLowercased()
                )
            }
        }
    }

    public var noteCommitmentHex: String {
        noteCommitment.hexLowercased()
    }

    public func issuedClaim() throws -> OfflineNoteIssuedClaim {
        try OfflineNoteIssuedClaim(
            noteCommitment: noteCommitment,
            keyCertificatePayloadHash: keyCertificate.payloadHash(),
            assetId: assetId,
            amount: amount
        )
    }

    public func inputNullifier() throws -> Data {
        try OfflineNoteInputNullifierPreimage(
            chainId: chainId,
            sourceNoteCommitment: noteCommitment,
            ownerKeyCertificatePayloadHash: keyCertificate.payloadHash(),
            noteSecret: noteSecret
        ).deriveInputNullifier()
    }

    private static func assetAccountId(_ assetId: String) -> String? {
        guard let separator = assetId.lastIndex(of: "#"),
              separator < assetId.index(before: assetId.endIndex) else {
            return nil
        }
        return String(assetId[assetId.index(after: separator)...])
    }

    private static func exactNonEmptyField(_ value: String, field: String) throws -> String {
        guard !value.isEmpty,
              value.trimmingCharacters(in: .whitespacesAndNewlines) == value else {
            throw OfflineNoteRedeemPlannerError.invalidField(field)
        }
        return value
    }

    static func normalizedAuditTrail(_ audits: [OfflineNoteAuditBundle]) -> [OfflineNoteAuditBundle] {
        var seen: Set<String> = []
        var result: [OfflineNoteAuditBundle] = []
        for audit in audits {
            let key = audit.tokenId.hexLowercased()
            guard seen.insert(key).inserted else { continue }
            result.append(audit)
        }
        return result
    }
}

public struct OfflineNoteOwnedOutput: Equatable, Sendable {
    public let chainId: String
    public let accountId: String
    public let assetId: String
    public let amount: String
    public let keyCertificate: OfflineNoteKeyCertificate
    public let noteCommitment: Data
    public let noteSecret: Data
    public let origin: OfflineNoteCommitmentOrigin

    public init(chainId: String,
                accountId: String,
                assetId: String,
                amount: String,
                keyCertificate: OfflineNoteKeyCertificate,
                noteSecret: Data,
                origin: OfflineNoteCommitmentOrigin) throws {
        let checkedChainId = try Self.exactNonEmptyField(chainId, field: "chain_id")
        let checkedAccountId = try Self.exactNonEmptyField(accountId, field: "account_id")
        let normalizedAmount = try OfflineNorito.parseCanonicalNumeric(amount).canonicalString
        let normalizedAssetId = try OfflineNorito.canonicalAssetIdLiteral(assetId)
        guard checkedAccountId == keyCertificate.accountId else {
            throw OfflineNoteRedeemPlannerError.accountMismatch
        }
        if let assetAccountId = Self.assetAccountId(normalizedAssetId),
           assetAccountId != checkedAccountId {
            throw OfflineNoteRedeemPlannerError.assetAccountMismatch
        }
        let commitment = try OfflineNoteCommitmentPreimage(
            chainId: checkedChainId,
            ownerKeyCertificatePayloadHash: keyCertificate.payloadHash(),
            assetId: normalizedAssetId,
            amount: normalizedAmount,
            noteSecret: noteSecret,
            origin: origin
        ).deriveNoteCommitment()
        self.chainId = checkedChainId
        self.accountId = checkedAccountId
        self.assetId = normalizedAssetId
        self.amount = normalizedAmount
        self.keyCertificate = keyCertificate
        self.noteCommitment = commitment
        self.noteSecret = noteSecret
        self.origin = origin
        _ = try issuedClaim()
    }

    public var noteCommitmentHex: String {
        noteCommitment.hexLowercased()
    }

    public func auditOutputClaim() throws -> OfflineNoteAuditOutputClaim {
        try OfflineNoteAuditOutputClaim(
            noteCommitment: noteCommitment,
            keyCertificate: keyCertificate,
            assetId: assetId,
            amount: amount
        )
    }

    public func issuedClaim() throws -> OfflineNoteIssuedClaim {
        try OfflineNoteIssuedClaim(
            noteCommitment: noteCommitment,
            keyCertificatePayloadHash: keyCertificate.payloadHash(),
            assetId: assetId,
            amount: amount
        )
    }

    public func inputNullifier() throws -> Data {
        try OfflineNoteInputNullifierPreimage(
            chainId: chainId,
            sourceNoteCommitment: noteCommitment,
            ownerKeyCertificatePayloadHash: keyCertificate.payloadHash(),
            noteSecret: noteSecret
        ).deriveInputNullifier()
    }

    private static func assetAccountId(_ assetId: String) -> String? {
        guard let separator = assetId.lastIndex(of: "#"),
              separator < assetId.index(before: assetId.endIndex) else {
            return nil
        }
        return String(assetId[assetId.index(after: separator)...])
    }

    private static func exactNonEmptyField(_ value: String, field: String) throws -> String {
        guard !value.isEmpty,
              value.trimmingCharacters(in: .whitespacesAndNewlines) == value else {
            throw OfflineNoteRedeemPlannerError.invalidField(field)
        }
        return value
    }
}

public struct OfflineNoteRedeemDraft: Equatable, Sendable {
    public let input: OfflineNoteOwnedInput
    public let inputNullifier: Data
    public let redemption: OfflineNoteRedeem
    public let instanceValues: OfflineNoteInstanceValues
}

public struct OfflineNotePartialRedeemDraft: Equatable, Sendable {
    public let sourceInput: OfflineNoteOwnedInput
    public let redeemAmount: String
    public let changeAmount: String
    public let paymentRequestId: String
    public let createdAtMs: UInt64
    public let tokenNonce: Data
    public let sourceInputNullifier: Data
    public let redeemOutput: OfflineNoteOwnedOutput
    public let changeOutput: OfflineNoteOwnedOutput
    public let audit: OfflineNoteAuditBundle
    public let auditInstanceValues: OfflineNoteInstanceValues
    public let redemption: OfflineNoteRedeem
    public let redemptionInstanceValues: OfflineNoteInstanceValues
}

public struct OfflineNoteRedeemPlan: Equatable, Sendable {
    public let redemption: OfflineNoteRedeem
    public let bearerAuditTrail: [OfflineNoteAuditBundle]
    public let splitPaymentToken: OfflineNotePaymentToken?
    public let redeemOutput: OfflineNoteOwnedOutput?
    public let changeOutput: OfflineNoteOwnedOutput?
}

public enum OfflineNoteRedeemPlanner {
    public static func exactRedeemDraft(
        input: OfflineNoteOwnedInput,
        recipient: String? = nil
    ) throws -> OfflineNoteRedeemDraft {
        let inputNullifier = try input.inputNullifier()
        let redemption = try redemptionDraft(
            sourceNoteCommitment: input.noteCommitment,
            inputNullifiers: [inputNullifier],
            senderKeyCertificate: input.keyCertificate,
            recipient: recipient ?? input.accountId,
            assetId: input.assetId,
            amount: input.amount
        )
        return try OfflineNoteRedeemDraft(
            input: input,
            inputNullifier: inputNullifier,
            redemption: redemption,
            instanceValues: OfflineNoteInstanceBuilder.redeemInstanceValues(for: redemption)
        )
    }

    public static func partialRedeemDraft(
        input: OfflineNoteOwnedInput,
        redeemAmount: String,
        recipient: String? = nil,
        paymentRequestId: String,
        createdAtMs: UInt64,
        tokenNonce: Data,
        redeemNoteSecret: Data,
        changeNoteSecret: Data
    ) throws -> OfflineNotePartialRedeemDraft {
        let sourceAmount = try positiveAmount(input.amount)
        let requestedAmount = try positiveAmount(redeemAmount)
        switch requestedAmount.compared(to: sourceAmount) {
        case .orderedAscending:
            break
        case .orderedSame:
            throw OfflineNoteRedeemPlannerError.exactRedeemRequired
        case .orderedDescending:
            throw OfflineNoteRedeemPlannerError.insufficientAmount(
                requested: requestedAmount.canonicalString,
                available: sourceAmount.canonicalString
            )
        }
        let changeAmount = try sourceAmount.subtracting(requestedAmount, maxBytes: 64)
        guard changeAmount.compared(to: try positiveAmount("0", allowZero: true)) == .orderedDescending else {
            throw OfflineNoteRedeemPlannerError.invalidAmount(changeAmount.canonicalString)
        }
        let redeemOrigin = try OfflineNoteCommitmentOrigin.p2pOutput(
            OfflineNoteP2pOutputOrigin(paymentRequestId: paymentRequestId, outputIndex: 0)
        )
        let changeOrigin = try OfflineNoteCommitmentOrigin.p2pOutput(
            OfflineNoteP2pOutputOrigin(paymentRequestId: paymentRequestId, outputIndex: 1)
        )
        let redeemOutput = try OfflineNoteOwnedOutput(
            chainId: input.chainId,
            accountId: input.accountId,
            assetId: input.assetId,
            amount: requestedAmount.canonicalString,
            keyCertificate: input.keyCertificate,
            noteSecret: redeemNoteSecret,
            origin: redeemOrigin
        )
        let changeOutput = try OfflineNoteOwnedOutput(
            chainId: input.chainId,
            accountId: input.accountId,
            assetId: input.assetId,
            amount: changeAmount.canonicalString,
            keyCertificate: input.keyCertificate,
            noteSecret: changeNoteSecret,
            origin: changeOrigin
        )
        let sourceInputNullifier = try input.inputNullifier()
        let outputClaims = try [redeemOutput.auditOutputClaim(), changeOutput.auditOutputClaim()]
        let outputCommitments = outputClaims.map(\.noteCommitment)
        let tokenId = try OfflineNotePaymentTokenIdPreimage(
            chainId: input.chainId,
            paymentRequestId: paymentRequestId,
            createdAtMs: createdAtMs,
            tokenNonce: tokenNonce,
            senderKeyCertificatePayloadHash: input.keyCertificate.payloadHash(),
            inputNullifiers: [sourceInputNullifier],
            outputCommitments: outputCommitments
        ).derivePaymentTokenId()
        let audit = try auditDraft(
            tokenId: tokenId,
            senderKeyCertificate: input.keyCertificate,
            inputNullifiers: [sourceInputNullifier],
            inputClaims: [input.issuedClaim()],
            outputCommitments: outputCommitments,
            outputClaims: outputClaims
        )
        let redemption = try redemptionDraft(
            sourceNoteCommitment: redeemOutput.noteCommitment,
            inputNullifiers: [redeemOutput.inputNullifier()],
            senderKeyCertificate: input.keyCertificate,
            recipient: recipient ?? input.accountId,
            assetId: input.assetId,
            amount: requestedAmount.canonicalString
        )
        return try OfflineNotePartialRedeemDraft(
            sourceInput: input,
            redeemAmount: requestedAmount.canonicalString,
            changeAmount: changeAmount.canonicalString,
            paymentRequestId: paymentRequestId,
            createdAtMs: createdAtMs,
            tokenNonce: tokenNonce,
            sourceInputNullifier: sourceInputNullifier,
            redeemOutput: redeemOutput,
            changeOutput: changeOutput,
            audit: audit,
            auditInstanceValues: OfflineNoteInstanceBuilder.auditInstanceValues(for: audit),
            redemption: redemption,
            redemptionInstanceValues: OfflineNoteInstanceBuilder.redeemInstanceValues(for: redemption)
        )
    }

    public static func finalizeExactRedeem(
        _ draft: OfflineNoteRedeemDraft,
        recursiveProof: OfflineNoteRecursiveProof
    ) throws -> OfflineNoteRedeemPlan {
        if case .p2pOutput = draft.input.origin, draft.input.bearerAuditTrail.isEmpty {
            throw OfflineNoteRedeemPlannerError.emptyBearerAuditTrail
        }
        let redemption = try draft.redemption.replacingRecursiveProof(recursiveProof)
        try redemption.validateProofBinding()
        return OfflineNoteRedeemPlan(
            redemption: redemption,
            bearerAuditTrail: draft.input.bearerAuditTrail,
            splitPaymentToken: nil,
            redeemOutput: nil,
            changeOutput: nil
        )
    }

    public static func finalizePartialRedeem(
        _ draft: OfflineNotePartialRedeemDraft,
        auditProof: OfflineNoteRecursiveProof,
        redeemProof: OfflineNoteRecursiveProof
    ) throws -> OfflineNoteRedeemPlan {
        let audit = try draft.audit.replacingRecursiveProof(auditProof)
        try audit.validateProofBinding()
        let redemption = try draft.redemption.replacingRecursiveProof(redeemProof)
        try redemption.validateProofBinding()
        let redeemedIssuedClaim = try redemption.issuedClaim()
        let splitIssuedClaim = try draft.redeemOutput.issuedClaim()
        guard redeemedIssuedClaim == splitIssuedClaim,
              redemption.sourceNoteCommitment == draft.redeemOutput.noteCommitment,
              redemption.amount == draft.redeemAmount else {
            throw OfflineNoteRedeemPlannerError.outputMismatch
        }
        let bearerAuditTrail = OfflineNoteOwnedInput.normalizedAuditTrail(
            draft.sourceInput.bearerAuditTrail + [audit]
        )
        guard bearerAuditTrail.last == audit else {
            throw OfflineNoteRedeemPlannerError.emptyBearerAuditTrail
        }
        let splitPaymentToken = OfflineNotePaymentToken(
            chainId: draft.sourceInput.chainId,
            paymentRequestId: draft.paymentRequestId,
            tokenNonce: draft.tokenNonce,
            tokenId: audit.tokenId,
            audit: audit,
            bearerAuditTrail: bearerAuditTrail,
            createdAtMs: draft.createdAtMs
        )
        return OfflineNoteRedeemPlan(
            redemption: redemption,
            bearerAuditTrail: bearerAuditTrail,
            splitPaymentToken: splitPaymentToken,
            redeemOutput: draft.redeemOutput,
            changeOutput: draft.changeOutput
        )
    }

    private static func positiveAmount(
        _ value: String,
        allowZero: Bool = false
    ) throws -> OfflineCanonicalNumeric {
        do {
            let parsed = try OfflineNorito.parseCanonicalNumeric(value)
            let zero = try OfflineNorito.parseCanonicalNumeric("0")
            let comparison = parsed.compared(to: zero)
            if allowZero {
                guard comparison != .orderedAscending else {
                    throw OfflineNoteRedeemPlannerError.invalidAmount(value)
                }
            } else {
                guard comparison == .orderedDescending else {
                    throw OfflineNoteRedeemPlannerError.amountNotPositive(value)
                }
            }
            return parsed
        } catch let error as OfflineNoteRedeemPlannerError {
            throw error
        } catch {
            throw OfflineNoteRedeemPlannerError.invalidAmount(value)
        }
    }

    private static func redemptionDraft(
        sourceNoteCommitment: Data,
        inputNullifiers: [Data],
        senderKeyCertificate: OfflineNoteKeyCertificate,
        recipient: String,
        assetId: String,
        amount: String
    ) throws -> OfflineNoteRedeem {
        let publicInputs = try OfflineNoteRedeemPublicInputs(
            sourceNoteCommitment: sourceNoteCommitment,
            inputNullifiers: inputNullifiers,
            keyCertificatePayloadHash: senderKeyCertificate.payloadHash(),
            recipient: recipient,
            assetId: assetId,
            amount: amount
        )
        return try OfflineNoteRedeem(
            sourceNoteCommitment: sourceNoteCommitment,
            inputNullifiers: inputNullifiers,
            senderKeyCertificate: senderKeyCertificate,
            recipient: recipient,
            assetId: assetId,
            amount: amount,
            recursiveProof: draftPlaceholderProof(publicInputsHash: publicInputs.publicInputsHash())
        )
    }

    private static func auditDraft(
        tokenId: Data,
        senderKeyCertificate: OfflineNoteKeyCertificate,
        inputNullifiers: [Data],
        inputClaims: [OfflineNoteIssuedClaim],
        outputCommitments: [Data],
        outputClaims: [OfflineNoteAuditOutputClaim]
    ) throws -> OfflineNoteAuditBundle {
        let publicInputs = try OfflineNoteAuditPublicInputs(
            tokenId: tokenId,
            keyCertificatePayloadHash: senderKeyCertificate.payloadHash(),
            inputNullifiers: inputNullifiers,
            inputClaims: inputClaims,
            outputCommitments: outputCommitments,
            outputClaims: outputClaims.map(OfflineNoteIssuedClaim.fromAuditOutput)
        )
        return try OfflineNoteAuditBundle(
            tokenId: tokenId,
            senderKeyCertificate: senderKeyCertificate,
            inputNullifiers: inputNullifiers,
            inputClaims: inputClaims,
            outputCommitments: outputCommitments,
            outputClaims: outputClaims,
            recursiveProof: draftPlaceholderProof(publicInputsHash: publicInputs.publicInputsHash())
        )
    }

    private static func draftPlaceholderProof(publicInputsHash: Data) throws -> OfflineNoteRecursiveProof {
        try OfflineNoteRecursiveProof(
            publicInputsHash: publicInputsHash,
            proofBytes: Data([0]),
            proofBackend: "offline-note/draft-placeholder"
        )
    }
}
