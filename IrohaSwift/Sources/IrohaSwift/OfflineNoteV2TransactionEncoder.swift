import Foundation

private enum OfflineNoteV2SwiftNoritoEncoder {
    private static let signedTransactionWireVersion: UInt8 = 1

    static func encodeIssue(chainId: String,
                            authority: String,
                            creationTimeMs: UInt64,
                            ttlMs: UInt64?,
                            nonce: UInt32?,
                            issue: OfflineNoteIssueV2,
                            signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let instruction = try encodeInstruction(
            wireName: OfflineNoteV2TypeNames.issueInstruction,
            typeName: OfflineNoteV2TypeNames.issueInstruction,
            modelPayload: OfflineNoteV2Encoding.encodeIssue(issue)
        )
        return try encodeTransaction(
            chainId: chainId,
            authority: authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            nonce: nonce,
            instructionPayload: instruction,
            signingKey: signingKey
        )
    }

    static func encodeRedeem(chainId: String,
                             authority: String,
                             creationTimeMs: UInt64,
                             ttlMs: UInt64?,
                             nonce: UInt32?,
                             redemption: OfflineNoteRedeemV2,
                             signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        try redemption.validateProofBinding()
        let instruction = try encodeInstruction(
            wireName: OfflineNoteV2TypeNames.redeemInstruction,
            typeName: OfflineNoteV2TypeNames.redeemInstruction,
            modelPayload: OfflineNoteV2Encoding.encodeRedeem(redemption)
        )
        return try encodeTransaction(
            chainId: chainId,
            authority: authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            nonce: nonce,
            instructionPayload: instruction,
            signingKey: signingKey
        )
    }

    static func encodeAudit(chainId: String,
                            authority: String,
                            creationTimeMs: UInt64,
                            ttlMs: UInt64?,
                            nonce: UInt32?,
                            audit: OfflineNoteAuditBundleV2,
                            signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        try audit.validateProofBinding()
        let instruction = try encodeInstruction(
            wireName: OfflineNoteV2TypeNames.auditInstruction,
            typeName: OfflineNoteV2TypeNames.auditInstruction,
            modelPayload: OfflineNoteV2Encoding.encodeAudit(audit)
        )
        return try encodeTransaction(
            chainId: chainId,
            authority: authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            nonce: nonce,
            instructionPayload: instruction,
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
                                          instructionPayload: Data,
                                          signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        let transactionPayload = try encodeTransactionPayload(
            chainId: chainId,
            authority: authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            nonce: nonce,
            instructionPayload: instructionPayload
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
                                                 instructionPayload: Data) throws -> Data {
        var transactionPayload = OfflineNoritoWriter()
        transactionPayload.writeField(OfflineNorito.encodeString(chainId))
        transactionPayload.writeField(OfflineNorito.encodeString(authority))
        transactionPayload.writeField(OfflineNorito.encodeUInt64(creationTimeMs))
        transactionPayload.writeField(encodeExecutable(instructionPayload: instructionPayload))
        transactionPayload.writeField(try OfflineNorito.encodeOption(ttlMs, encode: OfflineNorito.encodeUInt64))
        transactionPayload.writeField(try OfflineNorito.encodeOption(nonce, encode: OfflineNorito.encodeUInt32))
        transactionPayload.writeField(encodeEmptyMetadata())
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
    static func encodeIssueOfflineNoteV2(request: IssueOfflineNoteV2Request,
                                         keypair: Keypair,
                                         creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeIssueOfflineNoteV2(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeMs
        )
    }

    static func encodeIssueOfflineNoteV2(request: IssueOfflineNoteV2Request,
                                         signingKey: SigningKey,
                                         creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(
            chainId: request.chainId,
            authorityId: request.authority
        )
        return try OfflineNoteV2SwiftNoritoEncoder.encodeIssue(
            chainId: ids.chainId,
            authority: ids.authorityId,
            creationTimeMs: creationTimeMs,
            ttlMs: request.ttlMs,
            nonce: request.nonce,
            issue: request.issue,
            signingKey: signingKey
        )
    }

    static func encodeRedeemOfflineNoteV2(request: RedeemOfflineNoteV2Request,
                                          keypair: Keypair,
                                          creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeRedeemOfflineNoteV2(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeMs
        )
    }

    static func encodeRedeemOfflineNoteV2(request: RedeemOfflineNoteV2Request,
                                          signingKey: SigningKey,
                                          creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(
            chainId: request.chainId,
            authorityId: request.authority
        )
        return try OfflineNoteV2SwiftNoritoEncoder.encodeRedeem(
            chainId: ids.chainId,
            authority: ids.authorityId,
            creationTimeMs: creationTimeMs,
            ttlMs: request.ttlMs,
            nonce: request.nonce,
            redemption: request.redemption,
            signingKey: signingKey
        )
    }

    static func encodeAuditOfflineNoteV2(request: AuditOfflineNoteV2Request,
                                         keypair: Keypair,
                                         creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeAuditOfflineNoteV2(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeMs
        )
    }

    static func encodeAuditOfflineNoteV2(request: AuditOfflineNoteV2Request,
                                         signingKey: SigningKey,
                                         creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(
            chainId: request.chainId,
            authorityId: request.authority
        )
        return try OfflineNoteV2SwiftNoritoEncoder.encodeAudit(
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
    func buildIssueOfflineNoteV2(request: IssueOfflineNoteV2Request,
                                 keypair: Keypair) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeIssueOfflineNoteV2(
            request: request,
            keypair: keypair,
            creationTimeMs: creationTimeProvider()
        )
    }

    func buildIssueOfflineNoteV2(request: IssueOfflineNoteV2Request,
                                 signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeIssueOfflineNoteV2(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeProvider()
        )
    }

    func buildRedeemOfflineNoteV2(request: RedeemOfflineNoteV2Request,
                                  keypair: Keypair) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeRedeemOfflineNoteV2(
            request: request,
            keypair: keypair,
            creationTimeMs: creationTimeProvider()
        )
    }

    func buildRedeemOfflineNoteV2(request: RedeemOfflineNoteV2Request,
                                  signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeRedeemOfflineNoteV2(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeProvider()
        )
    }

    func buildAuditOfflineNoteV2(request: AuditOfflineNoteV2Request,
                                 keypair: Keypair) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeAuditOfflineNoteV2(
            request: request,
            keypair: keypair,
            creationTimeMs: creationTimeProvider()
        )
    }

    func buildAuditOfflineNoteV2(request: AuditOfflineNoteV2Request,
                                 signingKey: SigningKey) throws -> SignedTransactionEnvelope {
        try SwiftTransactionEncoder.encodeAuditOfflineNoteV2(
            request: request,
            signingKey: signingKey,
            creationTimeMs: creationTimeProvider()
        )
    }

    func submit(issueOfflineNoteV2 request: IssueOfflineNoteV2Request,
                keypair: Keypair,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildIssueOfflineNoteV2(request: request, keypair: keypair)
        submit(envelope: envelope, completion: completion)
    }

    func submit(issueOfflineNoteV2 request: IssueOfflineNoteV2Request,
                signingKey: SigningKey,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildIssueOfflineNoteV2(request: request, signingKey: signingKey)
        submit(envelope: envelope, completion: completion)
    }

    func submit(redeemOfflineNoteV2 request: RedeemOfflineNoteV2Request,
                keypair: Keypair,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildRedeemOfflineNoteV2(request: request, keypair: keypair)
        submit(envelope: envelope, completion: completion)
    }

    func submit(redeemOfflineNoteV2 request: RedeemOfflineNoteV2Request,
                signingKey: SigningKey,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildRedeemOfflineNoteV2(request: request, signingKey: signingKey)
        submit(envelope: envelope, completion: completion)
    }

    func submit(auditOfflineNoteV2 request: AuditOfflineNoteV2Request,
                keypair: Keypair,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildAuditOfflineNoteV2(request: request, keypair: keypair)
        submit(envelope: envelope, completion: completion)
    }

    func submit(auditOfflineNoteV2 request: AuditOfflineNoteV2Request,
                signingKey: SigningKey,
                completion: @Sendable @escaping (Error?) -> Void) throws {
        let envelope = try buildAuditOfflineNoteV2(request: request, signingKey: signingKey)
        submit(envelope: envelope, completion: completion)
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(issueOfflineNoteV2 request: IssueOfflineNoteV2Request,
                keypair: Keypair) async throws {
        try await submit(envelope: buildIssueOfflineNoteV2(request: request, keypair: keypair))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(issueOfflineNoteV2 request: IssueOfflineNoteV2Request,
                signingKey: SigningKey) async throws {
        try await submit(envelope: buildIssueOfflineNoteV2(request: request, signingKey: signingKey))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(redeemOfflineNoteV2 request: RedeemOfflineNoteV2Request,
                keypair: Keypair) async throws {
        try await submit(envelope: buildRedeemOfflineNoteV2(request: request, keypair: keypair))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(redeemOfflineNoteV2 request: RedeemOfflineNoteV2Request,
                signingKey: SigningKey) async throws {
        try await submit(envelope: buildRedeemOfflineNoteV2(request: request, signingKey: signingKey))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(auditOfflineNoteV2 request: AuditOfflineNoteV2Request,
                keypair: Keypair) async throws {
        try await submit(envelope: buildAuditOfflineNoteV2(request: request, keypair: keypair))
    }

    @available(iOS 15.0, macOS 12.0, *)
    func submit(auditOfflineNoteV2 request: AuditOfflineNoteV2Request,
                signingKey: SigningKey) async throws {
        try await submit(envelope: buildAuditOfflineNoteV2(request: request, signingKey: signingKey))
    }
}
