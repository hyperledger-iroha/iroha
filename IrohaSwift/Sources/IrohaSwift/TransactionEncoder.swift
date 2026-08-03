import Foundation

public enum TransactionInputError: Error, LocalizedError, Equatable {
    case emptyChainId
    case invalidChainId(String)
    case emptyAccountId(field: String)
    case malformedAccountId(field: String, value: String)
    case emptyRwaId(field: String)
    case malformedRwaId(field: String, value: String)
    case emptyAssetDefinitionId
    case malformedAssetDefinitionId(String)
    case emptyDomainId(field: String)
    case malformedDomainId(field: String, value: String)
    case emptyLabel(field: String)
    case malformedLabel(field: String, value: String)
    case emptyAssetId
    case malformedAssetId(String)
    case invalidGovernanceWindow(lower: UInt64, upper: UInt64)
    case invalidGovernanceAbiVersion(String)
    case invalidGovernanceSelector(field: String, value: String)
    case invalidGovernanceFinalizationId(field: String, value: String)
    case mismatchedGovernanceFinalizationIds
    case invalidZkBallotPublicInputs(String)

    public var errorDescription: String? {
        switch self {
        case .emptyChainId:
            return "Chain id must not be empty."
        case let .invalidChainId(value):
            return "Chain id must be 1...128 ASCII bytes, begin and end with an alphanumeric character, and contain only alphanumeric characters, '.', '_', ':' or '-' (received '\(value)')."
        case let .emptyAccountId(field):
            return "Account id for \(field) must not be empty."
        case let .malformedAccountId(field, value):
            return "Account id for \(field) must be a canonical bare I105 literal with no whitespace (received '\(value)')."
        case let .emptyRwaId(field):
            return "RWA id for \(field) must not be empty."
        case let .malformedRwaId(field, value):
            return "RWA id for \(field) must use '<64-hex-hash>$<domain>' public form with no whitespace (received '\(value)')."
        case .emptyAssetDefinitionId:
            return "Asset definition id must not be empty."
        case let .malformedAssetDefinitionId(value):
            return "Asset definition id must use canonical unprefixed Base58 form with optional canonical #dataspace:<id> suffix (received '\(value)')."
        case let .emptyDomainId(field):
            return "Domain id for \(field) must not be empty."
        case let .malformedDomainId(field, value):
            return "Domain id for \(field) must use canonical fully-qualified 'name.dataspace' form with no whitespace or reserved separators (received '\(value)')."
        case let .emptyLabel(field):
            return "Label for \(field) must not be empty."
        case let .malformedLabel(field, value):
            return "Label for \(field) must use lowercase a-z, 0-9, '_' or '-' and be 32 characters or fewer (received '\(value)')."
        case .emptyAssetId:
            return "Asset id must not be empty."
        case let .malformedAssetId(value):
            return "Asset id must use canonical unprefixed Base58 form with no whitespace (received '\(value)')."
        case let .invalidGovernanceWindow(lower, upper):
            return "Governance window upper bound \(upper) must not precede lower bound \(lower)."
        case let .invalidGovernanceAbiVersion(value):
            return "Governance ABI version must be exactly '1' in the first release (received '\(value)')."
        case let .invalidGovernanceSelector(field, value):
            return "Governance selector for \(field) must be 1...128 RFC 3986 unreserved ASCII bytes and must not start with a dot (received '\(value)')."
        case let .invalidGovernanceFinalizationId(field, value):
            return "Governance finalization id for \(field) must be exactly 64 lowercase hexadecimal characters (received '\(value)')."
        case .mismatchedGovernanceFinalizationIds:
            return "Governance finalization referendum_id must equal proposal_id."
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

    static func sanitizeGovernanceSelector(_ value: String, field: String) throws -> String {
        guard GovernanceSelectorV1.isValid(value) else {
            throw TransactionInputError.invalidGovernanceSelector(field: field, value: value)
        }
        return value
    }

    static func sanitizeGovernanceFinalizationId(_ value: String, field: String) throws -> String {
        let bytes = Array(value.utf8)
        guard bytes.count == 64,
              bytes.allSatisfy({ byte in
                  (byte >= 48 && byte <= 57) || (byte >= 97 && byte <= 102)
              }) else {
            throw TransactionInputError.invalidGovernanceFinalizationId(
                field: field,
                value: value
            )
        }
        return value
    }

    private static func sanitizeChainId(_ chainId: String) throws -> String {
        let checked = try requireExactNonEmpty(
            chainId,
            empty: .emptyChainId,
            invalid: { .invalidChainId($0) }
        )
        let bytes = Array(checked.utf8)
        guard bytes.count <= 128,
              let first = bytes.first,
              let last = bytes.last,
              isAsciiAlphanumeric(first),
              isAsciiAlphanumeric(last),
              bytes.allSatisfy(isChainIdByte) else {
            throw TransactionInputError.invalidChainId(checked)
        }
        return checked
    }

    private static func isAsciiAlphanumeric(_ byte: UInt8) -> Bool {
        (byte >= 48 && byte <= 57) ||
            (byte >= 65 && byte <= 90) ||
            (byte >= 97 && byte <= 122)
    }

    private static func isChainIdByte(_ byte: UInt8) -> Bool {
        isAsciiAlphanumeric(byte) || byte == 46 || byte == 95 || byte == 58 || byte == 45
    }

    static func sanitizeAccountId(_ accountId: String, field: String) throws -> String {
        let checked = try requireExactNonEmpty(
            accountId,
            empty: .emptyAccountId(field: field),
            invalid: { .malformedAccountId(field: field, value: $0) }
        )
        if checked.rangeOfCharacter(from: .whitespacesAndNewlines) != nil {
            throw TransactionInputError.malformedAccountId(field: field, value: checked)
        }
        if checked.contains("@") || checked.contains("#") || checked.contains("$") {
            throw TransactionInputError.malformedAccountId(field: field, value: checked)
        }
        do {
            let prefix = try AccountAddress.inspectI105NetworkPrefix(checked).chainDiscriminant
            let address = try AccountAddress.parseEncodedSwiftOnly(
                checked,
                expectedPrefix: prefix
            )
            let canonical = try address.toI105(networkPrefix: prefix)
            guard canonical.utf8.elementsEqual(checked.utf8) else {
                throw TransactionInputError.malformedAccountId(field: field, value: checked)
            }
            return canonical
        } catch {
            throw TransactionInputError.malformedAccountId(field: field, value: checked)
        }
    }

    static func sanitizeRwaId(_ rwaId: String, field: String) throws -> String {
        let checked = try requireExactNonEmpty(
            rwaId,
            empty: .emptyRwaId(field: field),
            invalid: { .malformedRwaId(field: field, value: $0) }
        )
        if checked.rangeOfCharacter(from: .whitespacesAndNewlines) != nil {
            throw TransactionInputError.malformedRwaId(field: field, value: checked)
        }
        let parts = checked.split(separator: "$", omittingEmptySubsequences: false)
        guard parts.count == 2 else {
            throw TransactionInputError.malformedRwaId(field: field, value: checked)
        }
        let hashPart = String(parts[0])
        let domainPart = String(parts[1])
        let hexScalars = CharacterSet(charactersIn: "0123456789abcdefABCDEF")
        guard hashPart.count == 64, hashPart.unicodeScalars.allSatisfy({ hexScalars.contains($0) }) else {
            throw TransactionInputError.malformedRwaId(field: field, value: checked)
        }
        let sanitizedDomain = try sanitizeDomainId(domainPart, field: field)
        return "\(hashPart.lowercased())$\(sanitizedDomain)"
    }

    private static func sanitizeAssetDefinitionId(_ assetDefinitionId: String) throws -> String {
        let checked = try requireExactNonEmpty(
            assetDefinitionId,
            empty: .emptyAssetDefinitionId,
            invalid: { .malformedAssetDefinitionId($0) }
        )
        if checked.rangeOfCharacter(from: .whitespacesAndNewlines) != nil {
            throw TransactionInputError.malformedAssetDefinitionId(checked)
        }
        let definitionLiteral: String
        let scopeSuffix: String
        if let (definition, scope) = parseAssetBalanceScopeSuffix(checked) {
            definitionLiteral = definition
            scopeSuffix = scope
        } else {
            definitionLiteral = checked
            scopeSuffix = ""
        }
        guard AssetDefinitionAddress.looksCanonical(definitionLiteral) else {
            throw TransactionInputError.malformedAssetDefinitionId(checked)
        }
        if AssetDefinitionAddress.decode(definitionLiteral) == nil {
            throw TransactionInputError.malformedAssetDefinitionId(checked)
        }
        return definitionLiteral + scopeSuffix
    }

    private static func parseAssetBalanceScopeSuffix(_ value: String) -> (String, String)? {
        guard value.contains("#") else {
            return nil
        }
        let marker = "#dataspace:"
        guard let markerRange = value.range(of: marker) else {
            return ("", "")
        }
        let definition = String(value[..<markerRange.lowerBound])
        let rawDataspaceId = String(value[markerRange.upperBound...])
        guard !definition.isEmpty,
              !rawDataspaceId.contains("#"),
              isCanonicalUnsignedDecimal(rawDataspaceId),
              let dataspaceId = UInt64(rawDataspaceId) else {
            return ("", "")
        }
        return (definition, "\(marker)\(dataspaceId)")
    }

    private static func isCanonicalUnsignedDecimal(_ value: String) -> Bool {
        guard !value.isEmpty,
              value == "0" || !value.hasPrefix("0") else {
            return false
        }
        return value.unicodeScalars.allSatisfy { scalar in
            scalar.value >= 48 && scalar.value <= 57
        }
    }

    static func sanitizeDomainId(_ domainId: String, field: String) throws -> String {
        let checked = try requireExactNonEmpty(
            domainId,
            empty: .emptyDomainId(field: field),
            invalid: { .malformedDomainId(field: field, value: $0) }
        )
        if checked.rangeOfCharacter(from: .whitespacesAndNewlines) != nil
            || checked.contains("@")
            || checked.contains("#")
            || checked.contains("$")
        {
            throw TransactionInputError.malformedDomainId(field: field, value: checked)
        }
        let parts = checked.split(separator: ".", omittingEmptySubsequences: false)
        guard parts.count == 2,
              !parts[0].isEmpty,
              !parts[1].isEmpty
        else {
            throw TransactionInputError.malformedDomainId(field: field, value: checked)
        }
        do {
            let name = try AccountAddress.canonicalizeDomainLabel(String(parts[0]))
            let dataspace = try AccountAddress.canonicalizeDomainLabel(String(parts[1]))
            return "\(name).\(dataspace)"
        } catch {
            throw TransactionInputError.malformedDomainId(field: field, value: checked)
        }
    }

    static func sanitizeLabel(_ label: String, field: String) throws -> String {
        let checked = try requireExactNonEmpty(
            label,
            empty: .emptyLabel(field: field),
            invalid: { .malformedLabel(field: field, value: $0) }
        )
        if checked.count > 32 || checked != checked.lowercased() {
            throw TransactionInputError.malformedLabel(field: field, value: checked)
        }
        let allowed = CharacterSet(charactersIn: "abcdefghijklmnopqrstuvwxyz0123456789_-")
        if checked.unicodeScalars.contains(where: { !allowed.contains($0) }) {
            throw TransactionInputError.malformedLabel(field: field, value: checked)
        }
        return checked
    }

    private static func sanitizeAssetId(_ assetId: String) throws -> String {
        let checked = try requireExactNonEmpty(
            assetId,
            empty: .emptyAssetId,
            invalid: { .malformedAssetId($0) }
        )
        do {
            return try sanitizeAssetDefinitionId(checked)
        } catch {
            throw TransactionInputError.malformedAssetId(checked)
        }
    }

    private static func requireExactNonEmpty(_ value: String,
                                             empty: TransactionInputError,
                                             invalid: (String) -> TransactionInputError) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw empty
        }
        guard trimmed == value else {
            throw invalid(value)
        }
        return value
    }

    static func sanitizeMetadataTarget(_ target: MetadataTarget) throws -> MetadataTarget {
        switch target {
        case let .domain(domainId):
            let sanitized = try sanitizeDomainId(domainId, field: "target")
            return .domain(sanitized)
        case let .account(accountId):
            let sanitized = try sanitizeAccountId(accountId, field: "target")
            return .account(sanitized)
        case let .rwa(rwaId):
            let sanitized = try sanitizeRwaId(rwaId, field: "target")
            return .rwa(sanitized)
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
    case invalidClaimIdentifierReceipt(String)
    case invalidInput(String)
    case invalidNativeSignedTransaction(String)

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
        case let .invalidClaimIdentifierReceipt(reason):
            return "ClaimIdentifier receipt is invalid: \(reason)"
        case let .invalidInput(reason):
            return "Transaction input is invalid: \(reason)"
        case let .invalidNativeSignedTransaction(reason):
            return "Native signed transaction is invalid: \(reason)"
        }
    }
}

private struct NativeClaimIdentifierExecutionEnvelope: Encodable, Sendable {
    let programId: String
    let programDigest: String
    let backend: String
    let verificationMode: String
    let inputCiphertextHash: String
    let outputCiphertextHash: String
    let parameterDigest: String
    let evaluationKeyDigest: String
    let outputHash: String
    let associatedDataHash: String
    let executedAtMs: UInt64
    let expiresAtMs: UInt64?

    private enum CodingKeys: String, CodingKey {
        case programId = "program_id"
        case programDigest = "program_digest"
        case backend
        case verificationMode = "verification_mode"
        case inputCiphertextHash = "input_ciphertext_hash"
        case outputCiphertextHash = "output_ciphertext_hash"
        case parameterDigest = "parameter_digest"
        case evaluationKeyDigest = "evaluation_key_digest"
        case outputHash = "output_hash"
        case associatedDataHash = "associated_data_hash"
        case executedAtMs = "executed_at_ms"
        case expiresAtMs = "expires_at_ms"
    }
}

private struct NativeClaimIdentifierPayloadEnvelope: Encodable, Sendable {
    let policyId: String
    let execution: NativeClaimIdentifierExecutionEnvelope
    let opening: ToriiRamLfeOutputOpening
    let opaqueId: String
    let receiptHash: String
    let uaid: String
    let accountId: String

    private enum CodingKeys: String, CodingKey {
        case policyId = "policy_id"
        case execution
        case opening
        case opaqueId = "opaque_id"
        case receiptHash = "receipt_hash"
        case uaid
        case accountId = "account_id"
    }
}

private struct NativeClaimIdentifierAttestationEnvelope: Encodable, Sendable {
    let kind: String
    let algorithm: String?
    let signature: String?
    let proofBackend: String?
    let proofB64: String?

    private enum CodingKeys: String, CodingKey {
        case kind
        case algorithm
        case signature
        case proofBackend = "proof_backend"
        case proofB64 = "proof_b64"
    }
}

private struct NativeClaimIdentifierReceiptEnvelope: Encodable, Sendable {
    let payload: NativeClaimIdentifierPayloadEnvelope
    let attestation: NativeClaimIdentifierAttestationEnvelope

    private enum CodingKeys: String, CodingKey {
        case payload
        case attestation
    }
}

private let signedTransactionWireVersion: UInt8 = 1

private func encodeVersionedSignedTransaction(_ signedTransaction: Data) -> Data {
    var bytes = Data([signedTransactionWireVersion])
    bytes.append(signedTransaction)
    return bytes
}

private func encodeNativeClaimIdentifierReceiptJSON(
    _ receipt: ToriiIdentifierResolutionReceipt
) throws -> Data {
    do {
        _ = try ToriiIdentifierReceiptCanonicalEncoder.canonicalPayloadBytes(for: receipt)
    } catch let ToriiClientError.invalidPayload(message) {
        throw SwiftTransactionEncoderError.invalidClaimIdentifierReceipt(message)
    }
    let execution = receipt.payload.execution

    let payload = NativeClaimIdentifierPayloadEnvelope(
        policyId: receipt.payload.policyId,
        execution: NativeClaimIdentifierExecutionEnvelope(
            programId: execution.programId,
            programDigest: execution.programDigest,
            backend: execution.backend,
            verificationMode: execution.verificationMode,
            inputCiphertextHash: execution.inputCiphertextHash,
            outputCiphertextHash: execution.outputCiphertextHash,
            parameterDigest: execution.parameterDigest,
            evaluationKeyDigest: execution.evaluationKeyDigest,
            outputHash: execution.outputHash,
            associatedDataHash: execution.associatedDataHash,
            executedAtMs: execution.executedAtMs,
            expiresAtMs: execution.expiresAtMs
        ),
        opening: receipt.payload.opening,
        opaqueId: receipt.payload.opaqueId,
        receiptHash: receipt.payload.receiptHash,
        uaid: receipt.payload.uaid,
        accountId: receipt.payload.accountId
    )
    let attestation = NativeClaimIdentifierAttestationEnvelope(
        kind: receipt.attestation.kind,
        algorithm: receipt.attestation.algorithm,
        signature: receipt.attestation.signature,
        proofBackend: receipt.attestation.proofBackend,
        proofB64: receipt.attestation.proofB64
    )
    return try JSONEncoder().encode(
        NativeClaimIdentifierReceiptEnvelope(
            payload: payload,
            attestation: attestation
        )
    )
}

enum SingleInstructionSwiftNoritoEncoder {
    static func encodeExecutableBatch(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        nonce: UInt32?,
        entries: [TransactionBatchEntry],
        feePayment: FeePaymentIntent,
        signingKey: SigningKey
    ) throws -> SignedTransactionEnvelope {
        guard !entries.isEmpty else {
            throw ExecutableBatchInputError.emptyBatch
        }
        let resolvedTtlMs = ttlMs ?? 100_000
        guard resolvedTtlMs != 0 else {
            throw ExecutableBatchInputError.zeroTimeToLive
        }
        guard nonce != 0 else {
            throw ExecutableBatchInputError.zeroNonce
        }
        if entries.contains(where: {
            if case .contractCall = $0 { return true }
            return false
        }), feePayment.gasLimit == nil {
            throw ExecutableBatchInputError.missingGasLimit
        }
        let ids = try TransactionInputValidator.validate(
            chainId: chainId,
            authorityId: authority
        )
        let executable = try encodeBatchExecutable(entries)
        // `ChainId` is a tuple-newtype around its canonical string.
        var chainIdPayload = CompactNoritoWriter()
        chainIdPayload.writeField(CompactNorito.encodeString(ids.chainId))

        var transactionPayload = CompactNoritoWriter()
        transactionPayload.writeField(chainIdPayload.data)
        transactionPayload.writeField(try CanonicalNorito.encodeCompactAccountId(ids.authorityId))
        transactionPayload.writeField(CompactNorito.encodeUInt64(creationTimeMs))
        transactionPayload.writeField(executable)
        transactionPayload.writeField(
            try CompactNorito.encodeOption(resolvedTtlMs, encode: CompactNorito.encodeUInt64)
        )
        transactionPayload.writeField(
            try CompactNorito.encodeOption(nonce, encode: CompactNorito.encodeUInt32)
        )
        transactionPayload.writeField(try feePayment.compactNorito())
        transactionPayload.writeField(encodeEmptyMetadata())
        transactionPayload.writeField(encodeNoneOption())

        let signature = try signingKey.sign(IrohaHash.hash(transactionPayload.data))
        let signed = encodeCompactSignedTransaction(
            signature: signature,
            transactionPayload: transactionPayload.data
        )
        return SignedTransactionEnvelope(
            norito: encodeVersionedSignedTransaction(signed),
            signedTransaction: signed,
            payload: nil,
            transactionHash: IrohaHash.hash(encodeTransactionEntrypoint(transactionPayload.data))
        )
    }

    static func encodeCommitContractDeployment(
        chainId: String, authority: String, creationTimeMs: UInt64, ttlMs: UInt64?,
        expectedDeployNonce: UInt64, contractAddress: String, codeHash: Data,
        contractAlias: String, leaseExpiryMs: UInt64?, expectedPreviousContractAddress: String?,
        feePayment: FeePaymentIntent,
        signingKey: SigningKey
    ) throws -> SignedTransactionEnvelope {
        var payload = CanonicalNoritoWriter()
        payload.writeField(CanonicalNorito.encodeUInt64(expectedDeployNonce))
        payload.writeField(CanonicalNorito.encodeString(contractAddress))
        payload.writeField(CanonicalNorito.encodeConstVec(codeHash))
        payload.writeField(CanonicalNorito.encodeString(contractAlias))
        payload.writeField(try CanonicalNorito.encodeOption(leaseExpiryMs, encode: CanonicalNorito.encodeUInt64))
        payload.writeField(try CanonicalNorito.encodeOption(expectedPreviousContractAddress, encode: CanonicalNorito.encodeString))
        let typeName = "iroha_data_model::isi::smart_contract_code::CommitContractDeployment"
        let framed = noritoEncode(typeName: typeName, payload: payload.data, flags: 0)
        var wire = CanonicalNoritoWriter()
        wire.writeField(CanonicalNorito.encodeString(typeName))
        wire.writeField(CanonicalNorito.encodeBytesVec(framed))
        let transactionPayload = try encodeTransactionPayload(
            chainId: chainId, authority: authority, creationTimeMs: creationTimeMs,
            ttlMs: ttlMs, feePayment: feePayment, instructionPayload: wire.data
        )
        let signature = try signingKey.sign(IrohaHash.hash(transactionPayload))
        let signed = encodeSignedTransaction(signature: signature, transactionPayload: transactionPayload)
        return SignedTransactionEnvelope(
            norito: encodeVersionedSignedTransaction(signed), signedTransaction: signed,
            payload: nil,
            transactionHash: IrohaHash.hash(encodeTransactionEntrypoint(transactionPayload))
        )
    }

    static func encodeAliasSetupPlan(
        request: AliasSetupPlanRequestV1,
        plan: AliasTransactionPlanV1,
        bodyEncoder: (AliasTransactionPlanBodyV1) throws -> Data,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        feePayment: FeePaymentIntent,
        signingKey: SigningKey,
        decodeAndReencode: (String, Data) throws -> DecodedEnsureAliasFrame
    ) throws -> SignedTransactionEnvelope {
        let canonicalBodyNorito = try bodyEncoder(plan.body)
        guard !canonicalBodyNorito.isEmpty else {
            throw AliasSetupModelError.planValidation(["alias.plan.body_encoding_empty"])
        }
        try AliasPlanVerifier.requireExecutableForRequest(
            request,
            plan: plan,
            canonicalBodyNorito: canonicalBodyNorito,
            decodeAndReencode: decodeAndReencode
        )
        guard creationTimeMs <= plan.body.validUntilMs else {
            throw AliasSetupModelError.planValidation(["alias.plan.expired"])
        }
        let instructionPayloads = plan.body.instructions.map { instruction -> Data in
            var wire = CanonicalNoritoWriter()
            wire.writeField(CanonicalNorito.encodeString(instruction.wireId))
            wire.writeField(CanonicalNorito.encodeBytesVec(instruction.framedPayload))
            return wire.data
        }
        let transactionPayload = try encodeTransactionPayload(
            chainId: plan.body.chainId,
            authority: plan.body.authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            feePayment: feePayment,
            instructionPayloads: instructionPayloads
        )
        let signature = try signingKey.sign(IrohaHash.hash(transactionPayload))
        let signedTransaction = encodeSignedTransaction(
            signature: signature,
            transactionPayload: transactionPayload
        )
        return SignedTransactionEnvelope(
            norito: encodeVersionedSignedTransaction(signedTransaction),
            signedTransaction: signedTransaction,
            payload: nil,
            transactionHash: IrohaHash.hash(encodeTransactionEntrypoint(transactionPayload))
        )
    }

    static func encodeAliasLifecyclePlan(
        request: AliasLifecyclePlanRequestV1,
        plan: AliasLifecycleTransactionPlanV1,
        bodyEncoder: (AliasLifecycleTransactionPlanBodyV1) throws -> Data,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        feePayment: FeePaymentIntent,
        signingKey: SigningKey,
        decodeAndReencode: (String, Data) throws -> DecodedAliasLifecycleFrame
    ) throws -> SignedTransactionEnvelope? {
        let canonicalBodyNorito = try bodyEncoder(plan.body)
        guard !canonicalBodyNorito.isEmpty else {
            throw AliasSetupModelError.planValidation(["alias.lifecycle.plan.body_encoding_empty"])
        }
        try AliasPlanVerifier.requireExecutableForRequest(
            request,
            plan: plan,
            canonicalBodyNorito: canonicalBodyNorito,
            decodeAndReencode: decodeAndReencode
        )
        guard creationTimeMs <= plan.body.validUntilMs else {
            throw AliasSetupModelError.planValidation(["alias.lifecycle.plan.expired"])
        }
        guard let instruction = plan.body.instruction else { return nil }
        var wire = CanonicalNoritoWriter()
        wire.writeField(CanonicalNorito.encodeString(instruction.wireId))
        wire.writeField(CanonicalNorito.encodeBytesVec(instruction.framedPayload))
        let transactionPayload = try encodeTransactionPayload(
            chainId: plan.body.chainId,
            authority: plan.body.authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            feePayment: feePayment,
            instructionPayload: wire.data
        )
        let signature = try signingKey.sign(IrohaHash.hash(transactionPayload))
        let signedTransaction = encodeSignedTransaction(
            signature: signature,
            transactionPayload: transactionPayload
        )
        return SignedTransactionEnvelope(
            norito: encodeVersionedSignedTransaction(signedTransaction),
            signedTransaction: signedTransaction,
            payload: nil,
            transactionHash: IrohaHash.hash(encodeTransactionEntrypoint(transactionPayload))
        )
    }

    private static func encodeTransactionPayload(chainId: String,
                                                 authority: String,
                                                 creationTimeMs: UInt64,
                                                 ttlMs: UInt64?,
                                                 feePayment: FeePaymentIntent,
                                                 instructionPayload: Data) throws -> Data {
        try encodeTransactionPayload(
            chainId: chainId,
            authority: authority,
            creationTimeMs: creationTimeMs,
            ttlMs: ttlMs,
            feePayment: feePayment,
            instructionPayloads: [instructionPayload]
        )
    }

    private static func encodeTransactionPayload(chainId: String,
                                                 authority: String,
                                                 creationTimeMs: UInt64,
                                                 ttlMs: UInt64?,
                                                 feePayment: FeePaymentIntent,
                                                 instructionPayloads: [Data]) throws -> Data {
        let resolvedTtlMs = ttlMs ?? 100_000
        guard resolvedTtlMs != 0 else {
            throw ExecutableBatchInputError.zeroTimeToLive
        }
        let executablePayload = encodeExecutable(instructionPayloads: instructionPayloads)
        var transactionPayload = CanonicalNoritoWriter()
        transactionPayload.writeField(CanonicalNorito.encodeString(chainId))
        transactionPayload.writeField(CanonicalNorito.encodeString(authority))
        transactionPayload.writeField(CanonicalNorito.encodeUInt64(creationTimeMs))
        transactionPayload.writeField(executablePayload)
        transactionPayload.writeField(try CanonicalNorito.encodeOption(resolvedTtlMs, encode: CanonicalNorito.encodeUInt64))
        transactionPayload.writeField(encodeNoneOption())
        transactionPayload.writeField(try feePayment.canonicalNorito())
        transactionPayload.writeField(encodeEmptyMetadata())
        transactionPayload.writeField(encodeNoneOption())
        return transactionPayload.data
    }

    private static func encodeExecutable(instructionPayloads: [Data]) -> Data {
        var instructions = CanonicalNoritoWriter()
        instructions.writeLength(UInt64(instructionPayloads.count))
        for instructionPayload in instructionPayloads {
            instructions.writeField(instructionPayload)
        }

        var executable = CanonicalNoritoWriter()
        executable.writeUInt32LE(0)
        executable.writeField(instructions.data)
        return executable.data
    }

    private static func encodeBatchExecutable(_ entries: [TransactionBatchEntry]) throws -> Data {
        var sequence = CompactNoritoWriter()
        sequence.writeUInt64LE(UInt64(entries.count))
        for entry in entries {
            var item = CompactNoritoWriter()
            switch entry {
            case let .instruction(frame):
                item.writeUInt32LE(0)
                item.writeField(frame.compactInstructionBoxPayload())
            case let .contractCall(invocation):
                item.writeUInt32LE(1)
                var call = CompactNoritoWriter()
                call.writeField(CompactNorito.encodeString(invocation.contractAddress))
                call.writeField(invocation.expectedCodeHash)
                call.writeField(CompactNorito.encodeString(invocation.entrypoint))
                call.writeField(
                    try CompactNorito.encodeOption(
                        invocation.arguments,
                        encode: CompactNorito.encodeBytesVec
                    )
                )
                item.writeField(call.data)
            }
            sequence.writeField(item.data)
        }

        var executable = CompactNoritoWriter()
        executable.writeUInt32LE(4)
        executable.writeField(sequence.data)
        return executable.data
    }

    private static func encodeCompactSignedTransaction(
        signature: Data,
        transactionPayload: Data
    ) -> Data {
        // `SignedTransaction.signature` contains the `TransactionSignature`
        // tuple-newtype. Its sole field wraps the transparent
        // `SignatureOf<TransactionPayload>`/`Signature` byte-vector payload.
        var transactionSignature = CompactNoritoWriter()
        transactionSignature.writeField(CompactNorito.encodeConstVec(signature))

        var signedTransaction = CompactNoritoWriter()
        signedTransaction.writeField(transactionSignature.data)
        signedTransaction.writeField(transactionPayload)
        signedTransaction.writeField(encodeNoneOption())
        return signedTransaction.data
    }

    private static func encodeSignedTransaction(signature: Data,
                                                transactionPayload: Data) -> Data {
        var signedTransaction = CanonicalNoritoWriter()
        signedTransaction.writeField(CanonicalNorito.encodeConstVec(signature))
        signedTransaction.writeField(transactionPayload)
        signedTransaction.writeField(encodeNoneOption())
        return signedTransaction.data
    }

    private static func encodeTransactionEntrypoint(_ transactionPayload: Data) -> Data {
        var entrypoint = CompactNoritoWriter()
        entrypoint.writeUInt32LE(0)
        entrypoint.writeField(transactionPayload)
        return entrypoint.data
    }

    private static func encodeEmptyMetadata() -> Data {
        var metadata = CanonicalNoritoWriter()
        metadata.writeLength(0)
        return metadata.data
    }

    private static func encodeNoneOption() -> Data {
        Data([0])
    }
}

struct SwiftTransactionEncoder {
    static func bridgeOrThrow(_ body: () throws -> NativeSignedTransaction?) throws -> NativeSignedTransaction {
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

    static func wrap(native: NativeSignedTransaction) throws -> SignedTransactionEnvelope {
        guard native.signedBytes.first == signedTransactionWireVersion else {
            throw SwiftTransactionEncoderError.invalidNativeSignedTransaction(
                "missing version byte \(signedTransactionWireVersion)"
            )
        }
        let signedTransaction = Data(native.signedBytes.dropFirst())
        guard !signedTransaction.isEmpty else {
            throw SwiftTransactionEncoderError.invalidNativeSignedTransaction("empty signed transaction payload")
        }
        return SignedTransactionEnvelope(norito: native.signedBytes,
                                         signedTransaction: signedTransaction,
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
        let quantity = try KotodamaNumericV1Codec.decodeQuantityJSON(transfer.quantity).canonicalString
        let feePaymentJSON = try transfer.feePayment.canonicalJSONData()
        let privateKey = try privateKeyBytes(from: signingKey)
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeTransfer(chainId: ids.chainId,
                                                         authority: ids.authorityId,
                                                         creationTimeMs: creationTimeMs,
                                                         ttlMs: transfer.ttlMs,
                                                         nonce: transfer.nonce,
                                                         assetDefinitionId: assetDefinitionId,
                                                         quantity: quantity,
                                                         destination: destination,
                                                         feePaymentJSON: feePaymentJSON,
                                                         privateKey: privateKey,
                                                         algorithm: signingKey.algorithm)
        }
        return try wrap(native: native)
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
        let quantity = try KotodamaNumericV1Codec.decodeQuantityJSON(request.quantity).canonicalString
        let privateKey = try privateKeyBytes(from: signingKey)
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeMint(chainId: ids.chainId,
                                                     authority: ids.authorityId,
                                                     creationTimeMs: creationTimeMs,
                                                     ttlMs: request.ttlMs,
                                                     nonce: request.nonce,
                                                     assetDefinitionId: assetDefinitionId,
                                                     quantity: quantity,
                                                     destination: destination,
                                                     feePaymentJSON: try request.feePayment.canonicalJSONData(),
                                                     privateKey: privateKey,
                                                     algorithm: signingKey.algorithm)
        }
        return try wrap(native: native)
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
        let quantity = try KotodamaNumericV1Codec.decodeQuantityJSON(request.quantity).canonicalString
        let privateKey = try privateKeyBytes(from: signingKey)
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeBurn(chainId: ids.chainId,
                                                     authority: ids.authorityId,
                                                     creationTimeMs: creationTimeMs,
                                                     ttlMs: request.ttlMs,
                                                     nonce: request.nonce,
                                                     assetDefinitionId: assetDefinitionId,
                                                     quantity: quantity,
                                                     destination: destination,
                                                     feePaymentJSON: try request.feePayment.canonicalJSONData(),
                                                     privateKey: privateKey,
                                                     algorithm: signingKey.algorithm)
        }
        return try wrap(native: native)
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
                feePaymentJSON: try request.feePayment.canonicalJSONData(),
                privateKey: privateKey,
                algorithm: signingKey.algorithm
            )
        }
        return try wrap(native: native)
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
                                                                 feePaymentJSON: try request.feePayment.canonicalJSONData(),
                                                                 privateKey: privateKey,
                                                                 algorithm: signingKey.algorithm)
        }
        return try wrap(native: native)
    }

    static func encodeClaimIdentifier(request: ClaimIdentifierRequest,
                                      keypair: Keypair,
                                      creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let signingKey = try SigningKey.ed25519(privateKey: keypair.privateKeyBytes)
        return try encodeClaimIdentifier(request: request,
                                         signingKey: signingKey,
                                         creationTimeMs: creationTimeMs)
    }

    static func encodeClaimIdentifier(request: ClaimIdentifierRequest,
                                      signingKey: SigningKey,
                                      creationTimeMs: UInt64) throws -> SignedTransactionEnvelope {
        let receiptAccountId = request.receipt.payload.accountId
        let ids = try TransactionInputValidator.validate(chainId: request.chainId,
                                                         authorityId: request.authority,
                                                         accountIds: [
                                                            TransactionInputValidator.NamedAccountId(field: "account", value: request.accountId),
                                                            TransactionInputValidator.NamedAccountId(field: "receipt_account", value: receiptAccountId)
                                                         ])
        let canonicalAccountId = ids.accountIds["account"] ?? request.accountId
        guard canonicalAccountId == ids.accountIds["receipt_account"] else {
            throw SwiftTransactionEncoderError.invalidClaimIdentifierReceipt(
                "accountId must match receipt.payload.account_id."
            )
        }

        let privateKey = try privateKeyBytes(from: signingKey)
        let receiptJSON = try encodeNativeClaimIdentifierReceiptJSON(request.receipt)
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeClaimIdentifier(
                chainId: ids.chainId,
                authority: ids.authorityId,
                creationTimeMs: creationTimeMs,
                ttlMs: request.ttlMs,
                accountId: canonicalAccountId,
                receiptJSON: receiptJSON,
                feePaymentJSON: try request.feePayment.canonicalJSONData(),
                privateKey: privateKey,
                algorithm: signingKey.algorithm
            )
        }
        return try wrap(native: native)
    }

    static func encodeCommitContractDeployment(
        request: CommitContractDeploymentRequest,
        signingKey: SigningKey,
        creationTimeMs: UInt64
    ) throws -> SignedTransactionEnvelope {
        let ids = try TransactionInputValidator.validate(
            chainId: request.chainId, authorityId: request.authority
        )
        guard request.contractAddress == request.contractAddress.trimmingCharacters(in: .whitespacesAndNewlines),
              !request.contractAddress.isEmpty,
              request.contractAlias == request.contractAlias.trimmingCharacters(in: .whitespacesAndNewlines),
              !request.contractAlias.isEmpty else {
            throw SwiftTransactionEncoderError.invalidInput("contract address and alias must be exact non-empty strings")
        }
        guard request.codeHashHex.count == 64,
              let codeHash = Data(hexString: request.codeHashHex) else {
            throw SwiftTransactionEncoderError.invalidInput("codeHashHex must contain exactly 64 hexadecimal characters")
        }
        return try SingleInstructionSwiftNoritoEncoder.encodeCommitContractDeployment(
            chainId: ids.chainId, authority: ids.authorityId, creationTimeMs: creationTimeMs,
            ttlMs: request.ttlMs, expectedDeployNonce: request.expectedDeployNonce,
            contractAddress: request.contractAddress, codeHash: codeHash,
            contractAlias: request.contractAlias, leaseExpiryMs: request.leaseExpiryMs,
            expectedPreviousContractAddress: request.expectedPreviousContractAddress,
            feePayment: request.feePayment,
            signingKey: signingKey
        )
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
                feePaymentJSON: try request.feePayment.canonicalJSONData(),
                privateKey: privateKey,
                algorithm: signingKey.algorithm
            )
        }
        return try wrap(native: native)
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
                feePaymentJSON: try request.feePayment.canonicalJSONData(),
                privateKey: privateKey,
                algorithm: signingKey.algorithm
            )
        }
        return try wrap(native: native)
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
                contractAddress: request.contractAddress,
                codeHashHex: request.codeHashHex,
                abiHashHex: request.abiHashHex,
                abiVersion: request.abiVersion,
                window: windowTuple,
                modeCode: request.mode?.rawValue,
                feePaymentJSON: try request.feePayment.canonicalJSONData(),
                privateKey: privateKey,
                algorithm: signingKey.algorithm
            )
        }
        return try wrap(native: native)
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
        let referendumId = try TransactionInputValidator.sanitizeGovernanceSelector(
            request.referendumId,
            field: "referendum_id"
        )
        let amount = try KotodamaNumericV1Codec
            .decodeQuantityJSON(request.amount)
            .canonicalString
        let privateKey = try privateKeyBytes(from: signingKey)
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeGovernanceCastPlainBallot(
                chainId: ids.chainId,
                authority: ids.authorityId,
                creationTimeMs: creationTimeMs,
                ttlMs: request.ttlMs,
                referendumId: referendumId,
                owner: owner,
                amount: amount,
                durationBlocks: request.durationBlocks,
                direction: request.direction.rawValue,
                feePaymentJSON: try request.feePayment.canonicalJSONData(),
                privateKey: privateKey,
                algorithm: signingKey.algorithm
            )
        }
        return try wrap(native: native)
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
        let electionId = try TransactionInputValidator.sanitizeGovernanceSelector(
            request.electionId,
            field: "election_id"
        )
        let privateKey = try privateKeyBytes(from: signingKey)
        let publicInputs = try normalizeZkBallotPublicInputs(request.publicInputs)
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeGovernanceCastZkBallot(
                chainId: ids.chainId,
                authority: ids.authorityId,
                creationTimeMs: creationTimeMs,
                ttlMs: request.ttlMs,
                electionId: electionId,
                proofB64: request.proofB64,
                publicInputs: publicInputs,
                feePaymentJSON: try request.feePayment.canonicalJSONData(),
                privateKey: privateKey,
                algorithm: signingKey.algorithm
            )
        }
        return try wrap(native: native)
    }

    static func normalizeZkBallotPublicInputs(
        _ inputs: GovernanceZkBallotPublicInputs
    ) throws -> Data {
        let encoder = JSONEncoder()
        if #available(iOS 11.0, macOS 10.13, *) {
            encoder.outputFormatting.insert(.sortedKeys)
        }
        let encoded: Data
        do {
            encoded = try encoder.encode(inputs)
        } catch let ToriiClientError.invalidPayload(reason) {
            throw TransactionInputError.invalidZkBallotPublicInputs(reason)
        } catch {
            throw TransactionInputError.invalidZkBallotPublicInputs(
                "public inputs could not be encoded canonically"
            )
        }
        try rejectGovernancePrivateKeyAliases(inJSONData: encoded)
        return encoded
    }

    private static let governancePrivateKeyAliases: Set<String> = [
        "private_key", "privateKey", "private_key_hex", "privateKeyHex",
        "private_key_bytes", "privateKeyBytes", "private_key_seed", "privateKeySeed",
        "private_key_multihash", "privateKeyMultihash", "private_key_algorithm",
        "privateKeyAlgorithm",
    ]

    static func rejectGovernancePrivateKeyAliases(inJSONData data: Data) throws {
        let value: Any
        do {
            value = try JSONSerialization.jsonObject(with: data)
        } catch {
            throw TransactionInputError.invalidZkBallotPublicInputs(
                "public inputs must be valid JSON"
            )
        }
        try rejectGovernancePrivateKeyAliases(in: value, path: "public_inputs")
    }

    private static func rejectGovernancePrivateKeyAliases(in value: Any,
                                                           path: String) throws {
        if let object = value as? [String: Any] {
            for key in object.keys.sorted() {
                if governancePrivateKeyAliases.contains(key) {
                    throw TransactionInputError.invalidZkBallotPublicInputs(
                        "\(path) does not accept private-key field \(key)"
                    )
                }
                if let nested = object[key] {
                    try rejectGovernancePrivateKeyAliases(
                        in: nested,
                        path: "\(path).\(key)"
                    )
                }
            }
        } else if let array = value as? [Any] {
            for (index, nested) in array.enumerated() {
                try rejectGovernancePrivateKeyAliases(
                    in: nested,
                    path: "\(path)[\(index)]"
                )
            }
        }
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
                feePaymentJSON: try request.feePayment.canonicalJSONData(),
                privateKey: privateKey,
                algorithm: signingKey.algorithm
            )
        }
        return try wrap(native: native)
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
        let referendumId = try TransactionInputValidator.sanitizeGovernanceFinalizationId(
            request.referendumId,
            field: "referendum_id"
        )
        let proposalIdHex = try TransactionInputValidator.sanitizeGovernanceFinalizationId(
            request.proposalIdHex,
            field: "proposal_id_hex"
        )
        guard referendumId == proposalIdHex else {
            throw TransactionInputError.mismatchedGovernanceFinalizationIds
        }
        let privateKey = try privateKeyBytes(from: signingKey)
        let native = try bridgeOrThrow {
            try NoritoNativeBridge.shared.encodeGovernanceFinalizeReferendum(
                chainId: ids.chainId,
                authority: ids.authorityId,
                creationTimeMs: creationTimeMs,
                ttlMs: request.ttlMs,
                referendumId: referendumId,
                proposalIdHex: proposalIdHex,
                feePaymentJSON: try request.feePayment.canonicalJSONData(),
                privateKey: privateKey,
                algorithm: signingKey.algorithm
            )
        }
        return try wrap(native: native)
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
                membersJson: membersJson,
                feePaymentJSON: try request.feePayment.canonicalJSONData(),
                privateKey: privateKey,
                algorithm: signingKey.algorithm
            )
        }
        return try wrap(native: native)
    }

    static func privateKeyBytes(from signingKey: SigningKey) throws -> Data {
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
