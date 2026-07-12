import Foundation
import CryptoKit

public enum AttestedOfflineNoteConstants {
    public static let keyCertificatePayloadDomain = "iroha:offline-note:key-certificate-payload"
    public static let issuedClaimDomain = "iroha:offline-note:issued-claim"
    public static let redeemPublicInputsDomain = "iroha:offline-note:redeem-public-inputs"
    public static let auditPublicInputsDomain = "iroha:offline-note:audit-public-inputs"
    public static let deviceAttestationChallengeDomain = "iroha:offline-note:device-attestation-challenge:v1"
    public static let deviceAttestationEvidencePrefix = "offline-device-attestation-evidence-v1"
    public static let recursiveBackend = "halo2/ipa"
    public static let recursiveVerifierName = "offline-note-v2-recursive-v1"
    public static let recursivePublicInputsSchemaV1 = #"{"schema":"offline_note_recursive","public_inputs":["public_inputs_hash_limb0","public_inputs_hash_limb1","public_inputs_hash_limb2","public_inputs_hash_limb3","proof_mode","input_count","output_count","input_amount_sum","output_amount_sum","input_nullifier_sum_limb0","output_commitment_sum_limb0","key_certificate_payload_hash_limb0","source_or_token_limb0","input_claim_hash_sum_limb0","output_claim_hash_sum_limb0","reserved_zero"]}"#
    public static let keyCertificateVersion: UInt16 = 1
    public static let iosPlatform = "ios"
    public static let androidPlatform = "android"
    public static let iosAppAttestPlatform = "ios-appattest"
    public static let iosAppAttestAssertionScheme = "apple-appattest-counter-v1"
    public static let iosAppAttestAssertionKeyAlgorithm = "app-attest-p256"
    public static let androidKeyMintPlatform = "android-keymint"
    public static let androidKeyMintAssertionScheme = "android-keymint-ecdsa-p256-usage-limit-v1"
    public static let androidKeyMintAssertionKeyAlgorithm = "ecdsa-p256-sha256"

    public static var recursivePublicInputsSchemaHash: Data {
        IrohaHash.hash(Data(recursivePublicInputsSchemaV1.utf8))
    }
}

public enum AttestedOfflineNoteError: Error, LocalizedError, Equatable {
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
    case auditInputCountMismatch(nullifiers: Int, claims: Int)
    case auditOutputClaimNotCommitted(String)
    case proofPublicInputsHashMismatch(expected: String, actual: String)
    case deviceAttestationChallengeHashMismatch(expected: String, actual: String)
    case deviceAttestationHashMismatch(field: String)
    case invalidDigestLength(field: String, expected: Int, actual: Int)
    case unsupportedRecursiveProofBackend(expected: String, actual: String)
    case unsupportedDomain(field: String, expected: String, actual: String)
    case unsupportedDeviceAttestationProfile(String)
    case nonCanonicalField(field: String)

    public var errorDescription: String? {
        switch self {
        case let .invalidHashLength(field, expected, actual):
            return "\(field) must be exactly \(expected) bytes (found \(actual))."
        case let .invalidHash(field):
            return "\(field) must be a canonical Iroha hash."
        case let .invalidCertificateVersion(version):
            return "Attested Offline Note key certificate version must be \(AttestedOfflineNoteConstants.keyCertificateVersion) (found \(version))."
        case .certificateMustBeOneUse:
            return "Attested Offline Note key certificate must be marked one-use."
        case let .invalidNotePublicKeyLength(expected, actual):
            return "Attested Offline Note public key must be \(expected) bytes (found \(actual))."
        case let .invalidIssuerSignatureLength(expected, actual):
            return "Attested Offline Note issuer signature must be \(expected) bytes (found \(actual))."
        case .emptyProofBytes:
            return "Attested Offline Note proof bytes must not be empty."
        case .emptyProofBackend:
            return "Attested Offline Note proof backend must not be empty."
        case .emptyInputNullifiers:
            return "Attested Offline Note input nullifiers must not be empty."
        case .emptyInputClaims:
            return "Attested Offline Note audit input claims must not be empty."
        case .emptyOutputCommitments:
            return "Attested Offline Note audit output commitments must not be empty."
        case .emptyOutputClaims:
            return "Attested Offline Note audit output claims must not be empty."
        case let .auditInputCountMismatch(nullifiers, claims):
            return "Attested Offline Note audit input nullifier count \(nullifiers) must match input claim count \(claims)."
        case let .auditOutputClaimNotCommitted(commitment):
            return "Attested Offline Note audit output claim \(commitment) is not listed in output commitments."
        case let .proofPublicInputsHashMismatch(expected, actual):
            return "Attested Offline Note recursive proof public input hash mismatch: expected \(expected), got \(actual)."
        case let .deviceAttestationChallengeHashMismatch(expected, actual):
            return "Attested Offline Note device attestation challenge hash mismatch: expected \(expected), got \(actual)."
        case let .deviceAttestationHashMismatch(field):
            return "Attested Offline Note device attestation \(field) does not match the submitted bytes."
        case let .invalidDigestLength(field, expected, actual):
            return "\(field) must be exactly \(expected) bytes (found \(actual))."
        case let .unsupportedRecursiveProofBackend(expected, actual):
            return "Attested Offline Note recursive proof backend must be \(expected), got \(actual)."
        case let .unsupportedDomain(field, expected, actual):
            return "\(field) must be \(expected), got \(actual)."
        case let .unsupportedDeviceAttestationProfile(reason):
            return "Unsupported Attested Offline Note device attestation profile: \(reason)."
        case let .nonCanonicalField(field):
            return "\(field) must be canonical."
        }
    }
}

public struct OfflineDeviceAttestationRegistration: Equatable, Sendable {
    public let version: UInt16
    public let platform: String
    public let keyId: String
    public let deviceId: String
    public let accountId: String
    public let assetDefinitionId: String?
    public let iosTeamId: String?
    public let iosBundleId: String?
    public let iosEnvironment: String?
    public let androidPackageName: String?
    public let androidSigningCertificateSha256: Data?
    public let publicKey: Data
    public let assertionScheme: String
    public let assertionKeyAlgorithm: String
    public let assertionPublicKey: Data
    public let assertionUsageCountLimit: UInt32?
    public let oneUse: Bool
    public let challengeHash: Data
    public let attestationReportHash: Data
    public let attestationReport: Data
    public let evidenceHash: Data
    public let evidence: Data
    public let recentBlockHeight: UInt64
    public let recentBlockHash: Data
    public let expiresAtMs: UInt64

    public init(version: UInt16 = AttestedOfflineNoteConstants.keyCertificateVersion,
                platform: String,
                keyId: String,
                deviceId: String,
                accountId: String,
                assetDefinitionId: String? = nil,
                iosTeamId: String? = nil,
                iosBundleId: String? = nil,
                iosEnvironment: String? = nil,
                androidPackageName: String? = nil,
                androidSigningCertificateSha256: Data? = nil,
                publicKey: Data,
                assertionScheme: String,
                assertionKeyAlgorithm: String,
                assertionPublicKey: Data,
                assertionUsageCountLimit: UInt32?,
                oneUse: Bool = true,
                challengeHash: Data? = nil,
                attestationReportHash: Data? = nil,
                attestationReport: Data = Data(),
                evidenceHash: Data? = nil,
                evidence: Data = Data(),
                recentBlockHeight: UInt64,
                recentBlockHash: Data,
                expiresAtMs: UInt64) throws {
        try AttestedOfflineNoteValidation.validateCertificateCore(
            version: version,
            accountId: accountId,
            publicKey: publicKey,
            oneUse: oneUse
        )
        try AttestedOfflineNoteValidation.validateAttestationIdentity(keyId: keyId, deviceId: deviceId)
        try AttestedOfflineNoteValidation.validateOptionalAttestationMetadata(
            iosTeamId: iosTeamId,
            iosBundleId: iosBundleId,
            iosEnvironment: iosEnvironment,
            androidPackageName: androidPackageName
        )
        if let assetDefinitionId, AssetDefinitionAddress.decode(assetDefinitionId) == nil {
            throw OfflineNoritoError.invalidAssetId(assetDefinitionId)
        }
        if let androidSigningCertificateSha256,
           androidSigningCertificateSha256.count != 32 {
            throw AttestedOfflineNoteError.invalidDigestLength(
                field: "android_signing_certificate_sha256",
                expected: 32,
                actual: androidSigningCertificateSha256.count
            )
        }
        try Self.validateAttestationProfile(
            platform: platform,
            keyId: keyId,
            assertionScheme: assertionScheme,
            assertionKeyAlgorithm: assertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey,
            assertionUsageCountLimit: assertionUsageCountLimit
        )
        try AttestedOfflineNoteValidation.validateHash(recentBlockHash, field: "recent_block_hash")

        let resolvedChallengeHash = try Self.preAttestationChallengeHash(
            version: version,
            platform: platform,
            keyId: keyId,
            deviceId: deviceId,
            accountId: accountId,
            assetDefinitionId: assetDefinitionId,
            iosTeamId: iosTeamId,
            iosBundleId: iosBundleId,
            iosEnvironment: iosEnvironment,
            androidPackageName: androidPackageName,
            androidSigningCertificateSha256: androidSigningCertificateSha256,
            publicKey: publicKey,
            assertionScheme: assertionScheme,
            assertionKeyAlgorithm: assertionKeyAlgorithm,
            assertionUsageCountLimit: assertionUsageCountLimit,
            oneUse: oneUse,
            recentBlockHeight: recentBlockHeight,
            recentBlockHash: recentBlockHash,
            expiresAtMs: expiresAtMs
        )
        if let challengeHash {
            try AttestedOfflineNoteValidation.validateHash(challengeHash, field: "challenge_hash")
            guard challengeHash == resolvedChallengeHash else {
                throw AttestedOfflineNoteError.deviceAttestationChallengeHashMismatch(
                    expected: resolvedChallengeHash.hexLowercased(),
                    actual: challengeHash.hexLowercased()
                )
            }
        }

        let resolvedAttestationReportHash = attestationReportHash ?? IrohaHash.hash(attestationReport)
        try AttestedOfflineNoteValidation.validateHash(resolvedAttestationReportHash, field: "attestation_report_hash")
        guard resolvedAttestationReportHash == IrohaHash.hash(attestationReport) else {
            throw AttestedOfflineNoteError.deviceAttestationHashMismatch(field: "attestation_report_hash")
        }

        let resolvedEvidence: Data
        if attestationReport.isEmpty, evidence.isEmpty, evidenceHash == nil {
            resolvedEvidence = AttestedOfflineNoteValidation.attestationEvidenceEnvelope(
                attestationReportHash: resolvedAttestationReportHash
            )
        } else {
            resolvedEvidence = evidence
        }
        try AttestedOfflineNoteValidation.validateAttestationEvidenceEnvelope(
            resolvedEvidence,
            attestationReportHash: resolvedAttestationReportHash
        )
        let expectedEvidenceHash = IrohaHash.hash(resolvedEvidence)
        let resolvedEvidenceHash = evidenceHash ?? expectedEvidenceHash
        try AttestedOfflineNoteValidation.validateHash(resolvedEvidenceHash, field: "evidence_hash")
        guard resolvedEvidenceHash == expectedEvidenceHash else {
            throw AttestedOfflineNoteError.deviceAttestationHashMismatch(field: "evidence_hash")
        }

        self.version = version
        self.platform = platform
        self.keyId = keyId
        self.deviceId = deviceId
        self.accountId = accountId
        self.assetDefinitionId = assetDefinitionId
        self.iosTeamId = iosTeamId
        self.iosBundleId = iosBundleId
        self.iosEnvironment = iosEnvironment
        self.androidPackageName = androidPackageName
        self.androidSigningCertificateSha256 = androidSigningCertificateSha256
        self.publicKey = publicKey
        self.assertionScheme = assertionScheme
        self.assertionKeyAlgorithm = assertionKeyAlgorithm
        self.assertionPublicKey = assertionPublicKey
        self.assertionUsageCountLimit = assertionUsageCountLimit
        self.oneUse = oneUse
        self.challengeHash = resolvedChallengeHash
        self.attestationReportHash = resolvedAttestationReportHash
        self.attestationReport = attestationReport
        self.evidenceHash = resolvedEvidenceHash
        self.evidence = resolvedEvidence
        self.recentBlockHeight = recentBlockHeight
        self.recentBlockHash = recentBlockHash
        self.expiresAtMs = expiresAtMs
    }

    private static func validateAttestationProfile(platform: String,
                                                   keyId: String,
                                                   assertionScheme: String,
                                                   assertionKeyAlgorithm: String,
                                                   assertionPublicKey: Data,
                                                   assertionUsageCountLimit: UInt32?) throws {
        try AttestedOfflineNoteValidation.validateDeviceAttestationProfile(
            platform: platform,
            keyId: keyId,
            assertionScheme: assertionScheme,
            assertionKeyAlgorithm: assertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey,
            assertionUsageCountLimit: assertionUsageCountLimit
        )
    }

    /// Build the canonical platform challenge before its assertion public key is available.
    ///
    /// Android uses a separate Norito schema that also excludes `keyId`, because
    /// KeyMint generates the public key from which that identifier is derived
    /// while processing this challenge. Chain admission still binds the returned
    /// certificate key to the final lowercase SHA-256 key id.
    public static func preAttestationChallengeHash(
        version: UInt16 = AttestedOfflineNoteConstants.keyCertificateVersion,
        platform: String,
        keyId: String,
        deviceId: String,
        accountId: String,
        assetDefinitionId: String? = nil,
        iosTeamId: String? = nil,
        iosBundleId: String? = nil,
        iosEnvironment: String? = nil,
        androidPackageName: String? = nil,
        androidSigningCertificateSha256: Data? = nil,
        publicKey: Data,
        assertionScheme: String,
        assertionKeyAlgorithm: String,
        assertionUsageCountLimit: UInt32?,
        oneUse: Bool = true,
        recentBlockHeight: UInt64,
        recentBlockHash: Data,
        expiresAtMs: UInt64
    ) throws -> Data {
        try AttestedOfflineNoteValidation.validateCertificateCore(
            version: version,
            accountId: accountId,
            publicKey: publicKey,
            oneUse: oneUse
        )
        try AttestedOfflineNoteValidation.validateAttestationIdentity(keyId: keyId, deviceId: deviceId)
        try AttestedOfflineNoteValidation.validateOptionalAttestationMetadata(
            iosTeamId: iosTeamId,
            iosBundleId: iosBundleId,
            iosEnvironment: iosEnvironment,
            androidPackageName: androidPackageName
        )
        if let assetDefinitionId, AssetDefinitionAddress.decode(assetDefinitionId) == nil {
            throw OfflineNoritoError.invalidAssetId(assetDefinitionId)
        }
        if let androidSigningCertificateSha256,
           androidSigningCertificateSha256.count != 32 {
            throw AttestedOfflineNoteError.invalidDigestLength(
                field: "android_signing_certificate_sha256",
                expected: 32,
                actual: androidSigningCertificateSha256.count
            )
        }
        guard !assertionScheme.isEmpty,
              assertionScheme == assertionScheme.trimmingCharacters(in: .whitespacesAndNewlines),
              !assertionKeyAlgorithm.isEmpty,
              assertionKeyAlgorithm == assertionKeyAlgorithm.trimmingCharacters(in: .whitespacesAndNewlines) else {
            throw AttestedOfflineNoteError.nonCanonicalField(field: "assertion_profile")
        }
        try AttestedOfflineNoteValidation.validateHash(recentBlockHash, field: "recent_block_hash")
        return try computeChallengeHash(
            version: version,
            platform: platform,
            keyId: keyId,
            deviceId: deviceId,
            accountId: accountId,
            assetDefinitionId: assetDefinitionId,
            iosTeamId: iosTeamId,
            iosBundleId: iosBundleId,
            iosEnvironment: iosEnvironment,
            androidPackageName: androidPackageName,
            androidSigningCertificateSha256: androidSigningCertificateSha256,
            publicKey: publicKey,
            assertionScheme: assertionScheme,
            assertionKeyAlgorithm: assertionKeyAlgorithm,
            assertionUsageCountLimit: assertionUsageCountLimit,
            oneUse: oneUse,
            recentBlockHeight: recentBlockHeight,
            recentBlockHash: recentBlockHash,
            expiresAtMs: expiresAtMs
        )
    }

    /// Build the Android KeyMint challenge before KeyMint generates the attested key.
    ///
    /// This canonical preimage has no key id or assertion public key. Final
    /// registration validation derives and checks both values from the returned
    /// certificate chain.
    public static func androidPreKeyGenerationChallengeHash(
        version: UInt16 = AttestedOfflineNoteConstants.keyCertificateVersion,
        deviceId: String,
        accountId: String,
        assetDefinitionId: String? = nil,
        iosTeamId: String? = nil,
        iosBundleId: String? = nil,
        iosEnvironment: String? = nil,
        androidPackageName: String,
        androidSigningCertificateSha256: Data,
        publicKey: Data,
        assertionScheme: String = AttestedOfflineNoteConstants.androidKeyMintAssertionScheme,
        assertionKeyAlgorithm: String = AttestedOfflineNoteConstants.androidKeyMintAssertionKeyAlgorithm,
        assertionUsageCountLimit: UInt32? = 1,
        oneUse: Bool = true,
        recentBlockHeight: UInt64,
        recentBlockHash: Data,
        expiresAtMs: UInt64
    ) throws -> Data {
        try AttestedOfflineNoteValidation.validateCertificateCore(
            version: version,
            accountId: accountId,
            publicKey: publicKey,
            oneUse: oneUse
        )
        try AttestedOfflineNoteValidation.validateAttestationDeviceId(deviceId)
        try AttestedOfflineNoteValidation.validateOptionalAttestationMetadata(
            iosTeamId: iosTeamId,
            iosBundleId: iosBundleId,
            iosEnvironment: iosEnvironment,
            androidPackageName: androidPackageName
        )
        if let assetDefinitionId, AssetDefinitionAddress.decode(assetDefinitionId) == nil {
            throw OfflineNoritoError.invalidAssetId(assetDefinitionId)
        }
        guard androidSigningCertificateSha256.count == 32 else {
            throw AttestedOfflineNoteError.invalidDigestLength(
                field: "android_signing_certificate_sha256",
                expected: 32,
                actual: androidSigningCertificateSha256.count
            )
        }
        guard assertionScheme == AttestedOfflineNoteConstants.androidKeyMintAssertionScheme,
              assertionKeyAlgorithm == AttestedOfflineNoteConstants.androidKeyMintAssertionKeyAlgorithm,
              assertionUsageCountLimit == 1 else {
            throw AttestedOfflineNoteError.unsupportedDeviceAttestationProfile(
                "Android KeyMint pre-key challenge requires the canonical one-use P-256 assertion profile"
            )
        }
        try AttestedOfflineNoteValidation.validateHash(recentBlockHash, field: "recent_block_hash")
        return try computeAndroidKeyMintChallengeHash(
            version: version,
            deviceId: deviceId,
            accountId: accountId,
            assetDefinitionId: assetDefinitionId,
            iosTeamId: iosTeamId,
            iosBundleId: iosBundleId,
            iosEnvironment: iosEnvironment,
            androidPackageName: androidPackageName,
            androidSigningCertificateSha256: androidSigningCertificateSha256,
            publicKey: publicKey,
            assertionScheme: assertionScheme,
            assertionKeyAlgorithm: assertionKeyAlgorithm,
            assertionUsageCountLimit: assertionUsageCountLimit,
            oneUse: oneUse,
            recentBlockHeight: recentBlockHeight,
            recentBlockHash: recentBlockHash,
            expiresAtMs: expiresAtMs
        )
    }

    public func canonicalChallengeHash() throws -> Data {
        try Self.preAttestationChallengeHash(
            version: version,
            platform: platform,
            keyId: keyId,
            deviceId: deviceId,
            accountId: accountId,
            assetDefinitionId: assetDefinitionId,
            iosTeamId: iosTeamId,
            iosBundleId: iosBundleId,
            iosEnvironment: iosEnvironment,
            androidPackageName: androidPackageName,
            androidSigningCertificateSha256: androidSigningCertificateSha256,
            publicKey: publicKey,
            assertionScheme: assertionScheme,
            assertionKeyAlgorithm: assertionKeyAlgorithm,
            assertionUsageCountLimit: assertionUsageCountLimit,
            oneUse: oneUse,
            recentBlockHeight: recentBlockHeight,
            recentBlockHash: recentBlockHash,
            expiresAtMs: expiresAtMs
        )
    }

    public func replacingAttestationEvidence(attestationReport: Data,
                                             evidence: Data,
                                             challengeHash: Data? = nil) throws -> OfflineDeviceAttestationRegistration {
        try OfflineDeviceAttestationRegistration(
            version: version,
            platform: platform,
            keyId: keyId,
            deviceId: deviceId,
            accountId: accountId,
            assetDefinitionId: assetDefinitionId,
            iosTeamId: iosTeamId,
            iosBundleId: iosBundleId,
            iosEnvironment: iosEnvironment,
            androidPackageName: androidPackageName,
            androidSigningCertificateSha256: androidSigningCertificateSha256,
            publicKey: publicKey,
            assertionScheme: assertionScheme,
            assertionKeyAlgorithm: assertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey,
            assertionUsageCountLimit: assertionUsageCountLimit,
            oneUse: oneUse,
            challengeHash: challengeHash ?? self.challengeHash,
            attestationReport: attestationReport,
            evidence: evidence,
            recentBlockHeight: recentBlockHeight,
            recentBlockHash: recentBlockHash,
            expiresAtMs: expiresAtMs
        )
    }

    public func keyCertificatePayload() throws -> AttestedOfflineNoteKeyCertificatePayload {
        try AttestedOfflineNoteKeyCertificatePayload(
            version: AttestedOfflineNoteConstants.keyCertificateVersion,
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

    public func keyCertificatePayloadHash() throws -> Data {
        IrohaHash.hash(try keyCertificatePayload().noritoEncoded())
    }

    public func noritoEncoded() throws -> Data {
        try AttestedOfflineNoteEncoding.wrap(
            typeName: AttestedOfflineNoteTypeNames.deviceAttestationRegistration,
            payload: AttestedOfflineNoteEncoding.encodeDeviceAttestationRegistration(self)
        )
    }

    private static func computeChallengeHash(version: UInt16,
                                             platform: String,
                                             keyId: String,
                                             deviceId: String,
                                             accountId: String,
                                             assetDefinitionId: String?,
                                             iosTeamId: String?,
                                             iosBundleId: String?,
                                             iosEnvironment: String?,
                                             androidPackageName: String?,
                                             androidSigningCertificateSha256: Data?,
                                             publicKey: Data,
                                             assertionScheme: String,
                                             assertionKeyAlgorithm: String,
                                             assertionUsageCountLimit: UInt32?,
                                             oneUse: Bool,
                                             recentBlockHeight: UInt64,
                                             recentBlockHash: Data,
                                             expiresAtMs: UInt64) throws -> Data {
        if platform == AttestedOfflineNoteConstants.androidKeyMintPlatform {
            return try computeAndroidKeyMintChallengeHash(
                version: version,
                deviceId: deviceId,
                accountId: accountId,
                assetDefinitionId: assetDefinitionId,
                iosTeamId: iosTeamId,
                iosBundleId: iosBundleId,
                iosEnvironment: iosEnvironment,
                androidPackageName: androidPackageName,
                androidSigningCertificateSha256: androidSigningCertificateSha256,
                publicKey: publicKey,
                assertionScheme: assertionScheme,
                assertionKeyAlgorithm: assertionKeyAlgorithm,
                assertionUsageCountLimit: assertionUsageCountLimit,
                oneUse: oneUse,
                recentBlockHeight: recentBlockHeight,
                recentBlockHash: recentBlockHash,
                expiresAtMs: expiresAtMs
            )
        }
        let preimage = OfflineDeviceAttestationChallengePreimage(
            version: version,
            platform: platform,
            keyId: keyId,
            deviceId: deviceId,
            accountId: accountId,
            assetDefinitionId: assetDefinitionId,
            iosTeamId: iosTeamId,
            iosBundleId: iosBundleId,
            iosEnvironment: iosEnvironment,
            androidPackageName: androidPackageName,
            androidSigningCertificateSha256: androidSigningCertificateSha256,
            publicKey: publicKey,
            assertionScheme: assertionScheme,
            assertionKeyAlgorithm: assertionKeyAlgorithm,
            assertionUsageCountLimit: assertionUsageCountLimit,
            oneUse: oneUse,
            recentBlockHeight: recentBlockHeight,
            recentBlockHash: recentBlockHash,
            expiresAtMs: expiresAtMs
        )
        return IrohaHash.hash(try preimage.noritoEncoded())
    }

    private static func computeAndroidKeyMintChallengeHash(
        version: UInt16,
        deviceId: String,
        accountId: String,
        assetDefinitionId: String?,
        iosTeamId: String?,
        iosBundleId: String?,
        iosEnvironment: String?,
        androidPackageName: String?,
        androidSigningCertificateSha256: Data?,
        publicKey: Data,
        assertionScheme: String,
        assertionKeyAlgorithm: String,
        assertionUsageCountLimit: UInt32?,
        oneUse: Bool,
        recentBlockHeight: UInt64,
        recentBlockHash: Data,
        expiresAtMs: UInt64
    ) throws -> Data {
        let preimage = OfflineAndroidKeyMintChallengePreimage(
            version: version,
            deviceId: deviceId,
            accountId: accountId,
            assetDefinitionId: assetDefinitionId,
            iosTeamId: iosTeamId,
            iosBundleId: iosBundleId,
            iosEnvironment: iosEnvironment,
            androidPackageName: androidPackageName,
            androidSigningCertificateSha256: androidSigningCertificateSha256,
            publicKey: publicKey,
            assertionScheme: assertionScheme,
            assertionKeyAlgorithm: assertionKeyAlgorithm,
            assertionUsageCountLimit: assertionUsageCountLimit,
            oneUse: oneUse,
            recentBlockHeight: recentBlockHeight,
            recentBlockHash: recentBlockHash,
            expiresAtMs: expiresAtMs
        )
        return IrohaHash.hash(try preimage.noritoEncoded())
    }
}

fileprivate struct OfflineDeviceAttestationChallengePreimage {
    let version: UInt16
    let platform: String
    let keyId: String
    let deviceId: String
    let accountId: String
    let assetDefinitionId: String?
    let iosTeamId: String?
    let iosBundleId: String?
    let iosEnvironment: String?
    let androidPackageName: String?
    let androidSigningCertificateSha256: Data?
    let publicKey: Data
    let assertionScheme: String
    let assertionKeyAlgorithm: String
    let assertionUsageCountLimit: UInt32?
    let oneUse: Bool
    let recentBlockHeight: UInt64
    let recentBlockHash: Data
    let expiresAtMs: UInt64

    func noritoEncoded() throws -> Data {
        try AttestedOfflineNoteEncoding.wrap(
            typeName: AttestedOfflineNoteTypeNames.deviceAttestationChallengePreimage,
            payload: AttestedOfflineNoteEncoding.encodeDeviceAttestationChallengePreimage(self)
        )
    }
}

fileprivate struct OfflineAndroidKeyMintChallengePreimage {
    let version: UInt16
    let deviceId: String
    let accountId: String
    let assetDefinitionId: String?
    let iosTeamId: String?
    let iosBundleId: String?
    let iosEnvironment: String?
    let androidPackageName: String?
    let androidSigningCertificateSha256: Data?
    let publicKey: Data
    let assertionScheme: String
    let assertionKeyAlgorithm: String
    let assertionUsageCountLimit: UInt32?
    let oneUse: Bool
    let recentBlockHeight: UInt64
    let recentBlockHash: Data
    let expiresAtMs: UInt64

    func noritoEncoded() throws -> Data {
        try AttestedOfflineNoteEncoding.wrap(
            typeName: AttestedOfflineNoteTypeNames.androidKeyMintChallengePreimage,
            payload: AttestedOfflineNoteEncoding.encodeAndroidKeyMintChallengePreimage(self)
        )
    }
}

public struct AttestedOfflineNoteProofBox: Equatable, Sendable {
    public let backend: String
    public let bytes: Data

    public init(backend: String, bytes: Data) throws {
        let trimmedBackend = backend.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedBackend.isEmpty else {
            throw AttestedOfflineNoteError.emptyProofBackend
        }
        guard trimmedBackend == backend else {
            throw AttestedOfflineNoteError.unsupportedRecursiveProofBackend(
                expected: AttestedOfflineNoteConstants.recursiveBackend,
                actual: backend
            )
        }
        guard !bytes.isEmpty else {
            throw AttestedOfflineNoteError.emptyProofBytes
        }
        self.backend = backend
        self.bytes = bytes
    }
}

public struct AttestedOfflineNoteRecursiveProof: Equatable, Sendable {
    public let verifierKeyId: VerifyingKeyIdReference
    public let publicInputsHash: Data
    public let proof: AttestedOfflineNoteProofBox

    public init(verifierKeyId: VerifyingKeyIdReference,
                publicInputsHash: Data,
                proof: AttestedOfflineNoteProofBox) throws {
        try AttestedOfflineNoteValidation.validateHash(publicInputsHash, field: "public_inputs_hash")
        self.verifierKeyId = verifierKeyId
        self.publicInputsHash = publicInputsHash
        self.proof = proof
    }

    public init(verifierBackend: String = AttestedOfflineNoteConstants.recursiveBackend,
                verifierName: String = AttestedOfflineNoteConstants.recursiveVerifierName,
                publicInputsHash: Data,
                proofBytes: Data,
                proofBackend: String = AttestedOfflineNoteConstants.recursiveBackend) throws {
        let verifierKeyId = try VerifyingKeyIdReference(backend: verifierBackend, name: verifierName)
        let proof = try AttestedOfflineNoteProofBox(backend: proofBackend, bytes: proofBytes)
        try self.init(verifierKeyId: verifierKeyId, publicInputsHash: publicInputsHash, proof: proof)
    }

    public func noritoEncoded() throws -> Data {
        try AttestedOfflineNoteEncoding.wrap(
            typeName: AttestedOfflineNoteTypeNames.recursiveProof,
            payload: AttestedOfflineNoteEncoding.encodeRecursiveProof(self)
        )
    }
}

public struct AttestedOfflineNoteKeyCertificatePayload: Equatable, Sendable {
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

    public init(domain: String = AttestedOfflineNoteConstants.keyCertificatePayloadDomain,
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
        try AttestedOfflineNoteValidation.validateDomain(
            domain,
            expected: AttestedOfflineNoteConstants.keyCertificatePayloadDomain,
            field: "domain"
        )
        try AttestedOfflineNoteValidation.validateCertificateCore(
            version: version,
            accountId: accountId,
            publicKey: publicKey,
            oneUse: oneUse
        )
        try AttestedOfflineNoteValidation.validateAttestationIdentity(keyId: keyId, deviceId: deviceId)
        try AttestedOfflineNoteValidation.validateKeyCertificateProfile(
            platform: platform,
            keyId: keyId,
            assertionScheme: assertionScheme,
            assertionKeyAlgorithm: assertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey,
            assertionUsageCountLimit: assertionUsageCountLimit
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
        try AttestedOfflineNoteEncoding.wrap(
            typeName: AttestedOfflineNoteTypeNames.keyCertificatePayload,
            payload: AttestedOfflineNoteEncoding.encodeCertificatePayload(self)
        )
    }
}

public struct AttestedOfflineNoteKeyCertificate: Equatable, Sendable {
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

    public init(version: UInt16 = AttestedOfflineNoteConstants.keyCertificateVersion,
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
        try AttestedOfflineNoteValidation.validateCertificateCore(
            version: version,
            accountId: accountId,
            publicKey: publicKey,
            oneUse: oneUse
        )
        try AttestedOfflineNoteValidation.validateAttestationIdentity(keyId: keyId, deviceId: deviceId)
        try AttestedOfflineNoteValidation.validateKeyCertificateProfile(
            platform: platform,
            keyId: keyId,
            assertionScheme: assertionScheme,
            assertionKeyAlgorithm: assertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey,
            assertionUsageCountLimit: assertionUsageCountLimit
        )
        guard issuerSignature.count == 64 else {
            throw AttestedOfflineNoteError.invalidIssuerSignatureLength(expected: 64, actual: issuerSignature.count)
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

    public func signingPayload() throws -> AttestedOfflineNoteKeyCertificatePayload {
        try AttestedOfflineNoteKeyCertificatePayload(
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
        try AttestedOfflineNoteEncoding.wrap(
            typeName: AttestedOfflineNoteTypeNames.keyCertificate,
            payload: AttestedOfflineNoteEncoding.encodeCertificate(self)
        )
    }
}

public struct AttestedOfflineNoteIssue: Equatable, Sendable {
    public let noteCommitment: Data
    public let keyCertificate: AttestedOfflineNoteKeyCertificate
    public let assetId: String
    public let amount: String

    public init(noteCommitment: Data,
                keyCertificate: AttestedOfflineNoteKeyCertificate,
                assetId: String,
                amount: String) throws {
        try AttestedOfflineNoteValidation.validateHash(noteCommitment, field: "note_commitment")
        self.noteCommitment = noteCommitment
        self.keyCertificate = keyCertificate
        self.assetId = try OfflineNorito.canonicalAssetIdLiteral(assetId)
        self.amount = try OfflineNorito.parseCanonicalNumeric(amount).canonicalString
    }

    public func issuedClaim() throws -> AttestedOfflineNoteIssuedClaim {
        try AttestedOfflineNoteIssuedClaim.fromIssue(self)
    }

    public func noritoEncoded() throws -> Data {
        try AttestedOfflineNoteEncoding.wrap(
            typeName: AttestedOfflineNoteTypeNames.issue,
            payload: AttestedOfflineNoteEncoding.encodeIssue(self)
        )
    }
}

public struct AttestedOfflineNoteIssuedClaim: Equatable, Sendable {
    public let domain: String
    public let noteCommitment: Data
    public let keyCertificatePayloadHash: Data
    public let assetId: String
    public let amount: String

    public init(domain: String = AttestedOfflineNoteConstants.issuedClaimDomain,
                noteCommitment: Data,
                keyCertificatePayloadHash: Data,
                assetId: String,
                amount: String) throws {
        try AttestedOfflineNoteValidation.validateDomain(
            domain,
            expected: AttestedOfflineNoteConstants.issuedClaimDomain,
            field: "domain"
        )
        try AttestedOfflineNoteValidation.validateHash(noteCommitment, field: "note_commitment")
        try AttestedOfflineNoteValidation.validateHash(
            keyCertificatePayloadHash,
            field: "key_certificate_payload_hash"
        )
        self.domain = domain
        self.noteCommitment = noteCommitment
        self.keyCertificatePayloadHash = keyCertificatePayloadHash
        let canonicalAssetId = try OfflineNorito.canonicalAssetIdLiteral(assetId)
        guard assetId == canonicalAssetId else {
            throw AttestedOfflineNoteError.nonCanonicalField(field: "asset_id")
        }
        let canonicalAmount = try OfflineNorito.parseCanonicalNumeric(amount).canonicalString
        guard amount == canonicalAmount else {
            throw AttestedOfflineNoteError.nonCanonicalField(field: "amount")
        }
        self.assetId = canonicalAssetId
        self.amount = canonicalAmount
    }

    public static func fromIssue(_ issue: AttestedOfflineNoteIssue) throws -> AttestedOfflineNoteIssuedClaim {
        try AttestedOfflineNoteIssuedClaim(
            noteCommitment: issue.noteCommitment,
            keyCertificatePayloadHash: issue.keyCertificate.payloadHash(),
            assetId: issue.assetId,
            amount: issue.amount
        )
    }

    public static func fromRedemption(_ redemption: AttestedOfflineNoteRedeem) throws -> AttestedOfflineNoteIssuedClaim {
        try AttestedOfflineNoteIssuedClaim(
            noteCommitment: redemption.sourceNoteCommitment,
            keyCertificatePayloadHash: redemption.senderKeyCertificate.payloadHash(),
            assetId: redemption.assetId,
            amount: redemption.amount
        )
    }

    public static func fromAuditOutput(_ output: AttestedOfflineNoteAuditOutputClaim) throws -> AttestedOfflineNoteIssuedClaim {
        try AttestedOfflineNoteIssuedClaim(
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
        try AttestedOfflineNoteEncoding.wrap(
            typeName: AttestedOfflineNoteTypeNames.issuedClaim,
            payload: AttestedOfflineNoteEncoding.encodeIssuedClaim(self)
        )
    }
}

public struct AttestedOfflineNoteAuditOutputClaim: Equatable, Sendable {
    public let noteCommitment: Data
    public let keyCertificate: AttestedOfflineNoteKeyCertificate
    public let assetId: String
    public let amount: String

    public init(noteCommitment: Data,
                keyCertificate: AttestedOfflineNoteKeyCertificate,
                assetId: String,
                amount: String) throws {
        try AttestedOfflineNoteValidation.validateHash(noteCommitment, field: "note_commitment")
        self.noteCommitment = noteCommitment
        self.keyCertificate = keyCertificate
        self.assetId = try OfflineNorito.canonicalAssetIdLiteral(assetId)
        self.amount = try OfflineNorito.parseCanonicalNumeric(amount).canonicalString
    }

    public func noritoEncoded() throws -> Data {
        try AttestedOfflineNoteEncoding.wrap(
            typeName: AttestedOfflineNoteTypeNames.auditOutputClaim,
            payload: AttestedOfflineNoteEncoding.encodeAuditOutputClaim(self)
        )
    }
}

public struct AttestedOfflineNoteRedeemPublicInputs: Equatable, Sendable {
    public let domain: String
    public let sourceNoteCommitment: Data
    public let inputNullifiers: [Data]
    public let keyCertificatePayloadHash: Data
    public let recipient: String
    public let assetId: String
    public let amount: String

    public init(domain: String = AttestedOfflineNoteConstants.redeemPublicInputsDomain,
                sourceNoteCommitment: Data,
                inputNullifiers: [Data],
                keyCertificatePayloadHash: Data,
                recipient: String,
                assetId: String,
                amount: String) throws {
        try AttestedOfflineNoteValidation.validateDomain(
            domain,
            expected: AttestedOfflineNoteConstants.redeemPublicInputsDomain,
            field: "domain"
        )
        try AttestedOfflineNoteValidation.validateHash(sourceNoteCommitment, field: "source_note_commitment")
        try AttestedOfflineNoteValidation.validateHashes(inputNullifiers, field: "input_nullifiers")
        try AttestedOfflineNoteValidation.validateHash(
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

    public static func fromRedemption(_ redemption: AttestedOfflineNoteRedeem) throws -> AttestedOfflineNoteRedeemPublicInputs {
        try AttestedOfflineNoteRedeemPublicInputs(
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
        try AttestedOfflineNoteEncoding.wrap(
            typeName: AttestedOfflineNoteTypeNames.redeemPublicInputs,
            payload: AttestedOfflineNoteEncoding.encodeRedeemPublicInputs(self)
        )
    }
}

public struct AttestedOfflineNoteRedeem: Equatable, Sendable {
    public let sourceNoteCommitment: Data
    public let inputNullifiers: [Data]
    public let senderKeyCertificate: AttestedOfflineNoteKeyCertificate
    public let recipient: String
    public let assetId: String
    public let amount: String
    public let recursiveProof: AttestedOfflineNoteRecursiveProof

    public init(sourceNoteCommitment: Data,
                inputNullifiers: [Data],
                senderKeyCertificate: AttestedOfflineNoteKeyCertificate,
                recipient: String,
                assetId: String,
                amount: String,
                recursiveProof: AttestedOfflineNoteRecursiveProof) throws {
        try AttestedOfflineNoteValidation.validateHash(sourceNoteCommitment, field: "source_note_commitment")
        try AttestedOfflineNoteValidation.validateHashes(inputNullifiers, field: "input_nullifiers")
        _ = try OfflineNorito.encodeAccountId(recipient)
        self.sourceNoteCommitment = sourceNoteCommitment
        self.inputNullifiers = inputNullifiers
        self.senderKeyCertificate = senderKeyCertificate
        self.recipient = recipient
        self.assetId = try OfflineNorito.canonicalAssetIdLiteral(assetId)
        self.amount = try OfflineNorito.parseCanonicalNumeric(amount).canonicalString
        self.recursiveProof = recursiveProof
    }

    public func issuedClaim() throws -> AttestedOfflineNoteIssuedClaim {
        try AttestedOfflineNoteIssuedClaim.fromRedemption(self)
    }

    public func publicInputs() throws -> AttestedOfflineNoteRedeemPublicInputs {
        try AttestedOfflineNoteRedeemPublicInputs.fromRedemption(self)
    }

    public func publicInputsHash() throws -> Data {
        try publicInputs().publicInputsHash()
    }

    public func validateProofBinding() throws {
        let expected = try publicInputsHash()
        guard recursiveProof.publicInputsHash == expected else {
            throw AttestedOfflineNoteError.proofPublicInputsHashMismatch(
                expected: expected.hexLowercased(),
                actual: recursiveProof.publicInputsHash.hexLowercased()
            )
        }
    }

    public func replacingRecursiveProof(_ recursiveProof: AttestedOfflineNoteRecursiveProof) throws -> AttestedOfflineNoteRedeem {
        try AttestedOfflineNoteRedeem(
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
        try AttestedOfflineNoteEncoding.wrap(
            typeName: AttestedOfflineNoteTypeNames.redeem,
            payload: AttestedOfflineNoteEncoding.encodeRedeem(self)
        )
    }
}

public struct AttestedOfflineNoteAuditPublicInputs: Equatable, Sendable {
    public let domain: String
    public let tokenId: Data
    public let keyCertificatePayloadHash: Data
    public let inputNullifiers: [Data]
    public let inputClaims: [AttestedOfflineNoteIssuedClaim]
    public let outputCommitments: [Data]
    public let outputClaims: [AttestedOfflineNoteIssuedClaim]

    public init(domain: String = AttestedOfflineNoteConstants.auditPublicInputsDomain,
                tokenId: Data,
                keyCertificatePayloadHash: Data,
                inputNullifiers: [Data],
                inputClaims: [AttestedOfflineNoteIssuedClaim],
                outputCommitments: [Data],
                outputClaims: [AttestedOfflineNoteIssuedClaim]) throws {
        try AttestedOfflineNoteValidation.validateDomain(
            domain,
            expected: AttestedOfflineNoteConstants.auditPublicInputsDomain,
            field: "domain"
        )
        try AttestedOfflineNoteValidation.validateHash(tokenId, field: "token_id")
        try AttestedOfflineNoteValidation.validateHash(
            keyCertificatePayloadHash,
            field: "key_certificate_payload_hash"
        )
        try AttestedOfflineNoteValidation.validateHashes(
            inputNullifiers,
            field: "input_nullifiers",
            emptyError: .emptyInputNullifiers
        )
        try AttestedOfflineNoteValidation.validateHashes(
            outputCommitments,
            field: "output_commitments",
            emptyError: .emptyOutputCommitments
        )
        guard !inputClaims.isEmpty else {
            throw AttestedOfflineNoteError.emptyInputClaims
        }
        guard !outputClaims.isEmpty else {
            throw AttestedOfflineNoteError.emptyOutputClaims
        }
        self.domain = domain
        self.tokenId = tokenId
        self.keyCertificatePayloadHash = keyCertificatePayloadHash
        self.inputNullifiers = inputNullifiers
        self.inputClaims = inputClaims
        self.outputCommitments = outputCommitments
        self.outputClaims = outputClaims
    }

    public static func fromAudit(_ audit: AttestedOfflineNoteAuditBundle) throws -> AttestedOfflineNoteAuditPublicInputs {
        let outputClaims = try audit.outputClaims.map(AttestedOfflineNoteIssuedClaim.fromAuditOutput)
        return try AttestedOfflineNoteAuditPublicInputs(
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
        try AttestedOfflineNoteEncoding.wrap(
            typeName: AttestedOfflineNoteTypeNames.auditPublicInputs,
            payload: AttestedOfflineNoteEncoding.encodeAuditPublicInputs(self)
        )
    }
}

public struct AttestedOfflineNoteAuditBundle: Equatable, Sendable {
    public let tokenId: Data
    public let senderKeyCertificate: AttestedOfflineNoteKeyCertificate
    public let inputNullifiers: [Data]
    public let inputClaims: [AttestedOfflineNoteIssuedClaim]
    public let outputCommitments: [Data]
    public let outputClaims: [AttestedOfflineNoteAuditOutputClaim]
    public let recursiveProof: AttestedOfflineNoteRecursiveProof

    public init(tokenId: Data,
                senderKeyCertificate: AttestedOfflineNoteKeyCertificate,
                inputNullifiers: [Data],
                inputClaims: [AttestedOfflineNoteIssuedClaim],
                outputCommitments: [Data],
                outputClaims: [AttestedOfflineNoteAuditOutputClaim],
                recursiveProof: AttestedOfflineNoteRecursiveProof) throws {
        try AttestedOfflineNoteValidation.validateHash(tokenId, field: "token_id")
        try AttestedOfflineNoteValidation.validateHashes(
            inputNullifiers,
            field: "input_nullifiers",
            emptyError: .emptyInputNullifiers
        )
        try AttestedOfflineNoteValidation.validateHashes(
            outputCommitments,
            field: "output_commitments",
            emptyError: .emptyOutputCommitments
        )
        guard !inputClaims.isEmpty else {
            throw AttestedOfflineNoteError.emptyInputClaims
        }
        guard inputClaims.count == inputNullifiers.count else {
            throw AttestedOfflineNoteError.auditInputCountMismatch(
                nullifiers: inputNullifiers.count,
                claims: inputClaims.count
            )
        }
        guard !outputClaims.isEmpty else {
            throw AttestedOfflineNoteError.emptyOutputClaims
        }
        let committed = Set(outputCommitments.map { $0.hexLowercased() })
        for claim in outputClaims {
            let commitment = claim.noteCommitment.hexLowercased()
            guard committed.contains(commitment) else {
                throw AttestedOfflineNoteError.auditOutputClaimNotCommitted(commitment)
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

    public func publicInputs() throws -> AttestedOfflineNoteAuditPublicInputs {
        try AttestedOfflineNoteAuditPublicInputs.fromAudit(self)
    }

    public func publicInputsHash() throws -> Data {
        try publicInputs().publicInputsHash()
    }

    public func validateProofBinding() throws {
        let expected = try publicInputsHash()
        guard recursiveProof.publicInputsHash == expected else {
            throw AttestedOfflineNoteError.proofPublicInputsHashMismatch(
                expected: expected.hexLowercased(),
                actual: recursiveProof.publicInputsHash.hexLowercased()
            )
        }
    }

    public func replacingRecursiveProof(_ recursiveProof: AttestedOfflineNoteRecursiveProof) throws -> AttestedOfflineNoteAuditBundle {
        try AttestedOfflineNoteAuditBundle(
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
        try AttestedOfflineNoteEncoding.wrap(
            typeName: AttestedOfflineNoteTypeNames.audit,
            payload: AttestedOfflineNoteEncoding.encodeAudit(self)
        )
    }
}

public struct AttestedOfflineNoteIssueRequest: Sendable {
    public let chainId: String
    public let authority: String
    public let issue: AttestedOfflineNoteIssue
    public let ttlMs: UInt64?
    public let nonce: UInt32?
    public let metadata: [String: ToriiJSONValue]

    public init(chainId: String,
                authority: String,
                issue: AttestedOfflineNoteIssue,
                ttlMs: UInt64? = nil,
                nonce: UInt32? = nil,
                metadata: [String: ToriiJSONValue] = [:]) {
        self.chainId = chainId
        self.authority = authority
        self.issue = issue
        self.ttlMs = ttlMs
        self.nonce = nonce
        self.metadata = metadata
    }
}

public struct AttestedOfflineNoteRedeemRequest: Sendable {
    public let chainId: String
    public let authority: String
    public let redemption: AttestedOfflineNoteRedeem
    public let ttlMs: UInt64?
    public let nonce: UInt32?
    public let metadata: [String: ToriiJSONValue]

    public init(chainId: String,
                authority: String,
                redemption: AttestedOfflineNoteRedeem,
                ttlMs: UInt64? = nil,
                nonce: UInt32? = nil,
                metadata: [String: ToriiJSONValue] = [:]) {
        self.chainId = chainId
        self.authority = authority
        self.redemption = redemption
        self.ttlMs = ttlMs
        self.nonce = nonce
        self.metadata = metadata
    }
}

public struct AttestedOfflineNoteAuditRequest: Sendable {
    public let chainId: String
    public let authority: String
    public let audit: AttestedOfflineNoteAuditBundle
    public let ttlMs: UInt64?
    public let nonce: UInt32?
    public let metadata: [String: ToriiJSONValue]

    public init(chainId: String,
                authority: String,
                audit: AttestedOfflineNoteAuditBundle,
                ttlMs: UInt64? = nil,
                nonce: UInt32? = nil,
                metadata: [String: ToriiJSONValue] = [:]) {
        self.chainId = chainId
        self.authority = authority
        self.audit = audit
        self.ttlMs = ttlMs
        self.nonce = nonce
        self.metadata = metadata
    }
}

public struct RegisterOfflineDeviceAttestationRequest: Sendable {
    public let chainId: String
    public let authority: String
    public let registration: OfflineDeviceAttestationRegistration
    public let ttlMs: UInt64?
    public let nonce: UInt32?
    public let metadata: [String: ToriiJSONValue]

    public init(chainId: String,
                authority: String,
                registration: OfflineDeviceAttestationRegistration,
                ttlMs: UInt64? = nil,
                nonce: UInt32? = nil,
                metadata: [String: ToriiJSONValue] = [:]) {
        self.chainId = chainId
        self.authority = authority
        self.registration = registration
        self.ttlMs = ttlMs
        self.nonce = nonce
        self.metadata = metadata
    }
}

enum AttestedOfflineNoteTypeNames {
    static let deviceAttestationRegistration =
        "iroha_data_model::offline::OfflineDeviceAttestationRegistration"
    static let deviceAttestationChallengePreimage =
        "iroha_data_model::offline::OfflineDeviceAttestationChallengePreimage"
    static let androidKeyMintChallengePreimage =
        "iroha_data_model::offline::OfflineAndroidKeyMintChallengePreimage"
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
    static let issueInstruction = "iroha_data_model::isi::offline::IssueOfflineNote"
    static let redeemInstruction = "iroha_data_model::isi::offline::RedeemOfflineNote"
    static let auditInstruction = "iroha_data_model::isi::offline::AuditOfflineNote"
    static let registerDeviceAttestationInstruction =
        "iroha_data_model::isi::offline::RegisterOfflineDeviceAttestation"
}

enum AttestedOfflineNoteValidation {
    static func validateDomain(_ value: String, expected: String, field: String) throws {
        guard value == expected else {
            throw AttestedOfflineNoteError.unsupportedDomain(
                field: field,
                expected: expected,
                actual: value
            )
        }
    }

    static func validateHash(_ value: Data, field: String) throws {
        guard value.count == 32 else {
            throw AttestedOfflineNoteError.invalidHashLength(field: field, expected: 32, actual: value.count)
        }
        guard let last = value.last, (last & 1) == 1 else {
            throw AttestedOfflineNoteError.invalidHash(field: field)
        }
    }

    static func validateHashes(_ values: [Data],
                               field: String,
                               emptyError: AttestedOfflineNoteError = .emptyInputNullifiers) throws {
        guard !values.isEmpty else {
            throw emptyError
        }
        for (index, value) in values.enumerated() {
            try validateHash(value, field: "\(field)[\(index)]")
        }
    }

    static func validateAttestationEvidenceEnvelope(_ evidence: Data,
                                                    attestationReportHash: Data) throws {
        let prefix = Data(AttestedOfflineNoteConstants.deviceAttestationEvidencePrefix.utf8)
        guard evidence.count == prefix.count + attestationReportHash.count,
              Data(evidence.prefix(prefix.count)) == prefix,
              Data(evidence.suffix(attestationReportHash.count)) == attestationReportHash else {
            throw AttestedOfflineNoteError.unsupportedDeviceAttestationProfile(
                "evidence envelope must be deviceAttestationEvidencePrefix || attestation_report_hash"
            )
        }
    }

    static func attestationEvidenceEnvelope(attestationReportHash: Data) -> Data {
        Data(AttestedOfflineNoteConstants.deviceAttestationEvidencePrefix.utf8) + attestationReportHash
    }

    static func validateCertificateCore(version: UInt16,
                                        accountId: String,
                                        publicKey: Data,
                                        oneUse: Bool) throws {
        guard version == AttestedOfflineNoteConstants.keyCertificateVersion else {
            throw AttestedOfflineNoteError.invalidCertificateVersion(version)
        }
        guard oneUse else {
            throw AttestedOfflineNoteError.certificateMustBeOneUse
        }
        guard publicKey.count == 32 else {
            throw AttestedOfflineNoteError.invalidNotePublicKeyLength(expected: 32, actual: publicKey.count)
        }
        _ = try OfflineNorito.encodeAccountId(accountId)
    }

    static func validateAttestationIdentity(keyId: String, deviceId: String) throws {
        let trimmedKeyId = keyId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedKeyId.isEmpty else {
            throw AttestedOfflineNoteError.unsupportedDeviceAttestationProfile("attestation key_id must not be empty")
        }
        guard trimmedKeyId == keyId else {
            throw AttestedOfflineNoteError.unsupportedDeviceAttestationProfile(
                "attestation key_id must not contain surrounding whitespace"
            )
        }
        try validateAttestationDeviceId(deviceId)
    }

    static func validateAttestationDeviceId(_ deviceId: String) throws {
        let trimmedDeviceId = deviceId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedDeviceId.isEmpty else {
            throw AttestedOfflineNoteError.unsupportedDeviceAttestationProfile("attestation device_id must not be empty")
        }
        guard trimmedDeviceId == deviceId else {
            throw AttestedOfflineNoteError.unsupportedDeviceAttestationProfile(
                "attestation device_id must not contain surrounding whitespace"
            )
        }
    }

    static func validateOptionalAttestationMetadata(iosTeamId: String?,
                                                    iosBundleId: String?,
                                                    iosEnvironment: String?,
                                                    androidPackageName: String?) throws {
        for (field, value) in [
            ("ios_team_id", iosTeamId),
            ("ios_bundle_id", iosBundleId),
            ("ios_environment", iosEnvironment),
            ("android_package_name", androidPackageName)
        ] {
            try validateOptionalAttestationMetadataValue(value, field: field)
        }
    }

    private static func validateOptionalAttestationMetadataValue(_ value: String?,
                                                                 field: String) throws {
        guard let value else {
            return
        }
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw AttestedOfflineNoteError.unsupportedDeviceAttestationProfile(
                "\(field) must not be empty when present"
            )
        }
        guard trimmed == value else {
            throw AttestedOfflineNoteError.unsupportedDeviceAttestationProfile(
                "\(field) must not contain surrounding whitespace"
            )
        }
    }

    static func validateDeviceAttestationProfile(platform: String,
                                                 keyId: String,
                                                 assertionScheme: String,
                                                 assertionKeyAlgorithm: String,
                                                 assertionPublicKey: Data,
                                                 assertionUsageCountLimit: UInt32?) throws {
        try validateP256AssertionPublicKey(assertionPublicKey)
        switch platform {
        case AttestedOfflineNoteConstants.iosAppAttestPlatform:
            try validateIosAppAttestRegistrationKeyId(keyId)
            try validateIosAppAttestProfile(
                assertionScheme: assertionScheme,
                assertionKeyAlgorithm: assertionKeyAlgorithm,
                assertionUsageCountLimit: assertionUsageCountLimit
            )
        case AttestedOfflineNoteConstants.androidKeyMintPlatform:
            try validateAndroidKeyMintProfile(
                keyId: keyId,
                assertionScheme: assertionScheme,
                assertionKeyAlgorithm: assertionKeyAlgorithm,
                assertionPublicKey: assertionPublicKey,
                assertionUsageCountLimit: assertionUsageCountLimit
            )
        default:
            throw AttestedOfflineNoteError.unsupportedDeviceAttestationProfile("unsupported platform \(platform)")
        }
    }

    static func validateKeyCertificateProfile(platform: String,
                                              keyId: String,
                                              assertionScheme: String,
                                              assertionKeyAlgorithm: String,
                                              assertionPublicKey: Data,
                                              assertionUsageCountLimit: UInt32?) throws {
        try validateP256AssertionPublicKey(assertionPublicKey)
        switch platform {
        case AttestedOfflineNoteConstants.iosAppAttestPlatform:
            try validateIosAppAttestProfile(
                assertionScheme: assertionScheme,
                assertionKeyAlgorithm: assertionKeyAlgorithm,
                assertionUsageCountLimit: assertionUsageCountLimit
            )
        case AttestedOfflineNoteConstants.androidKeyMintPlatform:
            try validateAndroidKeyMintProfile(
                keyId: keyId,
                assertionScheme: assertionScheme,
                assertionKeyAlgorithm: assertionKeyAlgorithm,
                assertionPublicKey: assertionPublicKey,
                assertionUsageCountLimit: assertionUsageCountLimit
            )
        default:
            throw AttestedOfflineNoteError.unsupportedDeviceAttestationProfile("unsupported platform \(platform)")
        }
    }

    private static func validateIosAppAttestRegistrationKeyId(_ keyId: String) throws {
        guard let decoded = Data(base64Encoded: keyId),
              !decoded.isEmpty,
              decoded.base64EncodedString() == keyId else {
            throw AttestedOfflineNoteError.unsupportedDeviceAttestationProfile(
                "iOS App Attest key_id must be canonical standard base64 credential bytes"
            )
        }
    }

    private static func validateIosAppAttestProfile(assertionScheme: String,
                                                    assertionKeyAlgorithm: String,
                                                    assertionUsageCountLimit: UInt32?) throws {
        guard assertionScheme == AttestedOfflineNoteConstants.iosAppAttestAssertionScheme,
              assertionKeyAlgorithm == AttestedOfflineNoteConstants.iosAppAttestAssertionKeyAlgorithm,
              assertionUsageCountLimit == nil else {
            throw AttestedOfflineNoteError.unsupportedDeviceAttestationProfile(
                "iOS App Attest requires \(AttestedOfflineNoteConstants.iosAppAttestAssertionScheme), \(AttestedOfflineNoteConstants.iosAppAttestAssertionKeyAlgorithm), and no assertion usage limit"
            )
        }
    }

    private static func validateAndroidKeyMintProfile(keyId: String,
                                                      assertionScheme: String,
                                                      assertionKeyAlgorithm: String,
                                                      assertionPublicKey: Data,
                                                      assertionUsageCountLimit: UInt32?) throws {
        guard assertionScheme == AttestedOfflineNoteConstants.androidKeyMintAssertionScheme,
              assertionKeyAlgorithm == AttestedOfflineNoteConstants.androidKeyMintAssertionKeyAlgorithm,
              assertionUsageCountLimit == 1 else {
            throw AttestedOfflineNoteError.unsupportedDeviceAttestationProfile(
                "Android KeyMint requires \(AttestedOfflineNoteConstants.androidKeyMintAssertionScheme), \(AttestedOfflineNoteConstants.androidKeyMintAssertionKeyAlgorithm), and assertion usage limit 1"
            )
        }
        let expectedKeyId = Data(SHA256.hash(data: assertionPublicKey)).hexLowercased()
        guard keyId == expectedKeyId else {
            throw AttestedOfflineNoteError.unsupportedDeviceAttestationProfile(
                "Android KeyMint key_id must be lowercase hex SHA-256 of the assertion public key"
            )
        }
    }

    private static func validateP256AssertionPublicKey(_ assertionPublicKey: Data) throws {
        guard assertionPublicKey.count == 65,
              assertionPublicKey.first == 0x04 else {
            throw AttestedOfflineNoteError.unsupportedDeviceAttestationProfile(
                "assertion public key must be an uncompressed P-256 SEC1 point"
            )
        }
        guard (try? P256.Signing.PublicKey(x963Representation: assertionPublicKey)) != nil else {
            throw AttestedOfflineNoteError.unsupportedDeviceAttestationProfile(
                "assertion public key must be a valid P-256 point"
            )
        }
    }
}

enum AttestedOfflineNoteEncoding {
    static func wrap(typeName: String, payload: Data) -> Data {
        noritoEncode(typeName: typeName, payload: payload, flags: 2)
    }

    static func encodeDeviceAttestationRegistration(
        _ registration: OfflineDeviceAttestationRegistration
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeUInt16(registration.version))
        writer.writeField(OfflineCompactNorito.encodeString(registration.platform))
        writer.writeField(OfflineCompactNorito.encodeString(registration.keyId))
        writer.writeField(OfflineCompactNorito.encodeString(registration.deviceId))
        writer.writeField(try encodeAccountId(registration.accountId))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            registration.assetDefinitionId,
            encode: encodeAssetDefinitionId
        ))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            registration.iosTeamId,
            encode: OfflineCompactNorito.encodeString
        ))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            registration.iosBundleId,
            encode: OfflineCompactNorito.encodeString
        ))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            registration.iosEnvironment,
            encode: OfflineCompactNorito.encodeString
        ))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            registration.androidPackageName,
            encode: OfflineCompactNorito.encodeString
        ))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            registration.androidSigningCertificateSha256,
            encode: encodeBytesVec
        ))
        writer.writeField(encodeBytesVec(registration.publicKey))
        writer.writeField(OfflineCompactNorito.encodeString(registration.assertionScheme))
        writer.writeField(OfflineCompactNorito.encodeString(registration.assertionKeyAlgorithm))
        writer.writeField(encodeBytesVec(registration.assertionPublicKey))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            registration.assertionUsageCountLimit,
            encode: OfflineCompactNorito.encodeUInt32
        ))
        writer.writeField(OfflineNorito.encodeBool(registration.oneUse))
        writer.writeField(try OfflineCompactNorito.encodeHash(registration.challengeHash))
        writer.writeField(try OfflineCompactNorito.encodeHash(registration.attestationReportHash))
        writer.writeField(encodeBytesVec(registration.attestationReport))
        writer.writeField(try OfflineCompactNorito.encodeHash(registration.evidenceHash))
        writer.writeField(encodeBytesVec(registration.evidence))
        writer.writeField(OfflineCompactNorito.encodeUInt64(registration.recentBlockHeight))
        writer.writeField(try OfflineCompactNorito.encodeHash(registration.recentBlockHash))
        writer.writeField(OfflineCompactNorito.encodeUInt64(registration.expiresAtMs))
        return writer.data
    }

    fileprivate static func encodeDeviceAttestationChallengePreimage(
        _ preimage: OfflineDeviceAttestationChallengePreimage
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(AttestedOfflineNoteConstants.deviceAttestationChallengeDomain))
        writer.writeField(OfflineCompactNorito.encodeUInt16(preimage.version))
        writer.writeField(OfflineCompactNorito.encodeString(preimage.platform))
        writer.writeField(OfflineCompactNorito.encodeString(preimage.keyId))
        writer.writeField(OfflineCompactNorito.encodeString(preimage.deviceId))
        writer.writeField(try encodeAccountId(preimage.accountId))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            preimage.assetDefinitionId,
            encode: encodeAssetDefinitionId
        ))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            preimage.iosTeamId,
            encode: OfflineCompactNorito.encodeString
        ))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            preimage.iosBundleId,
            encode: OfflineCompactNorito.encodeString
        ))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            preimage.iosEnvironment,
            encode: OfflineCompactNorito.encodeString
        ))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            preimage.androidPackageName,
            encode: OfflineCompactNorito.encodeString
        ))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            preimage.androidSigningCertificateSha256,
            encode: encodeBytesVec
        ))
        writer.writeField(encodeBytesVec(preimage.publicKey))
        writer.writeField(OfflineCompactNorito.encodeString(preimage.assertionScheme))
        writer.writeField(OfflineCompactNorito.encodeString(preimage.assertionKeyAlgorithm))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            preimage.assertionUsageCountLimit,
            encode: OfflineCompactNorito.encodeUInt32
        ))
        writer.writeField(OfflineNorito.encodeBool(preimage.oneUse))
        writer.writeField(OfflineCompactNorito.encodeUInt64(preimage.recentBlockHeight))
        writer.writeField(try OfflineCompactNorito.encodeHash(preimage.recentBlockHash))
        writer.writeField(OfflineCompactNorito.encodeUInt64(preimage.expiresAtMs))
        return writer.data
    }

    fileprivate static func encodeAndroidKeyMintChallengePreimage(
        _ preimage: OfflineAndroidKeyMintChallengePreimage
    ) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(
            AttestedOfflineNoteConstants.deviceAttestationChallengeDomain
        ))
        writer.writeField(OfflineCompactNorito.encodeUInt16(preimage.version))
        writer.writeField(OfflineCompactNorito.encodeString(
            AttestedOfflineNoteConstants.androidKeyMintPlatform
        ))
        writer.writeField(OfflineCompactNorito.encodeString(preimage.deviceId))
        writer.writeField(try encodeAccountId(preimage.accountId))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            preimage.assetDefinitionId,
            encode: encodeAssetDefinitionId
        ))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            preimage.iosTeamId,
            encode: OfflineCompactNorito.encodeString
        ))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            preimage.iosBundleId,
            encode: OfflineCompactNorito.encodeString
        ))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            preimage.iosEnvironment,
            encode: OfflineCompactNorito.encodeString
        ))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            preimage.androidPackageName,
            encode: OfflineCompactNorito.encodeString
        ))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            preimage.androidSigningCertificateSha256,
            encode: encodeBytesVec
        ))
        writer.writeField(encodeBytesVec(preimage.publicKey))
        writer.writeField(OfflineCompactNorito.encodeString(preimage.assertionScheme))
        writer.writeField(OfflineCompactNorito.encodeString(preimage.assertionKeyAlgorithm))
        writer.writeField(try OfflineCompactNorito.encodeOption(
            preimage.assertionUsageCountLimit,
            encode: OfflineCompactNorito.encodeUInt32
        ))
        writer.writeField(OfflineNorito.encodeBool(preimage.oneUse))
        writer.writeField(OfflineCompactNorito.encodeUInt64(preimage.recentBlockHeight))
        writer.writeField(try OfflineCompactNorito.encodeHash(preimage.recentBlockHash))
        writer.writeField(OfflineCompactNorito.encodeUInt64(preimage.expiresAtMs))
        return writer.data
    }

    static func encodeCertificatePayload(_ payload: AttestedOfflineNoteKeyCertificatePayload) throws -> Data {
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

    static func encodeCertificate(_ certificate: AttestedOfflineNoteKeyCertificate) throws -> Data {
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

    static func encodeProofBox(_ proof: AttestedOfflineNoteProofBox) -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(proof.backend))
        writer.writeField(encodeBytesVec(proof.bytes))
        return writer.data
    }

    static func encodeRecursiveProof(_ proof: AttestedOfflineNoteRecursiveProof) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(encodeVerifyingKeyId(proof.verifierKeyId))
        writer.writeField(try OfflineCompactNorito.encodeHash(proof.publicInputsHash))
        writer.writeField(encodeProofBox(proof.proof))
        return writer.data
    }

    static func encodeIssue(_ issue: AttestedOfflineNoteIssue) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try OfflineCompactNorito.encodeHash(issue.noteCommitment))
        writer.writeField(try encodeCertificate(issue.keyCertificate))
        writer.writeField(try encodeAssetId(issue.assetId))
        writer.writeField(try encodeNumeric(issue.amount))
        return writer.data
    }

    static func encodeIssuedClaim(_ claim: AttestedOfflineNoteIssuedClaim) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(OfflineCompactNorito.encodeString(claim.domain))
        writer.writeField(try OfflineCompactNorito.encodeHash(claim.noteCommitment))
        writer.writeField(try OfflineCompactNorito.encodeHash(claim.keyCertificatePayloadHash))
        writer.writeField(try encodeAssetId(claim.assetId))
        writer.writeField(try encodeNumeric(claim.amount))
        return writer.data
    }

    static func encodeAuditOutputClaim(_ claim: AttestedOfflineNoteAuditOutputClaim) throws -> Data {
        var writer = OfflineCompactNoritoWriter()
        writer.writeField(try OfflineCompactNorito.encodeHash(claim.noteCommitment))
        writer.writeField(try encodeCertificate(claim.keyCertificate))
        writer.writeField(try encodeAssetId(claim.assetId))
        writer.writeField(try encodeNumeric(claim.amount))
        return writer.data
    }

    static func encodeRedeemPublicInputs(_ inputs: AttestedOfflineNoteRedeemPublicInputs) throws -> Data {
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

    static func encodeRedeem(_ redemption: AttestedOfflineNoteRedeem) throws -> Data {
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

    static func encodeAuditPublicInputs(_ inputs: AttestedOfflineNoteAuditPublicInputs) throws -> Data {
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

    static func encodeAudit(_ audit: AttestedOfflineNoteAuditBundle) throws -> Data {
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

    private static func encodeAssetDefinitionId(_ assetDefinitionId: String) throws -> Data {
        guard let definitionBytes = AssetDefinitionAddress.decode(assetDefinitionId) else {
            throw OfflineNoritoError.invalidAssetId(assetDefinitionId)
        }
        return encodeAssetDefinitionAddress(definitionBytes)
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
