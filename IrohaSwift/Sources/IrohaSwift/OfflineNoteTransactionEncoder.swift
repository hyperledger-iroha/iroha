import Foundation

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
        let privateKey = try privateKeyBytes(from: signingKey)
        let issueModel = try request.issue.noritoEncoded()
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeIssueOfflineNote(
                chainId: ids.chainId,
                authority: ids.authorityId,
                creationTimeMs: creationTimeMs,
                ttlMs: request.ttlMs,
                nonce: request.nonce,
                issueModel: issueModel,
                privateKey: privateKey
            )
        }
        return try wrap(native: native)
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
        try request.redemption.validateProofBinding()
        let privateKey = try privateKeyBytes(from: signingKey)
        let redemptionModel = try request.redemption.noritoEncoded()
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeRedeemOfflineNote(
                chainId: ids.chainId,
                authority: ids.authorityId,
                creationTimeMs: creationTimeMs,
                ttlMs: request.ttlMs,
                nonce: request.nonce,
                redemptionModel: redemptionModel,
                privateKey: privateKey
            )
        }
        return try wrap(native: native)
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
        for audit in request.bearerAuditTrail {
            try audit.validateProofBinding()
        }
        try request.redemption.validateProofBinding()
        let privateKey = try privateKeyBytes(from: signingKey)
        let bearerAuditTrail = try request.bearerAuditTrail.map { try $0.noritoEncoded() }
        let redemptionModel = try request.redemption.noritoEncoded()
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeDefundOfflineNote(
                chainId: ids.chainId,
                authority: ids.authorityId,
                creationTimeMs: creationTimeMs,
                ttlMs: request.ttlMs,
                nonce: request.nonce,
                bearerAuditTrail: bearerAuditTrail,
                redemptionModel: redemptionModel,
                privateKey: privateKey
            )
        }
        return try wrap(native: native)
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
        try request.audit.validateProofBinding()
        let privateKey = try privateKeyBytes(from: signingKey)
        let auditModel = try request.audit.noritoEncoded()
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeAuditOfflineNote(
                chainId: ids.chainId,
                authority: ids.authorityId,
                creationTimeMs: creationTimeMs,
                ttlMs: request.ttlMs,
                nonce: request.nonce,
                auditModel: auditModel,
                privateKey: privateKey
            )
        }
        return try wrap(native: native)
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
