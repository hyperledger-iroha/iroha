import Foundation

public enum OfflineNoteConstants {
    public static let keyCertificatePayloadDomain = "iroha:offline-note:key-certificate-payload"
    public static let issuedClaimDomain = "iroha:offline-note:issued-claim"
    public static let redeemPublicInputsDomain = "iroha:offline-note:redeem-public-inputs"
    public static let auditPublicInputsDomain = "iroha:offline-note:audit-public-inputs"
    public static let noteCommitmentDomain = "iroha:offline-note:note-commitment"
    public static let inputNullifierDomain = "iroha:offline-note:input-nullifier"
    public static let paymentTokenIdDomain = "iroha:offline-note:payment-token-id"
    public static let recursiveBackend = "halo2/ipa"
    public static let recursiveVerifierName = "offline-note-recursive"
    public static let recursivePublicInputsSchema = #"{"schema":"offline_note_recursive","public_inputs":["public_inputs_hash_limb0","public_inputs_hash_limb1","public_inputs_hash_limb2","public_inputs_hash_limb3","proof_mode","input_count","output_count","input_amount_sum","output_amount_sum","input_nullifier_sum_limb0","output_commitment_sum_limb0","key_certificate_payload_hash_limb0","source_or_token_limb0","input_claim_hash_sum_limb0","output_claim_hash_sum_limb0","reserved_zero"]}"#
    public static let keyCertificateVersion: UInt16 = 1

    public static var recursivePublicInputsSchemaHash: Data {
        IrohaHash.hash(Data(recursivePublicInputsSchema.utf8))
    }
}

public enum OfflineNoteError: Error, LocalizedError, Equatable {
    case invalidHashLength(field: String, expected: Int, actual: Int)
    case invalidHash(field: String)
    case invalidCertificateVersion(UInt16)
    case certificateMustBeOneUse
    case invalidNotePublicKeyLength(expected: Int, actual: Int)
    case invalidIssuerSignatureLength(expected: Int, actual: Int)
    case emptyProofBytes
    case emptyProofBackend
    case emptyInputNullifiers
    case emptyInputClaims
    case emptyOutputCommitments
    case emptyOutputClaims
    case invalidRandomBytesLength(field: String, expected: Int, actual: Int)
    case unsupportedDerivationDomain(field: String, expected: String, actual: String)
    case auditInputCountMismatch(nullifiers: Int, claims: Int)
    case auditOutputCountMismatch(commitments: Int, claims: Int)
    case auditOutputClaimOrderMismatch(index: Int)
    case auditOutputClaimNotCommitted(String)
    case unsupportedRecursiveVerifierKey(expectedBackend: String, expectedName: String, actualBackend: String, actualName: String)
    case unsupportedRecursiveProofBackend(expected: String, actual: String)
    case proofPublicInputsHashMismatch(expected: String, actual: String)

    public var errorDescription: String? {
        switch self {
        case let .invalidHashLength(field, expected, actual):
            return "\(field) must be exactly \(expected) bytes (found \(actual))."
        case let .invalidHash(field):
            return "\(field) must be a canonical Iroha hash."
        case let .invalidCertificateVersion(version):
            return "Offline key certificate version must be \(OfflineNoteConstants.keyCertificateVersion) (found \(version))."
        case .certificateMustBeOneUse:
            return "Offline key certificate must be marked one-use."
        case let .invalidNotePublicKeyLength(expected, actual):
            return "Offline note public key must be \(expected) bytes (found \(actual))."
        case let .invalidIssuerSignatureLength(expected, actual):
            return "Offline issuer signature must be \(expected) bytes (found \(actual))."
        case .emptyProofBytes:
            return "Offline proof bytes must not be empty."
        case .emptyProofBackend:
            return "Offline proof backend must not be empty."
        case .emptyInputNullifiers:
            return "Offline input nullifiers must not be empty."
        case .emptyInputClaims:
            return "Offline audit input claims must not be empty."
        case .emptyOutputCommitments:
            return "Offline audit output commitments must not be empty."
        case .emptyOutputClaims:
            return "Offline audit output claims must not be empty."
        case let .invalidRandomBytesLength(field, expected, actual):
            return "\(field) must be exactly \(expected) bytes (found \(actual))."
        case let .unsupportedDerivationDomain(field, expected, actual):
            return "\(field) must be \(expected), got \(actual)."
        case let .auditInputCountMismatch(nullifiers, claims):
            return "Offline audit input nullifier count \(nullifiers) must match input claim count \(claims)."
        case let .auditOutputCountMismatch(commitments, claims):
            return "Offline audit output commitment count \(commitments) must match output claim count \(claims)."
        case let .auditOutputClaimOrderMismatch(index):
            return "Offline audit output claim at index \(index) must match the output commitment at the same index."
        case let .auditOutputClaimNotCommitted(commitment):
            return "Offline audit output claim \(commitment) is not listed in output commitments."
        case let .unsupportedRecursiveVerifierKey(expectedBackend, expectedName, actualBackend, actualName):
            return "Offline recursive proof verifier key must be \(expectedBackend):\(expectedName), got \(actualBackend):\(actualName)."
        case let .unsupportedRecursiveProofBackend(expected, actual):
            return "Offline recursive proof backend must be \(expected), got \(actual)."
        case let .proofPublicInputsHashMismatch(expected, actual):
            return "Offline recursive proof public input hash mismatch: expected \(expected), got \(actual)."
        }
    }
}

public struct OfflineNoteProofBox: Equatable, Sendable {
    public let backend: String
    public let bytes: Data

    public init(backend: String, bytes: Data) throws {
        let trimmedBackend = backend.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedBackend.isEmpty else {
            throw OfflineNoteError.emptyProofBackend
        }
        guard !bytes.isEmpty else {
            throw OfflineNoteError.emptyProofBytes
        }
        self.backend = trimmedBackend
        self.bytes = bytes
    }
}

public struct OfflineNoteRecursiveProof: Equatable, Sendable {
    public let verifierKeyId: VerifyingKeyIdReference
    public let publicInputsHash: Data
    public let proof: OfflineNoteProofBox

    public init(verifierKeyId: VerifyingKeyIdReference,
                publicInputsHash: Data,
                proof: OfflineNoteProofBox) throws {
        try OfflineNoteValidation.validateHash(publicInputsHash, field: "public_inputs_hash")
        self.verifierKeyId = verifierKeyId
        self.publicInputsHash = publicInputsHash
        self.proof = proof
    }

    public init(verifierBackend: String = OfflineNoteConstants.recursiveBackend,
                verifierName: String = OfflineNoteConstants.recursiveVerifierName,
                publicInputsHash: Data,
                proofBytes: Data,
                proofBackend: String = OfflineNoteConstants.recursiveBackend) throws {
        let verifierKeyId = try VerifyingKeyIdReference(backend: verifierBackend, name: verifierName)
        let proof = try OfflineNoteProofBox(backend: proofBackend, bytes: proofBytes)
        try self.init(verifierKeyId: verifierKeyId, publicInputsHash: publicInputsHash, proof: proof)
    }

    public func validateCanonicalMetadata() throws {
        guard verifierKeyId.backend == OfflineNoteConstants.recursiveBackend,
              verifierKeyId.name == OfflineNoteConstants.recursiveVerifierName else {
            throw OfflineNoteError.unsupportedRecursiveVerifierKey(
                expectedBackend: OfflineNoteConstants.recursiveBackend,
                expectedName: OfflineNoteConstants.recursiveVerifierName,
                actualBackend: verifierKeyId.backend,
                actualName: verifierKeyId.name
            )
        }
        guard proof.backend == OfflineNoteConstants.recursiveBackend else {
            throw OfflineNoteError.unsupportedRecursiveProofBackend(
                expected: OfflineNoteConstants.recursiveBackend,
                actual: proof.backend
            )
        }
    }
}

public struct OfflineNoteKeyCertificatePayload: Equatable, Sendable {
    public let domain: String
    public let version: UInt16
    public let platform: String
    public let keyId: String
    public let deviceId: String
    public let accountId: String
    public let publicKey: Data
    public let assertionScheme: String
    public let assertionKeyAlgorithm: String
    public let assertionPublicKey: Data
    public let assertionUsageCountLimit: UInt32?
    public let oneUse: Bool

    public init(domain: String = OfflineNoteConstants.keyCertificatePayloadDomain,
                version: UInt16,
                platform: String,
                keyId: String,
                deviceId: String,
                accountId: String,
                publicKey: Data,
                assertionScheme: String,
                assertionKeyAlgorithm: String,
                assertionPublicKey: Data,
                assertionUsageCountLimit: UInt32?,
                oneUse: Bool) throws {
        try OfflineNoteValidation.validateCertificateCore(
            version: version,
            accountId: accountId,
            publicKey: publicKey,
            oneUse: oneUse
        )
        self.domain = domain
        self.version = version
        self.platform = platform
        self.keyId = keyId
        self.deviceId = deviceId
        self.accountId = accountId
        self.publicKey = publicKey
        self.assertionScheme = assertionScheme
        self.assertionKeyAlgorithm = assertionKeyAlgorithm
        self.assertionPublicKey = assertionPublicKey
        self.assertionUsageCountLimit = assertionUsageCountLimit
        self.oneUse = oneUse
    }

    public func noritoEncoded() throws -> Data {
        try OfflineNoteEncoding.wrap(
            typeName: OfflineNoteTypeNames.keyCertificatePayload,
            payload: OfflineNoteEncoding.encodeCertificatePayload(self)
        )
    }
}

public struct OfflineNoteKeyCertificate: Equatable, Sendable {
    public let version: UInt16
    public let platform: String
    public let keyId: String
    public let deviceId: String
    public let accountId: String
    public let publicKey: Data
    public let assertionScheme: String
    public let assertionKeyAlgorithm: String
    public let assertionPublicKey: Data
    public let assertionUsageCountLimit: UInt32?
    public let oneUse: Bool
    public let issuerSignature: Data

    public init(version: UInt16 = OfflineNoteConstants.keyCertificateVersion,
                platform: String,
                keyId: String,
                deviceId: String,
                accountId: String,
                publicKey: Data,
                assertionScheme: String,
                assertionKeyAlgorithm: String,
                assertionPublicKey: Data,
                assertionUsageCountLimit: UInt32?,
                oneUse: Bool = true,
                issuerSignature: Data) throws {
        try OfflineNoteValidation.validateCertificateCore(
            version: version,
            accountId: accountId,
            publicKey: publicKey,
            oneUse: oneUse
        )
        guard issuerSignature.count == 64 else {
            throw OfflineNoteError.invalidIssuerSignatureLength(expected: 64, actual: issuerSignature.count)
        }
        self.version = version
        self.platform = platform
        self.keyId = keyId
        self.deviceId = deviceId
        self.accountId = accountId
        self.publicKey = publicKey
        self.assertionScheme = assertionScheme
        self.assertionKeyAlgorithm = assertionKeyAlgorithm
        self.assertionPublicKey = assertionPublicKey
        self.assertionUsageCountLimit = assertionUsageCountLimit
        self.oneUse = oneUse
        self.issuerSignature = issuerSignature
    }

    public func signingPayload() throws -> OfflineNoteKeyCertificatePayload {
        try OfflineNoteKeyCertificatePayload(
            version: version,
            platform: platform,
            keyId: keyId,
            deviceId: deviceId,
            accountId: accountId,
            publicKey: publicKey,
            assertionScheme: assertionScheme,
            assertionKeyAlgorithm: assertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey,
            assertionUsageCountLimit: assertionUsageCountLimit,
            oneUse: oneUse
        )
    }

    public func signingBytes() throws -> Data {
        try signingPayload().noritoEncoded()
    }

    public func payloadHash() throws -> Data {
        IrohaHash.hash(try signingBytes())
    }

    public func noritoEncoded() throws -> Data {
        try OfflineNoteEncoding.wrap(
            typeName: OfflineNoteTypeNames.keyCertificate,
            payload: OfflineNoteEncoding.encodeCertificate(self)
        )
    }
}

public struct OfflineNoteIssuerLoadOrigin: Equatable, Sendable {
    public let operationId: String
    public let lineageId: String
    public let localRevision: UInt64

    public init(operationId: String, lineageId: String, localRevision: UInt64) throws {
        guard !operationId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw OfflineNoritoError.invalidMetadata("operation_id")
        }
        guard !lineageId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw OfflineNoritoError.invalidMetadata("lineage_id")
        }
        self.operationId = operationId
        self.lineageId = lineageId
        self.localRevision = localRevision
    }
}

public struct OfflineNoteP2pOutputOrigin: Equatable, Sendable {
    public let paymentRequestId: String
    public let outputIndex: UInt32

    public init(paymentRequestId: String, outputIndex: UInt32) throws {
        guard !paymentRequestId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw OfflineNoritoError.invalidMetadata("payment_request_id")
        }
        self.paymentRequestId = paymentRequestId
        self.outputIndex = outputIndex
    }
}

public enum OfflineNoteCommitmentOrigin: Equatable, Sendable {
    case issuerLoad(OfflineNoteIssuerLoadOrigin)
    case p2pOutput(OfflineNoteP2pOutputOrigin)
}

public struct OfflineNoteCommitmentPreimage: Equatable, Sendable {
    public let domain: String
    public let chainId: String
    public let ownerKeyCertificatePayloadHash: Data
    public let assetId: String
    public let amount: String
    public let noteSecret: Data
    public let origin: OfflineNoteCommitmentOrigin

    public init(domain: String = OfflineNoteConstants.noteCommitmentDomain,
                chainId: String,
                ownerKeyCertificatePayloadHash: Data,
                assetId: String,
                amount: String,
                noteSecret: Data,
                origin: OfflineNoteCommitmentOrigin) throws {
        guard domain == OfflineNoteConstants.noteCommitmentDomain else {
            throw OfflineNoteError.unsupportedDerivationDomain(
                field: "domain",
                expected: OfflineNoteConstants.noteCommitmentDomain,
                actual: domain
            )
        }
        guard !chainId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw OfflineNoritoError.invalidMetadata("chain_id")
        }
        try OfflineNoteValidation.validateHash(
            ownerKeyCertificatePayloadHash,
            field: "owner_key_certificate_payload_hash"
        )
        try OfflineNoteValidation.validateRandomBytes(noteSecret, field: "note_secret")
        self.domain = domain
        self.chainId = chainId
        self.ownerKeyCertificatePayloadHash = ownerKeyCertificatePayloadHash
        self.assetId = try OfflineNorito.canonicalAssetIdLiteral(assetId)
        self.amount = try OfflineNorito.parseCanonicalNumeric(amount).canonicalString
        self.noteSecret = noteSecret
        self.origin = origin
    }

    public func noritoEncoded() throws -> Data {
        try OfflineNoteEncoding.wrap(
            typeName: OfflineNoteTypeNames.noteCommitmentPreimage,
            payload: OfflineNoteEncoding.encodeNoteCommitmentPreimage(self)
        )
    }

    public func deriveNoteCommitment() throws -> Data {
        IrohaHash.hash(try noritoEncoded())
    }
}

public struct OfflineNoteInputNullifierPreimage: Equatable, Sendable {
    public let domain: String
    public let chainId: String
    public let sourceNoteCommitment: Data
    public let ownerKeyCertificatePayloadHash: Data
    public let noteSecret: Data

    public init(domain: String = OfflineNoteConstants.inputNullifierDomain,
                chainId: String,
                sourceNoteCommitment: Data,
                ownerKeyCertificatePayloadHash: Data,
                noteSecret: Data) throws {
        guard domain == OfflineNoteConstants.inputNullifierDomain else {
            throw OfflineNoteError.unsupportedDerivationDomain(
                field: "domain",
                expected: OfflineNoteConstants.inputNullifierDomain,
                actual: domain
            )
        }
        guard !chainId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw OfflineNoritoError.invalidMetadata("chain_id")
        }
        try OfflineNoteValidation.validateHash(sourceNoteCommitment, field: "source_note_commitment")
        try OfflineNoteValidation.validateHash(
            ownerKeyCertificatePayloadHash,
            field: "owner_key_certificate_payload_hash"
        )
        try OfflineNoteValidation.validateRandomBytes(noteSecret, field: "note_secret")
        self.domain = domain
        self.chainId = chainId
        self.sourceNoteCommitment = sourceNoteCommitment
        self.ownerKeyCertificatePayloadHash = ownerKeyCertificatePayloadHash
        self.noteSecret = noteSecret
    }

    public func noritoEncoded() throws -> Data {
        try OfflineNoteEncoding.wrap(
            typeName: OfflineNoteTypeNames.inputNullifierPreimage,
            payload: OfflineNoteEncoding.encodeInputNullifierPreimage(self)
        )
    }

    public func deriveInputNullifier() throws -> Data {
        IrohaHash.hash(try noritoEncoded())
    }
}

public struct OfflineNotePaymentTokenIdPreimage: Equatable, Sendable {
    public let domain: String
    public let chainId: String
    public let paymentRequestId: String
    public let createdAtMs: UInt64
    public let tokenNonce: Data
    public let senderKeyCertificatePayloadHash: Data
    public let inputNullifiers: [Data]
    public let outputCommitments: [Data]

    public init(domain: String = OfflineNoteConstants.paymentTokenIdDomain,
                chainId: String,
                paymentRequestId: String,
                createdAtMs: UInt64,
                tokenNonce: Data,
                senderKeyCertificatePayloadHash: Data,
                inputNullifiers: [Data],
                outputCommitments: [Data]) throws {
        guard domain == OfflineNoteConstants.paymentTokenIdDomain else {
            throw OfflineNoteError.unsupportedDerivationDomain(
                field: "domain",
                expected: OfflineNoteConstants.paymentTokenIdDomain,
                actual: domain
            )
        }
        guard !chainId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw OfflineNoritoError.invalidMetadata("chain_id")
        }
        guard !paymentRequestId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw OfflineNoritoError.invalidMetadata("payment_request_id")
        }
        try OfflineNoteValidation.validateRandomBytes(tokenNonce, field: "token_nonce")
        try OfflineNoteValidation.validateHash(
            senderKeyCertificatePayloadHash,
            field: "sender_key_certificate_payload_hash"
        )
        try OfflineNoteValidation.validateHashes(inputNullifiers, field: "input_nullifiers")
        try OfflineNoteValidation.validateHashes(
            outputCommitments,
            field: "output_commitments",
            emptyError: .emptyOutputCommitments
        )
        self.domain = domain
        self.chainId = chainId
        self.paymentRequestId = paymentRequestId
        self.createdAtMs = createdAtMs
        self.tokenNonce = tokenNonce
        self.senderKeyCertificatePayloadHash = senderKeyCertificatePayloadHash
        self.inputNullifiers = inputNullifiers
        self.outputCommitments = outputCommitments
    }

    public func noritoEncoded() throws -> Data {
        try OfflineNoteEncoding.wrap(
            typeName: OfflineNoteTypeNames.paymentTokenIdPreimage,
            payload: OfflineNoteEncoding.encodePaymentTokenIdPreimage(self)
        )
    }

    public func derivePaymentTokenId() throws -> Data {
        IrohaHash.hash(try noritoEncoded())
    }
}

public struct OfflineNoteIssue: Equatable, Sendable {
    public let noteCommitment: Data
    public let keyCertificate: OfflineNoteKeyCertificate
    public let assetId: String
    public let amount: String

    public init(noteCommitment: Data,
                keyCertificate: OfflineNoteKeyCertificate,
                assetId: String,
                amount: String) throws {
        try OfflineNoteValidation.validateHash(noteCommitment, field: "note_commitment")
        self.noteCommitment = noteCommitment
        self.keyCertificate = keyCertificate
        self.assetId = try OfflineNorito.canonicalAssetIdLiteral(assetId)
        self.amount = try OfflineNorito.parseCanonicalNumeric(amount).canonicalString
    }

    public func issuedClaim() throws -> OfflineNoteIssuedClaim {
        try OfflineNoteIssuedClaim.fromIssue(self)
    }

    public func noritoEncoded() throws -> Data {
        try OfflineNoteEncoding.wrap(
            typeName: OfflineNoteTypeNames.issue,
            payload: OfflineNoteEncoding.encodeIssue(self)
        )
    }
}

public struct OfflineNoteIssuedClaim: Equatable, Sendable {
    public let domain: String
    public let noteCommitment: Data
    public let keyCertificatePayloadHash: Data
    public let assetId: String
    public let amount: String

    public init(domain: String = OfflineNoteConstants.issuedClaimDomain,
                noteCommitment: Data,
                keyCertificatePayloadHash: Data,
                assetId: String,
                amount: String) throws {
        try OfflineNoteValidation.validateHash(noteCommitment, field: "note_commitment")
        try OfflineNoteValidation.validateHash(
            keyCertificatePayloadHash,
            field: "key_certificate_payload_hash"
        )
        self.domain = domain
        self.noteCommitment = noteCommitment
        self.keyCertificatePayloadHash = keyCertificatePayloadHash
        self.assetId = try OfflineNorito.canonicalAssetIdLiteral(assetId)
        self.amount = try OfflineNorito.parseCanonicalNumeric(amount).canonicalString
    }

    public static func fromIssue(_ issue: OfflineNoteIssue) throws -> OfflineNoteIssuedClaim {
        try OfflineNoteIssuedClaim(
            noteCommitment: issue.noteCommitment,
            keyCertificatePayloadHash: issue.keyCertificate.payloadHash(),
            assetId: issue.assetId,
            amount: issue.amount
        )
    }

    public static func fromRedemption(_ redemption: OfflineNoteRedeem) throws -> OfflineNoteIssuedClaim {
        try OfflineNoteIssuedClaim(
            noteCommitment: redemption.sourceNoteCommitment,
            keyCertificatePayloadHash: redemption.senderKeyCertificate.payloadHash(),
            assetId: redemption.assetId,
            amount: redemption.amount
        )
    }

    public static func fromAuditOutput(_ output: OfflineNoteAuditOutputClaim) throws -> OfflineNoteIssuedClaim {
        try OfflineNoteIssuedClaim(
            noteCommitment: output.noteCommitment,
            keyCertificatePayloadHash: output.keyCertificate.payloadHash(),
            assetId: output.assetId,
            amount: output.amount
        )
    }

    public func claimHash() throws -> Data {
        IrohaHash.hash(try noritoEncoded())
    }

    public func noritoEncoded() throws -> Data {
        try OfflineNoteEncoding.wrap(
            typeName: OfflineNoteTypeNames.issuedClaim,
            payload: OfflineNoteEncoding.encodeIssuedClaim(self)
        )
    }
}

public struct OfflineNoteAuditOutputClaim: Equatable, Sendable {
    public let noteCommitment: Data
    public let keyCertificate: OfflineNoteKeyCertificate
    public let assetId: String
    public let amount: String

    public init(noteCommitment: Data,
                keyCertificate: OfflineNoteKeyCertificate,
                assetId: String,
                amount: String) throws {
        try OfflineNoteValidation.validateHash(noteCommitment, field: "note_commitment")
        self.noteCommitment = noteCommitment
        self.keyCertificate = keyCertificate
        self.assetId = try OfflineNorito.canonicalAssetIdLiteral(assetId)
        self.amount = try OfflineNorito.parseCanonicalNumeric(amount).canonicalString
    }
}

public struct OfflineNoteRedeemPublicInputs: Equatable, Sendable {
    public let domain: String
    public let sourceNoteCommitment: Data
    public let inputNullifiers: [Data]
    public let keyCertificatePayloadHash: Data
    public let recipient: String
    public let assetId: String
    public let amount: String

    public init(domain: String = OfflineNoteConstants.redeemPublicInputsDomain,
                sourceNoteCommitment: Data,
                inputNullifiers: [Data],
                keyCertificatePayloadHash: Data,
                recipient: String,
                assetId: String,
                amount: String) throws {
        try OfflineNoteValidation.validateHash(sourceNoteCommitment, field: "source_note_commitment")
        try OfflineNoteValidation.validateHashes(inputNullifiers, field: "input_nullifiers")
        try OfflineNoteValidation.validateHash(
            keyCertificatePayloadHash,
            field: "key_certificate_payload_hash"
        )
        _ = try OfflineNorito.encodeAccountId(recipient)
        self.domain = domain
        self.sourceNoteCommitment = sourceNoteCommitment
        self.inputNullifiers = inputNullifiers
        self.keyCertificatePayloadHash = keyCertificatePayloadHash
        self.recipient = recipient
        self.assetId = try OfflineNorito.canonicalAssetIdLiteral(assetId)
        self.amount = try OfflineNorito.parseCanonicalNumeric(amount).canonicalString
    }

    public static func fromRedemption(_ redemption: OfflineNoteRedeem) throws -> OfflineNoteRedeemPublicInputs {
        try OfflineNoteRedeemPublicInputs(
            sourceNoteCommitment: redemption.sourceNoteCommitment,
            inputNullifiers: redemption.inputNullifiers,
            keyCertificatePayloadHash: redemption.senderKeyCertificate.payloadHash(),
            recipient: redemption.recipient,
            assetId: redemption.assetId,
            amount: redemption.amount
        )
    }

    public func publicInputsHash() throws -> Data {
        IrohaHash.hash(try noritoEncoded())
    }

    public func noritoEncoded() throws -> Data {
        try OfflineNoteEncoding.wrap(
            typeName: OfflineNoteTypeNames.redeemPublicInputs,
            payload: OfflineNoteEncoding.encodeRedeemPublicInputs(self)
        )
    }
}

public struct OfflineNoteRedeem: Equatable, Sendable {
    public let sourceNoteCommitment: Data
    public let inputNullifiers: [Data]
    public let senderKeyCertificate: OfflineNoteKeyCertificate
    public let recipient: String
    public let assetId: String
    public let amount: String
    public let recursiveProof: OfflineNoteRecursiveProof

    public init(sourceNoteCommitment: Data,
                inputNullifiers: [Data],
                senderKeyCertificate: OfflineNoteKeyCertificate,
                recipient: String,
                assetId: String,
                amount: String,
                recursiveProof: OfflineNoteRecursiveProof) throws {
        try OfflineNoteValidation.validateHash(sourceNoteCommitment, field: "source_note_commitment")
        try OfflineNoteValidation.validateHashes(inputNullifiers, field: "input_nullifiers")
        _ = try OfflineNorito.encodeAccountId(recipient)
        self.sourceNoteCommitment = sourceNoteCommitment
        self.inputNullifiers = inputNullifiers
        self.senderKeyCertificate = senderKeyCertificate
        self.recipient = recipient
        self.assetId = try OfflineNorito.canonicalAssetIdLiteral(assetId)
        self.amount = try OfflineNorito.parseCanonicalNumeric(amount).canonicalString
        self.recursiveProof = recursiveProof
    }

    public func issuedClaim() throws -> OfflineNoteIssuedClaim {
        try OfflineNoteIssuedClaim.fromRedemption(self)
    }

    public func publicInputs() throws -> OfflineNoteRedeemPublicInputs {
        try OfflineNoteRedeemPublicInputs.fromRedemption(self)
    }

    public func publicInputsHash() throws -> Data {
        try publicInputs().publicInputsHash()
    }

    public func validateProofBinding() throws {
        try recursiveProof.validateCanonicalMetadata()
        let expected = try publicInputsHash()
        guard recursiveProof.publicInputsHash == expected else {
            throw OfflineNoteError.proofPublicInputsHashMismatch(
                expected: expected.hexLowercased(),
                actual: recursiveProof.publicInputsHash.hexLowercased()
            )
        }
    }

    public func replacingRecursiveProof(_ recursiveProof: OfflineNoteRecursiveProof) throws -> OfflineNoteRedeem {
        try OfflineNoteRedeem(
            sourceNoteCommitment: sourceNoteCommitment,
            inputNullifiers: inputNullifiers,
            senderKeyCertificate: senderKeyCertificate,
            recipient: recipient,
            assetId: assetId,
            amount: amount,
            recursiveProof: recursiveProof
        )
    }

    public func noritoEncoded() throws -> Data {
        try OfflineNoteEncoding.wrap(
            typeName: OfflineNoteTypeNames.redeem,
            payload: OfflineNoteEncoding.encodeRedeem(self)
        )
    }
}

public struct OfflineNoteAuditPublicInputs: Equatable, Sendable {
    public let domain: String
    public let tokenId: Data
    public let keyCertificatePayloadHash: Data
    public let inputNullifiers: [Data]
    public let inputClaims: [OfflineNoteIssuedClaim]
    public let outputCommitments: [Data]
    public let outputClaims: [OfflineNoteIssuedClaim]

    public init(domain: String = OfflineNoteConstants.auditPublicInputsDomain,
                tokenId: Data,
                keyCertificatePayloadHash: Data,
                inputNullifiers: [Data],
                inputClaims: [OfflineNoteIssuedClaim],
                outputCommitments: [Data],
                outputClaims: [OfflineNoteIssuedClaim]) throws {
        try OfflineNoteValidation.validateHash(tokenId, field: "token_id")
        try OfflineNoteValidation.validateHash(
            keyCertificatePayloadHash,
            field: "key_certificate_payload_hash"
        )
        try OfflineNoteValidation.validateHashes(
            inputNullifiers,
            field: "input_nullifiers",
            emptyError: .emptyInputNullifiers
        )
        try OfflineNoteValidation.validateHashes(
            outputCommitments,
            field: "output_commitments",
            emptyError: .emptyOutputCommitments
        )
        guard !inputClaims.isEmpty else {
            throw OfflineNoteError.emptyInputClaims
        }
        guard inputClaims.count == inputNullifiers.count else {
            throw OfflineNoteError.auditInputCountMismatch(
                nullifiers: inputNullifiers.count,
                claims: inputClaims.count
            )
        }
        guard !outputClaims.isEmpty else {
            throw OfflineNoteError.emptyOutputClaims
        }
        guard outputClaims.count == outputCommitments.count else {
            throw OfflineNoteError.auditOutputCountMismatch(
                commitments: outputCommitments.count,
                claims: outputClaims.count
            )
        }
        for (index, pair) in zip(outputCommitments, outputClaims).enumerated() where pair.0 != pair.1.noteCommitment {
            throw OfflineNoteError.auditOutputClaimOrderMismatch(index: index)
        }
        self.domain = domain
        self.tokenId = tokenId
        self.keyCertificatePayloadHash = keyCertificatePayloadHash
        self.inputNullifiers = inputNullifiers
        self.inputClaims = inputClaims
        self.outputCommitments = outputCommitments
        self.outputClaims = outputClaims
    }

    public static func fromAudit(_ audit: OfflineNoteAuditBundle) throws -> OfflineNoteAuditPublicInputs {
        let outputClaims = try audit.outputClaims.map(OfflineNoteIssuedClaim.fromAuditOutput)
        return try OfflineNoteAuditPublicInputs(
            tokenId: audit.tokenId,
            keyCertificatePayloadHash: audit.senderKeyCertificate.payloadHash(),
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: outputClaims
        )
    }

    public func publicInputsHash() throws -> Data {
        IrohaHash.hash(try noritoEncoded())
    }

    public func noritoEncoded() throws -> Data {
        try OfflineNoteEncoding.wrap(
            typeName: OfflineNoteTypeNames.auditPublicInputs,
            payload: OfflineNoteEncoding.encodeAuditPublicInputs(self)
        )
    }
}

public struct OfflineNoteAuditBundle: Equatable, Sendable {
    public let tokenId: Data
    public let senderKeyCertificate: OfflineNoteKeyCertificate
    public let inputNullifiers: [Data]
    public let inputClaims: [OfflineNoteIssuedClaim]
    public let outputCommitments: [Data]
    public let outputClaims: [OfflineNoteAuditOutputClaim]
    public let recursiveProof: OfflineNoteRecursiveProof

    public init(tokenId: Data,
                senderKeyCertificate: OfflineNoteKeyCertificate,
                inputNullifiers: [Data],
                inputClaims: [OfflineNoteIssuedClaim],
                outputCommitments: [Data],
                outputClaims: [OfflineNoteAuditOutputClaim],
                recursiveProof: OfflineNoteRecursiveProof) throws {
        try OfflineNoteValidation.validateHash(tokenId, field: "token_id")
        try OfflineNoteValidation.validateHashes(
            inputNullifiers,
            field: "input_nullifiers",
            emptyError: .emptyInputNullifiers
        )
        try OfflineNoteValidation.validateHashes(
            outputCommitments,
            field: "output_commitments",
            emptyError: .emptyOutputCommitments
        )
        guard !inputClaims.isEmpty else {
            throw OfflineNoteError.emptyInputClaims
        }
        guard inputClaims.count == inputNullifiers.count else {
            throw OfflineNoteError.auditInputCountMismatch(
                nullifiers: inputNullifiers.count,
                claims: inputClaims.count
            )
        }
        guard !outputClaims.isEmpty else {
            throw OfflineNoteError.emptyOutputClaims
        }
        guard outputClaims.count == outputCommitments.count else {
            throw OfflineNoteError.auditOutputCountMismatch(
                commitments: outputCommitments.count,
                claims: outputClaims.count
            )
        }
        for (index, pair) in zip(outputCommitments, outputClaims).enumerated() {
            guard pair.0 == pair.1.noteCommitment else {
                throw OfflineNoteError.auditOutputClaimOrderMismatch(index: index)
            }
        }
        self.tokenId = tokenId
        self.senderKeyCertificate = senderKeyCertificate
        self.inputNullifiers = inputNullifiers
        self.inputClaims = inputClaims
        self.outputCommitments = outputCommitments
        self.outputClaims = outputClaims
        self.recursiveProof = recursiveProof
    }

    public func publicInputs() throws -> OfflineNoteAuditPublicInputs {
        try OfflineNoteAuditPublicInputs.fromAudit(self)
    }

    public func outputClaim(matchingNoteCommitment noteCommitment: Data) -> OfflineNoteAuditOutputClaim? {
        outputClaims.first { $0.noteCommitment == noteCommitment }
    }

    public func outputClaim(matchingNoteCommitmentHex noteCommitmentHex: String) -> OfflineNoteAuditOutputClaim? {
        guard let noteCommitment = Data(hexString: normalizedNoteCommitmentHex(noteCommitmentHex)),
              noteCommitment.count == 32 else {
            return nil
        }
        return outputClaim(matchingNoteCommitment: noteCommitment)
    }

    public func containsOutputNoteCommitment(_ noteCommitment: Data) -> Bool {
        outputClaim(matchingNoteCommitment: noteCommitment) != nil
    }

    public func containsOutputNoteCommitment(hex noteCommitmentHex: String) -> Bool {
        outputClaim(matchingNoteCommitmentHex: noteCommitmentHex) != nil
    }

    public func publicInputsHash() throws -> Data {
        try publicInputs().publicInputsHash()
    }

    public func validateProofBinding() throws {
        try recursiveProof.validateCanonicalMetadata()
        let expected = try publicInputsHash()
        guard recursiveProof.publicInputsHash == expected else {
            throw OfflineNoteError.proofPublicInputsHashMismatch(
                expected: expected.hexLowercased(),
                actual: recursiveProof.publicInputsHash.hexLowercased()
            )
        }
    }

    public func replacingRecursiveProof(_ recursiveProof: OfflineNoteRecursiveProof) throws -> OfflineNoteAuditBundle {
        try OfflineNoteAuditBundle(
            tokenId: tokenId,
            senderKeyCertificate: senderKeyCertificate,
            inputNullifiers: inputNullifiers,
            inputClaims: inputClaims,
            outputCommitments: outputCommitments,
            outputClaims: outputClaims,
            recursiveProof: recursiveProof
        )
    }

    public func noritoEncoded() throws -> Data {
        try OfflineNoteEncoding.wrap(
            typeName: OfflineNoteTypeNames.audit,
            payload: OfflineNoteEncoding.encodeAudit(self)
        )
    }

    private func normalizedNoteCommitmentHex(_ value: String) -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if trimmed.hasPrefix("0x") {
            return String(trimmed.dropFirst(2))
        }
        return trimmed
    }
}

public struct IssueOfflineNoteRequest: Sendable {
    public let chainId: String
    public let authority: String
    public let issue: OfflineNoteIssue
    public let ttlMs: UInt64?
    public let nonce: UInt32?

    public init(chainId: String,
                authority: String,
                issue: OfflineNoteIssue,
                ttlMs: UInt64? = nil,
                nonce: UInt32? = nil) {
        self.chainId = chainId
        self.authority = authority
        self.issue = issue
        self.ttlMs = ttlMs
        self.nonce = nonce
    }
}

/// A single `RedeemOfflineNote` instruction.
///
/// Use this only when the source note's issued claim is already recorded on-chain, such as
/// issuer-loaded notes or P2P outputs whose audit lineage was already published. For offline
/// bearer notes, prefer `DefundOfflineNoteRequest` so the audit lineage and redemption are
/// submitted atomically in one transaction.
public struct RedeemOfflineNoteRequest: Sendable {
    public let chainId: String
    public let authority: String
    public let redemption: OfflineNoteRedeem
    public let ttlMs: UInt64?
    public let nonce: UInt32?

    public init(chainId: String,
                authority: String,
                redemption: OfflineNoteRedeem,
                ttlMs: UInt64? = nil,
                nonce: UInt32? = nil) {
        self.chainId = chainId
        self.authority = authority
        self.redemption = redemption
        self.ttlMs = ttlMs
        self.nonce = nonce
    }
}

/// Atomic defunding request for an offline bearer note.
///
/// `bearerAuditTrail` contains the ordered P2P audit lineage that anchors the bearer note's
/// issued claim before the final redemption instruction in the same signed transaction. Empty
/// lineage is valid for issuer-loaded notes, but P2P notes should carry at least the payment
/// token audit that created the redeemed output.
public struct DefundOfflineNoteRequest: Sendable {
    public let chainId: String
    public let authority: String
    public let bearerAuditTrail: [OfflineNoteAuditBundle]
    public let redemption: OfflineNoteRedeem
    public let ttlMs: UInt64?
    public let nonce: UInt32?

    public init(chainId: String,
                authority: String,
                bearerAuditTrail: [OfflineNoteAuditBundle],
                redemption: OfflineNoteRedeem,
                ttlMs: UInt64? = nil,
                nonce: UInt32? = nil) {
        self.chainId = chainId
        self.authority = authority
        self.bearerAuditTrail = bearerAuditTrail
        self.redemption = redemption
        self.ttlMs = ttlMs
        self.nonce = nonce
    }
}

public struct AuditOfflineNoteRequest: Sendable {
    public let chainId: String
    public let authority: String
    public let audit: OfflineNoteAuditBundle
    public let ttlMs: UInt64?
    public let nonce: UInt32?

    public init(chainId: String,
                authority: String,
                audit: OfflineNoteAuditBundle,
                ttlMs: UInt64? = nil,
                nonce: UInt32? = nil) {
        self.chainId = chainId
        self.authority = authority
        self.audit = audit
        self.ttlMs = ttlMs
        self.nonce = nonce
    }
}

enum OfflineNoteTypeNames {
    static let keyCertificate = "iroha_data_model::offline::model::OfflineNoteKeyCertificate"
    static let keyCertificatePayload = "iroha_data_model::offline::model::OfflineNoteKeyCertificatePayload"
    static let recursiveProof = "iroha_data_model::offline::model::OfflineNoteRecursiveProof"
    static let issue = "iroha_data_model::offline::model::OfflineNoteIssue"
    static let issuedClaim = "iroha_data_model::offline::model::OfflineNoteIssuedClaim"
    static let auditOutputClaim = "iroha_data_model::offline::model::OfflineNoteAuditOutputClaim"
    static let redeem = "iroha_data_model::offline::model::OfflineNoteRedeem"
    static let redeemPublicInputs = "iroha_data_model::offline::model::OfflineNoteRedeemPublicInputs"
    static let audit = "iroha_data_model::offline::model::OfflineNoteAuditBundle"
    static let auditPublicInputs = "iroha_data_model::offline::model::OfflineNoteAuditPublicInputs"
    static let noteCommitmentPreimage = "iroha_data_model::offline::model::OfflineNoteCommitmentPreimage"
    static let inputNullifierPreimage = "iroha_data_model::offline::model::OfflineNoteInputNullifierPreimage"
    static let paymentTokenIdPreimage = "iroha_data_model::offline::model::OfflineNotePaymentTokenIdPreimage"
    static let issueInstruction = "iroha_data_model::isi::offline::IssueOfflineNote"
    static let redeemInstruction = "iroha_data_model::isi::offline::RedeemOfflineNote"
    static let auditInstruction = "iroha_data_model::isi::offline::AuditOfflineNote"
}

enum OfflineNoteValidation {
    static func validateHash(_ value: Data, field: String) throws {
        guard value.count == 32 else {
            throw OfflineNoteError.invalidHashLength(field: field, expected: 32, actual: value.count)
        }
        guard let last = value.last, (last & 1) == 1 else {
            throw OfflineNoteError.invalidHash(field: field)
        }
    }

    static func validateHashes(_ values: [Data],
                               field: String,
                               emptyError: OfflineNoteError = .emptyInputNullifiers) throws {
        guard !values.isEmpty else {
            throw emptyError
        }
        for (index, value) in values.enumerated() {
            try validateHash(value, field: "\(field)[\(index)]")
        }
    }

    static func validateRandomBytes(_ value: Data, field: String) throws {
        guard value.count == 32 else {
            throw OfflineNoteError.invalidRandomBytesLength(
                field: field,
                expected: 32,
                actual: value.count
            )
        }
    }

    static func validateCertificateCore(version: UInt16,
                                        accountId: String,
                                        publicKey: Data,
                                        oneUse: Bool) throws {
        guard version == OfflineNoteConstants.keyCertificateVersion else {
            throw OfflineNoteError.invalidCertificateVersion(version)
        }
        guard oneUse else {
            throw OfflineNoteError.certificateMustBeOneUse
        }
        guard publicKey.count == 32 else {
            throw OfflineNoteError.invalidNotePublicKeyLength(expected: 32, actual: publicKey.count)
        }
        _ = try OfflineNorito.encodeAccountId(accountId)
    }
}

enum OfflineNoteEncoding {
    static func wrap(typeName: String, payload: Data) -> Data {
        noritoEncode(typeName: typeName, payload: payload, flags: 2)
    }

    static func encodeCertificatePayload(_ payload: OfflineNoteKeyCertificatePayload) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(payload.domain))
        writer.writeField(OfflineCompactNorito.encodeUInt16(payload.version))
        writer.writeField(OfflineCompactNorito.encodeString(payload.platform))
        writer.writeField(OfflineCompactNorito.encodeString(payload.keyId))
        writer.writeField(OfflineCompactNorito.encodeString(payload.deviceId))
        writer.writeField(try encodeAccountId(payload.accountId))
        writer.writeField(encodeBytesVec(payload.publicKey))
        writer.writeField(OfflineCompactNorito.encodeString(payload.assertionScheme))
        writer.writeField(OfflineCompactNorito.encodeString(payload.assertionKeyAlgorithm))
        writer.writeField(encodeBytesVec(payload.assertionPublicKey))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            payload.assertionUsageCountLimit,
            encode: OfflineCompactNorito.encodeUInt32
        ))
        writer.writeField(OfflineNorito.encodeBool(payload.oneUse))
        return writer.data
    }

    static func encodeCertificate(_ certificate: OfflineNoteKeyCertificate) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeUInt16(certificate.version))
        writer.writeField(OfflineCompactNorito.encodeString(certificate.platform))
        writer.writeField(OfflineCompactNorito.encodeString(certificate.keyId))
        writer.writeField(OfflineCompactNorito.encodeString(certificate.deviceId))
        writer.writeField(try encodeAccountId(certificate.accountId))
        writer.writeField(encodeBytesVec(certificate.publicKey))
        writer.writeField(OfflineCompactNorito.encodeString(certificate.assertionScheme))
        writer.writeField(OfflineCompactNorito.encodeString(certificate.assertionKeyAlgorithm))
        writer.writeField(encodeBytesVec(certificate.assertionPublicKey))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            certificate.assertionUsageCountLimit,
            encode: OfflineCompactNorito.encodeUInt32
        ))
        writer.writeField(OfflineNorito.encodeBool(certificate.oneUse))
        writer.writeField(encodeConstVec(certificate.issuerSignature))
        return writer.data
    }

    static func encodeVerifyingKeyId(_ id: VerifyingKeyIdReference) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(id.backend))
        writer.writeField(OfflineCompactNorito.encodeString(id.name))
        return writer.data
    }

    static func encodeProofBox(_ proof: OfflineNoteProofBox) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(proof.backend))
        writer.writeField(encodeBytesVec(proof.bytes))
        return writer.data
    }

    static func encodeRecursiveProof(_ proof: OfflineNoteRecursiveProof) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(encodeVerifyingKeyId(proof.verifierKeyId))
        writer.writeField(try OfflineCompactNorito.encodeHash(proof.publicInputsHash))
        writer.writeField(encodeProofBox(proof.proof))
        return writer.data
    }

    static func encodeIssue(_ issue: OfflineNoteIssue) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try OfflineCompactNorito.encodeHash(issue.noteCommitment))
        writer.writeField(try encodeCertificate(issue.keyCertificate))
        writer.writeField(try encodeAssetId(issue.assetId))
        writer.writeField(try encodeNumeric(issue.amount))
        return writer.data
    }

    static func encodeIssuedClaim(_ claim: OfflineNoteIssuedClaim) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(claim.domain))
        writer.writeField(try OfflineCompactNorito.encodeHash(claim.noteCommitment))
        writer.writeField(try OfflineCompactNorito.encodeHash(claim.keyCertificatePayloadHash))
        writer.writeField(try encodeAssetId(claim.assetId))
        writer.writeField(try encodeNumeric(claim.amount))
        return writer.data
    }

    static func encodeAuditOutputClaim(_ claim: OfflineNoteAuditOutputClaim) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try OfflineCompactNorito.encodeHash(claim.noteCommitment))
        writer.writeField(try encodeCertificate(claim.keyCertificate))
        writer.writeField(try encodeAssetId(claim.assetId))
        writer.writeField(try encodeNumeric(claim.amount))
        return writer.data
    }

    static func encodeRedeemPublicInputs(_ inputs: OfflineNoteRedeemPublicInputs) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(inputs.domain))
        writer.writeField(try OfflineCompactNorito.encodeHash(inputs.sourceNoteCommitment))
        writer.writeField(try encodeVec(inputs.inputNullifiers, encode: OfflineCompactNorito.encodeHash))
        writer.writeField(try OfflineCompactNorito.encodeHash(inputs.keyCertificatePayloadHash))
        writer.writeField(try encodeAccountId(inputs.recipient))
        writer.writeField(try encodeAssetId(inputs.assetId))
        writer.writeField(try encodeNumeric(inputs.amount))
        return writer.data
    }

    static func encodeRedeem(_ redemption: OfflineNoteRedeem) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try OfflineCompactNorito.encodeHash(redemption.sourceNoteCommitment))
        writer.writeField(try encodeVec(redemption.inputNullifiers, encode: OfflineCompactNorito.encodeHash))
        writer.writeField(try encodeCertificate(redemption.senderKeyCertificate))
        writer.writeField(try encodeAccountId(redemption.recipient))
        writer.writeField(try encodeAssetId(redemption.assetId))
        writer.writeField(try encodeNumeric(redemption.amount))
        writer.writeField(try encodeRecursiveProof(redemption.recursiveProof))
        return writer.data
    }

    static func encodeAuditPublicInputs(_ inputs: OfflineNoteAuditPublicInputs) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(inputs.domain))
        writer.writeField(try OfflineCompactNorito.encodeHash(inputs.tokenId))
        writer.writeField(try OfflineCompactNorito.encodeHash(inputs.keyCertificatePayloadHash))
        writer.writeField(try encodeVec(inputs.inputNullifiers, encode: OfflineCompactNorito.encodeHash))
        writer.writeField(try encodeVec(inputs.inputClaims, encode: encodeIssuedClaim))
        writer.writeField(try encodeVec(inputs.outputCommitments, encode: OfflineCompactNorito.encodeHash))
        writer.writeField(try encodeVec(inputs.outputClaims, encode: encodeIssuedClaim))
        return writer.data
    }

    static func encodeAudit(_ audit: OfflineNoteAuditBundle) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try OfflineCompactNorito.encodeHash(audit.tokenId))
        writer.writeField(try encodeCertificate(audit.senderKeyCertificate))
        writer.writeField(try encodeVec(audit.inputNullifiers, encode: OfflineCompactNorito.encodeHash))
        writer.writeField(try encodeVec(audit.inputClaims, encode: encodeIssuedClaim))
        writer.writeField(try encodeVec(audit.outputCommitments, encode: OfflineCompactNorito.encodeHash))
        writer.writeField(try encodeVec(audit.outputClaims, encode: encodeAuditOutputClaim))
        writer.writeField(try encodeRecursiveProof(audit.recursiveProof))
        return writer.data
    }

    static func encodeNoteCommitmentPreimage(_ preimage: OfflineNoteCommitmentPreimage) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(preimage.domain))
        writer.writeField(encodeChainId(preimage.chainId))
        writer.writeField(try OfflineCompactNorito.encodeHash(preimage.ownerKeyCertificatePayloadHash))
        writer.writeField(try encodeAssetId(preimage.assetId))
        writer.writeField(try encodeNumeric(preimage.amount))
        writer.writeField(encodeBytesVec(preimage.noteSecret))
        writer.writeField(encodeCommitmentOrigin(preimage.origin))
        return writer.data
    }

    static func encodeInputNullifierPreimage(_ preimage: OfflineNoteInputNullifierPreimage) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(preimage.domain))
        writer.writeField(encodeChainId(preimage.chainId))
        writer.writeField(try OfflineCompactNorito.encodeHash(preimage.sourceNoteCommitment))
        writer.writeField(try OfflineCompactNorito.encodeHash(preimage.ownerKeyCertificatePayloadHash))
        writer.writeField(encodeBytesVec(preimage.noteSecret))
        return writer.data
    }

    static func encodePaymentTokenIdPreimage(_ preimage: OfflineNotePaymentTokenIdPreimage) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(preimage.domain))
        writer.writeField(encodeChainId(preimage.chainId))
        writer.writeField(OfflineCompactNorito.encodeString(preimage.paymentRequestId))
        writer.writeField(OfflineCompactNorito.encodeUInt64(preimage.createdAtMs))
        writer.writeField(encodeBytesVec(preimage.tokenNonce))
        writer.writeField(try OfflineCompactNorito.encodeHash(preimage.senderKeyCertificatePayloadHash))
        writer.writeField(try encodeVec(preimage.inputNullifiers, encode: OfflineCompactNorito.encodeHash))
        writer.writeField(try encodeVec(preimage.outputCommitments, encode: OfflineCompactNorito.encodeHash))
        return writer.data
    }

    static func encodeCommitmentOrigin(_ origin: OfflineNoteCommitmentOrigin) -> Data {
        var writer = OfflineCompactNoritoWriter()
        switch origin {
        case let .issuerLoad(value):
            writer.writeUInt32LE(0)
            writer.writeField(encodeIssuerLoadOrigin(value))
        case let .p2pOutput(value):
            writer.writeUInt32LE(1)
            writer.writeField(encodeP2pOutputOrigin(value))
        }
        return writer.data
    }

    private static func encodeIssuerLoadOrigin(_ origin: OfflineNoteIssuerLoadOrigin) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(origin.operationId))
        writer.writeField(OfflineCompactNorito.encodeString(origin.lineageId))
        writer.writeField(OfflineCompactNorito.encodeUInt64(origin.localRevision))
        return writer.data
    }

    private static func encodeP2pOutputOrigin(_ origin: OfflineNoteP2pOutputOrigin) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(origin.paymentRequestId))
        writer.writeField(OfflineCompactNorito.encodeUInt32(origin.outputIndex))
        return writer.data
    }

    private static func encodeAccountId(_ value: String) throws -> Data {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        let address = try AccountAddress.parseEncodedSwiftOnly(trimmed, expectedPrefix: 0x02F1)
        return try address.compactNoritoAccountControllerPayload()
    }

    private static func encodeChainId(_ value: String) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(value))
        return writer.data
    }

    private static func encodeAssetId(_ assetId: String) throws -> Data {
        guard let parsed = OfflineNorito.parsePublicAssetIdLiteral(assetId),
              let definitionBytes = AssetDefinitionAddress.decode(parsed.assetDefinitionId) else {
            throw OfflineNoritoError.invalidAssetId(assetId)
        }
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try encodeAccountId(parsed.accountId))
        writer.writeField(encodeAssetDefinitionAddress(definitionBytes))
        writer.writeField(encodeAssetBalanceScope(dataspaceId: parsed.dataspaceId))
        return writer.data
    }

    private static func encodeAssetDefinitionAddress(_ bytes: Data) -> Data {
        var writer = OfflineCompactNoritoWriter()
        for byte in bytes {
            writer.writeLength(1)
            writer.writeUInt8(byte)
        }
        return writer.data
    }

    private static func encodeAssetBalanceScope(dataspaceId: UInt64?) -> Data {
        var writer = OfflineCompactNoritoWriter()
        guard let dataspaceId else {
            writer.writeUInt32LE(0)
            return writer.data
        }
        writer.writeUInt32LE(1)
        var dataspaceWriter = OfflineCompactNoritoWriter()
        dataspaceWriter.writeUInt64LE(dataspaceId)
        writer.writeField(dataspaceWriter.data)
        return writer.data
    }

    private static func encodeNumeric(_ value: String) throws -> Data {
        let numeric = try OfflineNorito.parseNumeric(value)
        let mantissaBytes = try numeric.mantissaBytes(maxBytes: OfflineNorito.maxBigIntBytes)
        var bigintWriter = OfflineCompactNoritoWriter()
        bigintWriter.writeUInt32LE(UInt32(mantissaBytes.count))
        bigintWriter.writeBytes(mantissaBytes)

        var writer = OfflineCompactNoritoWriter()
        writer.writeField(bigintWriter.data)
        writer.writeField(OfflineCompactNorito.encodeUInt32(numeric.scale))
        return writer.data
    }

    private static func encodeBytesVec(_ bytes: Data) -> Data {
        OfflineNorito.encodeBytesVec(bytes)
    }

    private static func encodeVec<T>(_ values: [T], encode: (T) throws -> Data) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt64LE(UInt64(values.count))
        for value in values {
            writer.writeField(try encode(value))
        }
        return writer.data
    }

    private static func encodeConstVec(_ bytes: Data) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeUInt64LE(UInt64(bytes.count))
        for byte in bytes {
            writer.writeLength(1)
            writer.writeUInt8(byte)
        }
        return writer.data
    }
}
