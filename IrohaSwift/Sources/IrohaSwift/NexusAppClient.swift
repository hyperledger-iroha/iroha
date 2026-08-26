import Foundation
import CryptoKit

public let NexusSignatureAlgorithmEd25519 = "ed25519"

public struct NexusAppError: Error, LocalizedError, Equatable {
    public let code: String
    public let message: String

    public init(code: String, message: String) {
        self.code = code
        self.message = message
    }

    public var errorDescription: String? { message }
}

public struct NexusAppConfig: Sendable {
    public let chainId: String
    public let accountChainDiscriminant: UInt16
    public let appId: String?
    public let relayURL: URL?
    public let node: URL?
    public let authority: String?
    public let signingPublicKey: Data?
    public let appMetadata: [String: String]

    public init(chainId: String,
                accountChainDiscriminant: UInt16,
                appId: String? = nil,
                relayURL: URL? = nil,
                node: URL? = nil,
                authority: String? = nil,
                signingPublicKey: Data? = nil,
                appMetadata: [String: String] = [:]) {
        self.chainId = chainId
        self.accountChainDiscriminant = accountChainDiscriminant
        self.appId = appId
        self.relayURL = relayURL
        self.node = node
        self.authority = authority
        self.signingPublicKey = signingPublicKey
        self.appMetadata = appMetadata
    }
}

public struct NexusConnectOptions: Sendable {
    public let scopes: Set<String>
    public let walletURIBase: URL?
    public let node: URL?
    public let metadata: [String: String]
    public let sessionID: String?

    public init(scopes: Set<String> = [],
                walletURIBase: URL? = nil,
                node: URL? = nil,
                metadata: [String: String] = [:],
                sessionID: String? = nil) {
        self.scopes = scopes
        self.walletURIBase = walletURIBase
        self.node = node
        self.metadata = metadata
        self.sessionID = sessionID
    }
}

public struct NexusConnectSession: Sendable {
    public let sessionID: String
    public let walletLaunchURI: URL
    public let appId: String?
    public let relayURL: URL?
    public let node: URL?
    public let approvedAccount: String?
    public let signingPublicKey: Data?
    public let metadata: [String: String]

    public init(sessionID: String,
                walletLaunchURI: URL,
                appId: String? = nil,
                relayURL: URL? = nil,
                node: URL? = nil,
                approvedAccount: String? = nil,
                signingPublicKey: Data? = nil,
                metadata: [String: String] = [:]) {
        self.sessionID = sessionID
        self.walletLaunchURI = walletLaunchURI
        self.appId = appId
        self.relayURL = relayURL
        self.node = node
        self.approvedAccount = approvedAccount
        self.signingPublicKey = signingPublicKey
        self.metadata = metadata
    }

    public func withApproval(account: String, signingPublicKey: Data) -> NexusConnectSession {
        NexusConnectSession(sessionID: sessionID,
                            walletLaunchURI: walletLaunchURI,
                            appId: appId,
                            relayURL: relayURL,
                            node: node,
                            approvedAccount: account,
                            signingPublicKey: signingPublicKey,
                            metadata: metadata)
    }
}

/// Wallet approval result. Transports leave `session` nil; the facade supplies its caller copy.
public struct NexusApprovedAccount: Sendable {
    public let accountID: String
    public let signingPublicKey: Data?
    public let session: NexusConnectSession?

    public init(accountID: String,
                signingPublicKey: Data? = nil,
                session: NexusConnectSession? = nil) {
        self.accountID = accountID
        self.signingPublicKey = signingPublicKey
        self.session = session
    }
}

public struct NexusTransferInput: Sendable {
    public let sourceAssetID: String
    public let quantity: String
    public let destinationAccountID: String
    public let feePayment: FeePaymentIntent
    public let authority: String?
    public let signingPublicKey: Data?
    public let creationTimeMs: UInt64?
    public let ttlMs: UInt64?
    public let nonce: UInt32?
    public let metadata: [String: String]

    public init(sourceAssetID: String,
                quantity: String,
                destinationAccountID: String,
                feePayment: FeePaymentIntent,
                authority: String? = nil,
                signingPublicKey: Data? = nil,
                creationTimeMs: UInt64? = nil,
                ttlMs: UInt64? = nil,
                nonce: UInt32? = nil,
                metadata: [String: String] = [:]) {
        self.sourceAssetID = sourceAssetID
        self.quantity = quantity
        self.destinationAccountID = destinationAccountID
        self.feePayment = feePayment
        self.authority = authority
        self.signingPublicKey = signingPublicKey
        self.creationTimeMs = creationTimeMs
        self.ttlMs = ttlMs
        self.nonce = nonce
        self.metadata = metadata
    }

    public func with(authority: String, signingPublicKey: Data) -> NexusTransferInput {
        NexusTransferInput(sourceAssetID: sourceAssetID,
                           quantity: quantity,
                           destinationAccountID: destinationAccountID,
                           feePayment: feePayment,
                           authority: authority,
                           signingPublicKey: signingPublicKey,
                           creationTimeMs: creationTimeMs,
                           ttlMs: ttlMs,
                           nonce: nonce,
                           metadata: metadata)
    }
}

public struct NexusSignableTransaction: Sendable {
    public let payloadBytes: Data
    public let payloadHashHex: String
    public let authority: String
    public let signingPublicKey: Data
    public let signatureAlgorithm: String

    public init(payloadBytes: Data,
                payloadHashHex: String,
                authority: String,
                signingPublicKey: Data,
                signatureAlgorithm: String = NexusSignatureAlgorithmEd25519) {
        self.payloadBytes = payloadBytes
        self.payloadHashHex = payloadHashHex
        self.authority = authority
        self.signingPublicKey = signingPublicKey
        self.signatureAlgorithm = signatureAlgorithm
    }
}

public struct NexusTransferDraft: Sendable {
    public let input: NexusTransferInput
    public let signable: NexusSignableTransaction

    public init(input: NexusTransferInput, signable: NexusSignableTransaction) {
        self.input = input
        self.signable = signable
    }
}

public struct NexusWalletSignature: Sendable {
    public let signature: Data
    public let algorithm: String

    public init(signature: Data, algorithm: String = NexusSignatureAlgorithmEd25519) {
        self.signature = signature
        self.algorithm = algorithm
    }
}

public struct NexusFinalizeOptions: Sendable {
    public let waitForFinalStatus: Bool
    public let pipelineStatusPollOptions: PipelineStatusPollOptions

    public init(waitForFinalStatus: Bool = true,
                pipelineStatusPollOptions: PipelineStatusPollOptions = .default) {
        self.waitForFinalStatus = waitForFinalStatus
        self.pipelineStatusPollOptions = pipelineStatusPollOptions
    }
}

public struct NexusTransferReceipt: Sendable {
    public let transactionHashHex: String
    public let signedTransaction: SignedTransactionEnvelope
    public let submission: ToriiSubmitTransactionResponse?
    public let finalStatus: String?

    public init(transactionHashHex: String,
                signedTransaction: SignedTransactionEnvelope,
                submission: ToriiSubmitTransactionResponse?,
                finalStatus: String?) {
        self.transactionHashHex = transactionHashHex
        self.signedTransaction = signedTransaction
        self.submission = submission
        self.finalStatus = finalStatus
    }
}

public protocol NexusConnectTransport: AnyObject {
    func startConnect(options: NexusConnectOptions, config: NexusAppConfig) async throws -> NexusConnectSession
    func awaitApproval(session: NexusConnectSession, config: NexusAppConfig) async throws -> NexusApprovedAccount
    func requestSignature(session: NexusConnectSession,
                          signable: NexusSignableTransaction,
                          config: NexusAppConfig) async throws -> NexusWalletSignature
}

public protocol NexusTransactionCodec: Sendable {
    func buildTransferPayload(input: NexusTransferInput,
                              config: NexusAppConfig,
                              authority: String) throws -> Data
    func finalizeSignedTransaction(signable: NexusSignableTransaction,
                                   signature: NexusWalletSignature) throws -> SignedTransactionEnvelope
}

public protocol NexusToriiSubmitting: AnyObject {
    func submitNexusTransaction(_ envelope: SignedTransactionEnvelope) async throws -> ToriiSubmitTransactionResponse?
    func waitForNexusTransactionStatus(hashHex: String,
                                       options: PipelineStatusPollOptions) async throws -> String
}

extension ToriiClient: NexusToriiSubmitting {
    public func submitNexusTransaction(_ envelope: SignedTransactionEnvelope) async throws -> ToriiSubmitTransactionResponse? {
        try await submitTransaction(data: envelope.norito,
                                    mode: .pipeline,
                                    idempotencyKey: envelope.hashHex)
    }

    public func waitForNexusTransactionStatus(hashHex: String,
                                              options: PipelineStatusPollOptions) async throws -> String {
        let status = try await waitForTransactionStatus(hashHex: hashHex, pollOptions: options)
        return status.status.kind
    }
}

public struct SwiftNexusTransactionCodec: NexusTransactionCodec {
    public init() {}

    public func buildTransferPayload(input: NexusTransferInput,
                                     config: NexusAppConfig,
                                     authority: String) throws -> Data {
        try SwiftNexusTransferPayloadEncoder.encode(input: input,
                                                    chainId: config.chainId,
                                                    accountChainDiscriminant: config.accountChainDiscriminant,
                                                    authority: authority)
    }

    public func buildTransferInstructionBox(input: NexusTransferInput,
                                            accountChainDiscriminant: UInt16) throws -> Data {
        let quantity = try KotodamaNumericV1Codec.decodeQuantityJSON(input.quantity).canonicalString
        if let parsed = CanonicalNorito.parsePublicAssetIdLiteral(input.sourceAssetID) {
            try Self.requireCanonicalAccountID(
                parsed.accountId,
                accountChainDiscriminant: accountChainDiscriminant,
                context: "Transfer source asset owner"
            )
            try Self.requireCanonicalAccountID(
                input.destinationAccountID,
                accountChainDiscriminant: accountChainDiscriminant,
                context: "Transfer destination account"
            )
            let nativeAssetDefinitionId: String
            if let dataspaceId = parsed.dataspaceId {
                nativeAssetDefinitionId = "\(parsed.assetDefinitionId)#dataspace:\(dataspaceId)"
            } else {
                nativeAssetDefinitionId = parsed.assetDefinitionId
            }
            if let nativeInstruction = try NoritoNativeBridge.shared.encodeTransferInstructionBox(
                authority: parsed.accountId,
                assetDefinitionId: nativeAssetDefinitionId,
                quantity: quantity,
                destination: input.destinationAccountID
            ) {
                return nativeInstruction
            }
        }
        return try SwiftNexusTransferPayloadEncoder.encodeInstructionBox(
            input: NexusTransferInput(
                sourceAssetID: input.sourceAssetID,
                quantity: quantity,
                destinationAccountID: input.destinationAccountID,
                feePayment: input.feePayment,
                authority: input.authority,
                signingPublicKey: input.signingPublicKey,
                creationTimeMs: input.creationTimeMs,
                ttlMs: input.ttlMs,
                nonce: input.nonce,
                metadata: input.metadata
            ),
            accountChainDiscriminant: accountChainDiscriminant
        )
    }

    private static func requireCanonicalAccountID(
        _ value: String,
        accountChainDiscriminant: UInt16,
        context: String
    ) throws {
        guard value.trimmingCharacters(in: .whitespacesAndNewlines) == value else {
            throw NexusAppError(
                code: "invalid_account_id",
                message: "\(context) must be an exact canonical I105 account for chain discriminant \(accountChainDiscriminant)."
            )
        }
        do {
            let address = try AccountAddress.parseEncoded(
                value,
                expectedPrefix: accountChainDiscriminant
            )
            guard try address.toI105(chainDiscriminant: accountChainDiscriminant) == value else {
                throw NexusAppError(code: "invalid_account_id",
                                    message: "\(context) must use its exact canonical I105 form.")
            }
        } catch let error as NexusAppError {
            throw error
        } catch {
            throw NexusAppError(
                code: "invalid_account_id",
                message: "\(context) must be an exact canonical I105 account for chain discriminant \(accountChainDiscriminant)."
            )
        }
    }

    public func finalizeSignedTransaction(signable: NexusSignableTransaction,
                                          signature: NexusWalletSignature) throws -> SignedTransactionEnvelope {
        var signedTransaction = CompactNoritoWriter()
        signedTransaction.writeField(Self.encodeSignatureOf(signature.signature))
        signedTransaction.writeField(signable.payloadBytes)
        signedTransaction.writeField(Data([0]))
        let signedBytes = signedTransaction.data
        let transactionHash = IrohaHash.hash(
            Self.encodeTransactionEntrypoint(signable.payloadBytes)
        )
        var versioned = Data([1])
        versioned.append(signedBytes)
        return SignedTransactionEnvelope(norito: versioned,
                                         signedTransaction: signedBytes,
                                         payload: signable.payloadBytes,
                                         transactionHash: transactionHash)
    }

    private static func encodeTransactionEntrypoint(_ transactionPayload: Data) -> Data {
        var entrypoint = CompactNoritoWriter()
        entrypoint.writeUInt32LE(0)
        entrypoint.writeField(transactionPayload)
        return entrypoint.data
    }

    private static func encodeSignatureOf(_ signature: Data) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(encodeSignature(signature))
        return writer.data
    }

    private static func encodeSignature(_ signature: Data) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt64LE(UInt64(signature.count))
        writer.writeByteFields(signature)
        return writer.data
    }
}

public final class NexusAppClient {
    private let config: NexusAppConfig
    private let connectTransport: NexusConnectTransport?
    private let transactionCodec: NexusTransactionCodec
    private let toriiSubmitter: NexusToriiSubmitting?

    public init(config: NexusAppConfig,
                connectTransport: NexusConnectTransport? = nil,
                transactionCodec: NexusTransactionCodec = SwiftNexusTransactionCodec(),
                toriiSubmitter: NexusToriiSubmitting? = nil) {
        self.config = config
        self.connectTransport = connectTransport
        self.transactionCodec = transactionCodec
        self.toriiSubmitter = toriiSubmitter
    }

    public func startConnect(options: NexusConnectOptions = NexusConnectOptions()) async throws -> NexusConnectSession {
        guard let connectTransport else {
            throw NexusAppError(code: "connect_transport_unavailable",
                                message: "Connect transport is required to start a Nexus Connect session.")
        }
        return try await connectTransport.startConnect(options: options, config: config)
    }

    public func awaitApproval(session: NexusConnectSession) async throws -> NexusApprovedAccount {
        guard let connectTransport else {
            throw NexusAppError(code: "connect_transport_unavailable",
                                message: "Connect transport is required to await wallet approval.")
        }
        let approved = try await connectTransport.awaitApproval(session: session, config: config)
        guard approved.session == nil else {
            throw NexusAppError(
                code: "approval_session_mismatch",
                message: "Wallet approval must not replace the caller's Connect session."
            )
        }
        guard !approved.accountID.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw NexusAppError(code: "approval_missing_account",
                                message: "Wallet approval did not include an account.")
        }
        try requireCanonicalAccountID(approved.accountID,
                                      context: "Wallet approval account")
        for (context, assertedAccount) in [
            ("Configured authority", config.authority),
            ("Connect session approved account", session.approvedAccount),
        ] {
            if let assertedAccount {
                try requireCanonicalAccountID(assertedAccount, context: context)
                guard assertedAccount == approved.accountID else {
                    throw NexusAppError(
                        code: "approval_account_mismatch",
                        message: "\(context) does not match the wallet approval account."
                    )
                }
            }
        }
        let signingPublicKey = try requireAccountSigningKey(
            approved.accountID,
            context: "Wallet approval account",
            sources: [
                ("Wallet approval signingPublicKey", approved.signingPublicKey),
                ("Connect session signingPublicKey", session.signingPublicKey),
                ("Config signingPublicKey", config.signingPublicKey),
            ]
        )
        let approvedSession = session.withApproval(account: approved.accountID,
                                                   signingPublicKey: signingPublicKey)
        return NexusApprovedAccount(accountID: approved.accountID,
                                    signingPublicKey: signingPublicKey,
                                    session: approvedSession)
    }

    public func buildTransferDraft(input: NexusTransferInput) throws -> NexusTransferDraft {
        if let configuredAuthority = config.authority {
            try requireCanonicalAccountID(configuredAuthority,
                                          context: "Configured authority")
        }
        guard let authority = input.authority ?? config.authority,
              !authority.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw NexusAppError(code: "missing_authority",
                                message: "Transfer authority is required.")
        }
        try requireCanonicalAccountID(authority, context: "Transfer authority")
        try requireCanonicalAccountID(input.destinationAccountID,
                                      context: "Transfer destination account")
        try requireCanonicalAccountID(Self.sourceAssetOwner(input.sourceAssetID),
                                      context: "Transfer source asset owner")
        let signingPublicKey = try requireAccountSigningKey(
            authority,
            context: "Transfer authority",
            sources: [
                ("Transfer input signingPublicKey", input.signingPublicKey),
                ("Config signingPublicKey", config.signingPublicKey),
            ]
        )
        let quantity = try KotodamaNumericV1Codec
            .decodeQuantityJSON(input.quantity).canonicalString
        let normalized = NexusTransferInput(
            sourceAssetID: input.sourceAssetID,
            quantity: quantity,
            destinationAccountID: input.destinationAccountID,
            feePayment: input.feePayment,
            authority: authority,
            signingPublicKey: signingPublicKey,
            creationTimeMs: input.creationTimeMs,
            ttlMs: input.ttlMs,
            nonce: input.nonce,
            metadata: input.metadata
        )
        let payloadBytes = try transactionCodec.buildTransferPayload(input: normalized,
                                                                     config: config,
                                                                     authority: authority)
        let signable = NexusSignableTransaction(payloadBytes: payloadBytes,
                                                payloadHashHex: IrohaHash.hash(payloadBytes).hexLowercase(),
                                                authority: authority,
                                                signingPublicKey: signingPublicKey)
        return NexusTransferDraft(input: normalized, signable: signable)
    }

    public func requestSignature(session: NexusConnectSession,
                                 signable: NexusSignableTransaction) async throws -> NexusWalletSignature {
        guard let connectTransport else {
            throw NexusAppError(code: "connect_transport_unavailable",
                                message: "Connect transport is required to request a wallet signature.")
        }
        try requireCanonicalAccountID(signable.authority, context: "Signable authority")
        if let configuredAuthority = config.authority {
            try requireCanonicalAccountID(configuredAuthority,
                                          context: "Configured authority")
        }
        if let approvedAccount = session.approvedAccount {
            try requireCanonicalAccountID(approvedAccount,
                                          context: "Connect session approved account")
        }
        for (context, assertedAccount) in [
            ("Configured authority", config.authority),
            ("Connect session approved account", session.approvedAccount),
        ] {
            if let assertedAccount, assertedAccount != signable.authority {
                throw NexusAppError(
                    code: "approval_account_mismatch",
                    message: "\(context) does not match the signable authority."
                )
            }
        }
        _ = try requireAccountSigningKey(
            signable.authority,
            context: "Signable authority",
            sources: [
                ("Signable signingPublicKey", signable.signingPublicKey),
                ("Connect session signingPublicKey", session.signingPublicKey),
                ("Config signingPublicKey", config.signingPublicKey),
            ]
        )
        try ensureEd25519(signable.signatureAlgorithm)
        let signature = try await connectTransport.requestSignature(session: session,
                                                                    signable: signable,
                                                                    config: config)
        try ensureEd25519(signature.algorithm)
        try validateEd25519Signature(signature.signature)
        return NexusWalletSignature(signature: signature.signature)
    }

    public func finalizeAndSubmit(signable: NexusSignableTransaction,
                                  signature: NexusWalletSignature,
                                  options: NexusFinalizeOptions = NexusFinalizeOptions()) async throws -> NexusTransferReceipt {
        _ = try requireAccountSigningKey(
            signable.authority,
            context: "Signable authority",
            sources: [
                ("Signable signingPublicKey", signable.signingPublicKey),
                ("Config signingPublicKey", config.signingPublicKey),
            ]
        )
        try ensureEd25519(signable.signatureAlgorithm)
        try ensureEd25519(signature.algorithm)
        try validateEd25519PublicKey(signable.signingPublicKey)
        try validateEd25519Signature(signature.signature)
        try validateEd25519SignatureForPayload(publicKey: signable.signingPublicKey,
                                               payloadBytes: signable.payloadBytes,
                                               signature: signature.signature)
        let envelope = try transactionCodec.finalizeSignedTransaction(signable: signable,
                                                                      signature: signature)
        guard let toriiSubmitter else {
            throw NexusAppError(code: "torii_client_unavailable",
                                message: "Torii submitter is required to submit a signed Nexus transfer.")
        }
        let submission: ToriiSubmitTransactionResponse?
        do {
            submission = try await toriiSubmitter.submitNexusTransaction(envelope)
        } catch {
            throw NexusAppError(code: "submit_failed",
                                message: "Failed to submit signed transfer to Torii: \(error.localizedDescription)")
        }
        if let submittedHash = submission?.hash, submittedHash != envelope.hashHex {
            throw NexusAppError(code: "transaction_hash_mismatch",
                                message: "Torii returned transaction hash \(submittedHash) but local hash is \(envelope.hashHex).")
        }
        let status: String?
        if options.waitForFinalStatus {
            let observedStatus: String
            do {
                observedStatus = try await toriiSubmitter.waitForNexusTransactionStatus(
                    hashHex: envelope.hashHex,
                    options: options.pipelineStatusPollOptions
                )
            } catch {
                throw NexusAppError(code: "status_wait_failed",
                                    message: "Failed while waiting for Torii pipeline status: \(error.localizedDescription)")
            }
            guard observedStatus == PipelineTransactionState.applied.kind else {
                throw NexusAppError(
                    code: "status_wait_non_applied",
                    message: "Torii status waiter returned \(observedStatus); canonical Applied status is required."
                )
            }
            status = observedStatus
        } else {
            status = nil
        }
        return NexusTransferReceipt(transactionHashHex: envelope.hashHex,
                                    signedTransaction: envelope,
                                    submission: submission,
                                    finalStatus: status)
    }

    public func transferWithWallet(session: NexusConnectSession,
                                   input: NexusTransferInput,
                                   options: NexusFinalizeOptions = NexusFinalizeOptions()) async throws -> NexusTransferReceipt {
        for (context, account) in [
            ("Connect session approved account", session.approvedAccount),
            ("Transfer authority", input.authority),
            ("Configured authority", config.authority),
        ] {
            if let account {
                try requireCanonicalAccountID(account, context: context)
            }
        }
        guard let authority = input.authority ?? session.approvedAccount ?? config.authority else {
            throw NexusAppError(code: "missing_authority",
                                message: "Transfer authority is required.")
        }
        if let approvedAccount = session.approvedAccount,
           let requestedAuthority = input.authority,
           approvedAccount != requestedAuthority {
            throw NexusAppError(code: "approval_account_mismatch",
                                message: "Transfer authority does not match the approved wallet account.")
        }
        let signingPublicKey = try requireAccountSigningKey(
            authority,
            context: "Transfer authority",
            sources: [
                ("Transfer input signingPublicKey", input.signingPublicKey),
                ("Connect session signingPublicKey", session.signingPublicKey),
                ("Config signingPublicKey", config.signingPublicKey),
            ]
        )
        let draft = try buildTransferDraft(input: input.with(authority: authority,
                                                            signingPublicKey: signingPublicKey))
        let walletSignature = try await requestSignature(session: session, signable: draft.signable)
        return try await finalizeAndSubmit(signable: draft.signable,
                                           signature: walletSignature,
                                           options: options)
    }

    private func requireCanonicalAccountID(_ value: String,
                                           context: String) throws {
        guard value.trimmingCharacters(in: .whitespacesAndNewlines) == value else {
            throw NexusAppError(
                code: "invalid_account_id",
                message: "\(context) must be an exact canonical I105 account for chain discriminant \(config.accountChainDiscriminant)."
            )
        }
        do {
            let address = try AccountAddress.parseEncoded(
                value,
                expectedPrefix: config.accountChainDiscriminant
            )
            guard try address.toI105(chainDiscriminant: config.accountChainDiscriminant) == value else {
                throw NexusAppError(code: "invalid_account_id",
                                    message: "\(context) must use its exact canonical I105 form.")
            }
        } catch let error as NexusAppError {
            throw error
        } catch {
            throw NexusAppError(
                code: "invalid_account_id",
                message: "\(context) must be an exact canonical I105 account for chain discriminant \(config.accountChainDiscriminant)."
            )
        }
    }

    private func requireAccountSigningKey(
        _ accountID: String,
        context: String,
        sources: [(String, Data?)]
    ) throws -> Data {
        try requireCanonicalAccountID(accountID, context: context)
        let address: AccountAddress
        do {
            address = try AccountAddress.parseEncoded(
                accountID,
                expectedPrefix: config.accountChainDiscriminant
            )
        } catch {
            throw NexusAppError(
                code: "missing_signing_public_key",
                message: "\(context) must encode one Ed25519 controller."
            )
        }
        guard let controller = address.singleControllerInfo(),
              controller.algorithm == .ed25519 else {
            throw NexusAppError(
                code: "missing_signing_public_key",
                message: "\(context) must encode one Ed25519 controller."
            )
        }
        try validateEd25519PublicKey(controller.publicKey)
        var supplied = false
        for (field, value) in sources {
            guard let value else { continue }
            supplied = true
            try validateEd25519PublicKey(value)
            guard value == controller.publicKey else {
                throw NexusAppError(
                    code: "approval_account_mismatch",
                    message: "\(field) does not control \(context)."
                )
            }
        }
        guard supplied else {
            throw NexusAppError(
                code: "missing_signing_public_key",
                message: "\(context) did not provide a signing public key."
            )
        }
        return controller.publicKey
    }

    private static func sourceAssetOwner(_ sourceAssetID: String) throws -> String {
        let parts = sourceAssetID.split(separator: "#", omittingEmptySubsequences: false)
        guard (2...3).contains(parts.count), !parts[1].isEmpty else {
            throw NexusAppError(code: "invalid_account_id",
                                message: "Transfer source asset must contain one canonical owner account.")
        }
        if parts.count == 3 {
            let scope = String(parts[2])
            let prefix = "dataspace:"
            let digits = scope.hasPrefix(prefix) ? String(scope.dropFirst(prefix.count)) : ""
            guard !digits.isEmpty,
                  digits.utf8.allSatisfy({ $0 >= 0x30 && $0 <= 0x39 }),
                  digits == "0" || digits.utf8.first != 0x30,
                  UInt64(digits) != nil else {
                throw NexusAppError(
                    code: "invalid_account_id",
                    message: "Transfer source asset scope must be a canonical dataspace:<u64> suffix."
                )
            }
        }
        return String(parts[1])
    }
}

private enum SwiftNexusTransferPayloadEncoder {
    private static let instructionWireName = "iroha.transfer"
    private static let transferBoxTypeName = "iroha_data_model::isi::transfer::TransferBox"
    private static let transferBoxAssetDiscriminant: UInt32 = 2

    private struct TransferInstructionInput {
        let sourceAssetID: String
        let quantity: String
        let destinationAccountID: String
    }

    static func encode(input: NexusTransferInput,
                       chainId: String,
                       accountChainDiscriminant: UInt16,
                       authority: String) throws -> Data {
        let quantity = try KotodamaNumericV1Codec.decodeQuantityJSON(input.quantity).canonicalString
        let instruction = TransferInstructionInput(
            sourceAssetID: input.sourceAssetID,
            quantity: quantity,
            destinationAccountID: input.destinationAccountID
        )
        return try encodePayload(
            instructions: [instruction],
            chainId: chainId,
            accountChainDiscriminant: accountChainDiscriminant,
            authority: authority,
            creationTimeMs: input.creationTimeMs ?? currentTimeMillis(),
            ttlMs: input.ttlMs,
            nonce: input.nonce,
            feePayment: input.feePayment,
            metadata: input.metadata.mapValues { .string($0) }
        )
    }

    private static func encodePayload(instructions instructionInputs: [TransferInstructionInput],
                                      chainId: String,
                                      accountChainDiscriminant: UInt16,
                                      authority: String,
                                      creationTimeMs: UInt64,
                                      ttlMs: UInt64?,
                                      nonce: UInt32?,
                                      feePayment: FeePaymentIntent,
                                      metadata: [String: ToriiJSONValue]) throws -> Data {
        guard !instructionInputs.isEmpty else {
            throw NexusAppError(code: "empty_transfer_instructions",
                                message: "At least one transfer instruction is required.")
        }
        var instructions = CompactNoritoWriter()
        instructions.writeUInt64LE(UInt64(instructionInputs.count))
        for input in instructionInputs {
            instructions.writeField(
                try encodeTransferInstruction(
                    input: input,
                    accountChainDiscriminant: accountChainDiscriminant
                )
            )
        }

        var executable = CompactNoritoWriter()
        executable.writeUInt32LE(0)
        executable.writeField(instructions.data)

        var payload = CompactNoritoWriter()
        payload.writeField(encodeChainId(chainId))
        payload.writeField(
            try encodeAccountId(authority,
                                accountChainDiscriminant: accountChainDiscriminant)
        )
        payload.writeField(CompactNorito.encodeUInt64(creationTimeMs))
        payload.writeField(executable.data)
        payload.writeField(try CompactNorito.encodeOption(ttlMs, encode: CompactNorito.encodeUInt64))
        payload.writeField(try CompactNorito.encodeOption(nonce, encode: CompactNorito.encodeUInt32))
        payload.writeField(try feePayment.compactNorito())
        payload.writeField(TransactionAdmissionIntentV1.queuePlanSynced.norito)
        payload.writeField(try encodeMetadata(metadata))
        payload.writeField(Data([0]))
        return payload.data
    }

    static func encodeInstructionBox(input: NexusTransferInput,
                                     accountChainDiscriminant: UInt16) throws -> Data {
        try encodeTransferInstruction(
            input: TransferInstructionInput(
                sourceAssetID: input.sourceAssetID,
                quantity: input.quantity,
                destinationAccountID: input.destinationAccountID
            ),
            accountChainDiscriminant: accountChainDiscriminant
        )
    }

    private static func encodeTransferInstruction(input: TransferInstructionInput,
                                                  accountChainDiscriminant: UInt16) throws -> Data {
        var transfer = CompactNoritoWriter()
        transfer.writeField(
            try encodeAssetId(input.sourceAssetID,
                              accountChainDiscriminant: accountChainDiscriminant)
        )
        transfer.writeField(try encodeNumeric(input.quantity))
        transfer.writeField(
            try encodeAccountId(input.destinationAccountID,
                                accountChainDiscriminant: accountChainDiscriminant)
        )

        var transferBox = CompactNoritoWriter()
        transferBox.writeUInt32LE(transferBoxAssetDiscriminant)
        transferBox.writeField(transfer.data)
        let framedTransfer = noritoEncode(typeName: transferBoxTypeName,
                                          payload: transferBox.data,
                                          flags: NoritoHeader.compactLen)

        var instruction = CompactNoritoWriter()
        instruction.writeField(CompactNorito.encodeString(instructionWireName))
        instruction.writeField(CanonicalNorito.encodeBytesVec(framedTransfer))
        return instruction.data
    }

    private static func encodeChainId(_ value: String) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(CompactNorito.encodeString(value))
        return writer.data
    }

    private static func encodeAccountId(_ value: String,
                                        accountChainDiscriminant: UInt16) throws -> Data {
        do {
            let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
            guard trimmed.utf8.elementsEqual(value.utf8) else {
                throw CanonicalNoritoError.invalidAccountId(value)
            }
            let address = try AccountAddress.parseEncodedSwiftOnly(
                trimmed,
                expectedPrefix: accountChainDiscriminant
            )
            guard try address.toI105(networkPrefix: accountChainDiscriminant).utf8
                .elementsEqual(trimmed.utf8) else {
                throw CanonicalNoritoError.invalidAccountId(value)
            }
            return try address.compactNoritoAccountControllerPayload()
        } catch {
            throw CanonicalNoritoError.invalidAccountId(value)
        }
    }

    private static func encodeAssetId(_ assetId: String,
                                      accountChainDiscriminant: UInt16) throws -> Data {
        guard let parsed = CanonicalNorito.parsePublicAssetIdLiteral(assetId),
              let definitionBytes = AssetDefinitionAddress.decode(parsed.assetDefinitionId) else {
            throw CanonicalNoritoError.invalidAssetId(assetId)
        }
        var writer = CompactNoritoWriter()
        writer.writeField(
            try encodeAccountId(parsed.accountId,
                                accountChainDiscriminant: accountChainDiscriminant)
        )
        writer.writeField(encodeAssetDefinitionAddress(definitionBytes))
        writer.writeField(encodeAssetBalanceScope(dataspaceId: parsed.dataspaceId))
        return writer.data
    }

    private static func encodeAssetDefinitionAddress(_ bytes: Data) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeByteFields(bytes)
        return writer.data
    }

    private static func encodeAssetBalanceScope(dataspaceId: UInt64?) -> Data {
        var writer = CompactNoritoWriter()
        guard let dataspaceId else {
            writer.writeUInt32LE(0)
            return writer.data
        }
        writer.writeUInt32LE(1)
        var dataspaceWriter = CompactNoritoWriter()
        dataspaceWriter.writeUInt64LE(dataspaceId)
        writer.writeField(dataspaceWriter.data)
        return writer.data
    }

    private static func encodeNumeric(_ value: String) throws -> Data {
        let canonical = try KotodamaNumericV1Codec.decodeQuantityJSON(value).canonicalString
        let numeric = try CanonicalNorito.parseNumeric(canonical)
        let mantissaBytes = try numeric.mantissaBytes(maxBytes: CanonicalNorito.maxBigIntBytes)
        var bigintWriter = CompactNoritoWriter()
        bigintWriter.writeUInt32LE(UInt32(mantissaBytes.count))
        bigintWriter.writeBytes(mantissaBytes)

        var writer = CompactNoritoWriter()
        writer.writeField(bigintWriter.data)
        writer.writeField(CompactNorito.encodeUInt32(numeric.scale))
        return writer.data
    }

    private static func encodeMetadata(_ metadata: [String: ToriiJSONValue]) throws -> Data {
        var writer = CompactNoritoWriter()
        let keys = metadata.keys.sorted()
        writer.writeUInt64LE(UInt64(keys.count))
        for key in keys {
            guard let value = metadata[key] else { continue }
            writer.writeField(try encodeMetadataEntry(key: key, value: value))
        }
        return writer.data
    }

    private static func encodeMetadataEntry(key: String, value: ToriiJSONValue) throws -> Data {
        var entry = CompactNoritoWriter()
        entry.writeField(CompactNorito.encodeString(key))
        let jsonString = try CanonicalNorito.jsonString(from: value)
        var jsonField = CompactNoritoWriter()
        jsonField.writeField(CompactNorito.encodeString(jsonString))
        entry.writeField(jsonField.data)
        return entry.data
    }

    private static func currentTimeMillis() -> UInt64 {
        UInt64((Date().timeIntervalSince1970 * 1_000).rounded())
    }

}

private func ensureEd25519(_ algorithm: String) throws {
    guard algorithm.allSatisfy({ scalar in
        guard let ascii = scalar.asciiValue else { return false }
        return ascii >= 0x20 && ascii <= 0x7E
    }), algorithm == NexusSignatureAlgorithmEd25519 || algorithm == "0" else {
        throw NexusAppError(code: "unsupported_signature_algorithm",
                            message: "Nexus App Facade V1 supports Ed25519 signatures only.")
    }
}

private func validateEd25519PublicKey(_ publicKey: Data) throws {
    guard publicKey.count == 32 else {
        throw NexusAppError(code: "invalid_signing_public_key",
                            message: "Ed25519 signing public key must be 32 bytes.")
    }
}

private func validateEd25519Signature(_ signature: Data) throws {
    guard signature.count == 64 else {
        throw NexusAppError(code: "invalid_signature",
                            message: "Ed25519 signature must be 64 bytes.")
    }
}

private func validateEd25519SignatureForPayload(publicKey: Data,
                                                payloadBytes: Data,
                                                signature: Data) throws {
    do {
        let key = try Curve25519.Signing.PublicKey(rawRepresentation: publicKey)
        let message = IrohaHash.hash(payloadBytes)
        guard key.isValidSignature(signature, for: message) else {
            throw NexusAppError(code: "invalid_signature",
                                message: "Ed25519 signature does not verify for the signable payload.")
        }
    } catch let error as NexusAppError {
        throw error
    } catch {
        throw NexusAppError(code: "invalid_signature",
                            message: "Ed25519 signature does not verify for the signable payload.")
    }
}

private extension Data {
    func hexLowercase() -> String {
        map { String(format: "%02x", $0) }.joined()
    }
}
