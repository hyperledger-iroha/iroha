import Foundation

/// Unsigned registration transaction for external/secure-element signing.
public struct OfflineDeviceAttestationUnsignedTransaction: Sendable {
    /// Digest that the account signing service must sign.
    public let signingHash: Data

    fileprivate let transactionPayload: Data

    /// Attach one canonical Ed25519 signature and produce the Torii envelope.
    public func signed(signature: Data) throws -> SignedTransactionEnvelope {
        try AttestedOfflineNoteSwiftNoritoEncoder.finalizeUnsignedTransaction(
            transactionPayload: transactionPayload,
            signature: signature
        )
    }
}

private enum AttestedOfflineNoteSwiftNoritoEncoder {
    private static let signedTransactionWireVersion: UInt8 = 1

    static func encodeIssue(chainId: String,
                            authority: String,
                            creationTimeMs: UInt64,
                            ttlMs: UInt64?,
                            nonce: UInt32?,
                            issue: AttestedOfflineNoteIssue,
                            metadata: [String: ToriiJSONValue],
                            signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let instruction = try encodeInstruction(
            wireName: AttestedOfflineNoteTypeNames.issueInstruction,
            typeName: AttestedOfflineNoteTypeNames.issueInstruction,
            modelPayload: AttestedOfflineNoteEncoding.encodeIssue(issue)
        )
        return try encodeTransaction(
            chainId: chainId,
            authority: authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            nonce: nonce,
            instructionPayload: instruction,
            metadata: metadata,
            signingKey: signingKey
        )
    }

    static func encodeRedeem(chainId: String,
                             authority: String,
                             creationTimeMs: UInt64,
                             ttlMs: UInt64?,
                             nonce: UInt32?,
                             redemption: AttestedOfflineNoteRedeem,
                             metadata: [String: ToriiJSONValue],
                             signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        try redemption.validateProofBinding()
        let instruction = try encodeInstruction(
            wireName: AttestedOfflineNoteTypeNames.redeemInstruction,
            typeName: AttestedOfflineNoteTypeNames.redeemInstruction,
            modelPayload: AttestedOfflineNoteEncoding.encodeRedeem(redemption)
        )
        return try encodeTransaction(
            chainId: chainId,
            authority: authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            nonce: nonce,
            instructionPayload: instruction,
            metadata: metadata,
            signingKey: signingKey
        )
    }

    static func encodeAudit(chainId: String,
                            authority: String,
                            creationTimeMs: UInt64,
                            ttlMs: UInt64?,
                            nonce: UInt32?,
                            audit: AttestedOfflineNoteAuditBundle,
                            metadata: [String: ToriiJSONValue],
                            signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        try audit.validateProofBinding()
        let instruction = try encodeInstruction(
            wireName: AttestedOfflineNoteTypeNames.auditInstruction,
            typeName: AttestedOfflineNoteTypeNames.auditInstruction,
            modelPayload: AttestedOfflineNoteEncoding.encodeAudit(audit)
        )
        return try encodeTransaction(
            chainId: chainId,
            authority: authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            nonce: nonce,
            instructionPayload: instruction,
            metadata: metadata,
            signingKey: signingKey
        )
    }

    static func encodeRegisterDeviceAttestation(chainId: String,
                                                authority: String,
                                                creationTimeMs: UInt64,
                                                ttlMs: UInt64?,
                                                nonce: UInt32?,
                                                registration: OfflineDeviceAttestationRegistration,
                                                metadata: [String: ToriiJSONValue],
                                                signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let unsigned = try encodeRegisterDeviceAttestationUnsigned(
            chainId: chainId,
            authority: authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            nonce: nonce,
            registration: registration,
            metadata: metadata
        )
        return try unsigned.signed(signature: signingKey.sign(unsigned.signingHash))
    }

    static func encodeRegisterDeviceAttestationUnsigned(chainId: String,
                                                        authority: String,
                                                        creationTimeMs: UInt64,
                                                        ttlMs: UInt64?,
                                                        nonce: UInt32?,
                                                        registration: OfflineDeviceAttestationRegistration,
                                                        metadata: [String: ToriiJSONValue]) throws
        -> OfflineDeviceAttestationUnsignedTransaction {
        let instruction = try encodeInstruction(
            wireName: AttestedOfflineNoteTypeNames.registerDeviceAttestationInstruction,
            typeName: AttestedOfflineNoteTypeNames.registerDeviceAttestationInstruction,
            modelPayload: AttestedOfflineNoteEncoding.encodeDeviceAttestationRegistration(registration)
        )
        let payload = try encodeTransactionPayload(
            chainId: chainId,
            authority: authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            nonce: nonce,
            instructionPayload: instruction,
            metadata: metadata
        )
        return OfflineDeviceAttestationUnsignedTransaction(
            signingHash: IrohaHash.hash(payload),
            transactionPayload: payload
        )
    }

    private static func encodeInstruction(wireName: String,
                                          typeName: String,
                                          modelPayload: Data) -> Data {
        var concreteInstruction = OfflineCompactNoritoWriter()
        concreteInstruction.writeField(modelPayload)
        let framedInstruction = noritoEncode(
            typeName: typeName,
            payload: concreteInstruction.data,
            flags: NoritoHeader.compactLen
        )

        var instructionBox = OfflineNoritoWriter()
        instructionBox.writeField(OfflineNorito.encodeString(wireName))
        instructionBox.writeField(OfflineNorito.encodeBytesVec(framedInstruction))
        return instructionBox.data
    }

    private static func encodeTransaction(chainId: String,
                                          authority: String,
                                          creationTimeMs: UInt64,
                                          ttlMs: UInt64?,
                                          nonce: UInt32?,
                                          instructionPayload: Data,
                                          metadata: [String: ToriiJSONValue],
                                          signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let transactionPayload = try encodeTransactionPayload(
            chainId: chainId,
            authority: authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            nonce: nonce,
            instructionPayload: instructionPayload,
            metadata: metadata
        )
        let signature = try signingKey.sign(IrohaHash.hash(transactionPayload))
        let signedTransaction = encodeSignedTransaction(
            signature: signature,
            transactionPayload: transactionPayload
        )
        let transactionHash = IrohaHash.hash(encodeTransactionEntrypoint(signedTransaction))
        var norito = Data([signedTransactionWireVersion])
        norito.append(signedTransaction)
        return SignedTransactionEnvelope(
            norito: norito,
            signedTransaction: signedTransaction,
            payload: nil,
            transactionHash: transactionHash
        )
    }

    fileprivate static func finalizeUnsignedTransaction(
        transactionPayload: Data,
        signature: Data
    ) throws -> SignedTransactionEnvelope {
        guard signature.count == 64, signature.contains(where: { $0 != 0 }) else {
            throw AttestedOfflineNoteError.nonCanonicalField(field: "transaction_signature")
        }
        let signedTransaction = encodeSignedTransaction(
            signature: signature,
            transactionPayload: transactionPayload
        )
        let transactionHash = IrohaHash.hash(encodeTransactionEntrypoint(signedTransaction))
        var norito = Data([signedTransactionWireVersion])
        norito.append(signedTransaction)
        return SignedTransactionEnvelope(
            norito: norito,
            signedTransaction: signedTransaction,
            payload: nil,
            transactionHash: transactionHash
        )
    }

    private static func encodeTransactionPayload(chainId: String,
                                                 authority: String,
                                                 creationTimeMs: UInt64,
                                                 ttlMs: UInt64?,
                                                 nonce: UInt32?,
                                                 instructionPayload: Data,
                                                 metadata: [String: ToriiJSONValue]) throws -> Data {
        var transactionPayload = OfflineNoritoWriter()
        transactionPayload.writeField(OfflineNorito.encodeString(chainId))
        transactionPayload.writeField(OfflineNorito.encodeString(authority))
        transactionPayload.writeField(OfflineNorito.encodeUInt64(creationTimeMs))
        transactionPayload.writeField(encodeExecutable(instructionPayload: instructionPayload))
        transactionPayload.writeField(try OfflineNorito.encodeOption(ttlMs, encode: OfflineNorito.encodeUInt64))
        transactionPayload.writeField(try OfflineNorito.encodeOption(nonce, encode: OfflineNorito.encodeUInt32))
        transactionPayload.writeField(try OfflineNorito.encodeMetadata(metadata))
        return transactionPayload.data
    }

    private static func encodeExecutable(instructionPayload: Data) -> Data {
        var instructions = OfflineNoritoWriter()
        instructions.writeLength(1)
        instructions.writeField(instructionPayload)

        var executable = OfflineNoritoWriter()
        executable.writeUInt32LE(0)
        executable.writeField(instructions.data)
        return executable.data
    }

    private static func encodeSignedTransaction(signature: Data,
                                                transactionPayload: Data) -> Data {
        var signedTransaction = OfflineNoritoWriter()
        signedTransaction.writeField(OfflineNorito.encodeConstVec(signature))
        signedTransaction.writeField(transactionPayload)
        signedTransaction.writeField(Data([0]))
        signedTransaction.writeField(Data([0]))
        return signedTransaction.data
    }

    private static func encodeTransactionEntrypoint(_ signedTransaction: Data) -> Data {
        var entrypoint = OfflineCompactNoritoWriter()
        entrypoint.writeUInt32LE(0)
        entrypoint.writeField(signedTransaction)
        return entrypoint.data
    }

}

extension SwiftTransactionEncoder {
    private static func retiredAttestedOfflineNotePaymentTransaction() throws -> SignedTransactionEnvelope {
        throw SwiftTransactionEncoderError.retiredOfflineNotePayment
    }

    static func encodeAttestedOfflineNoteIssue(request: AttestedOfflineNoteIssueRequest,
                                         keypair: Keypair,
                                         creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeAttestedOfflineNoteIssue(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeMs
        )
    }

    static func encodeAttestedOfflineNoteIssue(request: AttestedOfflineNoteIssueRequest,
                                         signingKey: SigningKey,
                                         creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        _ = try TransactionInputValidator.validate(
            chainId: request.chainId,
            authorityId: request.authority
        )
        _ = (signingKey, creationTimeMs)
        return try retiredAttestedOfflineNotePaymentTransaction()
    }

    static func encodeAttestedOfflineNoteRedeem(request: AttestedOfflineNoteRedeemRequest,
                                          keypair: Keypair,
                                          creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeAttestedOfflineNoteRedeem(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeMs
        )
    }

    static func encodeAttestedOfflineNoteRedeem(request: AttestedOfflineNoteRedeemRequest,
                                          signingKey: SigningKey,
                                          creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        _ = try TransactionInputValidator.validate(
            chainId: request.chainId,
            authorityId: request.authority
        )
        try request.redemption.validateProofBinding()
        _ = (signingKey, creationTimeMs)
        return try retiredAttestedOfflineNotePaymentTransaction()
    }

    static func encodeAttestedOfflineNoteAudit(request: AttestedOfflineNoteAuditRequest,
                                         keypair: Keypair,
                                         creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeAttestedOfflineNoteAudit(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeMs
        )
    }

    static func encodeAttestedOfflineNoteAudit(request: AttestedOfflineNoteAuditRequest,
                                         signingKey: SigningKey,
                                         creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        _ = try TransactionInputValidator.validate(
            chainId: request.chainId,
            authorityId: request.authority
        )
        try request.audit.validateProofBinding()
        _ = (signingKey, creationTimeMs)
        return try retiredAttestedOfflineNotePaymentTransaction()
    }

    static func encodeRegisterOfflineDeviceAttestation(request: RegisterOfflineDeviceAttestationRequest,
                                                       keypair: Keypair,
                                                       creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeRegisterOfflineDeviceAttestation(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeMs
        )
    }

    static func encodeRegisterOfflineDeviceAttestation(request: RegisterOfflineDeviceAttestationRequest,
                                                       signingKey: SigningKey,
                                                       creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(
            chainId: request.chainId,
            authorityId: request.authority
        )
        return try AttestedOfflineNoteSwiftNoritoEncoder.encodeRegisterDeviceAttestation(
            chainId: ids.chainId,
            authority: ids.authorityId,
            creationTimeMs: creationTimeMs,
            ttlMs: request.ttlMs,
            nonce: request.nonce,
            registration: request.registration,
            metadata: request.metadata,
            signingKey: signingKey
        )
    }

    static func encodeUnsignedRegisterOfflineDeviceAttestation(
        request: RegisterOfflineDeviceAttestationRequest,
        creationTimeMs: UInt64
    ) throws -> OfflineDeviceAttestationUnsignedTransaction {
        let ids = try TransactionInputValidator.validate(
            chainId: request.chainId,
            authorityId: request.authority
        )
        return try AttestedOfflineNoteSwiftNoritoEncoder.encodeRegisterDeviceAttestationUnsigned(
            chainId: ids.chainId,
            authority: ids.authorityId,
            creationTimeMs: creationTimeMs,
            ttlMs: request.ttlMs,
            nonce: request.nonce,
            registration: request.registration,
            metadata: request.metadata
        )
    }
}

public extension IrohaSDK {
    func buildAttestedOfflineNoteIssue(request: AttestedOfflineNoteIssueRequest,
                                 keypair: Keypair) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeAttestedOfflineNoteIssue(
            request: request,
            keypair: keypair,
            creationTimeMs: creationTimeProvider()
        )
    }

    func buildAttestedOfflineNoteIssue(request: AttestedOfflineNoteIssueRequest,
                                 signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeAttestedOfflineNoteIssue(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeProvider()
        )
    }

    func buildAttestedOfflineNoteRedeem(request: AttestedOfflineNoteRedeemRequest,
                                  keypair: Keypair) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeAttestedOfflineNoteRedeem(
            request: request,
            keypair: keypair,
            creationTimeMs: creationTimeProvider()
        )
    }

    func buildAttestedOfflineNoteRedeem(request: AttestedOfflineNoteRedeemRequest,
                                  signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeAttestedOfflineNoteRedeem(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeProvider()
        )
    }

    func buildAttestedOfflineNoteAudit(request: AttestedOfflineNoteAuditRequest,
                                 keypair: Keypair) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeAttestedOfflineNoteAudit(
            request: request,
            keypair: keypair,
            creationTimeMs: creationTimeProvider()
        )
    }

    func buildAttestedOfflineNoteAudit(request: AttestedOfflineNoteAuditRequest,
                                 signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeAttestedOfflineNoteAudit(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeProvider()
        )
    }

    func buildRegisterOfflineDeviceAttestation(request: RegisterOfflineDeviceAttestationRequest,
                                               keypair: Keypair) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeRegisterOfflineDeviceAttestation(
            request: request,
            keypair: keypair,
            creationTimeMs: creationTimeProvider()
        )
    }

    /// Build the exact transaction digest without exporting account key material.
    func buildUnsignedRegisterOfflineDeviceAttestation(
        request: RegisterOfflineDeviceAttestationRequest
    ) throws -> OfflineDeviceAttestationUnsignedTransaction {
        try SwiftTransactionEncoder.encodeUnsignedRegisterOfflineDeviceAttestation(
            request: request,
            creationTimeMs: creationTimeProvider()
        )
    }

    /// Build registration with an external signer such as a transient signing service.
    func buildRegisterOfflineDeviceAttestation(
        request: RegisterOfflineDeviceAttestationRequest,
        signer: (Data) throws -> Data
    ) throws -> SignedTransactionEnvelope {
        let unsigned = try buildUnsignedRegisterOfflineDeviceAttestation(request: request)
        return try unsigned.signed(signature: signer(unsigned.signingHash))
    }

    func buildRegisterOfflineDeviceAttestation(request: RegisterOfflineDeviceAttestationRequest,
                                               signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeRegisterOfflineDeviceAttestation(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeProvider()
        )
    }

    func submit(issueAttestedOfflineNote request: AttestedOfflineNoteIssueRequest,
                keypair: Keypair,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildAttestedOfflineNoteIssue(request: request, keypair: keypair)
        submit(envelope: envelope, completion: completion)
    }

    func submit(issueAttestedOfflineNote request: AttestedOfflineNoteIssueRequest,
                signingKey: SigningKey,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildAttestedOfflineNoteIssue(request: request, signingKey: signingKey)
        submit(envelope: envelope, completion: completion)
    }

    func submit(redeemAttestedOfflineNote request: AttestedOfflineNoteRedeemRequest,
                keypair: Keypair,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildAttestedOfflineNoteRedeem(request: request, keypair: keypair)
        submit(envelope: envelope, completion: completion)
    }

    func submit(redeemAttestedOfflineNote request: AttestedOfflineNoteRedeemRequest,
                signingKey: SigningKey,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildAttestedOfflineNoteRedeem(request: request, signingKey: signingKey)
        submit(envelope: envelope, completion: completion)
    }

    func submit(auditAttestedOfflineNote request: AttestedOfflineNoteAuditRequest,
                keypair: Keypair,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildAttestedOfflineNoteAudit(request: request, keypair: keypair)
        submit(envelope: envelope, completion: completion)
    }

    func submit(auditAttestedOfflineNote request: AttestedOfflineNoteAuditRequest,
                signingKey: SigningKey,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildAttestedOfflineNoteAudit(request: request, signingKey: signingKey)
        submit(envelope: envelope, completion: completion)
    }

    func submit(registerOfflineDeviceAttestation request: RegisterOfflineDeviceAttestationRequest,
                keypair: Keypair,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildRegisterOfflineDeviceAttestation(request: request, keypair: keypair)
        submit(envelope: envelope, completion: completion)
    }

    func submit(registerOfflineDeviceAttestation request: RegisterOfflineDeviceAttestationRequest,
                signingKey: SigningKey,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildRegisterOfflineDeviceAttestation(request: request, signingKey: signingKey)
        submit(envelope: envelope, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(issueAttestedOfflineNote request: AttestedOfflineNoteIssueRequest,
                keypair: Keypair) async throws {
        try await submit(envelope: buildAttestedOfflineNoteIssue(request: request, keypair: keypair))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(issueAttestedOfflineNote request: AttestedOfflineNoteIssueRequest,
                signingKey: SigningKey) async throws {
        try await submit(envelope: buildAttestedOfflineNoteIssue(request: request, signingKey: signingKey))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(redeemAttestedOfflineNote request: AttestedOfflineNoteRedeemRequest,
                keypair: Keypair) async throws {
        try await submit(envelope: buildAttestedOfflineNoteRedeem(request: request, keypair: keypair))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(redeemAttestedOfflineNote request: AttestedOfflineNoteRedeemRequest,
                signingKey: SigningKey) async throws {
        try await submit(envelope: buildAttestedOfflineNoteRedeem(request: request, signingKey: signingKey))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(auditAttestedOfflineNote request: AttestedOfflineNoteAuditRequest,
                keypair: Keypair) async throws {
        try await submit(envelope: buildAttestedOfflineNoteAudit(request: request, keypair: keypair))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(auditAttestedOfflineNote request: AttestedOfflineNoteAuditRequest,
                signingKey: SigningKey) async throws {
        try await submit(envelope: buildAttestedOfflineNoteAudit(request: request, signingKey: signingKey))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(registerOfflineDeviceAttestation request: RegisterOfflineDeviceAttestationRequest,
                keypair: Keypair) async throws {
        try await submit(envelope: buildRegisterOfflineDeviceAttestation(request: request, keypair: keypair))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(registerOfflineDeviceAttestation request: RegisterOfflineDeviceAttestationRequest,
                signingKey: SigningKey) async throws {
        try await submit(envelope: buildRegisterOfflineDeviceAttestation(request: request, signingKey: signingKey))
    }
}
