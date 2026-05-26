import Foundation
import CryptoKit
#if canImport(Security)
import Security
#endif

public enum OfflineNoteWalletNoteState: String, Sendable {
    case spendable
    case receivePending
    case spent
    case redeemPending
    case redeemed
    case cancelled
}

public enum OfflineNoteWalletError: Error, LocalizedError, Equatable {
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
    case certificateVerificationFailed
    case missingBearerAuditTrail

    public var errorDescription: String? {
        switch self {
        case .missingIssuerClient:
            return "Offline Note issuer client is required for load."
        case .missingTransactionSubmitter:
            return "Offline Note transaction submitter is required."
        case .issuerCommitmentMismatch:
            return "Issuer returned a different Offline Note commitment."
        case .insufficientBalance:
            return "Insufficient spendable Offline Note balance."
        case let .randomLength(expected, actual):
            return "Offline Note random source must return \(expected) bytes, got \(actual)."
        case .chainMismatch:
            return "Receive request chainId does not match wallet chainId."
        case .noPendingOutput:
            return "Payment token has no pending output for this wallet."
        case .outputMismatch:
            return "Payment token output does not match the pending receive request."
        case .invalidState:
            return "Offline Note note is not in the required wallet state."
        case .inputCertificateMismatch:
            return "Selected Offline Note input notes must use the same key certificate."
        case .proofVerificationFailed:
            return "Offline Note recursive proof verification failed."
        case .certificateVerificationFailed:
            return "Offline Note key certificate is not trusted for this wallet operation."
        case .missingBearerAuditTrail:
            return "Offline Note bearer note is missing the audit trail required for defunding."
        }
    }
}

public struct OfflineNoteWalletNote: Equatable, Sendable {
    public let chainId: String
    public let accountId: String
    public let assetId: String
    public let amount: String
    public let keyCertificate: OfflineNoteKeyCertificate
    public let noteCommitment: Data
    public let noteSecret: Data
    public let origin: OfflineNoteCommitmentOrigin
    public let bearerAuditTrail: [OfflineNoteAuditBundle]
    public let state: OfflineNoteWalletNoteState
    public let createdAtMs: UInt64
    public let updatedAtMs: UInt64

    public init(chainId: String,
                accountId: String,
                assetId: String,
                amount: String,
                keyCertificate: OfflineNoteKeyCertificate,
                noteCommitment: Data,
                noteSecret: Data,
                origin: OfflineNoteCommitmentOrigin,
                bearerAuditTrail: [OfflineNoteAuditBundle] = [],
                state: OfflineNoteWalletNoteState,
                createdAtMs: UInt64,
                updatedAtMs: UInt64) throws {
        try OfflineNoteValidation.validateRandomBytes(noteSecret, field: "note_secret")
        self.chainId = chainId
        self.accountId = accountId
        self.assetId = try OfflineNorito.canonicalAssetIdLiteral(assetId)
        self.amount = try OfflineNorito.parseCanonicalNumeric(amount).canonicalString
        self.keyCertificate = keyCertificate
        self.noteCommitment = noteCommitment
        self.noteSecret = noteSecret
        self.origin = origin
        self.bearerAuditTrail = Self.normalizedBearerAuditTrail(bearerAuditTrail)
        self.state = state
        self.createdAtMs = createdAtMs
        self.updatedAtMs = updatedAtMs
        _ = try issuedClaim()
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

    public func withState(_ state: OfflineNoteWalletNoteState, updatedAtMs: UInt64) throws -> OfflineNoteWalletNote {
        try OfflineNoteWalletNote(
            chainId: chainId,
            accountId: accountId,
            assetId: assetId,
            amount: amount,
            keyCertificate: keyCertificate,
            noteCommitment: noteCommitment,
            noteSecret: noteSecret,
            origin: origin,
            bearerAuditTrail: bearerAuditTrail,
            state: state,
            createdAtMs: createdAtMs,
            updatedAtMs: updatedAtMs
        )
    }

    public func withBearerAuditTrail(_ bearerAuditTrail: [OfflineNoteAuditBundle],
                                     updatedAtMs: UInt64) throws -> OfflineNoteWalletNote {
        try OfflineNoteWalletNote(
            chainId: chainId,
            accountId: accountId,
            assetId: assetId,
            amount: amount,
            keyCertificate: keyCertificate,
            noteCommitment: noteCommitment,
            noteSecret: noteSecret,
            origin: origin,
            bearerAuditTrail: bearerAuditTrail,
            state: state,
            createdAtMs: createdAtMs,
            updatedAtMs: updatedAtMs
        )
    }

    private static func normalizedBearerAuditTrail(
        _ audits: [OfflineNoteAuditBundle]
    ) -> [OfflineNoteAuditBundle] {
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

public protocol OfflineNoteStore: AnyObject {
    /// Atomically mutate wallet notes keyed by lower-case note commitment hex.
    func mutateNotes<T>(_ body: (inout [String: OfflineNoteWalletNote]) throws -> T) throws -> T
}

public extension OfflineNoteStore {
    /// List wallet notes; persistent implementations may throw storage or integrity errors.
    func listNotes() throws -> [OfflineNoteWalletNote] {
        try mutateNotes { notes in Array(notes.values) }
    }

    /// Find a wallet note by commitment; persistent implementations may throw storage or integrity errors.
    func findNote(noteCommitment: Data) throws -> OfflineNoteWalletNote? {
        try mutateNotes { notes in notes[noteCommitment.hexLowercased()] }
    }

    /// Insert or replace a wallet note; persistent implementations may throw storage or integrity errors.
    func upsert(_ note: OfflineNoteWalletNote) throws {
        try mutateNotes { notes in
            notes[note.noteCommitmentHex] = note
        }
    }
}

public final class InMemoryOfflineNoteStore: OfflineNoteStore {
    private var notes: [String: OfflineNoteWalletNote] = [:]
    private let lock = NSLock()

    public init() {}

    public func mutateNotes<T>(_ body: (inout [String: OfflineNoteWalletNote]) throws -> T) throws -> T {
        lock.lock()
        defer { lock.unlock() }
        return try body(&notes)
    }
}

public protocol OfflineNoteAttestationProvider {
    func currentKeyCertificate() throws -> OfflineNoteKeyCertificate
}

public protocol OfflineNoteRandomSource {
    func nextBytes(count: Int) throws -> Data
}

public struct SecureOfflineNoteRandomSource: OfflineNoteRandomSource {
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

public protocol OfflineNoteIdGenerator {
    func nextId(prefix: String) -> String
}

public struct UuidOfflineNoteIdGenerator: OfflineNoteIdGenerator {
    public init() {}

    public func nextId(prefix: String) -> String {
        "\(prefix)-\(UUID().uuidString)"
    }
}

public protocol OfflineNoteProofProvider {
    func proveAudit(_ audit: OfflineNoteAuditBundle) throws -> OfflineNoteRecursiveProof
    func proveRedeem(_ redemption: OfflineNoteRedeem) throws -> OfflineNoteRecursiveProof
}

public protocol OfflineNoteProofVerifier {
    func verifyAudit(_ audit: OfflineNoteAuditBundle) throws -> Bool
    func verifyRedeem(_ redemption: OfflineNoteRedeem) throws -> Bool
}

public struct Halo2OfflineNoteProofVerifier: OfflineNoteProofVerifier {
    public init() {}

    public func verifyAudit(_ audit: OfflineNoteAuditBundle) throws -> Bool {
        try Halo2OfflineNoteProver.verifyAudit(audit)
    }

    public func verifyRedeem(_ redemption: OfflineNoteRedeem) throws -> Bool {
        try Halo2OfflineNoteProver.verifyRedeem(redemption)
    }
}

public protocol OfflineNoteCertificateVerifier {
    func verifyCertificate(_ certificate: OfflineNoteKeyCertificate) throws -> Bool
}

public struct RejectingOfflineNoteCertificateVerifier: OfflineNoteCertificateVerifier {
    public init() {}

    public func verifyCertificate(_ certificate: OfflineNoteKeyCertificate) throws -> Bool {
        false
    }
}

public struct Ed25519OfflineNoteCertificateVerifier: OfflineNoteCertificateVerifier {
    private let trustedIssuerPublicKeys: [Data]

    public init(trustedIssuerPublicKeys: [Data]) {
        self.trustedIssuerPublicKeys = trustedIssuerPublicKeys
    }

    public func verifyCertificate(_ certificate: OfflineNoteKeyCertificate) throws -> Bool {
        guard !trustedIssuerPublicKeys.isEmpty,
              !certificate.platform.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !certificate.keyId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !certificate.deviceId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !certificate.assertionScheme.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !certificate.assertionKeyAlgorithm.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !certificate.assertionPublicKey.isEmpty
        else {
            return false
        }
        let message = try certificate.signingBytes()
        for root in trustedIssuerPublicKeys where root.count == 32 {
            let publicKey = try Curve25519.Signing.PublicKey(rawRepresentation: root)
            if publicKey.isValidSignature(certificate.issuerSignature, for: message) {
                return true
            }
        }
        return false
    }
}

public struct OfflineNoteLoadContext: Sendable {
    public let operationId: String
    public let lineageId: String
    public let localRevision: UInt64
    public let keyCertificate: OfflineNoteKeyCertificate

    public init(operationId: String,
                lineageId: String,
                localRevision: UInt64,
                keyCertificate: OfflineNoteKeyCertificate) {
        self.operationId = operationId
        self.lineageId = lineageId
        self.localRevision = localRevision
        self.keyCertificate = keyCertificate
    }
}

public struct OfflineNoteIssueRequest: Sendable {
    public let chainId: String
    public let accountId: String
    public let assetDefinitionId: String
    public let assetId: String
    public let amount: String
    public let loadContext: OfflineNoteLoadContext
    public let noteCommitment: Data

    public init(chainId: String,
                accountId: String,
                assetDefinitionId: String,
                assetId: String,
                amount: String,
                loadContext: OfflineNoteLoadContext,
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

public struct OfflineNoteIssueResponse: Sendable {
    public let noteCommitment: Data
    public let operationId: String
    public let lineageId: String
    public let localRevision: UInt64
    public let keyCertificate: OfflineNoteKeyCertificate?
    public let settlementEntryHashHex: String?

    public init(noteCommitment: Data,
                operationId: String,
                lineageId: String,
                localRevision: UInt64,
                keyCertificate: OfflineNoteKeyCertificate? = nil,
                settlementEntryHashHex: String? = nil) {
        self.noteCommitment = noteCommitment
        self.operationId = operationId
        self.lineageId = lineageId
        self.localRevision = localRevision
        self.keyCertificate = keyCertificate
        self.settlementEntryHashHex = settlementEntryHashHex
    }
}

public protocol OfflineNoteIssuerClient {
    func prepareLoad(chainId: String,
                     accountId: String,
                     assetDefinitionId: String,
                     amount: String) async throws -> OfflineNoteLoadContext
    func issueNote(_ request: OfflineNoteIssueRequest) async throws -> OfflineNoteIssueResponse
}

public struct OfflineNoteReceiveRequest: Equatable, Sendable {
    public let chainId: String
    public let paymentRequestId: String
    public let accountId: String
    public let assetDefinitionId: String
    public let assetId: String
    public let amount: String
    public let keyCertificate: OfflineNoteKeyCertificate
    public let outputCommitment: Data

    public init(chainId: String,
                paymentRequestId: String,
                accountId: String,
                assetDefinitionId: String,
                assetId: String,
                amount: String,
                keyCertificate: OfflineNoteKeyCertificate,
                outputCommitment: Data) throws {
        self.chainId = chainId
        self.paymentRequestId = paymentRequestId
        self.accountId = accountId
        self.assetDefinitionId = assetDefinitionId
        self.assetId = try OfflineNorito.canonicalAssetIdLiteral(assetId)
        self.amount = try OfflineNorito.parseCanonicalNumeric(amount).canonicalString
        self.keyCertificate = keyCertificate
        self.outputCommitment = outputCommitment
        _ = try OfflineNoteAuditOutputClaim(
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

public enum OfflineNoteReceiveRequestCodecError: Error, LocalizedError, Equatable {
    case invalidField(String)
    case invalidPrefix

    public var errorDescription: String? {
        switch self {
        case let .invalidField(field):
            return "Offline Note receive request field \(field) is invalid."
        case .invalidPrefix:
            return "Offline Note receive request prefix missing."
        }
    }
}

public enum OfflineNoteReceiveRequestCodec {
    public static let type = "offline_receive_request"
    public static let textPrefix = "wallet-offline-receive:"
    private static let envelopeTypeName = "iroha_data_model::offline::model::OfflineNoteReceiveRequestEnvelope"

    public static func encodeNorito(_ request: OfflineNoteReceiveRequest) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(request.chainId))
        writer.writeField(OfflineCompactNorito.encodeString(request.paymentRequestId))
        writer.writeField(OfflineCompactNorito.encodeString(request.accountId))
        writer.writeField(OfflineCompactNorito.encodeString(request.assetDefinitionId))
        writer.writeField(OfflineCompactNorito.encodeString(request.assetId))
        writer.writeField(OfflineCompactNorito.encodeString(request.amount))
        writer.writeField(OfflineNorito.encodeBytesVec(try request.keyCertificate.noritoEncoded()))
        writer.writeField(try OfflineCompactNorito.encodeHash(request.outputCommitment))
        return noritoEncode(typeName: envelopeTypeName, payload: writer.data, flags: NoritoHeader.compactLen)
    }

    public static func decodeNorito(_ payload: Data) throws -> OfflineNoteReceiveRequest {
        guard let frame = noritoDecodeFrame(payload) else {
            throw OfflineNoteReceiveRequestCodecError.invalidField("payload")
        }
        guard frame.header.schema == noritoSchemaHash(forTypeName: envelopeTypeName) else {
            throw OfflineNoteReceiveRequestCodecError.invalidField("schema")
        }
        guard frame.header.compression == .none,
              (frame.header.flags & NoritoHeader.compactLen) != 0
        else {
            throw OfflineNoteReceiveRequestCodecError.invalidField("layout")
        }
        var reader = OfflineNoritoReader(data: frame.payload)
        let chainId = try field(&reader, "chain_id", readString)
        let paymentRequestId = try field(&reader, "payment_request_id", readString)
        let accountId = try field(&reader, "account_id", readString)
        let assetDefinitionId = try field(&reader, "asset_definition_id", readString)
        let assetId = try field(&reader, "asset_id", readString)
        let amount = try field(&reader, "amount", readString)
        let certificateBytes = try field(&reader, "key_certificate", readBytesVec)
        let outputCommitment = try field(&reader, "output_commitment") {
            try readHash(reader: &$0, field: "output_commitment")
        }
        guard reader.remaining() == 0 else {
            throw OfflineNoteReceiveRequestCodecError.invalidField("trailing_bytes")
        }
        return try OfflineNoteReceiveRequest(
            chainId: chainId,
            paymentRequestId: paymentRequestId,
            accountId: accountId,
            assetDefinitionId: assetDefinitionId,
            assetId: assetId,
            amount: amount,
            keyCertificate: OfflineNoteDecoding.decodeKeyCertificate(certificateBytes),
            outputCommitment: outputCommitment
        )
    }

    public static func encodeJson(_ request: OfflineNoteReceiveRequest) throws -> Data {
        try encodeNorito(request)
    }

    public static func decodeJson(_ payload: Data) throws -> OfflineNoteReceiveRequest {
        try decodeNorito(payload)
    }

    public static func encodeText(_ request: OfflineNoteReceiveRequest) throws -> String {
        textPrefix + base64UrlEncode(try encodeNorito(request))
    }

    public static func decodeText(_ text: String) throws -> OfflineNoteReceiveRequest {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix(textPrefix) else {
            throw OfflineNoteReceiveRequestCodecError.invalidPrefix
        }
        guard let payload = base64UrlDecode(String(trimmed.dropFirst(textPrefix.count))) else {
            throw OfflineNoteReceiveRequestCodecError.invalidField("payload")
        }
        return try decodeNorito(payload)
    }

    public static func encodeQrFrameBytes(
        _ request: OfflineNoteReceiveRequest,
        options: OfflineQrStreamOptions = OfflineQrStreamOptions()
    ) throws -> [Data] {
        try OfflineQrStreamEncoder.encodeFrameBytes(
            payload: encodeNorito(request),
            payloadKind: .offlineReceiveRequest,
            options: options
        )
    }

    public static func decodeQrPayload(_ payload: Data) throws -> OfflineNoteReceiveRequest {
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
            throw OfflineNoteReceiveRequestCodecError.invalidField(field)
        }
        return value
    }

    private static func readString(_ reader: inout OfflineNoritoReader) throws -> String {
        let length = try reader.readVarint()
        guard length <= UInt64(Int.max) else {
            throw OfflineNoteReceiveRequestCodecError.invalidField("string")
        }
        guard let value = String(data: try reader.readBytes(Int(length)), encoding: .utf8),
              !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else {
            throw OfflineNoteReceiveRequestCodecError.invalidField("string")
        }
        return value
    }

    private static func readBytesVec(_ reader: inout OfflineNoritoReader) throws -> Data {
        let length = try reader.readUInt64LE()
        guard length <= UInt64(Int.max) else {
            throw OfflineNoteReceiveRequestCodecError.invalidField("bytes")
        }
        return try reader.readBytes(Int(length))
    }

    private static func readHash(reader: inout OfflineNoritoReader, field: String) throws -> Data {
        let bytes = try reader.readBytes(32)
        guard bytes.count == 32 else {
            throw OfflineNoteReceiveRequestCodecError.invalidField(field)
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

public struct OfflineNotePaymentToken: Equatable, Sendable {
    public let chainId: String
    public let paymentRequestId: String
    public let tokenNonce: Data
    public let tokenId: Data
    public let audit: OfflineNoteAuditBundle
    public let bearerAuditTrail: [OfflineNoteAuditBundle]
    public let createdAtMs: UInt64

    public init(chainId: String,
                paymentRequestId: String,
                tokenNonce: Data,
                tokenId: Data,
                audit: OfflineNoteAuditBundle,
                bearerAuditTrail: [OfflineNoteAuditBundle]? = nil,
                createdAtMs: UInt64) {
        self.chainId = chainId
        self.paymentRequestId = paymentRequestId
        self.tokenNonce = tokenNonce
        self.tokenId = tokenId
        self.audit = audit
        self.bearerAuditTrail = bearerAuditTrail ?? [audit]
        self.createdAtMs = createdAtMs
    }

    public var tokenIdHex: String {
        tokenId.hexLowercased()
    }

    public func outputClaim(matchingNoteCommitment noteCommitment: Data) -> OfflineNoteAuditOutputClaim? {
        audit.outputClaim(matchingNoteCommitment: noteCommitment)
    }

    public func outputClaim(matchingNoteCommitmentHex noteCommitmentHex: String) -> OfflineNoteAuditOutputClaim? {
        audit.outputClaim(matchingNoteCommitmentHex: noteCommitmentHex)
    }

    public func containsOutputNoteCommitment(_ noteCommitment: Data) -> Bool {
        audit.containsOutputNoteCommitment(noteCommitment)
    }

    public func containsOutputNoteCommitment(hex noteCommitmentHex: String) -> Bool {
        audit.containsOutputNoteCommitment(hex: noteCommitmentHex)
    }

}

public enum OfflineNotePaymentTokenCodecError: Error, LocalizedError, Equatable {
    case invalidJson
    case invalidField(String)
    case invalidPrefix
    case tokenIdMismatch

    public var errorDescription: String? {
        switch self {
        case .invalidJson:
            return "Offline Note payment token payload is invalid."
        case let .invalidField(field):
            return "Offline Note payment token field \(field) is invalid."
        case .invalidPrefix:
            return "Offline Note payment token prefix missing."
        case .tokenIdMismatch:
            return "Offline Note payment token id does not match the audit bundle."
        }
    }
}

public enum OfflineNotePaymentTokenCodec {
    public static let type = "offline_payment_token"
    public static let textPrefix = "wallet-offline-payment:"
    private static let envelopeTypeName = "iroha_data_model::offline::model::OfflineNotePaymentTokenEnvelope"

    public static func encodeNorito(_ token: OfflineNotePaymentToken) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(token.chainId))
        writer.writeField(OfflineCompactNorito.encodeString(token.paymentRequestId))
        writer.writeField(OfflineCompactNorito.encodeUInt64(token.createdAtMs))
        writer.writeField(OfflineNorito.encodeBytesVec(token.tokenNonce))
        writer.writeField(try OfflineCompactNorito.encodeHash(token.tokenId))
        writer.writeField(OfflineNorito.encodeBytesVec(try token.audit.noritoEncoded()))
        writer.writeField(try encodeAuditTrail(token.bearerAuditTrail))
        return noritoEncode(typeName: envelopeTypeName, payload: writer.data, flags: NoritoHeader.compactLen)
    }

    public static func decodeNorito(_ payload: Data) throws -> OfflineNotePaymentToken {
        guard let frame = noritoDecodeFrame(payload) else {
            throw OfflineNotePaymentTokenCodecError.invalidField("payload")
        }
        guard frame.header.schema == noritoSchemaHash(forTypeName: envelopeTypeName) else {
            throw OfflineNotePaymentTokenCodecError.invalidField("schema")
        }
        guard frame.header.compression == .none,
              (frame.header.flags & NoritoHeader.compactLen) != 0
        else {
            throw OfflineNotePaymentTokenCodecError.invalidField("layout")
        }
        var reader = OfflineNoritoReader(data: frame.payload)
        let chainId = try field(&reader, "chain_id", readString)
        let paymentRequestId = try field(&reader, "payment_request_id", readString)
        let createdAtMs = try field(&reader, "created_at_ms") { try $0.readUInt64LE() }
        let tokenNonce = try field(&reader, "token_nonce", readBytesVec)
        let tokenId = try field(&reader, "token_id") { try readHash(reader: &$0, field: "token_id") }
        let auditBytes = try field(&reader, "audit", readBytesVec)
        let bearerAuditTrail = try field(&reader, "bearer_audit_trail", readAuditTrail)
        guard reader.remaining() == 0 else {
            throw OfflineNotePaymentTokenCodecError.invalidField("trailing_bytes")
        }
        let audit = try OfflineNoteDecoding.decodeAudit(auditBytes)
        guard audit.tokenId == tokenId else {
            throw OfflineNotePaymentTokenCodecError.tokenIdMismatch
        }
        return OfflineNotePaymentToken(
            chainId: chainId,
            paymentRequestId: paymentRequestId,
            tokenNonce: tokenNonce,
            tokenId: tokenId,
            audit: audit,
            bearerAuditTrail: bearerAuditTrail,
            createdAtMs: createdAtMs
        )
    }

    public static func encodeJson(_ token: OfflineNotePaymentToken) throws -> Data {
        try encodeNorito(token)
    }

    public static func decodeJson(_ payload: Data) throws -> OfflineNotePaymentToken {
        try decodeNorito(payload)
    }

    public static func encodeText(_ token: OfflineNotePaymentToken) throws -> String {
        textPrefix + base64UrlEncode(try encodeNorito(token))
    }

    public static func decodeText(_ text: String) throws -> OfflineNotePaymentToken {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix(textPrefix) else {
            throw OfflineNotePaymentTokenCodecError.invalidPrefix
        }
        guard let payload = base64UrlDecode(String(trimmed.dropFirst(textPrefix.count))) else {
            throw OfflineNotePaymentTokenCodecError.invalidField("payload")
        }
        return try decodeNorito(payload)
    }

    public static func encodeQrFrameBytes(
        _ token: OfflineNotePaymentToken,
        options: OfflineQrStreamOptions = OfflineQrStreamOptions()
    ) throws -> [Data] {
        try OfflineQrStreamEncoder.encodeFrameBytes(
            payload: encodeNorito(token),
            payloadKind: .offlinePaymentToken,
            options: options
        )
    }

    public static func decodeQrPayload(_ payload: Data) throws -> OfflineNotePaymentToken {
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
            throw OfflineNotePaymentTokenCodecError.invalidField(field)
        }
        return value
    }

    private static func encodeAuditTrail(_ audits: [OfflineNoteAuditBundle]) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt64LE(UInt64(audits.count))
        for audit in audits {
            writer.writeField(OfflineNorito.encodeBytesVec(try audit.noritoEncoded()))
        }
        return writer.data
    }

    private static func readAuditTrail(_ reader: inout OfflineNoritoReader) throws -> [OfflineNoteAuditBundle] {
        let count = try reader.readUInt64LE()
        guard count <= UInt64(Int.max) else {
            throw OfflineNotePaymentTokenCodecError.invalidField("bearer_audit_trail")
        }
        var audits: [OfflineNoteAuditBundle] = []
        audits.reserveCapacity(Int(count))
        for index in 0..<count {
            var auditReader = OfflineNoritoReader(data: try reader.readCompactField())
            let auditBytes = try readBytesVec(&auditReader)
            guard auditReader.remaining() == 0 else {
                throw OfflineNotePaymentTokenCodecError.invalidField("bearer_audit_trail[\(index)]")
            }
            do {
                audits.append(try OfflineNoteDecoding.decodeAudit(auditBytes))
            } catch {
                throw OfflineNotePaymentTokenCodecError.invalidField("bearer_audit_trail[\(index)]")
            }
        }
        return audits
    }

    private static func readString(_ reader: inout OfflineNoritoReader) throws -> String {
        let length = try reader.readVarint()
        guard length <= UInt64(Int.max) else {
            throw OfflineNotePaymentTokenCodecError.invalidField("string")
        }
        guard let value = String(data: try reader.readBytes(Int(length)), encoding: .utf8),
              !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else {
            throw OfflineNotePaymentTokenCodecError.invalidField("string")
        }
        return value
    }

    private static func readBytesVec(_ reader: inout OfflineNoritoReader) throws -> Data {
        let length = try reader.readUInt64LE()
        guard length <= UInt64(Int.max) else {
            throw OfflineNotePaymentTokenCodecError.invalidField("bytes")
        }
        return try reader.readBytes(Int(length))
    }

    private static func readHash(reader: inout OfflineNoritoReader, field: String) throws -> Data {
        let bytes = try reader.readBytes(32)
        guard bytes.count == 32 else {
            throw OfflineNotePaymentTokenCodecError.invalidField(field)
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

public struct OfflineNoteReceiptAck: Equatable, Sendable {
    public let chainId: String
    public let paymentRequestId: String
    public let tokenId: Data
    public let recipientAccountId: String
    public let acceptedAtMs: UInt64

    public init(chainId: String,
                paymentRequestId: String,
                tokenId: Data,
                recipientAccountId: String,
                acceptedAtMs: UInt64) throws {
        guard !chainId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw OfflineNoteReceiptAckCodecError.invalidField("chain_id")
        }
        guard !paymentRequestId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw OfflineNoteReceiptAckCodecError.invalidField("payment_request_id")
        }
        try OfflineNoteValidation.validateHash(tokenId, field: "token_id")
        guard !recipientAccountId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw OfflineNoteReceiptAckCodecError.invalidField("recipient_account_id")
        }
        self.chainId = chainId
        self.paymentRequestId = paymentRequestId
        self.tokenId = tokenId
        self.recipientAccountId = recipientAccountId
        self.acceptedAtMs = acceptedAtMs
    }

    public var tokenIdHex: String {
        tokenId.hexLowercased()
    }

    public static func fromPaymentToken(
        _ token: OfflineNotePaymentToken,
        recipientAccountId: String,
        acceptedAtMs: UInt64
    ) throws -> OfflineNoteReceiptAck {
        let checkedRecipient = recipientAccountId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !checkedRecipient.isEmpty else {
            throw OfflineNoteReceiptAckCodecError.invalidField("recipient_account_id")
        }
        guard tokenHasRecipientOutput(token, recipientAccountId: checkedRecipient) else {
            throw OfflineNoteReceiptAckCodecError.tokenMismatch
        }
        return try OfflineNoteReceiptAck(
            chainId: token.chainId,
            paymentRequestId: token.paymentRequestId,
            tokenId: token.tokenId,
            recipientAccountId: checkedRecipient,
            acceptedAtMs: acceptedAtMs
        )
    }

    public func matchesPaymentToken(_ token: OfflineNotePaymentToken) -> Bool {
        chainId == token.chainId
            && paymentRequestId == token.paymentRequestId
            && tokenId == token.tokenId
            && Self.tokenHasRecipientOutput(token, recipientAccountId: recipientAccountId)
    }

    public func requireMatchesPaymentToken(_ token: OfflineNotePaymentToken) throws {
        guard matchesPaymentToken(token) else {
            throw OfflineNoteReceiptAckCodecError.tokenMismatch
        }
    }

    private static func tokenHasRecipientOutput(
        _ token: OfflineNotePaymentToken,
        recipientAccountId: String
    ) -> Bool {
        token.audit.outputClaims.contains { claim in
            claim.keyCertificate.accountId == recipientAccountId
        }
    }
}

public enum OfflineNoteReceiptAckCodecError: Error, LocalizedError, Equatable {
    case invalidField(String)
    case invalidPrefix
    case tokenMismatch

    public var errorDescription: String? {
        switch self {
        case let .invalidField(field):
            return "Offline Note receipt ACK field \(field) is invalid."
        case .invalidPrefix:
            return "Offline Note receipt ACK prefix missing."
        case .tokenMismatch:
            return "Offline Note receipt ACK does not match payment token."
        }
    }
}

public enum OfflineNoteReceiptAckCodec {
    public static let type = "offline_receipt_ack"
    public static let textPrefix = "wallet-offline-ack:"
    private static let envelopeTypeName = "iroha_data_model::offline::model::OfflineNoteReceiptAckEnvelope"

    public static func encodeNorito(_ ack: OfflineNoteReceiptAck) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(ack.chainId))
        writer.writeField(OfflineCompactNorito.encodeString(ack.paymentRequestId))
        writer.writeField(try OfflineCompactNorito.encodeHash(ack.tokenId))
        writer.writeField(OfflineCompactNorito.encodeString(ack.recipientAccountId))
        writer.writeField(OfflineCompactNorito.encodeUInt64(ack.acceptedAtMs))
        return noritoEncode(typeName: envelopeTypeName, payload: writer.data, flags: NoritoHeader.compactLen)
    }

    public static func decodeNorito(_ payload: Data) throws -> OfflineNoteReceiptAck {
        guard let frame = noritoDecodeFrame(payload) else {
            throw OfflineNoteReceiptAckCodecError.invalidField("payload")
        }
        guard frame.header.schema == noritoSchemaHash(forTypeName: envelopeTypeName) else {
            throw OfflineNoteReceiptAckCodecError.invalidField("schema")
        }
        guard frame.header.compression == .none,
              (frame.header.flags & NoritoHeader.compactLen) != 0
        else {
            throw OfflineNoteReceiptAckCodecError.invalidField("layout")
        }
        var reader = OfflineNoritoReader(data: frame.payload)
        let chainId = try field(&reader, "chain_id", readString)
        let paymentRequestId = try field(&reader, "payment_request_id", readString)
        let tokenId = try field(&reader, "token_id") { try readHash(reader: &$0, field: "token_id") }
        let recipientAccountId = try field(&reader, "recipient_account_id", readString)
        let acceptedAtMs = try field(&reader, "accepted_at_ms") { try $0.readUInt64LE() }
        guard reader.remaining() == 0 else {
            throw OfflineNoteReceiptAckCodecError.invalidField("trailing_bytes")
        }
        return try OfflineNoteReceiptAck(
            chainId: chainId,
            paymentRequestId: paymentRequestId,
            tokenId: tokenId,
            recipientAccountId: recipientAccountId,
            acceptedAtMs: acceptedAtMs
        )
    }

    public static func encodeText(_ ack: OfflineNoteReceiptAck) throws -> String {
        textPrefix + base64UrlEncode(try encodeNorito(ack))
    }

    public static func decodeText(_ text: String) throws -> OfflineNoteReceiptAck {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix(textPrefix) else {
            throw OfflineNoteReceiptAckCodecError.invalidPrefix
        }
        guard let payload = base64UrlDecode(String(trimmed.dropFirst(textPrefix.count))) else {
            throw OfflineNoteReceiptAckCodecError.invalidField("payload")
        }
        return try decodeNorito(payload)
    }

    public static func encodeQrFrameBytes(
        _ ack: OfflineNoteReceiptAck,
        options: OfflineQrStreamOptions = OfflineQrStreamOptions()
    ) throws -> [Data] {
        try OfflineQrStreamEncoder.encodeFrameBytes(
            payload: encodeNorito(ack),
            payloadKind: .offlineReceiptAck,
            options: options
        )
    }

    public static func decodeQrPayload(_ payload: Data) throws -> OfflineNoteReceiptAck {
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
            throw OfflineNoteReceiptAckCodecError.invalidField(field)
        }
        return value
    }

    private static func readString(_ reader: inout OfflineNoritoReader) throws -> String {
        let length = try reader.readVarint()
        guard length <= UInt64(Int.max) else {
            throw OfflineNoteReceiptAckCodecError.invalidField("string")
        }
        guard let value = String(data: try reader.readBytes(Int(length)), encoding: .utf8),
              !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else {
            throw OfflineNoteReceiptAckCodecError.invalidField("string")
        }
        return value
    }

    private static func readHash(reader: inout OfflineNoritoReader, field: String) throws -> Data {
        let bytes = try reader.readBytes(32)
        guard bytes.count == 32 else {
            throw OfflineNoteReceiptAckCodecError.invalidField(field)
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

public protocol OfflineNoteTransactionSubmitter {
    func submitAudit(_ audit: OfflineNoteAuditBundle) async throws
    func submitRedeem(_ redemption: OfflineNoteRedeem) async throws
    func submitDefund(_ redemption: OfflineNoteRedeem,
                      bearerAuditTrail: [OfflineNoteAuditBundle]) async throws
}

public struct OfflineNoteSyncResolution: Equatable, Sendable {
    public let state: OfflineNoteWalletNoteState
    public let transactionHashHex: String?

    public init(state: OfflineNoteWalletNoteState, transactionHashHex: String? = nil) {
        self.state = state
        self.transactionHashHex = transactionHashHex
    }
}

public protocol OfflineNoteSyncResolver {
    func resolvePendingNote(_ note: OfflineNoteWalletNote) async throws -> OfflineNoteSyncResolution?
}

public struct OfflineNoteExplorerInstructionOutcome: Equatable, Sendable {
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

public protocol OfflineNoteOutcomeProvider {
    func listOutcomes() async throws -> [OfflineNoteExplorerInstructionOutcome]
}

public final class OfflineNoteOutcomeIndex: @unchecked Sendable {
    public static let kindIssue = "IssueOfflineNote"
    public static let kindRedeem = "RedeemOfflineNote"
    public static let kindAudit = "AuditOfflineNote"

    private struct OutcomeHit {
        let transactionHashHex: String?
    }

    private var committedRedeems: [String: OutcomeHit] = [:]
    private var rejectedRedeems: [String: OutcomeHit] = [:]

    public init() {}

    @discardableResult
    public func recordCommittedAudit(_ audit: OfflineNoteAuditBundle,
                                     transactionHashHex: String? = nil) -> OfflineNoteOutcomeIndex {
        return self
    }

    @discardableResult
    public func recordRejectedAudit(_ audit: OfflineNoteAuditBundle,
                                    transactionHashHex: String? = nil) -> OfflineNoteOutcomeIndex {
        return self
    }

    @discardableResult
    public func recordCommittedRedeem(_ redemption: OfflineNoteRedeem,
                                      transactionHashHex: String? = nil) -> OfflineNoteOutcomeIndex {
        putFirst(&committedRedeems, key: redemption.sourceNoteCommitment, value: transactionHashHex)
        return self
    }

    @discardableResult
    public func recordRejectedRedeem(_ redemption: OfflineNoteRedeem,
                                     transactionHashHex: String? = nil) -> OfflineNoteOutcomeIndex {
        putFirst(&rejectedRedeems, key: redemption.sourceNoteCommitment, value: transactionHashHex)
        return self
    }

    public func resolve(_ note: OfflineNoteWalletNote) throws -> OfflineNoteSyncResolution? {
        switch note.state {
        case .redeemPending:
            return resolveRedeemPending(note)
        default:
            return nil
        }
    }

    private func resolveRedeemPending(_ note: OfflineNoteWalletNote) -> OfflineNoteSyncResolution? {
        let commitmentKey = note.noteCommitmentHex
        if let hit = committedRedeems[commitmentKey] {
            return OfflineNoteSyncResolution(state: .redeemed, transactionHashHex: hit.transactionHashHex)
        }
        if let hit = rejectedRedeems[commitmentKey] {
            return OfflineNoteSyncResolution(state: .spendable, transactionHashHex: hit.transactionHashHex)
        }
        return nil
    }

    private func putFirst(_ target: inout [String: OutcomeHit], key: Data, value: String?) {
        let hex = key.hexLowercased()
        if target[hex] == nil {
            target[hex] = OutcomeHit(transactionHashHex: value)
        }
    }

    public static func fromExplorerOutcomes(_ outcomes: [OfflineNoteExplorerInstructionOutcome]) throws -> OfflineNoteOutcomeIndex {
        let index = OfflineNoteOutcomeIndex()
        for outcome in outcomes {
            let status = outcome.transactionStatus.lowercased()
            let committed = status == "committed"
            let rejected = status == "rejected"
            guard committed || rejected else { continue }
            if outcome.kind.caseInsensitiveCompare(kindAudit) == .orderedSame {
                let audit = try OfflineNoteDecoding.decodeAuditInstruction(outcome.encodedInstruction)
                if committed {
                    index.recordCommittedAudit(audit, transactionHashHex: outcome.transactionHashHex)
                } else {
                    index.recordRejectedAudit(audit, transactionHashHex: outcome.transactionHashHex)
                }
            } else if outcome.kind.caseInsensitiveCompare(kindRedeem) == .orderedSame {
                let redemption = try OfflineNoteDecoding.decodeRedeemInstruction(outcome.encodedInstruction)
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

public final class OfflineNoteOutcomeIndexSyncResolver: OfflineNoteSyncResolver {
    private let provider: OfflineNoteOutcomeProvider

    public init(provider: OfflineNoteOutcomeProvider) {
        self.provider = provider
    }

    public func resolvePendingNote(_ note: OfflineNoteWalletNote) async throws -> OfflineNoteSyncResolution? {
        try OfflineNoteOutcomeIndex.fromExplorerOutcomes(
            try await provider.listOutcomes()
        ).resolve(note)
    }
}

@available(iOS 15.0, macOS 12.0, *)
public final class ToriiOfflineNoteOutcomeProvider: OfflineNoteOutcomeProvider {
    private let client: ToriiClient
    private let perPage: UInt64

    public init(client: ToriiClient, perPage: UInt64 = 100) {
        self.client = client
        self.perPage = perPage
    }

    public func listOutcomes() async throws -> [OfflineNoteExplorerInstructionOutcome] {
        let audit = try await fetch(kind: OfflineNoteOutcomeIndex.kindAudit)
        let redeem = try await fetch(kind: OfflineNoteOutcomeIndex.kindRedeem)
        return audit + redeem
    }

    private func fetch(kind: String) async throws -> [OfflineNoteExplorerInstructionOutcome] {
        let page = try await client.getExplorerInstructions(params: ToriiExplorerInstructionsParams(
            perPage: perPage,
            kind: kind
        ))
        return try page.items.map { item in
            let encoded = try encodedInstructionHex(from: item.box)
            guard let bytes = Data(hexString: encoded), !bytes.isEmpty else {
                throw OfflineNotePaymentTokenCodecError.invalidField("encoded")
            }
            return OfflineNoteExplorerInstructionOutcome(
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
        throw OfflineNotePaymentTokenCodecError.invalidField("encoded")
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
public final class IrohaOfflineNoteTransactionSubmitter: OfflineNoteTransactionSubmitter {
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

    public func submitAudit(_ audit: OfflineNoteAuditBundle) async throws {
        try await sdk.submit(
            auditOfflineNote: AuditOfflineNoteRequest(
                chainId: chainId,
                authority: authority,
                audit: audit
            ),
            signingKey: signingKey
        )
    }

    public func submitRedeem(_ redemption: OfflineNoteRedeem) async throws {
        try await sdk.submit(
            redeemOfflineNote: RedeemOfflineNoteRequest(
                chainId: chainId,
                authority: authority,
                redemption: redemption
            ),
            signingKey: signingKey
        )
    }

    public func submitDefund(_ redemption: OfflineNoteRedeem,
                             bearerAuditTrail: [OfflineNoteAuditBundle]) async throws {
        try await sdk.submit(
            defundOfflineNote: DefundOfflineNoteRequest(
                chainId: chainId,
                authority: authority,
                bearerAuditTrail: bearerAuditTrail,
                redemption: redemption
            ),
            signingKey: signingKey
        )
    }
}

public final class OfflineNoteWallet {
    private let chainId: String
    private let accountId: String
    private let attestationProvider: OfflineNoteAttestationProvider
    private let store: OfflineNoteStore
    private let issuerClient: OfflineNoteIssuerClient?
    private let transactionSubmitter: OfflineNoteTransactionSubmitter?
    private let syncResolver: OfflineNoteSyncResolver?
    private let proofProvider: OfflineNoteProofProvider
    private let proofVerifier: OfflineNoteProofVerifier
    private let certificateVerifier: OfflineNoteCertificateVerifier
    private let randomSource: OfflineNoteRandomSource
    private let idGenerator: OfflineNoteIdGenerator
    private let clock: () -> UInt64

    public init(chainId: String,
                accountId: String,
                attestationProvider: OfflineNoteAttestationProvider,
                store: OfflineNoteStore = InMemoryOfflineNoteStore(),
                issuerClient: OfflineNoteIssuerClient? = nil,
                transactionSubmitter: OfflineNoteTransactionSubmitter? = nil,
                syncResolver: OfflineNoteSyncResolver? = nil,
                proofProvider: OfflineNoteProofProvider,
                proofVerifier: OfflineNoteProofVerifier = Halo2OfflineNoteProofVerifier(),
                certificateVerifier: OfflineNoteCertificateVerifier = RejectingOfflineNoteCertificateVerifier(),
                randomSource: OfflineNoteRandomSource = SecureOfflineNoteRandomSource(),
                idGenerator: OfflineNoteIdGenerator = UuidOfflineNoteIdGenerator(),
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
        self.certificateVerifier = certificateVerifier
        self.randomSource = randomSource
        self.idGenerator = idGenerator
        self.clock = clock
    }

    public func listNotes() throws -> [OfflineNoteWalletNote] {
        try store.listNotes()
    }

    public func load(assetDefinitionId: String, amount: String) async throws -> OfflineNoteWalletNote {
        guard let issuerClient else {
            throw OfflineNoteWalletError.missingIssuerClient
        }
        let assetId = walletAssetId(assetDefinitionId: assetDefinitionId, accountId: accountId)
        // Pass the full `name#domain` assetDefinitionId to Torii — the
        // internal `assetId` is the SDK's 2-part `name#account` form
        // (domain stripped by walletAssetId), so deriving the definition
        // id from it would drop the domain and the server would reject
        // with `OFFLINE_INVALID_ASSET` (400) because Iroha asset
        // definition ids are always `name#domain`.
        let context = try await issuerClient.prepareLoad(
            chainId: chainId,
            accountId: accountId,
            assetDefinitionId: assetDefinitionId,
            amount: amount
        )
        try requireTrustedCertificate(context.keyCertificate, expectedAccountId: accountId)
        let noteSecret = try random32()
        let origin = try OfflineNoteCommitmentOrigin.issuerLoad(OfflineNoteIssuerLoadOrigin(
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
        let request = OfflineNoteIssueRequest(
            chainId: chainId,
            accountId: accountId,
            assetDefinitionId: assetDefinitionId,
            assetId: assetId,
            amount: amount,
            loadContext: context,
            noteCommitment: commitment
        )
        let response = try await issuerClient.issueNote(request)
        guard response.noteCommitment == commitment else {
            throw OfflineNoteWalletError.issuerCommitmentMismatch
        }
        let issuedCertificate = response.keyCertificate ?? context.keyCertificate
        try requireTrustedCertificate(issuedCertificate, expectedAccountId: accountId)
        let now = clock()
        let note = try OfflineNoteWalletNote(
            chainId: chainId,
            accountId: accountId,
            assetId: assetId,
            amount: amount,
            keyCertificate: issuedCertificate,
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

    public func prepareReceive(assetDefinitionId: String, amount: String) throws -> OfflineNoteReceiveRequest {
        let paymentRequestId = idGenerator.nextId(prefix: "payment-request")
        let keyCertificate = try attestationProvider.currentKeyCertificate()
        try requireTrustedCertificate(keyCertificate, expectedAccountId: accountId)
        let assetId = walletAssetId(assetDefinitionId: assetDefinitionId, accountId: accountId)
        let noteSecret = try random32()
        let origin = try OfflineNoteCommitmentOrigin.p2pOutput(OfflineNoteP2pOutputOrigin(
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
        let note = try OfflineNoteWalletNote(
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
        return try OfflineNoteReceiveRequest(
            chainId: chainId,
            paymentRequestId: paymentRequestId,
            accountId: accountId,
            // Preserve the full `name#domain` assetDefinitionId for the
            // peer / Torii — the SDK-internal `assetId` is the 2-part
            // `name#account` form and would drop the domain otherwise.
            assetDefinitionId: assetDefinitionId,
            assetId: assetId,
            amount: note.amount,
            keyCertificate: keyCertificate,
            outputCommitment: commitment
        )
    }

    public func pay(_ receiveRequest: OfflineNoteReceiveRequest) throws -> OfflineNotePaymentToken {
        guard receiveRequest.chainId == chainId else {
            throw OfflineNoteWalletError.chainMismatch
        }
        try requireTrustedCertificate(receiveRequest.keyCertificate, expectedAccountId: receiveRequest.accountId)
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
            throw OfflineNoteWalletError.insufficientBalance
        }

        let senderCertificate = selected[0].keyCertificate
        try requireTrustedCertificate(senderCertificate, expectedAccountId: accountId)
        let senderCertificateHash = try senderCertificate.payloadHash()
        for note in selected {
            _ = try bearerAuditTrail(for: note)
            try requireTrustedCertificate(note.keyCertificate, expectedAccountId: accountId)
            if try note.keyCertificate.payloadHash() != senderCertificateHash {
                throw OfflineNoteWalletError.inputCertificateMismatch
            }
        }
        let inputNullifiers = try selected.map(deriveInputNullifier)
        let inputClaims = try selected.map { try $0.issuedClaim() }
        var outputClaims = [
            try OfflineNoteAuditOutputClaim(
                noteCommitment: receiveRequest.outputCommitment,
                keyCertificate: receiveRequest.keyCertificate,
                assetId: receiveRequest.assetId,
                amount: receiveRequest.amount
            )
        ]
        let tokenNonce = try random32()
        var changeNote: OfflineNoteWalletNote?
        if changeAmount.digits != "0" {
            let changeSecret = try random32()
            let changeAssetId = walletAssetId(
                assetDefinitionId: receiveRequest.assetDefinitionId,
                accountId: accountId
            )
            let changeOrigin = try OfflineNoteCommitmentOrigin.p2pOutput(OfflineNoteP2pOutputOrigin(
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
            let note = try OfflineNoteWalletNote(
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
            outputClaims.append(try OfflineNoteAuditOutputClaim(
                noteCommitment: changeCommitment,
                keyCertificate: senderCertificate,
                assetId: changeAssetId,
                amount: note.amount
            ))
        }
        let outputCommitments = outputClaims.map(\.noteCommitment)
        let tokenId = try OfflineNotePaymentTokenIdPreimage(
            chainId: chainId,
            paymentRequestId: receiveRequest.paymentRequestId,
            createdAtMs: createdAtMs,
            tokenNonce: tokenNonce,
            senderKeyCertificatePayloadHash: senderCertificateHash,
            inputNullifiers: inputNullifiers,
            outputCommitments: outputCommitments
        ).derivePaymentTokenId()
        let draft = try OfflineNoteAuditBundle(
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
        try requireTrustedAuditCertificates(audit)
        guard try proofVerifier.verifyAudit(audit) else {
            throw OfflineNoteWalletError.proofVerificationFailed
        }
        let outputBearerAuditTrail = try bearerAuditTrail(forInputs: selected, appending: audit)
        try store.mutateNotes { notes in
            for note in selected {
                guard notes[note.noteCommitmentHex]?.state == .spendable else {
                    throw OfflineNoteWalletError.invalidState
                }
            }
            if let changeNote, notes[changeNote.noteCommitmentHex] != nil {
                throw OfflineNoteWalletError.invalidState
            }
            for note in selected {
                notes[note.noteCommitmentHex] = try note.withState(.spent, updatedAtMs: createdAtMs)
            }
            if let changeNote {
                notes[changeNote.noteCommitmentHex] = try changeNote.withBearerAuditTrail(
                    outputBearerAuditTrail,
                    updatedAtMs: createdAtMs
                )
            }
        }
        return OfflineNotePaymentToken(
            chainId: chainId,
            paymentRequestId: receiveRequest.paymentRequestId,
            tokenNonce: tokenNonce,
            tokenId: tokenId,
            audit: audit,
            bearerAuditTrail: outputBearerAuditTrail,
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
            throw OfflineNoteWalletError.invalidState
        }
    }

    private func bearerAuditTrail(for note: OfflineNoteWalletNote) throws -> [OfflineNoteAuditBundle] {
        switch note.origin {
        case .issuerLoad:
            return []
        case .p2pOutput:
            guard !note.bearerAuditTrail.isEmpty else {
                throw OfflineNoteWalletError.missingBearerAuditTrail
            }
            return note.bearerAuditTrail
        }
    }

    private func bearerAuditTrail(
        forInputs inputNotes: [OfflineNoteWalletNote],
        appending audit: OfflineNoteAuditBundle
    ) throws -> [OfflineNoteAuditBundle] {
        var seen: Set<String> = []
        var result: [OfflineNoteAuditBundle] = []
        for note in inputNotes {
            for inputAudit in try bearerAuditTrail(for: note) {
                let key = inputAudit.tokenId.hexLowercased()
                guard seen.insert(key).inserted else { continue }
                result.append(inputAudit)
            }
        }
        let auditKey = audit.tokenId.hexLowercased()
        if !seen.contains(auditKey) {
            result.append(audit)
        }
        return result
    }

    public func accept(_ paymentToken: OfflineNotePaymentToken) throws -> OfflineNoteWalletNote {
        try validatePaymentToken(paymentToken)
        guard try proofVerifier.verifyAudit(paymentToken.audit) else {
            throw OfflineNoteWalletError.proofVerificationFailed
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
                    throw OfflineNoteWalletError.outputMismatch
                }
                let now = clock()
                let accepted = try pending
                    .withState(.spendable, updatedAtMs: now)
                    .withBearerAuditTrail(paymentToken.bearerAuditTrail, updatedAtMs: now)
                notes[pending.noteCommitmentHex] = accepted
                return accepted
            }
            throw OfflineNoteWalletError.noPendingOutput
        }
        return accepted
    }

    public func publishAudit(_ paymentToken: OfflineNotePaymentToken) async throws {
        guard let transactionSubmitter else {
            throw OfflineNoteWalletError.missingTransactionSubmitter
        }
        try validatePaymentToken(paymentToken)
        guard try proofVerifier.verifyAudit(paymentToken.audit) else {
            throw OfflineNoteWalletError.proofVerificationFailed
        }
        try await transactionSubmitter.submitAudit(paymentToken.audit)
    }

    public func redeem(_ note: OfflineNoteWalletNote, recipient: String? = nil) async throws -> OfflineNoteWalletNote {
        guard let transactionSubmitter else {
            throw OfflineNoteWalletError.missingTransactionSubmitter
        }
        let current = try store.findNote(noteCommitment: note.noteCommitment) ?? note
        guard current.state == .spendable else {
            throw OfflineNoteWalletError.invalidState
        }
        let bearerAuditTrail = try bearerAuditTrail(for: current)
        try requireTrustedCertificate(current.keyCertificate, expectedAccountId: current.accountId)
        let inputNullifier = try deriveInputNullifier(current)
        let draft = try OfflineNoteRedeem(
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
        try requireTrustedCertificate(redemption.senderKeyCertificate, expectedAccountId: current.accountId)
        guard try proofVerifier.verifyRedeem(redemption) else {
            throw OfflineNoteWalletError.proofVerificationFailed
        }
        let pending = try store.mutateNotes { notes in
            let latest = notes[current.noteCommitmentHex] ?? current
            guard latest.state == .spendable else {
                throw OfflineNoteWalletError.invalidState
            }
            let pending = try latest.withState(.redeemPending, updatedAtMs: clock())
            notes[latest.noteCommitmentHex] = pending
            return pending
        }
        do {
            try await transactionSubmitter.submitDefund(redemption, bearerAuditTrail: bearerAuditTrail)
        } catch {
            try? rollbackRedeemReservation(pending)
            throw error
        }
        return pending
    }

    private func rollbackRedeemReservation(_ reserved: OfflineNoteWalletNote) throws {
        try store.mutateNotes { notes in
            guard let latest = notes[reserved.noteCommitmentHex],
                  latest == reserved,
                  latest.state == .redeemPending
            else {
                return
            }
            notes[reserved.noteCommitmentHex] = try latest.withState(.spendable, updatedAtMs: clock())
        }
    }

    public func sync() async throws -> [OfflineNoteWalletNote] {
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
    ) throws -> [OfflineNoteWalletNote] {
        var selected: [OfflineNoteWalletNote] = []
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
                throw OfflineNoteWalletError.insufficientBalance
            }
        }
        guard !selected.isEmpty,
              total.compared(to: requestedAmount) != .orderedAscending
        else {
            throw OfflineNoteWalletError.insufficientBalance
        }
        return selected
    }

    private func deriveNoteCommitment(keyCertificate: OfflineNoteKeyCertificate,
                                      assetId: String,
                                      amount: String,
                                      noteSecret: Data,
                                      origin: OfflineNoteCommitmentOrigin) throws -> Data {
        try OfflineNoteCommitmentPreimage(
            chainId: chainId,
            ownerKeyCertificatePayloadHash: keyCertificate.payloadHash(),
            assetId: assetId,
            amount: amount,
            noteSecret: noteSecret,
            origin: origin
        ).deriveNoteCommitment()
    }

    private func deriveInputNullifier(_ note: OfflineNoteWalletNote) throws -> Data {
        try OfflineNoteInputNullifierPreimage(
            chainId: chainId,
            sourceNoteCommitment: note.noteCommitment,
            ownerKeyCertificatePayloadHash: note.keyCertificate.payloadHash(),
            noteSecret: note.noteSecret
        ).deriveInputNullifier()
    }

    private func validatePaymentToken(_ paymentToken: OfflineNotePaymentToken) throws {
        guard paymentToken.chainId == chainId else {
            throw OfflineNoteWalletError.chainMismatch
        }
        try paymentToken.audit.validateProofBinding()
        let expectedTokenId = try OfflineNotePaymentTokenIdPreimage(
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
            throw OfflineNotePaymentTokenCodecError.tokenIdMismatch
        }
        try requireTrustedAuditCertificates(paymentToken.audit)
        try validateBearerAuditTrail(paymentToken.bearerAuditTrail, terminalAudit: paymentToken.audit)
    }

    private func validateBearerAuditTrail(
        _ audits: [OfflineNoteAuditBundle],
        terminalAudit: OfflineNoteAuditBundle
    ) throws {
        guard let last = audits.last, last == terminalAudit else {
            throw OfflineNotePaymentTokenCodecError.invalidField("bearer_audit_trail")
        }
        var tokenIds = Set<String>()
        var nullifiers = Set<String>()
        var outputs = Set<String>()
        var outputProducerIndex: [String: Int] = [:]
        for (index, audit) in audits.enumerated() {
            for output in audit.outputCommitments {
                let key = output.hexLowercased()
                guard outputProducerIndex[key] == nil else {
                    throw OfflineNotePaymentTokenCodecError.invalidField("bearer_audit_trail[\(index)].output_commitments")
                }
                outputProducerIndex[key] = index
            }
        }
        for (index, audit) in audits.enumerated() {
            try audit.validateProofBinding()
            guard tokenIds.insert(audit.tokenId.hexLowercased()).inserted else {
                throw OfflineNotePaymentTokenCodecError.invalidField("bearer_audit_trail[\(index)].token_id")
            }
            for nullifier in audit.inputNullifiers {
                guard nullifiers.insert(nullifier.hexLowercased()).inserted else {
                    throw OfflineNotePaymentTokenCodecError.invalidField("bearer_audit_trail[\(index)].input_nullifiers")
                }
            }
            for output in audit.outputCommitments {
                guard outputs.insert(output.hexLowercased()).inserted else {
                    throw OfflineNotePaymentTokenCodecError.invalidField("bearer_audit_trail[\(index)].output_commitments")
                }
            }
            for claim in audit.inputClaims {
                let noteCommitment = claim.noteCommitment.hexLowercased()
                if let producerIndex = outputProducerIndex[noteCommitment], producerIndex >= index {
                    throw OfflineNotePaymentTokenCodecError.invalidField("bearer_audit_trail[\(index)].input_claims")
                }
            }
            try requireTrustedAuditCertificates(audit)
            guard try proofVerifier.verifyAudit(audit) else {
                throw OfflineNoteWalletError.proofVerificationFailed
            }
        }
    }

    private func requireTrustedAuditCertificates(_ audit: OfflineNoteAuditBundle) throws {
        try requireTrustedCertificate(audit.senderKeyCertificate)
        let senderHash = try audit.senderKeyCertificate.payloadHash()
        for claim in audit.inputClaims {
            guard claim.keyCertificatePayloadHash == senderHash else {
                throw OfflineNoteWalletError.certificateVerificationFailed
            }
            try requireTrustedCertificate(audit.senderKeyCertificate, expectedAccountId: assetAccount(from: claim.assetId))
        }
        for output in audit.outputClaims {
            try requireTrustedCertificate(output.keyCertificate, expectedAccountId: assetAccount(from: output.assetId))
        }
    }

    private func requireTrustedCertificate(
        _ certificate: OfflineNoteKeyCertificate,
        expectedAccountId: String? = nil
    ) throws {
        if let expectedAccountId, certificate.accountId != expectedAccountId {
            throw OfflineNoteWalletError.certificateVerificationFailed
        }
        guard try certificateVerifier.verifyCertificate(certificate) else {
            throw OfflineNoteWalletError.certificateVerificationFailed
        }
    }

    private func random32() throws -> Data {
        let bytes = try randomSource.nextBytes(count: 32)
        guard bytes.count == 32 else {
            throw OfflineNoteWalletError.randomLength(expected: 32, actual: bytes.count)
        }
        return bytes
    }
}

private func placeholderProof() throws -> OfflineNoteRecursiveProof {
    try OfflineNoteRecursiveProof(
        publicInputsHash: IrohaHash.hash(Data("offline-note-draft-proof".utf8)),
        proofBytes: Data([0x01])
    )
}

private extension OfflineNoteWalletNoteState {
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

private func assetAccount(from assetId: String) -> String? {
    let parts = assetId.split(separator: "#", maxSplits: 1, omittingEmptySubsequences: false)
    guard parts.count == 2 else {
        return nil
    }
    return String(parts[1]).components(separatedBy: "#dataspace:").first
}
