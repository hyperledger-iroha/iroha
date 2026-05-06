import Foundation

public enum OfflineNoteV2Constants {
    public static let keyCertificatePayloadDomain = "iroha:offline-note-v2:key-certificate-payload:v1"
    public static let issuedClaimDomain = "iroha:offline-note-v2:issued-claim:v1"
    public static let redeemPublicInputsDomain = "iroha:offline-note-v2:redeem-public-inputs:v1"
    public static let auditPublicInputsDomain = "iroha:offline-note-v2:audit-public-inputs:v1"
    public static let recursiveBackend = "halo2/ipa"
    public static let recursiveVerifierName = "offline-note-v2-recursive-v1"
    public static let recursivePublicInputsSchemaV1 = #"{"schema":"offline_note_v2_recursive_v1","public_inputs":["public_inputs_hash_limb0","public_inputs_hash_limb1","public_inputs_hash_limb2","public_inputs_hash_limb3","proof_mode","input_count","output_count","input_amount_sum","output_amount_sum","input_nullifier_sum_limb0","output_commitment_sum_limb0","key_certificate_payload_hash_limb0","source_or_token_limb0","input_claim_hash_sum_limb0","output_claim_hash_sum_limb0","reserved_zero"]}"#

    public static var recursivePublicInputsSchemaHash: Data {
        IrohaHash.hash(Data(recursivePublicInputsSchemaV1.utf8))
    }
}

public enum OfflineNoteV2Error: Error, LocalizedError, Equatable {
    case invalidHashLength(field: String, expected: Int, actual: Int)
    case invalidHash(field: String)
    case invalidCertificateVersion(UInt16)
    case invalidNotePublicKeyLength(expected: Int, actual: Int)
    case invalidIssuerSignatureLength(expected: Int, actual: Int)
    case emptyProofBytes
    case emptyProofBackend
    case emptyInputNullifiers
    case emptyInputClaims
    case emptyOutputCommitments
    case emptyOutputClaims
    case auditInputCountMismatch(nullifiers: Int, claims: Int)
    case auditOutputClaimNotCommitted(String)
    case proofPublicInputsHashMismatch(expected: String, actual: String)

    public var errorDescription: String? {
        switch self {
        case let .invalidHashLength(field, expected, actual):
            return "\(field) must be exactly \(expected) bytes (found \(actual))."
        case let .invalidHash(field):
            return "\(field) must be a canonical Iroha hash."
        case let .invalidCertificateVersion(version):
            return "Offline V2 key certificate version must be 2 (found \(version))."
        case let .invalidNotePublicKeyLength(expected, actual):
            return "Offline V2 note public key must be \(expected) bytes (found \(actual))."
        case let .invalidIssuerSignatureLength(expected, actual):
            return "Offline V2 issuer signature must be \(expected) bytes (found \(actual))."
        case .emptyProofBytes:
            return "Offline V2 proof bytes must not be empty."
        case .emptyProofBackend:
            return "Offline V2 proof backend must not be empty."
        case .emptyInputNullifiers:
            return "Offline V2 input nullifiers must not be empty."
        case .emptyInputClaims:
            return "Offline V2 audit input claims must not be empty."
        case .emptyOutputCommitments:
            return "Offline V2 audit output commitments must not be empty."
        case .emptyOutputClaims:
            return "Offline V2 audit output claims must not be empty."
        case let .auditInputCountMismatch(nullifiers, claims):
            return "Offline V2 audit input nullifier count \(nullifiers) must match input claim count \(claims)."
        case let .auditOutputClaimNotCommitted(commitment):
            return "Offline V2 audit output claim \(commitment) is not listed in output commitments."
        case let .proofPublicInputsHashMismatch(expected, actual):
            return "Offline V2 recursive proof public input hash mismatch: expected \(expected), got \(actual)."
        }
    }
}

public struct OfflineNoteProofBox: Equatable, Sendable {
    public let backend: String
    public let bytes: Data

    public init(backend: String, bytes: Data) throws {
        let trimmedBackend = backend.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedBackend.isEmpty else {
            throw OfflineNoteV2Error.emptyProofBackend
        }
        guard !bytes.isEmpty else {
            throw OfflineNoteV2Error.emptyProofBytes
        }
        self.backend = trimmedBackend
        self.bytes = bytes
    }
}

public struct OfflineNoteRecursiveProofV2: Equatable, Sendable {
    public let verifierKeyId: VerifyingKeyIdReference
    public let publicInputsHash: Data
    public let proof: OfflineNoteProofBox

    public init(verifierKeyId: VerifyingKeyIdReference,
                publicInputsHash: Data,
                proof: OfflineNoteProofBox) throws {
        try OfflineNoteV2Validation.validateHash(publicInputsHash, field: "public_inputs_hash")
        self.verifierKeyId = verifierKeyId
        self.publicInputsHash = publicInputsHash
        self.proof = proof
    }

    public init(verifierBackend: String = OfflineNoteV2Constants.recursiveBackend,
                verifierName: String = OfflineNoteV2Constants.recursiveVerifierName,
                publicInputsHash: Data,
                proofBytes: Data,
                proofBackend: String = OfflineNoteV2Constants.recursiveBackend) throws {
        let verifierKeyId = try VerifyingKeyIdReference(backend: verifierBackend, name: verifierName)
        let proof = try OfflineNoteProofBox(backend: proofBackend, bytes: proofBytes)
        try self.init(verifierKeyId: verifierKeyId, publicInputsHash: publicInputsHash, proof: proof)
    }
}

public struct OfflineNoteKeyCertificatePayloadV2: Equatable, Sendable {
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

    public init(domain: String = OfflineNoteV2Constants.keyCertificatePayloadDomain,
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
        try OfflineNoteV2Validation.validateCertificateCore(
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
        try OfflineNoteV2Encoding.wrap(
            typeName: OfflineNoteV2TypeNames.keyCertificatePayload,
            payload: OfflineNoteV2Encoding.encodeCertificatePayload(self)
        )
    }
}

public struct OfflineNoteKeyCertificateV2: Equatable, Sendable {
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

    public init(version: UInt16 = 2,
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
        try OfflineNoteV2Validation.validateCertificateCore(
            version: version,
            accountId: accountId,
            publicKey: publicKey,
            oneUse: oneUse
        )
        guard issuerSignature.count == 64 else {
            throw OfflineNoteV2Error.invalidIssuerSignatureLength(expected: 64, actual: issuerSignature.count)
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

    public func signingPayload() throws -> OfflineNoteKeyCertificatePayloadV2 {
        try OfflineNoteKeyCertificatePayloadV2(
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
        try OfflineNoteV2Encoding.wrap(
            typeName: OfflineNoteV2TypeNames.keyCertificate,
            payload: OfflineNoteV2Encoding.encodeCertificate(self)
        )
    }
}

public struct OfflineNoteIssueV2: Equatable, Sendable {
    public let noteCommitment: Data
    public let keyCertificate: OfflineNoteKeyCertificateV2
    public let assetId: String
    public let amount: String

    public init(noteCommitment: Data,
                keyCertificate: OfflineNoteKeyCertificateV2,
                assetId: String,
                amount: String) throws {
        try OfflineNoteV2Validation.validateHash(noteCommitment, field: "note_commitment")
        self.noteCommitment = noteCommitment
        self.keyCertificate = keyCertificate
        self.assetId = try OfflineNorito.canonicalAssetIdLiteral(assetId)
        self.amount = try OfflineNorito.parseCanonicalNumeric(amount).canonicalString
    }

    public func issuedClaim() throws -> OfflineNoteIssuedClaimV2 {
        try OfflineNoteIssuedClaimV2.fromIssue(self)
    }

    public func noritoEncoded() throws -> Data {
        try OfflineNoteV2Encoding.wrap(
            typeName: OfflineNoteV2TypeNames.issue,
            payload: OfflineNoteV2Encoding.encodeIssue(self)
        )
    }
}

public struct OfflineNoteIssuedClaimV2: Equatable, Sendable {
    public let domain: String
    public let noteCommitment: Data
    public let keyCertificatePayloadHash: Data
    public let assetId: String
    public let amount: String

    public init(domain: String = OfflineNoteV2Constants.issuedClaimDomain,
                noteCommitment: Data,
                keyCertificatePayloadHash: Data,
                assetId: String,
                amount: String) throws {
        try OfflineNoteV2Validation.validateHash(noteCommitment, field: "note_commitment")
        try OfflineNoteV2Validation.validateHash(
            keyCertificatePayloadHash,
            field: "key_certificate_payload_hash"
        )
        self.domain = domain
        self.noteCommitment = noteCommitment
        self.keyCertificatePayloadHash = keyCertificatePayloadHash
        self.assetId = try OfflineNorito.canonicalAssetIdLiteral(assetId)
        self.amount = try OfflineNorito.parseCanonicalNumeric(amount).canonicalString
    }

    public static func fromIssue(_ issue: OfflineNoteIssueV2) throws -> OfflineNoteIssuedClaimV2 {
        try OfflineNoteIssuedClaimV2(
            noteCommitment: issue.noteCommitment,
            keyCertificatePayloadHash: issue.keyCertificate.payloadHash(),
            assetId: issue.assetId,
            amount: issue.amount
        )
    }

    public static func fromRedemption(_ redemption: OfflineNoteRedeemV2) throws -> OfflineNoteIssuedClaimV2 {
        try OfflineNoteIssuedClaimV2(
            noteCommitment: redemption.sourceNoteCommitment,
            keyCertificatePayloadHash: redemption.senderKeyCertificate.payloadHash(),
            assetId: redemption.assetId,
            amount: redemption.amount
        )
    }

    public static func fromAuditOutput(_ output: OfflineNoteAuditOutputClaimV2) throws -> OfflineNoteIssuedClaimV2 {
        try OfflineNoteIssuedClaimV2(
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
        try OfflineNoteV2Encoding.wrap(
            typeName: OfflineNoteV2TypeNames.issuedClaim,
            payload: OfflineNoteV2Encoding.encodeIssuedClaim(self)
        )
    }
}

public struct OfflineNoteAuditOutputClaimV2: Equatable, Sendable {
    public let noteCommitment: Data
    public let keyCertificate: OfflineNoteKeyCertificateV2
    public let assetId: String
    public let amount: String

    public init(noteCommitment: Data,
                keyCertificate: OfflineNoteKeyCertificateV2,
                assetId: String,
                amount: String) throws {
        try OfflineNoteV2Validation.validateHash(noteCommitment, field: "note_commitment")
        self.noteCommitment = noteCommitment
        self.keyCertificate = keyCertificate
        self.assetId = try OfflineNorito.canonicalAssetIdLiteral(assetId)
        self.amount = try OfflineNorito.parseCanonicalNumeric(amount).canonicalString
    }
}

public struct OfflineNoteRedeemPublicInputsV2: Equatable, Sendable {
    public let domain: String
    public let sourceNoteCommitment: Data
    public let inputNullifiers: [Data]
    public let keyCertificatePayloadHash: Data
    public let recipient: String
    public let assetId: String
    public let amount: String

    public init(domain: String = OfflineNoteV2Constants.redeemPublicInputsDomain,
                sourceNoteCommitment: Data,
                inputNullifiers: [Data],
                keyCertificatePayloadHash: Data,
                recipient: String,
                assetId: String,
                amount: String) throws {
        try OfflineNoteV2Validation.validateHash(sourceNoteCommitment, field: "source_note_commitment")
        try OfflineNoteV2Validation.validateHashes(inputNullifiers, field: "input_nullifiers")
        try OfflineNoteV2Validation.validateHash(
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

    public static func fromRedemption(_ redemption: OfflineNoteRedeemV2) throws -> OfflineNoteRedeemPublicInputsV2 {
        try OfflineNoteRedeemPublicInputsV2(
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
        try OfflineNoteV2Encoding.wrap(
            typeName: OfflineNoteV2TypeNames.redeemPublicInputs,
            payload: OfflineNoteV2Encoding.encodeRedeemPublicInputs(self)
        )
    }
}

public struct OfflineNoteRedeemV2: Equatable, Sendable {
    public let sourceNoteCommitment: Data
    public let inputNullifiers: [Data]
    public let senderKeyCertificate: OfflineNoteKeyCertificateV2
    public let recipient: String
    public let assetId: String
    public let amount: String
    public let recursiveProof: OfflineNoteRecursiveProofV2

    public init(sourceNoteCommitment: Data,
                inputNullifiers: [Data],
                senderKeyCertificate: OfflineNoteKeyCertificateV2,
                recipient: String,
                assetId: String,
                amount: String,
                recursiveProof: OfflineNoteRecursiveProofV2) throws {
        try OfflineNoteV2Validation.validateHash(sourceNoteCommitment, field: "source_note_commitment")
        try OfflineNoteV2Validation.validateHashes(inputNullifiers, field: "input_nullifiers")
        _ = try OfflineNorito.encodeAccountId(recipient)
        self.sourceNoteCommitment = sourceNoteCommitment
        self.inputNullifiers = inputNullifiers
        self.senderKeyCertificate = senderKeyCertificate
        self.recipient = recipient
        self.assetId = try OfflineNorito.canonicalAssetIdLiteral(assetId)
        self.amount = try OfflineNorito.parseCanonicalNumeric(amount).canonicalString
        self.recursiveProof = recursiveProof
    }

    public func issuedClaim() throws -> OfflineNoteIssuedClaimV2 {
        try OfflineNoteIssuedClaimV2.fromRedemption(self)
    }

    public func publicInputs() throws -> OfflineNoteRedeemPublicInputsV2 {
        try OfflineNoteRedeemPublicInputsV2.fromRedemption(self)
    }

    public func publicInputsHash() throws -> Data {
        try publicInputs().publicInputsHash()
    }

    public func validateProofBinding() throws {
        let expected = try publicInputsHash()
        guard recursiveProof.publicInputsHash == expected else {
            throw OfflineNoteV2Error.proofPublicInputsHashMismatch(
                expected: expected.hexLowercased(),
                actual: recursiveProof.publicInputsHash.hexLowercased()
            )
        }
    }

    public func replacingRecursiveProof(_ recursiveProof: OfflineNoteRecursiveProofV2) throws -> OfflineNoteRedeemV2 {
        try OfflineNoteRedeemV2(
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
        try OfflineNoteV2Encoding.wrap(
            typeName: OfflineNoteV2TypeNames.redeem,
            payload: OfflineNoteV2Encoding.encodeRedeem(self)
        )
    }
}

public struct OfflineNoteAuditPublicInputsV2: Equatable, Sendable {
    public let domain: String
    public let tokenId: Data
    public let keyCertificatePayloadHash: Data
    public let inputNullifiers: [Data]
    public let inputClaims: [OfflineNoteIssuedClaimV2]
    public let outputCommitments: [Data]
    public let outputClaims: [OfflineNoteIssuedClaimV2]

    public init(domain: String = OfflineNoteV2Constants.auditPublicInputsDomain,
                tokenId: Data,
                keyCertificatePayloadHash: Data,
                inputNullifiers: [Data],
                inputClaims: [OfflineNoteIssuedClaimV2],
                outputCommitments: [Data],
                outputClaims: [OfflineNoteIssuedClaimV2]) throws {
        try OfflineNoteV2Validation.validateHash(tokenId, field: "token_id")
        try OfflineNoteV2Validation.validateHash(
            keyCertificatePayloadHash,
            field: "key_certificate_payload_hash"
        )
        try OfflineNoteV2Validation.validateHashes(
            inputNullifiers,
            field: "input_nullifiers",
            emptyError: .emptyInputNullifiers
        )
        try OfflineNoteV2Validation.validateHashes(
            outputCommitments,
            field: "output_commitments",
            emptyError: .emptyOutputCommitments
        )
        guard !inputClaims.isEmpty else {
            throw OfflineNoteV2Error.emptyInputClaims
        }
        guard !outputClaims.isEmpty else {
            throw OfflineNoteV2Error.emptyOutputClaims
        }
        self.domain = domain
        self.tokenId = tokenId
        self.keyCertificatePayloadHash = keyCertificatePayloadHash
        self.inputNullifiers = inputNullifiers
        self.inputClaims = inputClaims
        self.outputCommitments = outputCommitments
        self.outputClaims = outputClaims
    }

    public static func fromAudit(_ audit: OfflineNoteAuditBundleV2) throws -> OfflineNoteAuditPublicInputsV2 {
        let outputClaims = try audit.outputClaims.map(OfflineNoteIssuedClaimV2.fromAuditOutput)
        return try OfflineNoteAuditPublicInputsV2(
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
        try OfflineNoteV2Encoding.wrap(
            typeName: OfflineNoteV2TypeNames.auditPublicInputs,
            payload: OfflineNoteV2Encoding.encodeAuditPublicInputs(self)
        )
    }
}

public struct OfflineNoteAuditBundleV2: Equatable, Sendable {
    public let tokenId: Data
    public let senderKeyCertificate: OfflineNoteKeyCertificateV2
    public let inputNullifiers: [Data]
    public let inputClaims: [OfflineNoteIssuedClaimV2]
    public let outputCommitments: [Data]
    public let outputClaims: [OfflineNoteAuditOutputClaimV2]
    public let recursiveProof: OfflineNoteRecursiveProofV2

    public init(tokenId: Data,
                senderKeyCertificate: OfflineNoteKeyCertificateV2,
                inputNullifiers: [Data],
                inputClaims: [OfflineNoteIssuedClaimV2],
                outputCommitments: [Data],
                outputClaims: [OfflineNoteAuditOutputClaimV2],
                recursiveProof: OfflineNoteRecursiveProofV2) throws {
        try OfflineNoteV2Validation.validateHash(tokenId, field: "token_id")
        try OfflineNoteV2Validation.validateHashes(
            inputNullifiers,
            field: "input_nullifiers",
            emptyError: .emptyInputNullifiers
        )
        try OfflineNoteV2Validation.validateHashes(
            outputCommitments,
            field: "output_commitments",
            emptyError: .emptyOutputCommitments
        )
        guard !inputClaims.isEmpty else {
            throw OfflineNoteV2Error.emptyInputClaims
        }
        guard inputClaims.count == inputNullifiers.count else {
            throw OfflineNoteV2Error.auditInputCountMismatch(
                nullifiers: inputNullifiers.count,
                claims: inputClaims.count
            )
        }
        guard !outputClaims.isEmpty else {
            throw OfflineNoteV2Error.emptyOutputClaims
        }
        let committed = Set(outputCommitments.map { $0.hexLowercased() })
        for claim in outputClaims {
            let commitment = claim.noteCommitment.hexLowercased()
            guard committed.contains(commitment) else {
                throw OfflineNoteV2Error.auditOutputClaimNotCommitted(commitment)
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

    public func publicInputs() throws -> OfflineNoteAuditPublicInputsV2 {
        try OfflineNoteAuditPublicInputsV2.fromAudit(self)
    }

    public func publicInputsHash() throws -> Data {
        try publicInputs().publicInputsHash()
    }

    public func validateProofBinding() throws {
        let expected = try publicInputsHash()
        guard recursiveProof.publicInputsHash == expected else {
            throw OfflineNoteV2Error.proofPublicInputsHashMismatch(
                expected: expected.hexLowercased(),
                actual: recursiveProof.publicInputsHash.hexLowercased()
            )
        }
    }

    public func replacingRecursiveProof(_ recursiveProof: OfflineNoteRecursiveProofV2) throws -> OfflineNoteAuditBundleV2 {
        try OfflineNoteAuditBundleV2(
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
        try OfflineNoteV2Encoding.wrap(
            typeName: OfflineNoteV2TypeNames.audit,
            payload: OfflineNoteV2Encoding.encodeAudit(self)
        )
    }
}

public struct IssueOfflineNoteV2Request: Sendable {
    public let chainId: String
    public let authority: String
    public let issue: OfflineNoteIssueV2
    public let ttlMs: UInt64?
    public let nonce: UInt32?

    public init(chainId: String,
                authority: String,
                issue: OfflineNoteIssueV2,
                ttlMs: UInt64? = nil,
                nonce: UInt32? = nil) {
        self.chainId = chainId
        self.authority = authority
        self.issue = issue
        self.ttlMs = ttlMs
        self.nonce = nonce
    }
}

public struct RedeemOfflineNoteV2Request: Sendable {
    public let chainId: String
    public let authority: String
    public let redemption: OfflineNoteRedeemV2
    public let ttlMs: UInt64?
    public let nonce: UInt32?

    public init(chainId: String,
                authority: String,
                redemption: OfflineNoteRedeemV2,
                ttlMs: UInt64? = nil,
                nonce: UInt32? = nil) {
        self.chainId = chainId
        self.authority = authority
        self.redemption = redemption
        self.ttlMs = ttlMs
        self.nonce = nonce
    }
}

public struct AuditOfflineNoteV2Request: Sendable {
    public let chainId: String
    public let authority: String
    public let audit: OfflineNoteAuditBundleV2
    public let ttlMs: UInt64?
    public let nonce: UInt32?

    public init(chainId: String,
                authority: String,
                audit: OfflineNoteAuditBundleV2,
                ttlMs: UInt64? = nil,
                nonce: UInt32? = nil) {
        self.chainId = chainId
        self.authority = authority
        self.audit = audit
        self.ttlMs = ttlMs
        self.nonce = nonce
    }
}

enum OfflineNoteV2TypeNames {
    static let keyCertificate = "iroha_data_model::offline::model::OfflineNoteKeyCertificateV2"
    static let keyCertificatePayload = "iroha_data_model::offline::model::OfflineNoteKeyCertificatePayloadV2"
    static let recursiveProof = "iroha_data_model::offline::model::OfflineNoteRecursiveProofV2"
    static let issue = "iroha_data_model::offline::model::OfflineNoteIssueV2"
    static let issuedClaim = "iroha_data_model::offline::model::OfflineNoteIssuedClaimV2"
    static let auditOutputClaim = "iroha_data_model::offline::model::OfflineNoteAuditOutputClaimV2"
    static let redeem = "iroha_data_model::offline::model::OfflineNoteRedeemV2"
    static let redeemPublicInputs = "iroha_data_model::offline::model::OfflineNoteRedeemPublicInputsV2"
    static let audit = "iroha_data_model::offline::model::OfflineNoteAuditBundleV2"
    static let auditPublicInputs = "iroha_data_model::offline::model::OfflineNoteAuditPublicInputsV2"
    static let issueInstruction = "iroha_data_model::isi::offline::IssueOfflineNoteV2"
    static let redeemInstruction = "iroha_data_model::isi::offline::RedeemOfflineNoteV2"
    static let auditInstruction = "iroha_data_model::isi::offline::AuditOfflineNoteV2"
}

enum OfflineNoteV2Validation {
    static func validateHash(_ value: Data, field: String) throws {
        guard value.count == 32 else {
            throw OfflineNoteV2Error.invalidHashLength(field: field, expected: 32, actual: value.count)
        }
        guard let last = value.last, (last & 1) == 1 else {
            throw OfflineNoteV2Error.invalidHash(field: field)
        }
    }

    static func validateHashes(_ values: [Data],
                               field: String,
                               emptyError: OfflineNoteV2Error = .emptyInputNullifiers) throws {
        guard !values.isEmpty else {
            throw emptyError
        }
        for (index, value) in values.enumerated() {
            try validateHash(value, field: "\(field)[\(index)]")
        }
    }

    static func validateCertificateCore(version: UInt16,
                                        accountId: String,
                                        publicKey: Data,
                                        oneUse: Bool) throws {
        guard version == 2 else {
            throw OfflineNoteV2Error.invalidCertificateVersion(version)
        }
        _ = oneUse
        guard publicKey.count == 32 else {
            throw OfflineNoteV2Error.invalidNotePublicKeyLength(expected: 32, actual: publicKey.count)
        }
        _ = try OfflineNorito.encodeAccountId(accountId)
    }
}

enum OfflineNoteV2Encoding {
    static func wrap(typeName: String, payload: Data) -> Data {
        noritoEncode(typeName: typeName, payload: payload, flags: 2)
    }

    static func encodeCertificatePayload(_ payload: OfflineNoteKeyCertificatePayloadV2) throws -> Data {
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

    static func encodeCertificate(_ certificate: OfflineNoteKeyCertificateV2) throws -> Data {
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

    static func encodeRecursiveProof(_ proof: OfflineNoteRecursiveProofV2) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(encodeVerifyingKeyId(proof.verifierKeyId))
        writer.writeField(try OfflineCompactNorito.encodeHash(proof.publicInputsHash))
        writer.writeField(encodeProofBox(proof.proof))
        return writer.data
    }

    static func encodeIssue(_ issue: OfflineNoteIssueV2) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try OfflineCompactNorito.encodeHash(issue.noteCommitment))
        writer.writeField(try encodeCertificate(issue.keyCertificate))
        writer.writeField(try encodeAssetId(issue.assetId))
        writer.writeField(try encodeNumeric(issue.amount))
        return writer.data
    }

    static func encodeIssuedClaim(_ claim: OfflineNoteIssuedClaimV2) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(claim.domain))
        writer.writeField(try OfflineCompactNorito.encodeHash(claim.noteCommitment))
        writer.writeField(try OfflineCompactNorito.encodeHash(claim.keyCertificatePayloadHash))
        writer.writeField(try encodeAssetId(claim.assetId))
        writer.writeField(try encodeNumeric(claim.amount))
        return writer.data
    }

    static func encodeAuditOutputClaim(_ claim: OfflineNoteAuditOutputClaimV2) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try OfflineCompactNorito.encodeHash(claim.noteCommitment))
        writer.writeField(try encodeCertificate(claim.keyCertificate))
        writer.writeField(try encodeAssetId(claim.assetId))
        writer.writeField(try encodeNumeric(claim.amount))
        return writer.data
    }

    static func encodeRedeemPublicInputs(_ inputs: OfflineNoteRedeemPublicInputsV2) throws -> Data {
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

    static func encodeRedeem(_ redemption: OfflineNoteRedeemV2) throws -> Data {
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

    static func encodeAuditPublicInputs(_ inputs: OfflineNoteAuditPublicInputsV2) throws -> Data {
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

    static func encodeAudit(_ audit: OfflineNoteAuditBundleV2) throws -> Data {
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

    private static func encodeAccountId(_ value: String) throws -> Data {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        let address = try AccountAddress.parseEncodedSwiftOnly(trimmed, expectedPrefix: 0x02F1)
        return try address.compactNoritoAccountControllerPayload()
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
