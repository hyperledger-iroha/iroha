import Foundation
#if canImport(Security)
import Security
#endif

public enum OfflineNoteV2WalletNoteState: String, Sendable {
    case spendable
    case receivePending
    case changePending
    case spendPending
    case spent
    case redeemPending
    case redeemed
    case cancelled
}

public enum OfflineNoteV2WalletError: Error, LocalizedError, Equatable {
    case missingIssuerClient
    case missingTransactionSubmitter
    case issuerCommitmentMismatch
    case insufficientBalance
    case randomLength(expected: Int, actual: Int)
    case chainMismatch
    case noPendingOutput
    case outputMismatch
    case invalidState
    case inputCertificateMismatch

    public var errorDescription: String? {
        switch self {
        case .missingIssuerClient:
            return "Offline Note V2 issuer client is required for load."
        case .missingTransactionSubmitter:
            return "Offline Note V2 transaction submitter is required."
        case .issuerCommitmentMismatch:
            return "Issuer returned a different Offline Note V2 commitment."
        case .insufficientBalance:
            return "Insufficient spendable Offline Note V2 balance."
        case let .randomLength(expected, actual):
            return "Offline Note V2 random source must return \(expected) bytes, got \(actual)."
        case .chainMismatch:
            return "Receive request chainId does not match wallet chainId."
        case .noPendingOutput:
            return "Payment token has no pending output for this wallet."
        case .outputMismatch:
            return "Payment token output does not match the pending receive request."
        case .invalidState:
            return "Offline Note V2 note is not in the required wallet state."
        case .inputCertificateMismatch:
            return "Selected Offline Note V2 input notes must use the same key certificate."
        }
    }
}

public struct OfflineNoteV2WalletNote: Equatable, Sendable {
    public let chainId: String
    public let accountId: String
    public let assetId: String
    public let amount: String
    public let keyCertificate: OfflineNoteKeyCertificateV2
    public let noteCommitment: Data
    public let noteSecret: Data
    public let origin: OfflineNoteCommitmentOriginV2
    public let state: OfflineNoteV2WalletNoteState
    public let createdAtMs: UInt64
    public let updatedAtMs: UInt64

    public init(chainId: String,
                accountId: String,
                assetId: String,
                amount: String,
                keyCertificate: OfflineNoteKeyCertificateV2,
                noteCommitment: Data,
                noteSecret: Data,
                origin: OfflineNoteCommitmentOriginV2,
                state: OfflineNoteV2WalletNoteState,
                createdAtMs: UInt64,
                updatedAtMs: UInt64) throws {
        try OfflineNoteV2Validation.validateRandomBytes(noteSecret, field: "note_secret")
        self.chainId = chainId
        self.accountId = accountId
        self.assetId = try OfflineNorito.canonicalAssetIdLiteral(assetId)
        self.amount = try OfflineNorito.parseCanonicalNumeric(amount).canonicalString
        self.keyCertificate = keyCertificate
        self.noteCommitment = noteCommitment
        self.noteSecret = noteSecret
        self.origin = origin
        self.state = state
        self.createdAtMs = createdAtMs
        self.updatedAtMs = updatedAtMs
        _ = try issuedClaim()
    }

    public var noteCommitmentHex: String {
        noteCommitment.hexLowercased()
    }

    public func issuedClaim() throws -> OfflineNoteIssuedClaimV2 {
        try OfflineNoteIssuedClaimV2(
            noteCommitment: noteCommitment,
            keyCertificatePayloadHash: keyCertificate.payloadHash(),
            assetId: assetId,
            amount: amount
        )
    }

    public func withState(_ state: OfflineNoteV2WalletNoteState, updatedAtMs: UInt64) throws -> OfflineNoteV2WalletNote {
        try OfflineNoteV2WalletNote(
            chainId: chainId,
            accountId: accountId,
            assetId: assetId,
            amount: amount,
            keyCertificate: keyCertificate,
            noteCommitment: noteCommitment,
            noteSecret: noteSecret,
            origin: origin,
            state: state,
            createdAtMs: createdAtMs,
            updatedAtMs: updatedAtMs
        )
    }
}

public protocol OfflineNoteV2Store: AnyObject {
    func listNotes() -> [OfflineNoteV2WalletNote]
    func findNote(noteCommitment: Data) -> OfflineNoteV2WalletNote?
    func upsert(_ note: OfflineNoteV2WalletNote)
}

public final class InMemoryOfflineNoteV2Store: OfflineNoteV2Store {
    private var notes: [String: OfflineNoteV2WalletNote] = [:]
    private let lock = NSLock()

    public init() {}

    public func listNotes() -> [OfflineNoteV2WalletNote] {
        lock.lock()
        defer { lock.unlock() }
        return Array(notes.values)
    }

    public func findNote(noteCommitment: Data) -> OfflineNoteV2WalletNote? {
        lock.lock()
        defer { lock.unlock() }
        return notes[noteCommitment.hexLowercased()]
    }

    public func upsert(_ note: OfflineNoteV2WalletNote) {
        lock.lock()
        defer { lock.unlock() }
        notes[note.noteCommitmentHex] = note
    }
}

public protocol OfflineNoteV2AttestationProvider {
    func currentKeyCertificate() throws -> OfflineNoteKeyCertificateV2
}

public protocol OfflineNoteV2RandomSource {
    func nextBytes(count: Int) throws -> Data
}

public struct SecureOfflineNoteV2RandomSource: OfflineNoteV2RandomSource {
    public init() {}

    public func nextBytes(count: Int) throws -> Data {
        var data = Data(count: count)
        #if canImport(Security)
        let status = data.withUnsafeMutableBytes { buffer in
            SecRandomCopyBytes(kSecRandomDefault, count, buffer.baseAddress!)
        }
        guard status == errSecSuccess else {
            throw OfflineNoritoError.invalidMetadata("secure_random")
        }
        #else
        for index in data.indices {
            data[index] = UInt8.random(in: UInt8.min...UInt8.max)
        }
        #endif
        return data
    }
}

public protocol OfflineNoteV2IdGenerator {
    func nextId(prefix: String) -> String
}

public struct UuidOfflineNoteV2IdGenerator: OfflineNoteV2IdGenerator {
    public init() {}

    public func nextId(prefix: String) -> String {
        "\(prefix)-\(UUID().uuidString)"
    }
}

public protocol OfflineNoteV2ProofProvider {
    func proveAudit(_ audit: OfflineNoteAuditBundleV2) throws -> OfflineNoteRecursiveProofV2
    func proveRedeem(_ redemption: OfflineNoteRedeemV2) throws -> OfflineNoteRecursiveProofV2
}

public struct OfflineNoteV2LoadContext: Sendable {
    public let operationId: String
    public let lineageId: String
    public let localRevision: UInt64
    public let keyCertificate: OfflineNoteKeyCertificateV2

    public init(operationId: String,
                lineageId: String,
                localRevision: UInt64,
                keyCertificate: OfflineNoteKeyCertificateV2) {
        self.operationId = operationId
        self.lineageId = lineageId
        self.localRevision = localRevision
        self.keyCertificate = keyCertificate
    }
}

public struct OfflineNoteV2IssueRequest: Sendable {
    public let chainId: String
    public let accountId: String
    public let assetDefinitionId: String
    public let assetId: String
    public let amount: String
    public let loadContext: OfflineNoteV2LoadContext
    public let noteCommitment: Data

    public init(chainId: String,
                accountId: String,
                assetDefinitionId: String,
                assetId: String,
                amount: String,
                loadContext: OfflineNoteV2LoadContext,
                noteCommitment: Data) {
        self.chainId = chainId
        self.accountId = accountId
        self.assetDefinitionId = assetDefinitionId
        self.assetId = assetId
        self.amount = amount
        self.loadContext = loadContext
        self.noteCommitment = noteCommitment
    }
}

public struct OfflineNoteV2IssueResponse: Sendable {
    public let noteCommitment: Data
    public let operationId: String
    public let lineageId: String
    public let localRevision: UInt64
    public let keyCertificate: OfflineNoteKeyCertificateV2?
    public let settlementEntryHashHex: String?

    public init(noteCommitment: Data,
                operationId: String,
                lineageId: String,
                localRevision: UInt64,
                keyCertificate: OfflineNoteKeyCertificateV2? = nil,
                settlementEntryHashHex: String? = nil) {
        self.noteCommitment = noteCommitment
        self.operationId = operationId
        self.lineageId = lineageId
        self.localRevision = localRevision
        self.keyCertificate = keyCertificate
        self.settlementEntryHashHex = settlementEntryHashHex
    }
}

public protocol OfflineNoteV2IssuerClient {
    func prepareLoad(chainId: String,
                     accountId: String,
                     assetDefinitionId: String,
                     amount: String) async throws -> OfflineNoteV2LoadContext
    func issueNote(_ request: OfflineNoteV2IssueRequest) async throws -> OfflineNoteV2IssueResponse
}

public struct OfflineNoteV2ReceiveRequest: Equatable, Sendable {
    public let chainId: String
    public let paymentRequestId: String
    public let accountId: String
    public let assetDefinitionId: String
    public let assetId: String
    public let amount: String
    public let keyCertificate: OfflineNoteKeyCertificateV2
    public let outputCommitment: Data

    public init(chainId: String,
                paymentRequestId: String,
                accountId: String,
                assetDefinitionId: String,
                assetId: String,
                amount: String,
                keyCertificate: OfflineNoteKeyCertificateV2,
                outputCommitment: Data) throws {
        self.chainId = chainId
        self.paymentRequestId = paymentRequestId
        self.accountId = accountId
        self.assetDefinitionId = assetDefinitionId
        self.assetId = try OfflineNorito.canonicalAssetIdLiteral(assetId)
        self.amount = try OfflineNorito.parseCanonicalNumeric(amount).canonicalString
        self.keyCertificate = keyCertificate
        self.outputCommitment = outputCommitment
        _ = try OfflineNoteAuditOutputClaimV2(
            noteCommitment: outputCommitment,
            keyCertificate: keyCertificate,
            assetId: assetId,
            amount: amount
        )
    }

    public var outputCommitmentHex: String {
        outputCommitment.hexLowercased()
    }
}

public struct OfflineNoteV2PaymentToken: Equatable, Sendable {
    public let paymentRequestId: String
    public let tokenId: Data
    public let audit: OfflineNoteAuditBundleV2
    public let createdAtMs: UInt64

    public init(paymentRequestId: String,
                tokenId: Data,
                audit: OfflineNoteAuditBundleV2,
                createdAtMs: UInt64) {
        self.paymentRequestId = paymentRequestId
        self.tokenId = tokenId
        self.audit = audit
        self.createdAtMs = createdAtMs
    }

    public var tokenIdHex: String {
        tokenId.hexLowercased()
    }
}

public protocol OfflineNoteV2TransactionSubmitter {
    func submitAudit(_ audit: OfflineNoteAuditBundleV2) async throws
    func submitRedeem(_ redemption: OfflineNoteRedeemV2) async throws
}

public struct OfflineNoteV2SyncResolution: Equatable, Sendable {
    public let state: OfflineNoteV2WalletNoteState
    public let transactionHashHex: String?

    public init(state: OfflineNoteV2WalletNoteState, transactionHashHex: String? = nil) {
        self.state = state
        self.transactionHashHex = transactionHashHex
    }
}

public protocol OfflineNoteV2SyncResolver {
    func resolvePendingNote(_ note: OfflineNoteV2WalletNote) async throws -> OfflineNoteV2SyncResolution?
}

@available(iOS 15.0, macOS 12.0, *)
public final class IrohaOfflineNoteV2TransactionSubmitter: OfflineNoteV2TransactionSubmitter {
    private let sdk: IrohaSDK
    private let signingKey: SigningKey
    private let chainId: String
    private let authority: String

    public init(sdk: IrohaSDK,
                signingKey: SigningKey,
                chainId: String,
                authority: String) {
        self.sdk = sdk
        self.signingKey = signingKey
        self.chainId = chainId
        self.authority = authority
    }

    public func submitAudit(_ audit: OfflineNoteAuditBundleV2) async throws {
        try await sdk.submit(
            auditOfflineNoteV2: AuditOfflineNoteV2Request(
                chainId: chainId,
                authority: authority,
                audit: audit
            ),
            signingKey: signingKey
        )
    }

    public func submitRedeem(_ redemption: OfflineNoteRedeemV2) async throws {
        try await sdk.submit(
            redeemOfflineNoteV2: RedeemOfflineNoteV2Request(
                chainId: chainId,
                authority: authority,
                redemption: redemption
            ),
            signingKey: signingKey
        )
    }
}

public final class OfflineNoteV2Wallet {
    private let chainId: String
    private let accountId: String
    private let attestationProvider: OfflineNoteV2AttestationProvider
    private let store: OfflineNoteV2Store
    private let issuerClient: OfflineNoteV2IssuerClient?
    private let transactionSubmitter: OfflineNoteV2TransactionSubmitter?
    private let syncResolver: OfflineNoteV2SyncResolver?
    private let proofProvider: OfflineNoteV2ProofProvider
    private let randomSource: OfflineNoteV2RandomSource
    private let idGenerator: OfflineNoteV2IdGenerator
    private let clock: () -> UInt64

    public init(chainId: String,
                accountId: String,
                attestationProvider: OfflineNoteV2AttestationProvider,
                store: OfflineNoteV2Store = InMemoryOfflineNoteV2Store(),
                issuerClient: OfflineNoteV2IssuerClient? = nil,
                transactionSubmitter: OfflineNoteV2TransactionSubmitter? = nil,
                syncResolver: OfflineNoteV2SyncResolver? = nil,
                proofProvider: OfflineNoteV2ProofProvider,
                randomSource: OfflineNoteV2RandomSource = SecureOfflineNoteV2RandomSource(),
                idGenerator: OfflineNoteV2IdGenerator = UuidOfflineNoteV2IdGenerator(),
                clock: @escaping () -> UInt64 = { UInt64(Date().timeIntervalSince1970 * 1000) }) {
        self.chainId = chainId
        self.accountId = accountId
        self.attestationProvider = attestationProvider
        self.store = store
        self.issuerClient = issuerClient
        self.transactionSubmitter = transactionSubmitter
        self.syncResolver = syncResolver
        self.proofProvider = proofProvider
        self.randomSource = randomSource
        self.idGenerator = idGenerator
        self.clock = clock
    }

    public func listNotes() -> [OfflineNoteV2WalletNote] {
        store.listNotes()
    }

    public func load(assetDefinitionId: String, amount: String) async throws -> OfflineNoteV2WalletNote {
        guard let issuerClient else {
            throw OfflineNoteV2WalletError.missingIssuerClient
        }
        let assetId = walletAssetId(assetDefinitionId: assetDefinitionId, accountId: accountId)
        let context = try await issuerClient.prepareLoad(
            chainId: chainId,
            accountId: accountId,
            assetDefinitionId: assetDefinition(from: assetId),
            amount: amount
        )
        let noteSecret = try random32()
        let origin = try OfflineNoteCommitmentOriginV2.issuerLoad(OfflineNoteIssuerLoadOriginV2(
            operationId: context.operationId,
            lineageId: context.lineageId,
            localRevision: context.localRevision
        ))
        let commitment = try deriveNoteCommitment(
            keyCertificate: context.keyCertificate,
            assetId: assetId,
            amount: amount,
            noteSecret: noteSecret,
            origin: origin
        )
        let request = OfflineNoteV2IssueRequest(
            chainId: chainId,
            accountId: accountId,
            assetDefinitionId: assetDefinition(from: assetId),
            assetId: assetId,
            amount: amount,
            loadContext: context,
            noteCommitment: commitment
        )
        let response = try await issuerClient.issueNote(request)
        guard response.noteCommitment == commitment else {
            throw OfflineNoteV2WalletError.issuerCommitmentMismatch
        }
        let now = clock()
        let note = try OfflineNoteV2WalletNote(
            chainId: chainId,
            accountId: accountId,
            assetId: assetId,
            amount: amount,
            keyCertificate: response.keyCertificate ?? context.keyCertificate,
            noteCommitment: commitment,
            noteSecret: noteSecret,
            origin: origin,
            state: .spendable,
            createdAtMs: now,
            updatedAtMs: now
        )
        store.upsert(note)
        return note
    }

    public func prepareReceive(assetDefinitionId: String, amount: String) throws -> OfflineNoteV2ReceiveRequest {
        let paymentRequestId = idGenerator.nextId(prefix: "payment-request")
        let keyCertificate = try attestationProvider.currentKeyCertificate()
        let assetId = walletAssetId(assetDefinitionId: assetDefinitionId, accountId: accountId)
        let noteSecret = try random32()
        let origin = try OfflineNoteCommitmentOriginV2.p2pOutput(OfflineNoteP2pOutputOriginV2(
            paymentRequestId: paymentRequestId,
            outputIndex: 0
        ))
        let commitment = try deriveNoteCommitment(
            keyCertificate: keyCertificate,
            assetId: assetId,
            amount: amount,
            noteSecret: noteSecret,
            origin: origin
        )
        let now = clock()
        let note = try OfflineNoteV2WalletNote(
            chainId: chainId,
            accountId: accountId,
            assetId: assetId,
            amount: amount,
            keyCertificate: keyCertificate,
            noteCommitment: commitment,
            noteSecret: noteSecret,
            origin: origin,
            state: .receivePending,
            createdAtMs: now,
            updatedAtMs: now
        )
        store.upsert(note)
        return try OfflineNoteV2ReceiveRequest(
            chainId: chainId,
            paymentRequestId: paymentRequestId,
            accountId: accountId,
            assetDefinitionId: assetDefinition(from: assetId),
            assetId: assetId,
            amount: note.amount,
            keyCertificate: keyCertificate,
            outputCommitment: commitment
        )
    }

    public func pay(_ receiveRequest: OfflineNoteV2ReceiveRequest) throws -> OfflineNoteV2PaymentToken {
        guard receiveRequest.chainId == chainId else {
            throw OfflineNoteV2WalletError.chainMismatch
        }
        let requestedAmount = try OfflineNorito.parseCanonicalNumeric(receiveRequest.amount)
        let selected = try selectSpendableNotes(
            assetDefinitionId: receiveRequest.assetDefinitionId,
            requestedAmount: requestedAmount
        )
        var inputAmount = OfflineCanonicalNumeric(isNegative: false, scale: 0, digits: "0")
        for note in selected {
            inputAmount = try inputAmount.adding(
                OfflineNorito.parseCanonicalNumeric(note.amount),
                maxBytes: 64
            )
        }
        let changeAmount = try inputAmount.subtracting(requestedAmount, maxBytes: 64)
        guard !changeAmount.isNegative else {
            throw OfflineNoteV2WalletError.insufficientBalance
        }

        let senderCertificate = selected[0].keyCertificate
        let senderCertificateHash = try senderCertificate.payloadHash()
        for note in selected {
            if try note.keyCertificate.payloadHash() != senderCertificateHash {
                throw OfflineNoteV2WalletError.inputCertificateMismatch
            }
        }
        let inputNullifiers = try selected.map(deriveInputNullifier)
        let inputClaims = try selected.map { try $0.issuedClaim() }
        var outputClaims = [
            try OfflineNoteAuditOutputClaimV2(
                noteCommitment: receiveRequest.outputCommitment,
                keyCertificate: receiveRequest.keyCertificate,
                assetId: receiveRequest.assetId,
                amount: receiveRequest.amount
            )
        ]
        let tokenNonce = try random32()
        var changeNote: OfflineNoteV2WalletNote?
        if changeAmount.digits != "0" {
            let changeSecret = try random32()
            let changeAssetId = walletAssetId(
                assetDefinitionId: receiveRequest.assetDefinitionId,
                accountId: accountId
            )
            let changeOrigin = try OfflineNoteCommitmentOriginV2.p2pOutput(OfflineNoteP2pOutputOriginV2(
                paymentRequestId: receiveRequest.paymentRequestId,
                outputIndex: 1
            ))
            let changeCommitment = try deriveNoteCommitment(
                keyCertificate: senderCertificate,
                assetId: changeAssetId,
                amount: changeAmount.canonicalString,
                noteSecret: changeSecret,
                origin: changeOrigin
            )
            let now = clock()
            let note = try OfflineNoteV2WalletNote(
                chainId: chainId,
                accountId: accountId,
                assetId: changeAssetId,
                amount: changeAmount.canonicalString,
                keyCertificate: senderCertificate,
                noteCommitment: changeCommitment,
                noteSecret: changeSecret,
                origin: changeOrigin,
                state: .changePending,
                createdAtMs: now,
                updatedAtMs: now
            )
            changeNote = note
            outputClaims.append(try OfflineNoteAuditOutputClaimV2(
                noteCommitment: changeCommitment,
                keyCertificate: senderCertificate,
                assetId: changeAssetId,
                amount: note.amount
            ))
        }
        let outputCommitments = outputClaims.map(\.noteCommitment)
        let tokenId = try OfflineNotePaymentTokenIdPreimageV2(
            chainId: chainId,
            tokenNonce: tokenNonce,
            senderKeyCertificatePayloadHash: senderCertificateHash,
            inputNullifiers: inputNullifiers,
            outputCommitments: outputCommitments
        ).derivePaymentTokenId()
        let draft = try OfflineNoteAuditBundleV2(
            tokenId: tokenId,
            senderKeyCertificate: senderCertificate,
            inputNullifiers: inputNullifiers,
            inputClaims: inputClaims,
            outputCommitments: outputCommitments,
            outputClaims: outputClaims,
            recursiveProof: placeholderProof()
        )
        let audit = try draft.replacingRecursiveProof(proofProvider.proveAudit(draft))
        try audit.validateProofBinding()
        let now = clock()
        for note in selected {
            store.upsert(try note.withState(.spendPending, updatedAtMs: now))
        }
        if let changeNote {
            store.upsert(changeNote)
        }
        return OfflineNoteV2PaymentToken(
            paymentRequestId: receiveRequest.paymentRequestId,
            tokenId: tokenId,
            audit: audit,
            createdAtMs: now
        )
    }

    public func accept(_ paymentToken: OfflineNoteV2PaymentToken) async throws -> OfflineNoteV2WalletNote {
        guard let transactionSubmitter else {
            throw OfflineNoteV2WalletError.missingTransactionSubmitter
        }
        try paymentToken.audit.validateProofBinding()
        guard let output = paymentToken.audit.outputClaims.first(where: { claim in
            store.findNote(noteCommitment: claim.noteCommitment)?.state == .receivePending
        }) else {
            throw OfflineNoteV2WalletError.noPendingOutput
        }
        guard let pending = store.findNote(noteCommitment: output.noteCommitment) else {
            throw OfflineNoteV2WalletError.noPendingOutput
        }
        guard pending.assetId == output.assetId,
              pending.amount == output.amount,
              try pending.keyCertificate.payloadHash() == output.keyCertificate.payloadHash()
        else {
            throw OfflineNoteV2WalletError.outputMismatch
        }
        try await transactionSubmitter.submitAudit(paymentToken.audit)
        let accepted = try pending.withState(.spendable, updatedAtMs: clock())
        store.upsert(accepted)
        return accepted
    }

    public func redeem(_ note: OfflineNoteV2WalletNote, recipient: String? = nil) async throws -> OfflineNoteV2WalletNote {
        guard let transactionSubmitter else {
            throw OfflineNoteV2WalletError.missingTransactionSubmitter
        }
        let current = store.findNote(noteCommitment: note.noteCommitment) ?? note
        guard current.state == .spendable else {
            throw OfflineNoteV2WalletError.invalidState
        }
        let inputNullifier = try deriveInputNullifier(current)
        let draft = try OfflineNoteRedeemV2(
            sourceNoteCommitment: current.noteCommitment,
            inputNullifiers: [inputNullifier],
            senderKeyCertificate: current.keyCertificate,
            recipient: recipient ?? accountId,
            assetId: current.assetId,
            amount: current.amount,
            recursiveProof: placeholderProof()
        )
        let redemption = try draft.replacingRecursiveProof(proofProvider.proveRedeem(draft))
        try redemption.validateProofBinding()
        let pending = try current.withState(.redeemPending, updatedAtMs: clock())
        store.upsert(pending)
        try await transactionSubmitter.submitRedeem(redemption)
        return pending
    }

    public func sync() async throws -> [OfflineNoteV2WalletNote] {
        guard let syncResolver else {
            return store.listNotes()
        }
        for snapshot in store.listNotes() where snapshot.state.isPending {
            guard let current = store.findNote(noteCommitment: snapshot.noteCommitment),
                  current.state.isPending
            else {
                continue
            }
            if let resolution = try await syncResolver.resolvePendingNote(current),
               resolution.state != current.state {
                store.upsert(try current.withState(resolution.state, updatedAtMs: clock()))
            }
        }
        return store.listNotes()
    }

    private func selectSpendableNotes(
        assetDefinitionId: String,
        requestedAmount: OfflineCanonicalNumeric
    ) throws -> [OfflineNoteV2WalletNote] {
        var selected: [OfflineNoteV2WalletNote] = []
        var total = OfflineCanonicalNumeric(isNegative: false, scale: 0, digits: "0")
        for note in store.listNotes() {
            guard note.state == .spendable else { continue }
            guard assetDefinition(from: note.assetId) == assetDefinition(from: assetDefinitionId) else { continue }
            selected.append(note)
            total = try total.adding(OfflineNorito.parseCanonicalNumeric(note.amount), maxBytes: 64)
            if total.compared(to: requestedAmount) != .orderedAscending {
                break
            }
            guard selected.count < 4 else {
                throw OfflineNoteV2WalletError.insufficientBalance
            }
        }
        guard !selected.isEmpty,
              total.compared(to: requestedAmount) != .orderedAscending
        else {
            throw OfflineNoteV2WalletError.insufficientBalance
        }
        return selected
    }

    private func deriveNoteCommitment(keyCertificate: OfflineNoteKeyCertificateV2,
                                      assetId: String,
                                      amount: String,
                                      noteSecret: Data,
                                      origin: OfflineNoteCommitmentOriginV2) throws -> Data {
        try OfflineNoteCommitmentPreimageV2(
            chainId: chainId,
            ownerKeyCertificatePayloadHash: keyCertificate.payloadHash(),
            assetId: assetId,
            amount: amount,
            noteSecret: noteSecret,
            origin: origin
        ).deriveNoteCommitment()
    }

    private func deriveInputNullifier(_ note: OfflineNoteV2WalletNote) throws -> Data {
        try OfflineNoteInputNullifierPreimageV2(
            chainId: chainId,
            sourceNoteCommitment: note.noteCommitment,
            ownerKeyCertificatePayloadHash: note.keyCertificate.payloadHash(),
            noteSecret: note.noteSecret
        ).deriveInputNullifier()
    }

    private func random32() throws -> Data {
        let bytes = try randomSource.nextBytes(count: 32)
        guard bytes.count == 32 else {
            throw OfflineNoteV2WalletError.randomLength(expected: 32, actual: bytes.count)
        }
        return bytes
    }
}

private func placeholderProof() throws -> OfflineNoteRecursiveProofV2 {
    try OfflineNoteRecursiveProofV2(
        publicInputsHash: IrohaHash.hash(Data("offline-note-v2-draft-proof".utf8)),
        proofBytes: Data([0x01])
    )
}

private extension OfflineNoteV2WalletNoteState {
    var isPending: Bool {
        switch self {
        case .receivePending, .changePending, .spendPending, .redeemPending:
            return true
        case .spendable, .spent, .redeemed, .cancelled:
            return false
        }
    }
}

private func walletAssetId(assetDefinitionId: String, accountId: String) -> String {
    "\(assetDefinition(from: assetDefinitionId))#\(accountId)"
}

private func assetDefinition(from assetIdOrDefinition: String) -> String {
    assetIdOrDefinition.split(separator: "#", maxSplits: 1).first.map(String.init) ?? assetIdOrDefinition
}
