import Foundation

public enum TransactionInputError: Error, LocalizedError, Equatable {
    case emptyChainId
    case invalidChainId(String)
    case emptyAccountId(field: String)
    case malformedAccountId(field: String, value: String)
    case emptyAssetDefinitionId
    case malformedAssetDefinitionId(String)
    case emptyDomainId(field: String)
    case malformedDomainId(field: String, value: String)
    case emptyAssetId
    case malformedAssetId(String)
    case invalidZkBallotPublicInputs(String)

    public var errorDescription: String? {
        switch self {
        case .emptyChainId:
            return "Chain id must not be empty."
        case let .invalidChainId(value):
            return "Chain id must not contain whitespace characters (received '\(value)')."
        case let .emptyAccountId(field):
            return "Account id for \(field) must not be empty."
        case let .malformedAccountId(field, value):
            return "Account id for \(field) must include a name and domain separated by '@' with no whitespace or reserved characters (@, #, $) in either component (received '\(value)')."
        case .emptyAssetDefinitionId:
            return "Asset definition id must not be empty."
        case let .malformedAssetDefinitionId(value):
            return "Asset definition id must follow 'asset#domain' with no whitespace or reserved characters (@, #, $) in either component (received '\(value)')."
        case let .emptyDomainId(field):
            return "Domain id for \(field) must not be empty."
        case let .malformedDomainId(field, value):
            return "Domain id for \(field) must not contain whitespace, '@', '#', or '$' (received '\(value)')."
        case .emptyAssetId:
            return "Asset id must not be empty."
        case let .malformedAssetId(value):
            return "Asset id must follow 'asset#domain#account@domain' (or 'asset##account@domain' when the asset and account domains match) with no whitespace or reserved characters (@, #, $) in the asset name/domain components (received '\(value)')."
        case let .invalidZkBallotPublicInputs(reason):
            return "Governance ZK public inputs are invalid: \(reason)"
        }
    }
}

struct TransactionInputValidator {
    struct NamedAccountId {
        let field: String
        let value: String
    }

    struct ValidatedIds {
        let chainId: String
        let authorityId: String
        let assetDefinitionId: String?
        let accountIds: [String: String]
    }

    private static func containsReservedIdCharacters(_ value: String) -> Bool {
        value.contains("@") || value.contains("#") || value.contains("$")
    }

    static func validate(chainId: String,
                         authorityId: String,
                         assetDefinitionId: String? = nil,
                         accountIds: [NamedAccountId] = []) throws -> ValidatedIds {
        let sanitizedChainId = try sanitizeChainId(chainId)
        let sanitizedAuthority = try sanitizeAccountId(authorityId, field: "authority")
        var sanitizedAccounts: [String: String] = [:]
        for account in accountIds {
            sanitizedAccounts[account.field] = try sanitizeAccountId(account.value, field: account.field)
        }
        let sanitizedAssetDefinitionId = try assetDefinitionId.map { try sanitizeAssetDefinitionId($0) }
        return ValidatedIds(chainId: sanitizedChainId,
                            authorityId: sanitizedAuthority,
                            assetDefinitionId: sanitizedAssetDefinitionId,
                            accountIds: sanitizedAccounts)
    }

    private static func sanitizeChainId(_ chainId: String) throws -> String {
        let trimmed = chainId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw TransactionInputError.emptyChainId
        }
        if trimmed.rangeOfCharacter(from: .whitespacesAndNewlines) != nil {
            throw TransactionInputError.invalidChainId(trimmed)
        }
        return trimmed
    }

    private static func sanitizeAccountId(_ accountId: String, field: String) throws -> String {
        let trimmed = accountId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw TransactionInputError.emptyAccountId(field: field)
        }
        if trimmed.rangeOfCharacter(from: .whitespacesAndNewlines) != nil {
            throw TransactionInputError.malformedAccountId(field: field, value: trimmed)
        }
        let components = trimmed.split(separator: "@", omittingEmptySubsequences: false)
        guard components.count == 2,
              !components[0].isEmpty,
              !components[1].isEmpty else {
            throw TransactionInputError.malformedAccountId(field: field, value: trimmed)
        }
        if containsReservedIdCharacters(String(components[0]))
            || containsReservedIdCharacters(String(components[1])) {
            throw TransactionInputError.malformedAccountId(field: field, value: trimmed)
        }
        return trimmed
    }

    private static func sanitizeAssetDefinitionId(_ assetDefinitionId: String) throws -> String {
        let trimmed = assetDefinitionId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw TransactionInputError.emptyAssetDefinitionId
        }
        if trimmed.rangeOfCharacter(from: .whitespacesAndNewlines) != nil {
            throw TransactionInputError.malformedAssetDefinitionId(trimmed)
        }
        let components = trimmed.split(separator: "#", omittingEmptySubsequences: false)
        guard components.count == 2,
              !components[0].isEmpty,
              !components[1].isEmpty else {
            throw TransactionInputError.malformedAssetDefinitionId(trimmed)
        }
        if containsReservedIdCharacters(String(components[0]))
            || containsReservedIdCharacters(String(components[1])) {
            throw TransactionInputError.malformedAssetDefinitionId(trimmed)
        }
        return trimmed
    }

    private static func sanitizeDomainId(_ domainId: String, field: String) throws -> String {
        let trimmed = domainId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw TransactionInputError.emptyDomainId(field: field)
        }
        if trimmed.rangeOfCharacter(from: .whitespacesAndNewlines) != nil {
            throw TransactionInputError.malformedDomainId(field: field, value: trimmed)
        }
        if trimmed.contains("@") || trimmed.contains("#") || trimmed.contains("$") {
            throw TransactionInputError.malformedDomainId(field: field, value: trimmed)
        }
        return trimmed
    }

    private static func sanitizeAssetId(_ assetId: String) throws -> String {
        let trimmed = assetId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw TransactionInputError.emptyAssetId
        }
        if trimmed.rangeOfCharacter(from: .whitespacesAndNewlines) != nil {
            throw TransactionInputError.malformedAssetId(trimmed)
        }
        let parts = trimmed.split(separator: "#", maxSplits: 2, omittingEmptySubsequences: false)
        guard parts.count == 3 else {
            throw TransactionInputError.malformedAssetId(trimmed)
        }
        let assetName = parts[0]
        let definitionDomain = parts[1]
        let ownerAccount = String(parts[2])
        guard !assetName.isEmpty else {
            throw TransactionInputError.malformedAssetId(trimmed)
        }
        if containsReservedIdCharacters(String(assetName)) {
            throw TransactionInputError.malformedAssetId(trimmed)
        }
        if !definitionDomain.isEmpty {
            _ = try sanitizeDomainId(String(definitionDomain), field: "assetId")
        }
        _ = try sanitizeAccountId(ownerAccount, field: "assetId")
        return trimmed
    }

    static func sanitizeMetadataTarget(_ target: MetadataTarget) throws -> MetadataTarget {
        switch target {
        case let .domain(domainId):
            let sanitized = try sanitizeDomainId(domainId, field: "target")
            return .domain(sanitized)
        case let .account(accountId):
            let sanitized = try sanitizeAccountId(accountId, field: "target")
            return .account(sanitized)
        case let .assetDefinition(assetDefinitionId):
            let sanitized = try sanitizeAssetDefinitionId(assetDefinitionId)
            return .assetDefinition(sanitized)
        case let .asset(assetId):
            let sanitized = try sanitizeAssetId(assetId)
            return .asset(sanitized)
        }
    }
}

enum SwiftTransactionEncoderError: Error, LocalizedError, Sendable {
    case nativeBridgeUnavailable
    case nativeBridgeError(NativeBridgeError)
    case unsupportedSigningAlgorithm(SigningAlgorithm)

    public var errorDescription: String? {
        switch self {
        case .nativeBridgeUnavailable:
            return NoritoNativeBridge.bridgeUnavailableMessage(
                "Norito native bridge is unavailable on this platform."
            )
        case let .nativeBridgeError(error):
            return "Norito native bridge call failed: \(error)"
        case let .unsupportedSigningAlgorithm(algorithm):
            return "Signing algorithm \(algorithm) is not supported by this encoder."
        }
    }
}

struct SwiftTransactionEncoder {
    private static let signedTransactionType = "iroha_data_model::transaction::signed::SignedTransaction"

    private static func bridgeOrThrow(_ body: () throws -> NativeSignedTransaction?) throws -> NativeSignedTransaction {
        guard NoritoNativeBridge.shared.isAvailable else {
            throw SwiftTransactionEncoderError.nativeBridgeUnavailable
        }
        do {
            if let native = try body() {
                return native
            }
            throw SwiftTransactionEncoderError.nativeBridgeUnavailable
        } catch let error as NativeBridgeError {
            throw SwiftTransactionEncoderError.nativeBridgeError(error)
        }
    }

    private static func wrap(native: NativeSignedTransaction) -> SignedTransactionEnvelope {
        if let framed = noritoDecodeFrame(native.signedBytes) {
            return SignedTransactionEnvelope(norito: native.signedBytes,
                                             signedTransaction: framed.payload,
                                             payload: nil,
                                             transactionHash: native.hash)
        }
        let norito = noritoEncode(typeName: signedTransactionType,
                                  payload: native.signedBytes,
                                  flags: 0x04)
        return SignedTransactionEnvelope(norito: norito,
                                         signedTransaction: native.signedBytes,
                                         payload: nil,
                                         transactionHash: native.hash)
    }

    static func encodeTransfer(transfer: TransferRequest,
                               keypair: Keypair,
                               creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeTransfer(transfer: transfer, signingKey: signingKey, creationTimeMs: creationTimeMs)
    }

    static func encodeTransfer(transfer: TransferRequest,
                               signingKey: SigningKey,
                               creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(chainId: transfer.chainId,
                                                         authorityId: transfer.authority,
                                                         assetDefinitionId: transfer.assetDefinitionId,
                                                         accountIds: [.init(field: "destination", value: transfer.destination)])
        guard let assetDefinitionId = ids.assetDefinitionId else {
            throw TransactionInputError.emptyAssetDefinitionId
        }
        let destination = ids.accountIds["destination"] ?? transfer.destination
        let privateKey = try privateKeyBytes(from: signingKey)
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeTransfer(chainId: ids.chainId,
                                                         authority: ids.authorityId,
                                                         creationTimeMs: creationTimeMs,
                                                         ttlMs: transfer.ttlMs,
                                                         nonce: transfer.nonce,
                                                         assetDefinitionId: assetDefinitionId,
                                                         quantity: transfer.quantity,
                                                         destination: destination,
                                                         privateKey: privateKey,
                                                         algorithm: signingKey.algorithm)
        }
        return wrap(native: native)
    }

    static func encodeMint(request: MintRequest,
                            keypair: Keypair,
                            creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeMint(request: request, signingKey: signingKey, creationTimeMs: creationTimeMs)
    }

    static func encodeMint(request: MintRequest,
                            signingKey: SigningKey,
                            creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(chainId: request.chainId,
                                                         authorityId: request.authority,
                                                         assetDefinitionId: request.assetDefinitionId,
                                                         accountIds: [.init(field: "destination", value: request.destination)])
        guard let assetDefinitionId = ids.assetDefinitionId else {
            throw TransactionInputError.emptyAssetDefinitionId
        }
        let destination = ids.accountIds["destination"] ?? request.destination
        let privateKey = try privateKeyBytes(from: signingKey)
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeMint(chainId: ids.chainId,
                                                     authority: ids.authorityId,
                                                     creationTimeMs: creationTimeMs,
                                                     ttlMs: request.ttlMs,
                                                     nonce: request.nonce,
                                                     assetDefinitionId: assetDefinitionId,
                                                     quantity: request.quantity,
                                                     destination: destination,
                                                     privateKey: privateKey,
                                                     algorithm: signingKey.algorithm)
        }
        return wrap(native: native)
    }

    static func encodeBurn(request: BurnRequest,
                            keypair: Keypair,
                            creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeBurn(request: request, signingKey: signingKey, creationTimeMs: creationTimeMs)
    }

    static func encodeBurn(request: BurnRequest,
                            signingKey: SigningKey,
                            creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(chainId: request.chainId,
                                                         authorityId: request.authority,
                                                         assetDefinitionId: request.assetDefinitionId,
                                                         accountIds: [.init(field: "destination", value: request.destination)])
        guard let assetDefinitionId = ids.assetDefinitionId else {
            throw TransactionInputError.emptyAssetDefinitionId
        }
        let destination = ids.accountIds["destination"] ?? request.destination
        let privateKey = try privateKeyBytes(from: signingKey)
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeBurn(chainId: ids.chainId,
                                                     authority: ids.authorityId,
                                                     creationTimeMs: creationTimeMs,
                                                     ttlMs: request.ttlMs,
                                                     nonce: request.nonce,
                                                     assetDefinitionId: assetDefinitionId,
                                                     quantity: request.quantity,
                                                     destination: destination,
                                                     privateKey: privateKey,
                                                     algorithm: signingKey.algorithm)
        }
        return wrap(native: native)
    }

    static func encodeShield(request: ShieldRequest,
                             keypair: Keypair,
                             creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeShield(request: request, signingKey: signingKey, creationTimeMs: creationTimeMs)
    }

    static func encodeShield(request: ShieldRequest,
                             signingKey: SigningKey,
                             creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(chainId: request.chainId,
                                                         authorityId: request.authority,
                                                         assetDefinitionId: request.assetDefinitionId,
                                                         accountIds: [.init(field: "fromAccountId", value: request.fromAccountId)])
        guard let assetDefinitionId = ids.assetDefinitionId else {
            throw TransactionInputError.emptyAssetDefinitionId
        }
        let fromAccountId = ids.accountIds["fromAccountId"] ?? request.fromAccountId
        let privateKey = try privateKeyBytes(from: signingKey)
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeShield(chainId: ids.chainId,
                                                       authority: ids.authorityId,
                                                       creationTimeMs: creationTimeMs,
                                                       ttlMs: request.ttlMs,
                                                       assetDefinitionId: assetDefinitionId,
                                                       fromAccountId: fromAccountId,
                                                       amount: request.amount,
                                                       noteCommitment: request.noteCommitment,
                                                       payloadEphemeral: request.payload.ephemeralPublicKey,
                                                       payloadNonce: request.payload.nonce,
                                                       payloadCiphertext: request.payload.ciphertext,
                                                       privateKey: privateKey,
                                                       algorithm: signingKey.algorithm)
        }
        return wrap(native: native)
    }

    static func encodeUnshield(request: UnshieldRequest,
                               keypair: Keypair,
                               creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeUnshield(request: request, signingKey: signingKey, creationTimeMs: creationTimeMs)
    }

    static func encodeUnshield(request: UnshieldRequest,
                               signingKey: SigningKey,
                               creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(chainId: request.chainId,
                                                         authorityId: request.authority,
                                                         assetDefinitionId: request.assetDefinitionId,
                                                         accountIds: [.init(field: "toAccountId", value: request.toAccountId)])
        guard let assetDefinitionId = ids.assetDefinitionId else {
            throw TransactionInputError.emptyAssetDefinitionId
        }
        let destinationAccountId = ids.accountIds["toAccountId"] ?? request.toAccountId
        let privateKey = try privateKeyBytes(from: signingKey)
        let proofJSON = try request.proof.encodedJSON()
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeUnshield(chainId: ids.chainId,
                                                         authority: ids.authorityId,
                                                         creationTimeMs: creationTimeMs,
                                                         ttlMs: request.ttlMs,
                                                         assetDefinitionId: assetDefinitionId,
                                                         destinationAccountId: destinationAccountId,
                                                         amount: request.publicAmount,
                                                         inputs: request.flattenedInputs,
                                                         proofJSON: proofJSON,
                                                         rootHint: request.rootHint,
                                                         privateKey: privateKey,
                                                         algorithm: signingKey.algorithm)
        }
        return wrap(native: native)
    }

    static func encodeZkTransfer(request: ZkTransferRequest,
                                 keypair: Keypair,
                                 creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeZkTransfer(request: request, signingKey: signingKey, creationTimeMs: creationTimeMs)
    }

    static func encodeZkTransfer(request: ZkTransferRequest,
                                 signingKey: SigningKey,
                                 creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(chainId: request.chainId,
                                                         authorityId: request.authority,
                                                         assetDefinitionId: request.assetDefinitionId)
        guard let assetDefinitionId = ids.assetDefinitionId else {
            throw TransactionInputError.emptyAssetDefinitionId
        }
        let privateKey = try privateKeyBytes(from: signingKey)
        let proofJSON = try request.proof.encodedJSON()
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeZkTransfer(chainId: ids.chainId,
                                                           authority: ids.authorityId,
                                                           creationTimeMs: creationTimeMs,
                                                           ttlMs: request.ttlMs,
                                                           assetDefinitionId: assetDefinitionId,
                                                           inputs: request.flattenedInputs,
                                                           outputs: request.flattenedOutputs,
                                                           proofJSON: proofJSON,
                                                           rootHint: request.rootHint,
                                                           privateKey: privateKey,
                                                           algorithm: signingKey.algorithm)
        }
        return wrap(native: native)
    }

    static func encodeRegisterZkAsset(request: RegisterZkAssetRequest,
                                      keypair: Keypair,
                                      creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeRegisterZkAsset(request: request,
                                         signingKey: signingKey,
                                         creationTimeMs: creationTimeMs)
    }

    static func encodeRegisterZkAsset(request: RegisterZkAssetRequest,
                                      signingKey: SigningKey,
                                      creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(chainId: request.chainId,
                                                         authorityId: request.authority,
                                                         assetDefinitionId: request.assetDefinitionId)
        guard let assetDefinitionId = ids.assetDefinitionId else {
            throw TransactionInputError.emptyAssetDefinitionId
        }
        let privateKey = try privateKeyBytes(from: signingKey)
        let transferVk = request.transferVerifyingKey?.encodedValue
        let unshieldVk = request.unshieldVerifyingKey?.encodedValue
        let shieldVk = request.shieldVerifyingKey?.encodedValue
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeRegisterZkAsset(
                chainId: ids.chainId,
                authority: ids.authorityId,
                creationTimeMs: creationTimeMs,
                ttlMs: request.ttlMs,
                assetDefinitionId: assetDefinitionId,
                modeCode: request.mode.rawValue,
                allowShield: request.allowShield,
                allowUnshield: request.allowUnshield,
                transferVerifyingKey: transferVk,
                unshieldVerifyingKey: unshieldVk,
                shieldVerifyingKey: shieldVk,
                privateKey: privateKey,
                algorithm: signingKey.algorithm
            )
        }
        return wrap(native: native)
    }

    static func encodeMultisigRegister(request: MultisigRegisterRequest,
                                       keypair: Keypair,
                                       creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeMultisigRegister(request: request,
                                          signingKey: signingKey,
                                          creationTimeMs: creationTimeMs)
    }

    static func encodeMultisigRegister(request: MultisigRegisterRequest,
                                       signingKey: SigningKey,
                                       creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(chainId: request.chainId,
                                                         authorityId: request.authority,
                                                         accountIds: [
                                                            TransactionInputValidator.NamedAccountId(field: "account", value: request.accountId)
                                                         ])
        let privateKey = try privateKeyBytes(from: signingKey)
        let specJSON = try request.spec.encodeJSON()
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeMultisigRegister(chainId: ids.chainId,
                                                                 authority: ids.authorityId,
                                                                 creationTimeMs: creationTimeMs,
                                                                 ttlMs: request.ttlMs,
                                                                 accountId: ids.accountIds["account"] ?? request.accountId,
                                                                 specJSON: specJSON,
                                                                 privateKey: privateKey,
                                                                 algorithm: signingKey.algorithm)
        }
        return wrap(native: native)
    }

    static func encodeSetMetadata(request: SetMetadataRequest,
                                  keypair: Keypair,
                                  creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeSetMetadata(request: request, signingKey: signingKey, creationTimeMs: creationTimeMs)
    }

    static func encodeSetMetadata(request: SetMetadataRequest,
                                  signingKey: SigningKey,
                                  creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(chainId: request.chainId,
                                                         authorityId: request.authority)
        let target = try TransactionInputValidator.sanitizeMetadataTarget(request.target)
        let privateKey = try privateKeyBytes(from: signingKey)
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeSetKeyValue(
                chainId: ids.chainId,
                authority: ids.authorityId,
                creationTimeMs: creationTimeMs,
                ttlMs: request.ttlMs,
                targetKind: target.targetKind,
                objectId: target.objectId,
                key: request.key,
                valueJson: request.value.data,
                privateKey: privateKey,
                algorithm: signingKey.algorithm
            )
        }
        return wrap(native: native)
    }

    static func encodeRemoveMetadata(request: RemoveMetadataRequest,
                                     keypair: Keypair,
                                     creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeRemoveMetadata(request: request, signingKey: signingKey, creationTimeMs: creationTimeMs)
    }

    static func encodeRemoveMetadata(request: RemoveMetadataRequest,
                                     signingKey: SigningKey,
                                     creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(chainId: request.chainId,
                                                         authorityId: request.authority)
        let target = try TransactionInputValidator.sanitizeMetadataTarget(request.target)
        let privateKey = try privateKeyBytes(from: signingKey)
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeRemoveKeyValue(
                chainId: ids.chainId,
                authority: ids.authorityId,
                creationTimeMs: creationTimeMs,
                ttlMs: request.ttlMs,
                targetKind: target.targetKind,
                objectId: target.objectId,
                key: request.key,
                privateKey: privateKey,
                algorithm: signingKey.algorithm
            )
        }
        return wrap(native: native)
    }

    static func encodeProposeDeploy(request: ProposeDeployContractRequest,
                                    keypair: Keypair,
                                    creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeProposeDeploy(request: request, signingKey: signingKey, creationTimeMs: creationTimeMs)
    }

    static func encodeProposeDeploy(request: ProposeDeployContractRequest,
                                    signingKey: SigningKey,
                                    creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(chainId: request.chainId,
                                                         authorityId: request.authority)
        let privateKey = try privateKeyBytes(from: signingKey)
        let windowTuple = request.window.map { ($0.lower, $0.upper) }
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeGovernanceProposeDeploy(
                chainId: ids.chainId,
                authority: ids.authorityId,
                creationTimeMs: creationTimeMs,
                ttlMs: request.ttlMs,
                namespace: request.namespace,
                contractId: request.contractId,
                codeHashHex: request.codeHashHex,
                abiHashHex: request.abiHashHex,
                abiVersion: request.abiVersion,
                window: windowTuple,
                modeCode: request.mode?.rawValue,
                privateKey: privateKey,
                algorithm: signingKey.algorithm
            )
        }
        return wrap(native: native)
    }

    static func encodeCastPlainBallot(request: CastPlainBallotRequest,
                                      keypair: Keypair,
                                      creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeCastPlainBallot(request: request, signingKey: signingKey, creationTimeMs: creationTimeMs)
    }

    static func encodeCastPlainBallot(request: CastPlainBallotRequest,
                                      signingKey: SigningKey,
                                      creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(
            chainId: request.chainId,
            authorityId: request.authority,
            accountIds: [.init(field: "owner", value: request.owner)]
        )
        let owner = ids.accountIds["owner"] ?? request.owner
        let privateKey = try privateKeyBytes(from: signingKey)
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeGovernanceCastPlainBallot(
                chainId: ids.chainId,
                authority: ids.authorityId,
                creationTimeMs: creationTimeMs,
                ttlMs: request.ttlMs,
                referendumId: request.referendumId,
                owner: owner,
                amount: request.amount,
                durationBlocks: request.durationBlocks,
                direction: request.direction.rawValue,
                privateKey: privateKey,
                algorithm: signingKey.algorithm
            )
        }
        return wrap(native: native)
    }

    static func encodeCastZkBallot(request: CastZkBallotRequest,
                                   keypair: Keypair,
                                   creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeCastZkBallot(request: request, signingKey: signingKey, creationTimeMs: creationTimeMs)
    }

    static func encodeCastZkBallot(request: CastZkBallotRequest,
                                   signingKey: SigningKey,
                                   creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(chainId: request.chainId,
                                                         authorityId: request.authority)
        let privateKey = try privateKeyBytes(from: signingKey)
        let publicInputs = try normalizeZkBallotPublicInputs(request.publicInputs.data)
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeGovernanceCastZkBallot(
                chainId: ids.chainId,
                authority: ids.authorityId,
                creationTimeMs: creationTimeMs,
                ttlMs: request.ttlMs,
                electionId: request.electionId,
                proofB64: request.proofB64,
                publicInputs: publicInputs,
                privateKey: privateKey,
                algorithm: signingKey.algorithm
            )
        }
        return wrap(native: native)
    }

    private static func normalizeZkBallotPublicInputs(_ data: Data) throws -> Data {
        let decoded: ToriiJSONValue
        do {
            decoded = try JSONDecoder().decode(ToriiJSONValue.self, from: data)
        } catch {
            throw TransactionInputError.invalidZkBallotPublicInputs("public_inputs_json must be valid JSON.")
        }
        guard case let .object(map) = decoded else {
            throw TransactionInputError.invalidZkBallotPublicInputs("public_inputs_json must be a JSON object.")
        }
        var normalized = map
        try rejectZkBallotPublicInputKey(&normalized,
                                         key: "durationBlocks",
                                         canonicalKey: "duration_blocks")
        try rejectZkBallotPublicInputKey(&normalized,
                                         key: "root_hint_hex",
                                         canonicalKey: "root_hint")
        try rejectZkBallotPublicInputKey(&normalized,
                                         key: "rootHintHex",
                                         canonicalKey: "root_hint")
        try rejectZkBallotPublicInputKey(&normalized,
                                         key: "rootHint",
                                         canonicalKey: "root_hint")
        try rejectZkBallotPublicInputKey(&normalized,
                                         key: "nullifier_hex",
                                         canonicalKey: "nullifier")
        try rejectZkBallotPublicInputKey(&normalized,
                                         key: "nullifierHex",
                                         canonicalKey: "nullifier")
        try normalizeZkBallotPublicInputHex(&normalized, key: "root_hint")
        try normalizeZkBallotPublicInputHex(&normalized, key: "nullifier")
        let hasOwner = zkHintPresent(normalized["owner"])
        let hasAmount = zkHintPresent(normalized["amount"])
        let hasDuration = zkHintPresent(normalized["duration_blocks"])
        let hasAnyHint = hasOwner || hasAmount || hasDuration
        if hasAnyHint && !(hasOwner && hasAmount && hasDuration) {
            throw TransactionInputError.invalidZkBallotPublicInputs(
                "lock hints must include owner, amount, duration_blocks"
            )
        }
        try ensureZkBallotOwnerCanonical(normalized)
        let encoder = JSONEncoder()
        if #available(iOS 11.0, macOS 10.13, *) {
            encoder.outputFormatting.insert(.sortedKeys)
        }
        return try encoder.encode(ToriiJSONValue.object(normalized))
    }

    private static func rejectZkBallotPublicInputKey(_ inputs: inout [String: ToriiJSONValue],
                                                     key: String,
                                                     canonicalKey: String) throws {
        guard inputs[key] != nil else { return }
        throw TransactionInputError.invalidZkBallotPublicInputs(
            "public_inputs_json must use \(canonicalKey) (unsupported key \(key))"
        )
    }

    private static func normalizeZkBallotPublicInputHex(_ inputs: inout [String: ToriiJSONValue],
                                                        key: String) throws {
        guard let value = inputs[key] else { return }
        if case .null = value { return }
        guard case let .string(raw) = value else {
            throw TransactionInputError.invalidZkBallotPublicInputs(
                "\(key) must be 32-byte hex"
            )
        }
        let canonical = try canonicalizeZkBallotHexHint(raw, field: key)
        inputs[key] = .string(canonical)
    }

    private static func canonicalizeZkBallotHexHint(_ raw: String, field: String) throws -> String {
        var trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if let colonIndex = trimmed.firstIndex(of: ":") {
            let scheme = String(trimmed[..<colonIndex])
            let rest = String(trimmed[trimmed.index(after: colonIndex)...])
            if !scheme.isEmpty && scheme.lowercased() != "blake2b32" {
                throw TransactionInputError.invalidZkBallotPublicInputs(
                    "\(field) must be 32-byte hex"
                )
            }
            trimmed = rest.trimmingCharacters(in: .whitespacesAndNewlines)
        }
        if trimmed.hasPrefix("0x") || trimmed.hasPrefix("0X") {
            trimmed = String(trimmed.dropFirst(2))
        }
        guard trimmed.count == 64, Data(hexString: trimmed) != nil else {
            throw TransactionInputError.invalidZkBallotPublicInputs(
                "\(field) must be 32-byte hex"
            )
        }
        return trimmed.lowercased()
    }

    private static func ensureZkBallotOwnerCanonical(_ inputs: [String: ToriiJSONValue]) throws {
        guard let value = inputs["owner"] else { return }
        if case .null = value { return }
        guard case let .string(owner) = value else {
            throw TransactionInputError.invalidZkBallotPublicInputs(
                "owner must be a canonical account id"
            )
        }
        let canonical = try canonicalizeZkBallotOwnerLiteral(owner)
        if canonical != owner {
            throw TransactionInputError.invalidZkBallotPublicInputs(
                "owner must use canonical account id form"
            )
        }
    }

    private static func canonicalizeZkBallotOwnerLiteral(_ raw: String) throws -> String {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, trimmed == raw else {
            throw TransactionInputError.invalidZkBallotPublicInputs(
                "owner must be a canonical account id"
            )
        }
        if trimmed.rangeOfCharacter(from: .whitespacesAndNewlines) != nil {
            throw TransactionInputError.invalidZkBallotPublicInputs(
                "owner must be a canonical account id"
            )
        }
        let parts = trimmed.split(separator: "@", omittingEmptySubsequences: false)
        guard parts.count == 2, !parts[0].isEmpty, !parts[1].isEmpty else {
            throw TransactionInputError.invalidZkBallotPublicInputs(
                "owner must be a canonical account id"
            )
        }
        let addressPart = String(parts[0])
        let domainPart = String(parts[1])
        let canonicalDomain = try AccountAddress.canonicalizeDomainLabel(domainPart)
        let (address, format) = try AccountAddress.parseAny(
            addressPart,
            expectedPrefix: 0x02F1
        )
        guard format == .ih58 else {
            throw TransactionInputError.invalidZkBallotPublicInputs(
                "owner must be a canonical account id"
            )
        }
        guard address.matchesDomainLabel(canonicalDomain) else {
            throw TransactionInputError.invalidZkBallotPublicInputs(
                "owner must be a canonical account id"
            )
        }
        let ih58 = try address.toIH58(networkPrefix: 0x02F1)
        return "\(ih58)@\(canonicalDomain)"
    }

    private static func zkHintPresent(_ value: ToriiJSONValue?) -> Bool {
        guard let value else { return false }
        if case .null = value {
            return false
        }
        return true
    }

    static func encodeEnactReferendum(request: EnactReferendumRequest,
                                      keypair: Keypair,
                                      creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeEnactReferendum(request: request, signingKey: signingKey, creationTimeMs: creationTimeMs)
    }

    static func encodeEnactReferendum(request: EnactReferendumRequest,
                                      signingKey: SigningKey,
                                      creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(chainId: request.chainId,
                                                         authorityId: request.authority)
        let privateKey = try privateKeyBytes(from: signingKey)
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeGovernanceEnactReferendum(
                chainId: ids.chainId,
                authority: ids.authorityId,
                creationTimeMs: creationTimeMs,
                ttlMs: request.ttlMs,
                referendumIdHex: request.referendumIdHex,
                preimageHashHex: request.preimageHashHex,
                windowLower: request.window.lower,
                windowUpper: request.window.upper,
                privateKey: privateKey,
                algorithm: signingKey.algorithm
            )
        }
        return wrap(native: native)
    }

    static func encodeFinalizeReferendum(request: FinalizeReferendumRequest,
                                         keypair: Keypair,
                                         creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeFinalizeReferendum(request: request, signingKey: signingKey, creationTimeMs: creationTimeMs)
    }

    static func encodeFinalizeReferendum(request: FinalizeReferendumRequest,
                                         signingKey: SigningKey,
                                         creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(chainId: request.chainId,
                                                         authorityId: request.authority)
        let privateKey = try privateKeyBytes(from: signingKey)
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeGovernanceFinalizeReferendum(
                chainId: ids.chainId,
                authority: ids.authorityId,
                creationTimeMs: creationTimeMs,
                ttlMs: request.ttlMs,
                referendumId: request.referendumId,
                proposalIdHex: request.proposalIdHex,
                privateKey: privateKey,
                algorithm: signingKey.algorithm
            )
        }
        return wrap(native: native)
    }

    static func encodePersistCouncil(request: PersistCouncilRequest,
                                     keypair: Keypair,
                                     creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodePersistCouncil(request: request, signingKey: signingKey, creationTimeMs: creationTimeMs)
    }

    static func encodePersistCouncil(request: PersistCouncilRequest,
                                     signingKey: SigningKey,
                                     creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let memberAccounts = request.members.enumerated().map {
            TransactionInputValidator.NamedAccountId(field: "members[\($0.offset)]", value: $0.element)
        }
        let ids = try TransactionInputValidator.validate(chainId: request.chainId,
                                                         authorityId: request.authority,
                                                         accountIds: memberAccounts)
        let sanitizedMembers = memberAccounts.map { ids.accountIds[$0.field] ?? $0.value }
        let privateKey = try privateKeyBytes(from: signingKey)
        let membersJson = try NoritoJSON(sanitizedMembers).data
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeGovernancePersistCouncil(
                chainId: ids.chainId,
                authority: ids.authorityId,
                creationTimeMs: creationTimeMs,
                ttlMs: request.ttlMs,
                epoch: request.epoch,
                candidatesCount: request.candidatesCount,
                derivedBy: request.derivedBy.rawValue,
                membersJson: membersJson,
                privateKey: privateKey,
                algorithm: signingKey.algorithm
            )
        }
        return wrap(native: native)
    }

    private static func privateKeyBytes(from signingKey: SigningKey) throws -> Data {
        if signingKey.algorithm != .ed25519 {
            guard NoritoNativeBridge.shared.supportsTransactions(using: signingKey.algorithm) else {
                throw SwiftTransactionEncoderError.unsupportedSigningAlgorithm(signingKey.algorithm)
            }
        }
        if let privateKey = signingKey.exportPrivateKeyBytes() {
            return privateKey
        }
        throw SwiftTransactionEncoderError.unsupportedSigningAlgorithm(signingKey.algorithm)
    }
}
