import Foundation

private enum OfflineNoteSwiftNoritoEncoder {
    private static let signedTransactionWireVersion: UInt8 = 1

    static func encodeIssue(chainId: String,
                            authority: String,
                            creationTimeMs: UInt64,
                            ttlMs: UInt64?,
                            nonce: UInt32?,
                            issue: OfflineNoteIssue,
                            signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let instruction = try encodeInstruction(
            wireName: OfflineNoteTypeNames.issueInstruction,
            typeName: OfflineNoteTypeNames.issueInstruction,
            modelPayload: OfflineNoteEncoding.encodeIssue(issue)
        )
        return try encodeTransaction(
            chainId: chainId,
            authority: authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            nonce: nonce,
            instructionPayloads: [instruction],
            signingKey: signingKey
        )
    }

    static func encodeRedeem(chainId: String,
                             authority: String,
                             creationTimeMs: UInt64,
                             ttlMs: UInt64?,
                             nonce: UInt32?,
                             redemption: OfflineNoteRedeem,
                             signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        try redemption.validateProofBinding()
        let instruction = try encodeInstruction(
            wireName: OfflineNoteTypeNames.redeemInstruction,
            typeName: OfflineNoteTypeNames.redeemInstruction,
            modelPayload: OfflineNoteEncoding.encodeRedeem(redemption)
        )
        return try encodeTransaction(
            chainId: chainId,
            authority: authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            nonce: nonce,
            instructionPayloads: [instruction],
            signingKey: signingKey
        )
    }

    static func encodeDefund(chainId: String,
                             authority: String,
                             creationTimeMs: UInt64,
                             ttlMs: UInt64?,
                             nonce: UInt32?,
                             bearerAuditTrail: [OfflineNoteAuditBundle],
                             redemption: OfflineNoteRedeem,
                             signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        var instructions: [Data] = []
        instructions.reserveCapacity(bearerAuditTrail.count + 1)
        for audit in bearerAuditTrail {
            try audit.validateProofBinding()
            instructions.append(try encodeInstruction(
                wireName: OfflineNoteTypeNames.auditInstruction,
                typeName: OfflineNoteTypeNames.auditInstruction,
                modelPayload: OfflineNoteEncoding.encodeAudit(audit)
            ))
        }
        try redemption.validateProofBinding()
        instructions.append(try encodeInstruction(
            wireName: OfflineNoteTypeNames.redeemInstruction,
            typeName: OfflineNoteTypeNames.redeemInstruction,
            modelPayload: OfflineNoteEncoding.encodeRedeem(redemption)
        ))
        return try encodeTransaction(
            chainId: chainId,
            authority: authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            nonce: nonce,
            instructionPayloads: instructions,
            signingKey: signingKey
        )
    }

    static func encodeAudit(chainId: String,
                            authority: String,
                            creationTimeMs: UInt64,
                            ttlMs: UInt64?,
                            nonce: UInt32?,
                            audit: OfflineNoteAuditBundle,
                            signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        try audit.validateProofBinding()
        let instruction = try encodeInstruction(
            wireName: OfflineNoteTypeNames.auditInstruction,
            typeName: OfflineNoteTypeNames.auditInstruction,
            modelPayload: OfflineNoteEncoding.encodeAudit(audit)
        )
        return try encodeTransaction(
            chainId: chainId,
            authority: authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            nonce: nonce,
            instructionPayloads: [instruction],
            signingKey: signingKey
        )
    }

    private static func encodeInstruction(wireName: String,
                                          typeName: String,
                                          modelPayload: Data) -> Data {
        var concreteInstruction = OfflineNoritoWriter()
        concreteInstruction.writeField(modelPayload)
        let framedInstruction = noritoEncode(
            typeName: typeName,
            payload: concreteInstruction.data,
            flags: 0
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
                                          instructionPayloads: [Data],
                                          signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let transactionPayload = try encodeTransactionPayload(
            chainId: chainId,
            authority: authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            nonce: nonce,
            instructionPayloads: instructionPayloads
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

    private static func encodeTransactionPayload(chainId: String,
                                                 authority: String,
                                                 creationTimeMs: UInt64,
                                                 ttlMs: UInt64?,
                                                 nonce: UInt32?,
                                                 instructionPayloads: [Data]) throws -> Data {
        var transactionPayload = OfflineNoritoWriter()
        transactionPayload.writeField(OfflineNorito.encodeString(chainId))
        transactionPayload.writeField(OfflineNorito.encodeString(authority))
        transactionPayload.writeField(OfflineNorito.encodeUInt64(creationTimeMs))
        transactionPayload.writeField(encodeExecutable(instructionPayloads: instructionPayloads))
        transactionPayload.writeField(try OfflineNorito.encodeOption(ttlMs, encode: OfflineNorito.encodeUInt64))
        transactionPayload.writeField(try OfflineNorito.encodeOption(nonce, encode: OfflineNorito.encodeUInt32))
        transactionPayload.writeField(encodeEmptyMetadata())
        return transactionPayload.data
    }

    private static func encodeExecutable(instructionPayloads: [Data]) -> Data {
        var instructions = OfflineNoritoWriter()
        instructions.writeLength(UInt64(instructionPayloads.count))
        for instructionPayload in instructionPayloads {
            instructions.writeField(instructionPayload)
        }

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
        var entrypoint = OfflineNoritoWriter()
        entrypoint.writeUInt32LE(0)
        entrypoint.writeField(signedTransaction)
        return entrypoint.data
    }

    private static func encodeEmptyMetadata() -> Data {
        var metadata = OfflineNoritoWriter()
        metadata.writeLength(0)
        return metadata.data
    }
}

extension SwiftTransactionEncoder {
    static func encodeIssueOfflineNote(request: IssueOfflineNoteRequest,
                                         keypair: Keypair,
                                         creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeIssueOfflineNote(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeMs
        )
    }

    static func encodeIssueOfflineNote(request: IssueOfflineNoteRequest,
                                         signingKey: SigningKey,
                                         creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(
            chainId: request.chainId,
            authorityId: request.authority
        )
        return try OfflineNoteSwiftNoritoEncoder.encodeIssue(
            chainId: ids.chainId,
            authority: ids.authorityId,
            creationTimeMs: creationTimeMs,
            ttlMs: request.ttlMs,
            nonce: request.nonce,
            issue: request.issue,
            signingKey: signingKey
        )
    }

    static func encodeRedeemOfflineNote(request: RedeemOfflineNoteRequest,
                                          keypair: Keypair,
                                          creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeRedeemOfflineNote(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeMs
        )
    }

    static func encodeRedeemOfflineNote(request: RedeemOfflineNoteRequest,
                                          signingKey: SigningKey,
                                          creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(
            chainId: request.chainId,
            authorityId: request.authority
        )
        return try OfflineNoteSwiftNoritoEncoder.encodeRedeem(
            chainId: ids.chainId,
            authority: ids.authorityId,
            creationTimeMs: creationTimeMs,
            ttlMs: request.ttlMs,
            nonce: request.nonce,
            redemption: request.redemption,
            signingKey: signingKey
        )
    }

    static func encodeDefundOfflineNote(request: DefundOfflineNoteRequest,
                                          keypair: Keypair,
                                          creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeDefundOfflineNote(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeMs
        )
    }

    static func encodeDefundOfflineNote(request: DefundOfflineNoteRequest,
                                          signingKey: SigningKey,
                                          creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(
            chainId: request.chainId,
            authorityId: request.authority
        )
        return try OfflineNoteSwiftNoritoEncoder.encodeDefund(
            chainId: ids.chainId,
            authority: ids.authorityId,
            creationTimeMs: creationTimeMs,
            ttlMs: request.ttlMs,
            nonce: request.nonce,
            bearerAuditTrail: request.bearerAuditTrail,
            redemption: request.redemption,
            signingKey: signingKey
        )
    }

    static func encodeAuditOfflineNote(request: AuditOfflineNoteRequest,
                                         keypair: Keypair,
                                         creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeAuditOfflineNote(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeMs
        )
    }

    static func encodeAuditOfflineNote(request: AuditOfflineNoteRequest,
                                         signingKey: SigningKey,
                                         creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(
            chainId: request.chainId,
            authorityId: request.authority
        )
        return try OfflineNoteSwiftNoritoEncoder.encodeAudit(
            chainId: ids.chainId,
            authority: ids.authorityId,
            creationTimeMs: creationTimeMs,
            ttlMs: request.ttlMs,
            nonce: request.nonce,
            audit: request.audit,
            signingKey: signingKey
        )
    }
}

public extension IrohaSDK {
    func buildIssueOfflineNote(request: IssueOfflineNoteRequest,
                                 keypair: Keypair) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeIssueOfflineNote(
            request: request,
            keypair: keypair,
            creationTimeMs: creationTimeProvider()
        )
    }

    func buildIssueOfflineNote(request: IssueOfflineNoteRequest,
                                 signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeIssueOfflineNote(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeProvider()
        )
    }

    func buildRedeemOfflineNote(request: RedeemOfflineNoteRequest,
                                  keypair: Keypair) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeRedeemOfflineNote(
            request: request,
            keypair: keypair,
            creationTimeMs: creationTimeProvider()
        )
    }

    func buildRedeemOfflineNote(request: RedeemOfflineNoteRequest,
                                  signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeRedeemOfflineNote(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeProvider()
        )
    }

    func buildDefundOfflineNote(request: DefundOfflineNoteRequest,
                                  keypair: Keypair) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeDefundOfflineNote(
            request: request,
            keypair: keypair,
            creationTimeMs: creationTimeProvider()
        )
    }

    func buildDefundOfflineNote(request: DefundOfflineNoteRequest,
                                  signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeDefundOfflineNote(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeProvider()
        )
    }

    func buildAuditOfflineNote(request: AuditOfflineNoteRequest,
                                 keypair: Keypair) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeAuditOfflineNote(
            request: request,
            keypair: keypair,
            creationTimeMs: creationTimeProvider()
        )
    }

    func buildAuditOfflineNote(request: AuditOfflineNoteRequest,
                                 signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeAuditOfflineNote(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeProvider()
        )
    }

    func submit(issueOfflineNote request: IssueOfflineNoteRequest,
                keypair: Keypair,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildIssueOfflineNote(request: request, keypair: keypair)
        submit(envelope: envelope, completion: completion)
    }

    func submit(issueOfflineNote request: IssueOfflineNoteRequest,
                signingKey: SigningKey,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildIssueOfflineNote(request: request, signingKey: signingKey)
        submit(envelope: envelope, completion: completion)
    }

    func submit(redeemOfflineNote request: RedeemOfflineNoteRequest,
                keypair: Keypair,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildRedeemOfflineNote(request: request, keypair: keypair)
        submit(envelope: envelope, completion: completion)
    }

    func submit(redeemOfflineNote request: RedeemOfflineNoteRequest,
                signingKey: SigningKey,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildRedeemOfflineNote(request: request, signingKey: signingKey)
        submit(envelope: envelope, completion: completion)
    }

    func submit(defundOfflineNote request: DefundOfflineNoteRequest,
                keypair: Keypair,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildDefundOfflineNote(request: request, keypair: keypair)
        submit(envelope: envelope, completion: completion)
    }

    func submit(defundOfflineNote request: DefundOfflineNoteRequest,
                signingKey: SigningKey,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildDefundOfflineNote(request: request, signingKey: signingKey)
        submit(envelope: envelope, completion: completion)
    }

    func submit(auditOfflineNote request: AuditOfflineNoteRequest,
                keypair: Keypair,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildAuditOfflineNote(request: request, keypair: keypair)
        submit(envelope: envelope, completion: completion)
    }

    func submit(auditOfflineNote request: AuditOfflineNoteRequest,
                signingKey: SigningKey,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildAuditOfflineNote(request: request, signingKey: signingKey)
        submit(envelope: envelope, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(issueOfflineNote request: IssueOfflineNoteRequest,
                keypair: Keypair) async throws {
        try await submit(envelope: buildIssueOfflineNote(request: request, keypair: keypair))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(issueOfflineNote request: IssueOfflineNoteRequest,
                signingKey: SigningKey) async throws {
        try await submit(envelope: buildIssueOfflineNote(request: request, signingKey: signingKey))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(redeemOfflineNote request: RedeemOfflineNoteRequest,
                keypair: Keypair) async throws {
        try await submit(envelope: buildRedeemOfflineNote(request: request, keypair: keypair))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(redeemOfflineNote request: RedeemOfflineNoteRequest,
                signingKey: SigningKey) async throws {
        try await submit(envelope: buildRedeemOfflineNote(request: request, signingKey: signingKey))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(defundOfflineNote request: DefundOfflineNoteRequest,
                keypair: Keypair) async throws {
        try await submit(envelope: buildDefundOfflineNote(request: request, keypair: keypair))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(defundOfflineNote request: DefundOfflineNoteRequest,
                signingKey: SigningKey) async throws {
        try await submit(envelope: buildDefundOfflineNote(request: request, signingKey: signingKey))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(auditOfflineNote request: AuditOfflineNoteRequest,
                keypair: Keypair) async throws {
        try await submit(envelope: buildAuditOfflineNote(request: request, keypair: keypair))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(auditOfflineNote request: AuditOfflineNoteRequest,
                signingKey: SigningKey) async throws {
        try await submit(envelope: buildAuditOfflineNote(request: request, signingKey: signingKey))
    }
}
