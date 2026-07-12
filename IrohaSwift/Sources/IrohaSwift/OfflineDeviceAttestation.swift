import Foundation
import CryptoKit

public enum KagemushaDeviceAttestation {
    public static let deviceAttestationChallengeDomain = "iroha:offline-note:device-attestation-challenge:v1"
    public static let deviceAttestationEvidencePrefix = "offline-device-attestation-evidence-v1"
    public static let registrationVersion: UInt16 = 1
    public static let iosAppAttestPlatform = "ios-appattest"
    public static let iosAppAttestAssertionScheme = "apple-appattest-counter-v1"
    public static let iosAppAttestAssertionKeyAlgorithm = "app-attest-p256"
    public static let androidKeyMintPlatform = "android-keymint"
    public static let androidKeyMintAssertionScheme = "android-keymint-ecdsa-p256-usage-limit-v1"
    public static let androidKeyMintAssertionKeyAlgorithm = "ecdsa-p256-sha256"

}

public enum KagemushaDeviceAttestationError: Error, LocalizedError, Equatable {
    case invalidHashLength(field: String, expected: Int, actual: Int)
    case invalidHash(field: String)
    case invalidRegistrationVersion(UInt16)
    case authorityMustBeOneUse
    case invalidAuthorityPublicKeyLength(expected: Int, actual: Int)
    case deviceAttestationChallengeHashMismatch(expected: String, actual: String)
    case deviceAttestationHashMismatch(field: String)
    case invalidDigestLength(field: String, expected: Int, actual: Int)
    case unsupportedDeviceAttestationProfile(String)
    case nonCanonicalField(field: String)

    public var errorDescription: String? {
        switch self {
        case let .invalidHashLength(field, expected, actual):
            return "\(field) must be exactly \(expected) bytes (found \(actual))."
        case let .invalidHash(field):
            return "\(field) must be a canonical Iroha hash."
        case let .invalidRegistrationVersion(version):
            return "Kagemusha device-attestation registration version must be \(KagemushaDeviceAttestation.registrationVersion) (found \(version))."
        case .authorityMustBeOneUse:
            return "Kagemusha offline authority must be marked one-use."
        case let .invalidAuthorityPublicKeyLength(expected, actual):
            return "Kagemusha offline authority public key must be \(expected) bytes (found \(actual))."
        case let .deviceAttestationChallengeHashMismatch(expected, actual):
            return "Kagemusha device attestation challenge hash mismatch: expected \(expected), got \(actual)."
        case let .deviceAttestationHashMismatch(field):
            return "Kagemusha device attestation \(field) does not match the submitted bytes."
        case let .invalidDigestLength(field, expected, actual):
            return "\(field) must be exactly \(expected) bytes (found \(actual))."
        case let .unsupportedDeviceAttestationProfile(reason):
            return "Unsupported Kagemusha device attestation profile: \(reason)."
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

    public init(version: UInt16 = KagemushaDeviceAttestation.registrationVersion,
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
        try KagemushaDeviceAttestationValidation.validateRegistrationCore(
            version: version,
            accountId: accountId,
            publicKey: publicKey,
            oneUse: oneUse
        )
        try KagemushaDeviceAttestationValidation.validateAttestationIdentity(keyId: keyId, deviceId: deviceId)
        try KagemushaDeviceAttestationValidation.validateOptionalAttestationMetadata(
            iosTeamId: iosTeamId,
            iosBundleId: iosBundleId,
            iosEnvironment: iosEnvironment,
            androidPackageName: androidPackageName
        )
        if let assetDefinitionId, AssetDefinitionAddress.decode(assetDefinitionId) == nil {
            throw CanonicalNoritoError.invalidAssetId(assetDefinitionId)
        }
        if let androidSigningCertificateSha256,
           androidSigningCertificateSha256.count != 32 {
            throw KagemushaDeviceAttestationError.invalidDigestLength(
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
        guard !attestationReport.isEmpty else {
            throw KagemushaDeviceAttestationError.nonCanonicalField(field: "attestation_report")
        }
        try KagemushaDeviceAttestationValidation.validateRegistrationLifetime(
            recentBlockHeight: recentBlockHeight,
            expiresAtMs: expiresAtMs
        )
        try KagemushaDeviceAttestationValidation.validateHash(recentBlockHash, field: "recent_block_hash")

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
            try KagemushaDeviceAttestationValidation.validateHash(challengeHash, field: "challenge_hash")
            guard challengeHash == resolvedChallengeHash else {
                throw KagemushaDeviceAttestationError.deviceAttestationChallengeHashMismatch(
                    expected: resolvedChallengeHash.hexLowercased(),
                    actual: challengeHash.hexLowercased()
                )
            }
        }

        let resolvedAttestationReportHash = attestationReportHash ?? IrohaHash.hash(attestationReport)
        try KagemushaDeviceAttestationValidation.validateHash(resolvedAttestationReportHash, field: "attestation_report_hash")
        guard resolvedAttestationReportHash == IrohaHash.hash(attestationReport) else {
            throw KagemushaDeviceAttestationError.deviceAttestationHashMismatch(field: "attestation_report_hash")
        }

        let resolvedEvidence: Data
        if evidence.isEmpty, evidenceHash == nil {
            resolvedEvidence = KagemushaDeviceAttestationValidation.attestationEvidenceEnvelope(
                attestationReportHash: resolvedAttestationReportHash
            )
        } else {
            resolvedEvidence = evidence
        }
        try KagemushaDeviceAttestationValidation.validateAttestationEvidenceEnvelope(
            resolvedEvidence,
            attestationReportHash: resolvedAttestationReportHash
        )
        let expectedEvidenceHash = IrohaHash.hash(resolvedEvidence)
        let resolvedEvidenceHash = evidenceHash ?? expectedEvidenceHash
        try KagemushaDeviceAttestationValidation.validateHash(resolvedEvidenceHash, field: "evidence_hash")
        guard resolvedEvidenceHash == expectedEvidenceHash else {
            throw KagemushaDeviceAttestationError.deviceAttestationHashMismatch(field: "evidence_hash")
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
        try KagemushaDeviceAttestationValidation.validateDeviceAttestationProfile(
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
        version: UInt16 = KagemushaDeviceAttestation.registrationVersion,
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
        try KagemushaDeviceAttestationValidation.validateRegistrationCore(
            version: version,
            accountId: accountId,
            publicKey: publicKey,
            oneUse: oneUse
        )
        try KagemushaDeviceAttestationValidation.validateAttestationIdentity(keyId: keyId, deviceId: deviceId)
        try KagemushaDeviceAttestationValidation.validateOptionalAttestationMetadata(
            iosTeamId: iosTeamId,
            iosBundleId: iosBundleId,
            iosEnvironment: iosEnvironment,
            androidPackageName: androidPackageName
        )
        if let assetDefinitionId, AssetDefinitionAddress.decode(assetDefinitionId) == nil {
            throw CanonicalNoritoError.invalidAssetId(assetDefinitionId)
        }
        if let androidSigningCertificateSha256,
           androidSigningCertificateSha256.count != 32 {
            throw KagemushaDeviceAttestationError.invalidDigestLength(
                field: "android_signing_certificate_sha256",
                expected: 32,
                actual: androidSigningCertificateSha256.count
            )
        }
        guard !assertionScheme.isEmpty,
              assertionScheme == assertionScheme.trimmingCharacters(in: .whitespacesAndNewlines),
              !assertionKeyAlgorithm.isEmpty,
              assertionKeyAlgorithm == assertionKeyAlgorithm.trimmingCharacters(in: .whitespacesAndNewlines) else {
            throw KagemushaDeviceAttestationError.nonCanonicalField(field: "assertion_profile")
        }
        try KagemushaDeviceAttestationValidation.validateRegistrationLifetime(
            recentBlockHeight: recentBlockHeight,
            expiresAtMs: expiresAtMs
        )
        try KagemushaDeviceAttestationValidation.validateHash(recentBlockHash, field: "recent_block_hash")
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
        version: UInt16 = KagemushaDeviceAttestation.registrationVersion,
        deviceId: String,
        accountId: String,
        assetDefinitionId: String? = nil,
        iosTeamId: String? = nil,
        iosBundleId: String? = nil,
        iosEnvironment: String? = nil,
        androidPackageName: String,
        androidSigningCertificateSha256: Data,
        publicKey: Data,
        assertionScheme: String = KagemushaDeviceAttestation.androidKeyMintAssertionScheme,
        assertionKeyAlgorithm: String = KagemushaDeviceAttestation.androidKeyMintAssertionKeyAlgorithm,
        assertionUsageCountLimit: UInt32? = 1,
        oneUse: Bool = true,
        recentBlockHeight: UInt64,
        recentBlockHash: Data,
        expiresAtMs: UInt64
    ) throws -> Data {
        try KagemushaDeviceAttestationValidation.validateRegistrationCore(
            version: version,
            accountId: accountId,
            publicKey: publicKey,
            oneUse: oneUse
        )
        try KagemushaDeviceAttestationValidation.validateAttestationDeviceId(deviceId)
        try KagemushaDeviceAttestationValidation.validateOptionalAttestationMetadata(
            iosTeamId: iosTeamId,
            iosBundleId: iosBundleId,
            iosEnvironment: iosEnvironment,
            androidPackageName: androidPackageName
        )
        if let assetDefinitionId, AssetDefinitionAddress.decode(assetDefinitionId) == nil {
            throw CanonicalNoritoError.invalidAssetId(assetDefinitionId)
        }
        guard androidSigningCertificateSha256.count == 32 else {
            throw KagemushaDeviceAttestationError.invalidDigestLength(
                field: "android_signing_certificate_sha256",
                expected: 32,
                actual: androidSigningCertificateSha256.count
            )
        }
        guard assertionScheme == KagemushaDeviceAttestation.androidKeyMintAssertionScheme,
              assertionKeyAlgorithm == KagemushaDeviceAttestation.androidKeyMintAssertionKeyAlgorithm,
              assertionUsageCountLimit == 1 else {
            throw KagemushaDeviceAttestationError.unsupportedDeviceAttestationProfile(
                "Android KeyMint pre-key challenge requires the canonical one-use P-256 assertion profile"
            )
        }
        try KagemushaDeviceAttestationValidation.validateRegistrationLifetime(
            recentBlockHeight: recentBlockHeight,
            expiresAtMs: expiresAtMs
        )
        try KagemushaDeviceAttestationValidation.validateHash(recentBlockHash, field: "recent_block_hash")
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

    public func noritoEncoded() throws -> Data {
        try KagemushaDeviceAttestationEncoding.wrap(
            typeName: KagemushaDeviceAttestationTypeNames.deviceAttestationRegistration,
            payload: KagemushaDeviceAttestationEncoding.encodeDeviceAttestationRegistration(self)
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
        if platform == KagemushaDeviceAttestation.androidKeyMintPlatform {
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
        try KagemushaDeviceAttestationEncoding.wrap(
            typeName: KagemushaDeviceAttestationTypeNames.deviceAttestationChallengePreimage,
            payload: KagemushaDeviceAttestationEncoding.encodeDeviceAttestationChallengePreimage(self)
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
        try KagemushaDeviceAttestationEncoding.wrap(
            typeName: KagemushaDeviceAttestationTypeNames.androidKeyMintChallengePreimage,
            payload: KagemushaDeviceAttestationEncoding.encodeAndroidKeyMintChallengePreimage(self)
        )
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

enum KagemushaDeviceAttestationTypeNames {
    static let deviceAttestationRegistration =
        "iroha_data_model::offline::OfflineDeviceAttestationRegistration"
    static let deviceAttestationChallengePreimage =
        "iroha_data_model::offline::OfflineDeviceAttestationChallengePreimage"
    static let androidKeyMintChallengePreimage =
        "iroha_data_model::offline::OfflineAndroidKeyMintChallengePreimage"
}

enum KagemushaDeviceAttestationValidation {
    static func validateRegistrationLifetime(
        recentBlockHeight: UInt64,
        expiresAtMs: UInt64
    ) throws {
        guard recentBlockHeight > 0, expiresAtMs > 0 else {
            throw KagemushaDeviceAttestationError.nonCanonicalField(
                field: "registration_lifetime"
            )
        }
    }

    static func validateHash(_ value: Data, field: String) throws {
        guard value.count == 32 else {
            throw KagemushaDeviceAttestationError.invalidHashLength(field: field, expected: 32, actual: value.count)
        }
        guard let last = value.last, (last & 1) == 1 else {
            throw KagemushaDeviceAttestationError.invalidHash(field: field)
        }
    }

    static func validateAttestationEvidenceEnvelope(_ evidence: Data,
                                                    attestationReportHash: Data) throws {
        let prefix = Data(KagemushaDeviceAttestation.deviceAttestationEvidencePrefix.utf8)
        guard evidence.count == prefix.count + attestationReportHash.count,
              Data(evidence.prefix(prefix.count)) == prefix,
              Data(evidence.suffix(attestationReportHash.count)) == attestationReportHash else {
            throw KagemushaDeviceAttestationError.unsupportedDeviceAttestationProfile(
                "evidence envelope must be deviceAttestationEvidencePrefix || attestation_report_hash"
            )
        }
    }

    static func attestationEvidenceEnvelope(attestationReportHash: Data) -> Data {
        Data(KagemushaDeviceAttestation.deviceAttestationEvidencePrefix.utf8) + attestationReportHash
    }

    static func validateRegistrationCore(version: UInt16,
                                         accountId: String,
                                         publicKey: Data,
                                         oneUse: Bool) throws {
        guard version == KagemushaDeviceAttestation.registrationVersion else {
            throw KagemushaDeviceAttestationError.invalidRegistrationVersion(version)
        }
        guard oneUse else {
            throw KagemushaDeviceAttestationError.authorityMustBeOneUse
        }
        guard publicKey.count == 32 else {
            throw KagemushaDeviceAttestationError.invalidAuthorityPublicKeyLength(
                expected: 32,
                actual: publicKey.count
            )
        }
        _ = try CanonicalNorito.encodeAccountId(accountId)
    }

    static func validateAttestationIdentity(keyId: String, deviceId: String) throws {
        let trimmedKeyId = keyId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedKeyId.isEmpty else {
            throw KagemushaDeviceAttestationError.unsupportedDeviceAttestationProfile("attestation key_id must not be empty")
        }
        guard trimmedKeyId == keyId else {
            throw KagemushaDeviceAttestationError.unsupportedDeviceAttestationProfile(
                "attestation key_id must not contain surrounding whitespace"
            )
        }
        try validateAttestationDeviceId(deviceId)
    }

    static func validateAttestationDeviceId(_ deviceId: String) throws {
        let trimmedDeviceId = deviceId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedDeviceId.isEmpty else {
            throw KagemushaDeviceAttestationError.unsupportedDeviceAttestationProfile("attestation device_id must not be empty")
        }
        guard trimmedDeviceId == deviceId else {
            throw KagemushaDeviceAttestationError.unsupportedDeviceAttestationProfile(
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
            throw KagemushaDeviceAttestationError.unsupportedDeviceAttestationProfile(
                "\(field) must not be empty when present"
            )
        }
        guard trimmed == value else {
            throw KagemushaDeviceAttestationError.unsupportedDeviceAttestationProfile(
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
        case KagemushaDeviceAttestation.iosAppAttestPlatform:
            try validateIosAppAttestRegistrationKeyId(keyId)
            try validateIosAppAttestProfile(
                assertionScheme: assertionScheme,
                assertionKeyAlgorithm: assertionKeyAlgorithm,
                assertionUsageCountLimit: assertionUsageCountLimit
            )
        case KagemushaDeviceAttestation.androidKeyMintPlatform:
            try validateAndroidKeyMintProfile(
                keyId: keyId,
                assertionScheme: assertionScheme,
                assertionKeyAlgorithm: assertionKeyAlgorithm,
                assertionPublicKey: assertionPublicKey,
                assertionUsageCountLimit: assertionUsageCountLimit
            )
        default:
            throw KagemushaDeviceAttestationError.unsupportedDeviceAttestationProfile("unsupported platform \(platform)")
        }
    }

    private static func validateIosAppAttestRegistrationKeyId(_ keyId: String) throws {
        guard let decoded = Data(base64Encoded: keyId),
              !decoded.isEmpty,
              decoded.base64EncodedString() == keyId else {
            throw KagemushaDeviceAttestationError.unsupportedDeviceAttestationProfile(
                "iOS App Attest key_id must be canonical standard base64 credential bytes"
            )
        }
    }

    private static func validateIosAppAttestProfile(assertionScheme: String,
                                                    assertionKeyAlgorithm: String,
                                                    assertionUsageCountLimit: UInt32?) throws {
        guard assertionScheme == KagemushaDeviceAttestation.iosAppAttestAssertionScheme,
              assertionKeyAlgorithm == KagemushaDeviceAttestation.iosAppAttestAssertionKeyAlgorithm,
              assertionUsageCountLimit == nil else {
            throw KagemushaDeviceAttestationError.unsupportedDeviceAttestationProfile(
                "iOS App Attest requires \(KagemushaDeviceAttestation.iosAppAttestAssertionScheme), \(KagemushaDeviceAttestation.iosAppAttestAssertionKeyAlgorithm), and no assertion usage limit"
            )
        }
    }

    private static func validateAndroidKeyMintProfile(keyId: String,
                                                      assertionScheme: String,
                                                      assertionKeyAlgorithm: String,
                                                      assertionPublicKey: Data,
                                                      assertionUsageCountLimit: UInt32?) throws {
        guard assertionScheme == KagemushaDeviceAttestation.androidKeyMintAssertionScheme,
              assertionKeyAlgorithm == KagemushaDeviceAttestation.androidKeyMintAssertionKeyAlgorithm,
              assertionUsageCountLimit == 1 else {
            throw KagemushaDeviceAttestationError.unsupportedDeviceAttestationProfile(
                "Android KeyMint requires \(KagemushaDeviceAttestation.androidKeyMintAssertionScheme), \(KagemushaDeviceAttestation.androidKeyMintAssertionKeyAlgorithm), and assertion usage limit 1"
            )
        }
        let expectedKeyId = Data(SHA256.hash(data: assertionPublicKey)).hexLowercased()
        guard keyId == expectedKeyId else {
            throw KagemushaDeviceAttestationError.unsupportedDeviceAttestationProfile(
                "Android KeyMint key_id must be lowercase hex SHA-256 of the assertion public key"
            )
        }
    }

    private static func validateP256AssertionPublicKey(_ assertionPublicKey: Data) throws {
        guard assertionPublicKey.count == 65,
              assertionPublicKey.first == 0x04 else {
            throw KagemushaDeviceAttestationError.unsupportedDeviceAttestationProfile(
                "assertion public key must be an uncompressed P-256 SEC1 point"
            )
        }
        guard (try? P256.Signing.PublicKey(x963Representation: assertionPublicKey)) != nil else {
            throw KagemushaDeviceAttestationError.unsupportedDeviceAttestationProfile(
                "assertion public key must be a valid P-256 point"
            )
        }
    }
}

enum KagemushaDeviceAttestationEncoding {
    static func wrap(typeName: String, payload: Data) -> Data {
        noritoEncode(typeName: typeName, payload: payload, flags: 2)
    }

    static func encodeDeviceAttestationRegistration(
        _ registration: OfflineDeviceAttestationRegistration
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(CompactNorito.encodeUInt16(registration.version))
        writer.writeField(CompactNorito.encodeString(registration.platform))
        writer.writeField(CompactNorito.encodeString(registration.keyId))
        writer.writeField(CompactNorito.encodeString(registration.deviceId))
        writer.writeField(try encodeAccountId(registration.accountId))
        writer.writeField(try CompactNorito.encodeOption(
            registration.assetDefinitionId,
            encode: encodeAssetDefinitionId
        ))
        writer.writeField(try CompactNorito.encodeOption(
            registration.iosTeamId,
            encode: CompactNorito.encodeString
        ))
        writer.writeField(try CompactNorito.encodeOption(
            registration.iosBundleId,
            encode: CompactNorito.encodeString
        ))
        writer.writeField(try CompactNorito.encodeOption(
            registration.iosEnvironment,
            encode: CompactNorito.encodeString
        ))
        writer.writeField(try CompactNorito.encodeOption(
            registration.androidPackageName,
            encode: CompactNorito.encodeString
        ))
        writer.writeField(try CompactNorito.encodeOption(
            registration.androidSigningCertificateSha256,
            encode: encodeBytesVec
        ))
        writer.writeField(encodeBytesVec(registration.publicKey))
        writer.writeField(CompactNorito.encodeString(registration.assertionScheme))
        writer.writeField(CompactNorito.encodeString(registration.assertionKeyAlgorithm))
        writer.writeField(encodeBytesVec(registration.assertionPublicKey))
        writer.writeField(try CompactNorito.encodeOption(
            registration.assertionUsageCountLimit,
            encode: CompactNorito.encodeUInt32
        ))
        writer.writeField(CanonicalNorito.encodeBool(registration.oneUse))
        writer.writeField(try CompactNorito.encodeHash(registration.challengeHash))
        writer.writeField(try CompactNorito.encodeHash(registration.attestationReportHash))
        writer.writeField(encodeBytesVec(registration.attestationReport))
        writer.writeField(try CompactNorito.encodeHash(registration.evidenceHash))
        writer.writeField(encodeBytesVec(registration.evidence))
        writer.writeField(CompactNorito.encodeUInt64(registration.recentBlockHeight))
        writer.writeField(try CompactNorito.encodeHash(registration.recentBlockHash))
        writer.writeField(CompactNorito.encodeUInt64(registration.expiresAtMs))
        return writer.data
    }

    fileprivate static func encodeDeviceAttestationChallengePreimage(
        _ preimage: OfflineDeviceAttestationChallengePreimage
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(CompactNorito.encodeString(KagemushaDeviceAttestation.deviceAttestationChallengeDomain))
        writer.writeField(CompactNorito.encodeUInt16(preimage.version))
        writer.writeField(CompactNorito.encodeString(preimage.platform))
        writer.writeField(CompactNorito.encodeString(preimage.keyId))
        writer.writeField(CompactNorito.encodeString(preimage.deviceId))
        writer.writeField(try encodeAccountId(preimage.accountId))
        writer.writeField(try CompactNorito.encodeOption(
            preimage.assetDefinitionId,
            encode: encodeAssetDefinitionId
        ))
        writer.writeField(try CompactNorito.encodeOption(
            preimage.iosTeamId,
            encode: CompactNorito.encodeString
        ))
        writer.writeField(try CompactNorito.encodeOption(
            preimage.iosBundleId,
            encode: CompactNorito.encodeString
        ))
        writer.writeField(try CompactNorito.encodeOption(
            preimage.iosEnvironment,
            encode: CompactNorito.encodeString
        ))
        writer.writeField(try CompactNorito.encodeOption(
            preimage.androidPackageName,
            encode: CompactNorito.encodeString
        ))
        writer.writeField(try CompactNorito.encodeOption(
            preimage.androidSigningCertificateSha256,
            encode: encodeBytesVec
        ))
        writer.writeField(encodeBytesVec(preimage.publicKey))
        writer.writeField(CompactNorito.encodeString(preimage.assertionScheme))
        writer.writeField(CompactNorito.encodeString(preimage.assertionKeyAlgorithm))
        writer.writeField(try CompactNorito.encodeOption(
            preimage.assertionUsageCountLimit,
            encode: CompactNorito.encodeUInt32
        ))
        writer.writeField(CanonicalNorito.encodeBool(preimage.oneUse))
        writer.writeField(CompactNorito.encodeUInt64(preimage.recentBlockHeight))
        writer.writeField(try CompactNorito.encodeHash(preimage.recentBlockHash))
        writer.writeField(CompactNorito.encodeUInt64(preimage.expiresAtMs))
        return writer.data
    }

    fileprivate static func encodeAndroidKeyMintChallengePreimage(
        _ preimage: OfflineAndroidKeyMintChallengePreimage
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(CompactNorito.encodeString(
            KagemushaDeviceAttestation.deviceAttestationChallengeDomain
        ))
        writer.writeField(CompactNorito.encodeUInt16(preimage.version))
        writer.writeField(CompactNorito.encodeString(
            KagemushaDeviceAttestation.androidKeyMintPlatform
        ))
        writer.writeField(CompactNorito.encodeString(preimage.deviceId))
        writer.writeField(try encodeAccountId(preimage.accountId))
        writer.writeField(try CompactNorito.encodeOption(
            preimage.assetDefinitionId,
            encode: encodeAssetDefinitionId
        ))
        writer.writeField(try CompactNorito.encodeOption(
            preimage.iosTeamId,
            encode: CompactNorito.encodeString
        ))
        writer.writeField(try CompactNorito.encodeOption(
            preimage.iosBundleId,
            encode: CompactNorito.encodeString
        ))
        writer.writeField(try CompactNorito.encodeOption(
            preimage.iosEnvironment,
            encode: CompactNorito.encodeString
        ))
        writer.writeField(try CompactNorito.encodeOption(
            preimage.androidPackageName,
            encode: CompactNorito.encodeString
        ))
        writer.writeField(try CompactNorito.encodeOption(
            preimage.androidSigningCertificateSha256,
            encode: encodeBytesVec
        ))
        writer.writeField(encodeBytesVec(preimage.publicKey))
        writer.writeField(CompactNorito.encodeString(preimage.assertionScheme))
        writer.writeField(CompactNorito.encodeString(preimage.assertionKeyAlgorithm))
        writer.writeField(try CompactNorito.encodeOption(
            preimage.assertionUsageCountLimit,
            encode: CompactNorito.encodeUInt32
        ))
        writer.writeField(CanonicalNorito.encodeBool(preimage.oneUse))
        writer.writeField(CompactNorito.encodeUInt64(preimage.recentBlockHeight))
        writer.writeField(try CompactNorito.encodeHash(preimage.recentBlockHash))
        writer.writeField(CompactNorito.encodeUInt64(preimage.expiresAtMs))
        return writer.data
    }

    private static func encodeAccountId(_ value: String) throws -> Data {
        try CanonicalNorito.encodeAccountId(value)
    }

    private static func encodeAssetDefinitionId(_ assetDefinitionId: String) throws -> Data {
        guard let definitionBytes = AssetDefinitionAddress.decode(assetDefinitionId) else {
            throw CanonicalNoritoError.invalidAssetId(assetDefinitionId)
        }
        return encodeAssetDefinitionAddress(definitionBytes)
    }

    private static func encodeAssetDefinitionAddress(_ bytes: Data) -> Data {
        var writer = CompactNoritoWriter()
        for byte in bytes {
            writer.writeLength(1)
            writer.writeUInt8(byte)
        }
        return writer.data
    }

    private static func encodeBytesVec(_ bytes: Data) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeLength(UInt64(bytes.count))
        writer.writeBytes(bytes)
        return writer.data
    }
}

public struct OfflineDeviceAttestationUnsignedTransaction: Sendable {
    public let signingHash: Data
    fileprivate let transactionPayload: Data

    public func signed(signature: Data) throws -> SignedTransactionEnvelope {
        try OfflineDeviceAttestationTransactionEncoder.finalizeUnsignedTransaction(
            transactionPayload: transactionPayload,
            signature: signature
        )
    }
}

private enum OfflineDeviceAttestationTransactionEncoder {
    private static let signedTransactionWireVersion: UInt8 = 1
    private static let instructionWireName =
        "iroha_data_model::isi::offline::RegisterOfflineDeviceAttestation"

    static func encodeUnsigned(
        request: RegisterOfflineDeviceAttestationRequest,
        creationTimeMs: UInt64
    ) throws -> OfflineDeviceAttestationUnsignedTransaction {
        let ids = try TransactionInputValidator.validate(
            chainId: request.chainId,
            authorityId: request.authority
        )
        let instruction = try encodeInstruction(registration: request.registration)
        let payload = try encodeTransactionPayload(
            chainId: ids.chainId,
            authority: ids.authorityId,
            creationTimeMs: creationTimeMs,
            ttlMs: request.ttlMs,
            nonce: request.nonce,
            instructionPayload: instruction,
            metadata: request.metadata
        )
        return OfflineDeviceAttestationUnsignedTransaction(
            signingHash: IrohaHash.hash(payload),
            transactionPayload: payload
        )
    }

    private static func encodeInstruction(
        registration: OfflineDeviceAttestationRegistration
    ) throws -> Data {
        var concrete = CompactNoritoWriter()
        concrete.writeField(
            try KagemushaDeviceAttestationEncoding.encodeDeviceAttestationRegistration(registration)
        )
        let framed = noritoEncode(
            typeName: instructionWireName,
            payload: concrete.data,
            flags: NoritoHeader.compactLen
        )
        var boxed = CanonicalNoritoWriter()
        boxed.writeField(CanonicalNorito.encodeString(instructionWireName))
        boxed.writeField(CanonicalNorito.encodeBytesVec(framed))
        return boxed.data
    }

    private static func encodeTransactionPayload(
        chainId: String,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64?,
        nonce: UInt32?,
        instructionPayload: Data,
        metadata: [String: ToriiJSONValue]
    ) throws -> Data {
        var payload = CanonicalNoritoWriter()
        payload.writeField(CanonicalNorito.encodeString(chainId))
        payload.writeField(CanonicalNorito.encodeString(authority))
        payload.writeField(CanonicalNorito.encodeUInt64(creationTimeMs))
        payload.writeField(encodeExecutable(instructionPayload))
        payload.writeField(try CanonicalNorito.encodeOption(ttlMs, encode: CanonicalNorito.encodeUInt64))
        payload.writeField(try CanonicalNorito.encodeOption(nonce, encode: CanonicalNorito.encodeUInt32))
        payload.writeField(try CanonicalNorito.encodeMetadata(metadata))
        return payload.data
    }

    private static func encodeExecutable(_ instructionPayload: Data) -> Data {
        var instructions = CanonicalNoritoWriter()
        instructions.writeLength(1)
        instructions.writeField(instructionPayload)
        var executable = CanonicalNoritoWriter()
        executable.writeUInt32LE(0)
        executable.writeField(instructions.data)
        return executable.data
    }

    fileprivate static func finalizeUnsignedTransaction(
        transactionPayload: Data,
        signature: Data
    ) throws -> SignedTransactionEnvelope {
        guard signature.count == 64, signature.contains(where: { $0 != 0 }) else {
            throw KagemushaDeviceAttestationError.nonCanonicalField(field: "transaction_signature")
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

    private static func encodeSignedTransaction(
        signature: Data,
        transactionPayload: Data
    ) -> Data {
        var signed = CanonicalNoritoWriter()
        signed.writeField(CanonicalNorito.encodeConstVec(signature))
        signed.writeField(transactionPayload)
        signed.writeField(Data([0]))
        signed.writeField(Data([0]))
        return signed.data
    }

    private static func encodeTransactionEntrypoint(_ signedTransaction: Data) -> Data {
        var entrypoint = CompactNoritoWriter()
        entrypoint.writeUInt32LE(0)
        entrypoint.writeUInt32LE(0)
        entrypoint.writeField(signedTransaction)
        return entrypoint.data
    }
}

public extension IrohaSDK {
    func buildUnsignedRegisterOfflineDeviceAttestation(
        request: RegisterOfflineDeviceAttestationRequest
    ) throws -> OfflineDeviceAttestationUnsignedTransaction {
        try OfflineDeviceAttestationTransactionEncoder.encodeUnsigned(
            request: request,
            creationTimeMs: creationTimeProvider()
        )
    }
}
