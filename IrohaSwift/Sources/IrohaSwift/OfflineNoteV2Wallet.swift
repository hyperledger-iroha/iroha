import Foundation
#if canImport(Security)
import Security
#endif

public enum OfflineNoteV2WalletNoteState: String, Sendable {
    case spendable
    case receivePending
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
    case proofVerificationFailed

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
        case .proofVerificationFailed:
            return "Offline Note V2 recursive proof verification failed."
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
    /// Atomically mutate wallet notes keyed by lower-case note commitment hex.
    func mutateNotes<T>(_ body: (inout [String: OfflineNoteV2WalletNote]) throws -> T) throws -> T
}

public extension OfflineNoteV2Store {
    /// List wallet notes; persistent implementations may throw storage or integrity errors.
    func listNotes() throws -> [OfflineNoteV2WalletNote] {
        try mutateNotes { notes in Array(notes.values) }
    }

    /// Find a wallet note by commitment; persistent implementations may throw storage or integrity errors.
    func findNote(noteCommitment: Data) throws -> OfflineNoteV2WalletNote? {
        try mutateNotes { notes in notes[noteCommitment.hexLowercased()] }
    }

    /// Insert or replace a wallet note; persistent implementations may throw storage or integrity errors.
    func upsert(_ note: OfflineNoteV2WalletNote) throws {
        try mutateNotes { notes in
            notes[note.noteCommitmentHex] = note
        }
    }
}

public final class InMemoryOfflineNoteV2Store: OfflineNoteV2Store {
    private var notes: [String: OfflineNoteV2WalletNote] = [:]
    private let lock = NSLock()

    public init() {}

    public func mutateNotes<T>(_ body: (inout [String: OfflineNoteV2WalletNote]) throws -> T) throws -> T {
        lock.lock()
        defer { lock.unlock() }
        return try body(&notes)
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

public protocol OfflineNoteV2ProofVerifier {
    func verifyAudit(_ audit: OfflineNoteAuditBundleV2) throws -> Bool
    func verifyRedeem(_ redemption: OfflineNoteRedeemV2) throws -> Bool
}

public struct Halo2OfflineNoteV2ProofVerifier: OfflineNoteV2ProofVerifier {
    public init() {}

    public func verifyAudit(_ audit: OfflineNoteAuditBundleV2) throws -> Bool {
        try Halo2OfflineNoteV2Prover.verifyAudit(audit)
    }

    public func verifyRedeem(_ redemption: OfflineNoteRedeemV2) throws -> Bool {
        try Halo2OfflineNoteV2Prover.verifyRedeem(redemption)
    }
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
    public let chainId: String
    public let paymentRequestId: String
    public let tokenNonce: Data
    public let tokenId: Data
    public let audit: OfflineNoteAuditBundleV2
    public let createdAtMs: UInt64

    public init(chainId: String,
                paymentRequestId: String,
                tokenNonce: Data,
                tokenId: Data,
                audit: OfflineNoteAuditBundleV2,
                createdAtMs: UInt64) {
        self.chainId = chainId
        self.paymentRequestId = paymentRequestId
        self.tokenNonce = tokenNonce
        self.tokenId = tokenId
        self.audit = audit
        self.createdAtMs = createdAtMs
    }

    public var tokenIdHex: String {
        tokenId.hexLowercased()
    }
}

public enum OfflineNoteV2PaymentTokenCodecError: Error, LocalizedError, Equatable {
    case invalidJson
    case invalidField(String)
    case invalidPrefix
    case tokenIdMismatch

    public var errorDescription: String? {
        switch self {
        case .invalidJson:
            return "Offline Note V2 payment token payload is invalid."
        case let .invalidField(field):
            return "Offline Note V2 payment token field \(field) is invalid."
        case .invalidPrefix:
            return "Offline Note V2 payment token prefix missing."
        case .tokenIdMismatch:
            return "Offline Note V2 payment token id does not match the audit bundle."
        }
    }
}

public enum OfflineNoteV2PaymentTokenCodec {
    public static let type = "offline_payment_token_v2"
    public static let version: UInt64 = 2
    public static let textPrefix = "wallet-offline-payment-v2:"
    private static let envelopeTypeName = "iroha_data_model::offline::model::OfflineNotePaymentTokenEnvelopeV2"

    public static func encodeNorito(_ token: OfflineNoteV2PaymentToken) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeUInt64(version))
        writer.writeField(OfflineCompactNorito.encodeString(token.chainId))
        writer.writeField(OfflineCompactNorito.encodeString(token.paymentRequestId))
        writer.writeField(OfflineCompactNorito.encodeUInt64(token.createdAtMs))
        writer.writeField(OfflineNorito.encodeBytesVec(token.tokenNonce))
        writer.writeField(try OfflineCompactNorito.encodeHash(token.tokenId))
        writer.writeField(OfflineNorito.encodeBytesVec(try token.audit.noritoEncoded()))
        return noritoEncode(typeName: envelopeTypeName, payload: writer.data, flags: NoritoHeader.compactLen)
    }

    public static func decodeNorito(_ payload: Data) throws -> OfflineNoteV2PaymentToken {
        guard let frame = noritoDecodeFrame(payload) else {
            throw OfflineNoteV2PaymentTokenCodecError.invalidField("payload")
        }
        guard frame.header.schema == noritoSchemaHash(forTypeName: envelopeTypeName) else {
            throw OfflineNoteV2PaymentTokenCodecError.invalidField("schema")
        }
        guard frame.header.compression == .none,
              (frame.header.flags & NoritoHeader.compactLen) != 0
        else {
            throw OfflineNoteV2PaymentTokenCodecError.invalidField("layout")
        }
        var reader = OfflineNoritoReader(data: frame.payload)
        let decodedVersion = try field(&reader, "version") { try $0.readUInt64LE() }
        guard decodedVersion == version else {
            throw OfflineNoteV2PaymentTokenCodecError.invalidField("version")
        }
        let chainId = try field(&reader, "chain_id", readString)
        let paymentRequestId = try field(&reader, "payment_request_id", readString)
        let createdAtMs = try field(&reader, "created_at_ms") { try $0.readUInt64LE() }
        let tokenNonce = try field(&reader, "token_nonce", readBytesVec)
        let tokenId = try field(&reader, "token_id") { try readHash(reader: &$0, field: "token_id") }
        let auditBytes = try field(&reader, "audit", readBytesVec)
        guard reader.remaining() == 0 else {
            throw OfflineNoteV2PaymentTokenCodecError.invalidField("trailing_bytes")
        }
        let audit = try OfflineNoteV2Decoding.decodeAudit(auditBytes)
        guard audit.tokenId == tokenId else {
            throw OfflineNoteV2PaymentTokenCodecError.tokenIdMismatch
        }
        return OfflineNoteV2PaymentToken(
            chainId: chainId,
            paymentRequestId: paymentRequestId,
            tokenNonce: tokenNonce,
            tokenId: tokenId,
            audit: audit,
            createdAtMs: createdAtMs
        )
    }

    public static func encodeJson(_ token: OfflineNoteV2PaymentToken) throws -> Data {
        try encodeNorito(token)
    }

    public static func decodeJson(_ payload: Data) throws -> OfflineNoteV2PaymentToken {
        try decodeNorito(payload)
    }

    public static func encodeText(_ token: OfflineNoteV2PaymentToken) throws -> String {
        textPrefix + base64UrlEncode(try encodeNorito(token))
    }

    public static func decodeText(_ text: String) throws -> OfflineNoteV2PaymentToken {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix(textPrefix) else {
            throw OfflineNoteV2PaymentTokenCodecError.invalidPrefix
        }
        guard let payload = base64UrlDecode(String(trimmed.dropFirst(textPrefix.count))) else {
            throw OfflineNoteV2PaymentTokenCodecError.invalidField("payload")
        }
        return try decodeNorito(payload)
    }

    public static func encodeQrFrameBytes(
        _ token: OfflineNoteV2PaymentToken,
        options: OfflineQrStreamOptions = OfflineQrStreamOptions()
    ) throws -> [Data] {
        try OfflineQrStreamEncoder.encodeFrameBytes(
            payload: encodeNorito(token),
            payloadKind: .offlinePaymentTokenV2,
            options: options
        )
    }

    public static func decodeQrPayload(_ payload: Data) throws -> OfflineNoteV2PaymentToken {
        try decodeNorito(payload)
    }

    private static func field<T>(
        _ reader: inout OfflineNoritoReader,
        _ field: String,
        _ decode: (inout OfflineNoritoReader) throws -> T
    ) throws -> T {
        var child = OfflineNoritoReader(data: try reader.readCompactField())
        let value = try decode(&child)
        guard child.remaining() == 0 else {
            throw OfflineNoteV2PaymentTokenCodecError.invalidField(field)
        }
        return value
    }

    private static func readString(_ reader: inout OfflineNoritoReader) throws -> String {
        let length = try reader.readVarint()
        guard length <= UInt64(Int.max) else {
            throw OfflineNoteV2PaymentTokenCodecError.invalidField("string")
        }
        guard let value = String(data: try reader.readBytes(Int(length)), encoding: .utf8),
              !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else {
            throw OfflineNoteV2PaymentTokenCodecError.invalidField("string")
        }
        return value
    }

    private static func readBytesVec(_ reader: inout OfflineNoritoReader) throws -> Data {
        let length = try reader.readUInt64LE()
        guard length <= UInt64(Int.max) else {
            throw OfflineNoteV2PaymentTokenCodecError.invalidField("bytes")
        }
        return try reader.readBytes(Int(length))
    }

    private static func readHash(reader: inout OfflineNoritoReader, field: String) throws -> Data {
        let bytes = try reader.readBytes(32)
        guard bytes.count == 32 else {
            throw OfflineNoteV2PaymentTokenCodecError.invalidField(field)
        }
        return bytes
    }

    private static func base64UrlEncode(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .trimmingCharacters(in: CharacterSet(charactersIn: "="))
    }

    private static func base64UrlDecode(_ value: String) -> Data? {
        var normalized = value
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        let padding = (4 - normalized.count % 4) % 4
        normalized.append(String(repeating: "=", count: padding))
        return Data(base64Encoded: normalized)
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

public struct OfflineNoteV2ExplorerInstructionOutcome: Equatable, Sendable {
    public let kind: String
    public let transactionStatus: String
    public let transactionHashHex: String?
    public let encodedInstruction: Data

    public init(kind: String,
                transactionStatus: String,
                transactionHashHex: String? = nil,
                encodedInstruction: Data) {
        self.kind = kind
        self.transactionStatus = transactionStatus
        self.transactionHashHex = transactionHashHex
        self.encodedInstruction = encodedInstruction
    }
}

public protocol OfflineNoteV2OutcomeProvider {
    func listOutcomes() async throws -> [OfflineNoteV2ExplorerInstructionOutcome]
}

public final class OfflineNoteV2OutcomeIndex: @unchecked Sendable {
    public static let kindIssue = "IssueOfflineNoteV2"
    public static let kindRedeem = "RedeemOfflineNoteV2"
    public static let kindAudit = "AuditOfflineNoteV2"

    private struct OutcomeHit {
        let transactionHashHex: String?
    }

    private var committedRedeems: [String: OutcomeHit] = [:]
    private var rejectedRedeems: [String: OutcomeHit] = [:]

    public init() {}

    @discardableResult
    public func recordCommittedAudit(_ audit: OfflineNoteAuditBundleV2,
                                     transactionHashHex: String? = nil) -> OfflineNoteV2OutcomeIndex {
        return self
    }

    @discardableResult
    public func recordRejectedAudit(_ audit: OfflineNoteAuditBundleV2,
                                    transactionHashHex: String? = nil) -> OfflineNoteV2OutcomeIndex {
        return self
    }

    @discardableResult
    public func recordCommittedRedeem(_ redemption: OfflineNoteRedeemV2,
                                      transactionHashHex: String? = nil) -> OfflineNoteV2OutcomeIndex {
        putFirst(&committedRedeems, key: redemption.sourceNoteCommitment, value: transactionHashHex)
        return self
    }

    @discardableResult
    public func recordRejectedRedeem(_ redemption: OfflineNoteRedeemV2,
                                     transactionHashHex: String? = nil) -> OfflineNoteV2OutcomeIndex {
        putFirst(&rejectedRedeems, key: redemption.sourceNoteCommitment, value: transactionHashHex)
        return self
    }

    public func resolve(_ note: OfflineNoteV2WalletNote) throws -> OfflineNoteV2SyncResolution? {
        switch note.state {
        case .redeemPending:
            return resolveRedeemPending(note)
        default:
            return nil
        }
    }

    private func resolveRedeemPending(_ note: OfflineNoteV2WalletNote) -> OfflineNoteV2SyncResolution? {
        let commitmentKey = note.noteCommitmentHex
        if let hit = committedRedeems[commitmentKey] {
            return OfflineNoteV2SyncResolution(state: .redeemed, transactionHashHex: hit.transactionHashHex)
        }
        if let hit = rejectedRedeems[commitmentKey] {
            return OfflineNoteV2SyncResolution(state: .spendable, transactionHashHex: hit.transactionHashHex)
        }
        return nil
    }

    private func putFirst(_ target: inout [String: OutcomeHit], key: Data, value: String?) {
        let hex = key.hexLowercased()
        if target[hex] == nil {
            target[hex] = OutcomeHit(transactionHashHex: value)
        }
    }

    public static func fromExplorerOutcomes(_ outcomes: [OfflineNoteV2ExplorerInstructionOutcome]) throws -> OfflineNoteV2OutcomeIndex {
        let index = OfflineNoteV2OutcomeIndex()
        for outcome in outcomes {
            let status = outcome.transactionStatus.lowercased()
            let committed = status == "committed"
            let rejected = status == "rejected"
            guard committed || rejected else { continue }
            if outcome.kind.caseInsensitiveCompare(kindAudit) == .orderedSame {
                let audit = try OfflineNoteV2Decoding.decodeAuditInstruction(outcome.encodedInstruction)
                if committed {
                    index.recordCommittedAudit(audit, transactionHashHex: outcome.transactionHashHex)
                } else {
                    index.recordRejectedAudit(audit, transactionHashHex: outcome.transactionHashHex)
                }
            } else if outcome.kind.caseInsensitiveCompare(kindRedeem) == .orderedSame {
                let redemption = try OfflineNoteV2Decoding.decodeRedeemInstruction(outcome.encodedInstruction)
                if committed {
                    index.recordCommittedRedeem(redemption, transactionHashHex: outcome.transactionHashHex)
                } else {
                    index.recordRejectedRedeem(redemption, transactionHashHex: outcome.transactionHashHex)
                }
            }
        }
        return index
    }
}

public final class OfflineNoteV2OutcomeIndexSyncResolver: OfflineNoteV2SyncResolver {
    private let provider: OfflineNoteV2OutcomeProvider

    public init(provider: OfflineNoteV2OutcomeProvider) {
        self.provider = provider
    }

    public func resolvePendingNote(_ note: OfflineNoteV2WalletNote) async throws -> OfflineNoteV2SyncResolution? {
        try OfflineNoteV2OutcomeIndex.fromExplorerOutcomes(
            try await provider.listOutcomes()
        ).resolve(note)
    }
}

@available(iOS 15.0, macOS 12.0, *)
public final class ToriiOfflineNoteV2OutcomeProvider: OfflineNoteV2OutcomeProvider {
    private let client: ToriiClient
    private let perPage: UInt64

    public init(client: ToriiClient, perPage: UInt64 = 100) {
        self.client = client
        self.perPage = perPage
    }

    public func listOutcomes() async throws -> [OfflineNoteV2ExplorerInstructionOutcome] {
        let audit = try await fetch(kind: OfflineNoteV2OutcomeIndex.kindAudit)
        let redeem = try await fetch(kind: OfflineNoteV2OutcomeIndex.kindRedeem)
        return audit + redeem
    }

    private func fetch(kind: String) async throws -> [OfflineNoteV2ExplorerInstructionOutcome] {
        let page = try await client.getExplorerInstructions(params: ToriiExplorerInstructionsParams(
            perPage: perPage,
            kind: kind
        ))
        return try page.items.map { item in
            let encoded = try encodedInstructionHex(from: item.box)
            guard let bytes = Data(hexString: encoded), !bytes.isEmpty else {
                throw OfflineNoteV2PaymentTokenCodecError.invalidField("encoded")
            }
            return OfflineNoteV2ExplorerInstructionOutcome(
                kind: item.kind,
                transactionStatus: item.transactionStatus,
                transactionHashHex: item.transactionHash,
                encodedInstruction: bytes
            )
        }
    }

    private func encodedInstructionHex(from box: ToriiExplorerInstructionBox) throws -> String {
        if let encoded = box.encoded?.trimmingCharacters(in: .whitespacesAndNewlines),
           !encoded.isEmpty {
            return stripHexPrefix(encoded)
        }
        if let nested = box.json["encoded"],
           case let .string(encoded) = nested,
           !encoded.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return stripHexPrefix(encoded)
        }
        throw OfflineNoteV2PaymentTokenCodecError.invalidField("encoded")
    }

    private func stripHexPrefix(_ value: String) -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.lowercased().hasPrefix("0x") {
            return String(trimmed.dropFirst(2))
        }
        return trimmed
    }
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
    private let proofVerifier: OfflineNoteV2ProofVerifier
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
                proofVerifier: OfflineNoteV2ProofVerifier = Halo2OfflineNoteV2ProofVerifier(),
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
        self.proofVerifier = proofVerifier
        self.randomSource = randomSource
        self.idGenerator = idGenerator
        self.clock = clock
    }

    public func listNotes() throws -> [OfflineNoteV2WalletNote] {
        try store.listNotes()
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
        try store.upsert(note)
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
        try store.upsert(note)
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
        try rejectReusedReceiveRequest(receiveRequest.paymentRequestId)
        let createdAtMs = clock()
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
            let note = try OfflineNoteV2WalletNote(
                chainId: chainId,
                accountId: accountId,
                assetId: changeAssetId,
                amount: changeAmount.canonicalString,
                keyCertificate: senderCertificate,
                noteCommitment: changeCommitment,
                noteSecret: changeSecret,
                origin: changeOrigin,
                state: .spendable,
                createdAtMs: createdAtMs,
                updatedAtMs: createdAtMs
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
            paymentRequestId: receiveRequest.paymentRequestId,
            createdAtMs: createdAtMs,
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
        guard try proofVerifier.verifyAudit(audit) else {
            throw OfflineNoteV2WalletError.proofVerificationFailed
        }
        try store.mutateNotes { notes in
            for note in selected {
                guard notes[note.noteCommitmentHex]?.state == .spendable else {
                    throw OfflineNoteV2WalletError.invalidState
                }
            }
            if let changeNote, notes[changeNote.noteCommitmentHex] != nil {
                throw OfflineNoteV2WalletError.invalidState
            }
            for note in selected {
                notes[note.noteCommitmentHex] = try note.withState(.spent, updatedAtMs: createdAtMs)
            }
            if let changeNote {
                notes[changeNote.noteCommitmentHex] = changeNote
            }
        }
        return OfflineNoteV2PaymentToken(
            chainId: chainId,
            paymentRequestId: receiveRequest.paymentRequestId,
            tokenNonce: tokenNonce,
            tokenId: tokenId,
            audit: audit,
            createdAtMs: createdAtMs
        )
    }

    private func rejectReusedReceiveRequest(_ paymentRequestId: String) throws {
        for note in try store.listNotes() {
            guard note.state != .receivePending else {
                continue
            }
            guard case let .p2pOutput(origin) = note.origin,
                  origin.paymentRequestId == paymentRequestId else {
                continue
            }
            throw OfflineNoteV2WalletError.invalidState
        }
    }

    public func accept(_ paymentToken: OfflineNoteV2PaymentToken) throws -> OfflineNoteV2WalletNote {
        try validatePaymentToken(paymentToken)
        guard try proofVerifier.verifyAudit(paymentToken.audit) else {
            throw OfflineNoteV2WalletError.proofVerificationFailed
        }
        let accepted = try store.mutateNotes { notes in
            for (index, output) in paymentToken.audit.outputClaims.enumerated() {
                guard let pending = notes[output.noteCommitment.hexLowercased()],
                      pending.state == .receivePending
                else {
                    continue
                }
                guard pending.assetId == output.assetId,
                      pending.amount == output.amount,
                      try pending.keyCertificate.payloadHash() == output.keyCertificate.payloadHash(),
                      case let .p2pOutput(origin) = pending.origin,
                      origin.paymentRequestId == paymentToken.paymentRequestId,
                      origin.outputIndex == UInt32(index)
                else {
                    throw OfflineNoteV2WalletError.outputMismatch
                }
                let accepted = try pending.withState(.spendable, updatedAtMs: clock())
                notes[pending.noteCommitmentHex] = accepted
                return accepted
            }
            throw OfflineNoteV2WalletError.noPendingOutput
        }
        return accepted
    }

    public func publishAudit(_ paymentToken: OfflineNoteV2PaymentToken) async throws {
        guard let transactionSubmitter else {
            throw OfflineNoteV2WalletError.missingTransactionSubmitter
        }
        try validatePaymentToken(paymentToken)
        guard try proofVerifier.verifyAudit(paymentToken.audit) else {
            throw OfflineNoteV2WalletError.proofVerificationFailed
        }
        try await transactionSubmitter.submitAudit(paymentToken.audit)
    }

    public func redeem(_ note: OfflineNoteV2WalletNote, recipient: String? = nil) async throws -> OfflineNoteV2WalletNote {
        guard let transactionSubmitter else {
            throw OfflineNoteV2WalletError.missingTransactionSubmitter
        }
        let current = try store.findNote(noteCommitment: note.noteCommitment) ?? note
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
        guard try proofVerifier.verifyRedeem(redemption) else {
            throw OfflineNoteV2WalletError.proofVerificationFailed
        }
        let pending = try store.mutateNotes { notes in
            guard (notes[current.noteCommitmentHex] ?? current).state == .spendable else {
                throw OfflineNoteV2WalletError.invalidState
            }
            let pending = try current.withState(.redeemPending, updatedAtMs: clock())
            notes[current.noteCommitmentHex] = pending
            return pending
        }
        try await transactionSubmitter.submitRedeem(redemption)
        return pending
    }

    public func sync() async throws -> [OfflineNoteV2WalletNote] {
        guard let syncResolver else {
            return try store.listNotes()
        }
        for snapshot in try store.listNotes() where snapshot.state.isPending {
            guard let current = try store.findNote(noteCommitment: snapshot.noteCommitment),
                  current.state.isPending
            else {
                continue
            }
            if let resolution = try await syncResolver.resolvePendingNote(current),
               resolution.state != current.state {
                try store.upsert(try current.withState(resolution.state, updatedAtMs: clock()))
            }
        }
        return try store.listNotes()
    }

    private func selectSpendableNotes(
        assetDefinitionId: String,
        requestedAmount: OfflineCanonicalNumeric
    ) throws -> [OfflineNoteV2WalletNote] {
        var selected: [OfflineNoteV2WalletNote] = []
        var total = OfflineCanonicalNumeric(isNegative: false, scale: 0, digits: "0")
        for note in try store.listNotes() {
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

    private func validatePaymentToken(_ paymentToken: OfflineNoteV2PaymentToken) throws {
        guard paymentToken.chainId == chainId else {
            throw OfflineNoteV2WalletError.chainMismatch
        }
        try paymentToken.audit.validateProofBinding()
        let expectedTokenId = try OfflineNotePaymentTokenIdPreimageV2(
            chainId: paymentToken.chainId,
            paymentRequestId: paymentToken.paymentRequestId,
            createdAtMs: paymentToken.createdAtMs,
            tokenNonce: paymentToken.tokenNonce,
            senderKeyCertificatePayloadHash: paymentToken.audit.senderKeyCertificate.payloadHash(),
            inputNullifiers: paymentToken.audit.inputNullifiers,
            outputCommitments: paymentToken.audit.outputCommitments
        ).derivePaymentTokenId()
        guard paymentToken.audit.tokenId == paymentToken.tokenId,
              paymentToken.tokenId == expectedTokenId
        else {
            throw OfflineNoteV2PaymentTokenCodecError.tokenIdMismatch
        }
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
        case .redeemPending:
            return true
        case .receivePending, .spendable, .spent, .redeemed, .cancelled:
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
