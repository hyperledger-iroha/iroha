import Foundation
import CryptoKit

public enum KagemushaDeviceAttestation {
    public static let deviceAttestationChallengeDomain = "iroha:kagemusha:device-attestation-challenge:v1"
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

public struct KagemushaDeviceAttestationRegistration: Equatable, Sendable {
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
    public let publicKey: KagemushaDevicePublicKeyV2
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
    /// Canonical chain registration identifier (`Hash(Norito(registration))`).
    public private(set) var canonicalRegistrationId = Data()

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
                publicKey: KagemushaDevicePublicKeyV2,
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
        self.canonicalRegistrationId = IrohaHash.hash(try noritoEncoded())
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
        publicKey: KagemushaDevicePublicKeyV2,
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
        publicKey: KagemushaDevicePublicKeyV2,
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
                                             challengeHash: Data? = nil) throws -> KagemushaDeviceAttestationRegistration {
        try KagemushaDeviceAttestationRegistration(
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
                                             publicKey: KagemushaDevicePublicKeyV2,
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
        publicKey: KagemushaDevicePublicKeyV2,
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
    let publicKey: KagemushaDevicePublicKeyV2
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
    let publicKey: KagemushaDevicePublicKeyV2
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

public struct RegisterKagemushaDeviceAttestationRequest: Sendable {
    public let networkId: NetworkId
    public let authority: String
    public let registration: KagemushaDeviceAttestationRegistration
    public let feePayment: FeePaymentIntent
    /// Required nonzero signature-bound transaction lifetime.
    public let ttlMs: UInt64
    public let nonce: UInt32?
    public let metadata: [String: ToriiJSONValue]

    public init(networkId: NetworkId,
                authority: String,
                registration: KagemushaDeviceAttestationRegistration,
                feePayment: FeePaymentIntent,
                ttlMs: UInt64,
                nonce: UInt32? = nil,
                metadata: [String: ToriiJSONValue] = [:]) {
        self.networkId = networkId
        self.authority = authority
        self.registration = registration
        self.feePayment = feePayment
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
                                         publicKey: KagemushaDevicePublicKeyV2,
                                         oneUse: Bool) throws {
        guard version == KagemushaDeviceAttestation.registrationVersion else {
            throw KagemushaDeviceAttestationError.invalidRegistrationVersion(version)
        }
        guard oneUse else {
            throw KagemushaDeviceAttestationError.authorityMustBeOneUse
        }
        _ = publicKey
        _ = try canonicalAccountAddress(accountId)
    }

    static func canonicalAccountAddress(_ value: String) throws -> AccountAddress {
        do {
            guard !value.isEmpty,
                  value.utf8.elementsEqual(
                      value.trimmingCharacters(in: .whitespacesAndNewlines).utf8
                  ),
                  !value.contains("@"),
                  !value.contains("#"),
                  !value.contains("$") else {
                throw KagemushaDeviceAttestationError.nonCanonicalField(
                    field: "account_id"
                )
            }
            let chainDiscriminant = try AccountAddress
                .inspectI105NetworkPrefix(value).chainDiscriminant
            let address = try AccountAddress.parseEncodedSwiftOnly(
                value,
                expectedPrefix: chainDiscriminant
            )
            let canonical = try address.toI105(
                networkPrefix: chainDiscriminant
            )
            guard canonical.utf8.elementsEqual(value.utf8) else {
                throw KagemushaDeviceAttestationError.nonCanonicalField(
                    field: "account_id"
                )
            }
            return address
        } catch let error as KagemushaDeviceAttestationError {
            throw error
        } catch {
            throw KagemushaDeviceAttestationError.nonCanonicalField(
                field: "account_id"
            )
        }
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
        _ registration: KagemushaDeviceAttestationRegistration
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
        writer.writeField(registration.publicKey.sec1Bytes)
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
        writer.writeField(preimage.publicKey.sec1Bytes)
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
        writer.writeField(preimage.publicKey.sec1Bytes)
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
        do {
            let address = try KagemushaDeviceAttestationValidation
                .canonicalAccountAddress(value)
            return try address.compactNoritoAccountControllerPayload()
        } catch {
            throw CanonicalNoritoError.invalidAccountId(value)
        }
    }

    private static func encodeAssetDefinitionId(_ assetDefinitionId: String) throws -> Data {
        guard let definitionBytes = AssetDefinitionAddress.decode(assetDefinitionId) else {
            throw CanonicalNoritoError.invalidAssetId(assetDefinitionId)
        }
        return encodeAssetDefinitionAddress(definitionBytes)
    }

    private static func encodeAssetDefinitionAddress(_ bytes: Data) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeByteFields(bytes)
        return writer.data
    }

    private static func encodeBytesVec(_ bytes: Data) -> Data {
        var writer = CompactNoritoWriter()
        // `Vec<u8>` retains its fixed-width u64 element count even when the
        // enclosing struct uses COMPACT_LEN for field framing.
        writer.writeUInt64LE(UInt64(bytes.count))
        writer.writeBytes(bytes)
        return writer.data
    }
}

public struct KagemushaDeviceAttestationUnsignedTransaction: Sendable {
    public let signingHash: Data
    fileprivate let transactionPayload: Data

    public func signed(signature: Data) throws -> SignedTransactionEnvelope {
        try KagemushaDeviceAttestationTransactionEncoder.finalizeUnsignedTransaction(
            transactionPayload: transactionPayload,
            signature: signature
        )
    }
}

public enum KagemushaDeviceAttestationSignedTransactionError: Error, LocalizedError, Equatable {
    case invalidCanonicalNorito(String)
    case registrationIdMismatch
    case networkIdMismatch
    case authorityMismatch
    case transactionHashMismatch
    case statusTransactionHashMismatch

    public var errorDescription: String? {
        switch self {
        case let .invalidCanonicalNorito(reason):
            return "Invalid canonical Kagemusha device-registration transaction: \(reason)"
        case .registrationIdMismatch:
            return "The embedded Kagemusha device registration does not match the expected registration id."
        case .networkIdMismatch:
            return "The embedded transaction NetworkId does not match the expected genesis-derived identity."
        case .authorityMismatch:
            return "The embedded transaction authority does not match the expected authority."
        case .transactionHashMismatch:
            return "The transaction payload does not match the expected canonical transaction hash."
        case .statusTransactionHashMismatch:
            return "The pipeline status transaction hash does not match the persisted transaction payload."
        }
    }
}

/// Strict inspection of persisted bytes for a signed device-registration transaction.
///
/// This type is intended for crash-safe status-first replay. It retains the exact
/// submitted bytes, recomputes their canonical payload identity, and proves that the sole
/// embedded instruction carries the expected canonical device registration.
public struct KagemushaDeviceAttestationSignedTransaction: Sendable {
    public let envelope: SignedTransactionEnvelope
    public let registrationId: Data
    public let networkId: NetworkId
    public let authority: String

    public var registrationIdHex: String {
        registrationId.hexLowercased()
    }

    public init(
        canonicalNorito: Data,
        expectedRegistrationId: Data,
        expectedNetworkId: NetworkId,
        expectedAuthority: String,
        expectedTransactionHash: Data? = nil
    ) throws {
        try KagemushaDeviceAttestationValidation.validateHash(
            expectedRegistrationId,
            field: "expected_registration_id"
        )
        let inspected = try KagemushaDeviceAttestationSignedTransactionInspector.inspect(
            canonicalNorito: canonicalNorito,
            expectedAuthority: expectedAuthority
        )
        guard inspected.registrationId == expectedRegistrationId else {
            throw KagemushaDeviceAttestationSignedTransactionError.registrationIdMismatch
        }
        if inspected.networkId != expectedNetworkId {
            throw KagemushaDeviceAttestationSignedTransactionError.networkIdMismatch
        }
        if inspected.authority != expectedAuthority {
            throw KagemushaDeviceAttestationSignedTransactionError.authorityMismatch
        }
        if let expectedTransactionHash {
            guard expectedTransactionHash.count == 32,
                  inspected.envelope.transactionHash == expectedTransactionHash else {
                throw KagemushaDeviceAttestationSignedTransactionError.transactionHashMismatch
            }
        }
        self.envelope = inspected.envelope
        self.registrationId = inspected.registrationId
        self.networkId = inspected.networkId
        self.authority = inspected.authority
    }

    /// Require a pipeline/status response to name this persisted transaction payload.
    public func validateStatusTransactionHash(_ hashHex: String) throws {
        guard hashHex.count == 64,
              hashHex == hashHex.lowercased(),
              Data(hexString: hashHex) != nil,
              hashHex == envelope.hashHex else {
            throw KagemushaDeviceAttestationSignedTransactionError.statusTransactionHashMismatch
        }
    }
}

private enum KagemushaDeviceAttestationSignedTransactionInspector {
    struct Inspection {
        let envelope: SignedTransactionEnvelope
        let registrationId: Data
        let networkId: NetworkId
        let authority: String
    }

    static func inspect(
        canonicalNorito: Data,
        expectedAuthority: String
    ) throws -> Inspection {
        do {
            return try inspectExact(
                canonicalNorito: canonicalNorito,
                expectedAuthority: expectedAuthority
            )
        } catch let error as KagemushaDeviceAttestationSignedTransactionError {
            throw error
        } catch {
            throw invalid("canonical compact-V1 signed transaction")
        }
    }

    private static func inspectExact(
        canonicalNorito: Data,
        expectedAuthority: String
    ) throws -> Inspection {
        guard canonicalNorito.first
                == KagemushaDeviceAttestationTransactionEncoder.signedTransactionWireVersion,
              canonicalNorito.count > 1 else {
            throw invalid("wire version")
        }
        guard NoritoNativeBridge.shared.decodeSignedTransaction(canonicalNorito) != nil else {
            throw invalid("Rust-decoded device-registration transaction")
        }
        let decoded = try ToriiCanonicalTransactionDraft.inspectVersionedSignedTransaction(
            canonicalNorito,
            context: "Kagemusha device-registration transaction"
        )
        let validated = try TransactionInputValidator.validate(
            networkId: decoded.payload.networkId,
            authorityId: expectedAuthority
        )
        let expectedAuthorityPayload = try CanonicalNorito.encodeCompactAccountId(
            validated.authorityId
        )
        guard decoded.payload.authority == expectedAuthorityPayload else {
            throw KagemushaDeviceAttestationSignedTransactionError.authorityMismatch
        }

        let registrationPayload = try registrationPayload(from: decoded.payload.executable)
        let registrationArchive = noritoEncode(
            typeName: KagemushaDeviceAttestationTypeNames.deviceAttestationRegistration,
            payload: registrationPayload,
            flags: NoritoHeader.compactLen
        )
        let registrationId = IrohaHash.hash(registrationArchive)

        let transactionHash = IrohaHash.hash(
            KagemushaDeviceAttestationTransactionEncoder.encodeTransactionEntrypoint(
                decoded.transactionPayload
            )
        )
        let envelope = SignedTransactionEnvelope(
            norito: canonicalNorito,
            signedTransaction: Data(canonicalNorito.dropFirst()),
            payload: nil,
            transactionHash: transactionHash
        )
        return Inspection(
            envelope: envelope,
            registrationId: registrationId,
            networkId: decoded.payload.networkId,
            authority: validated.authorityId
        )
    }

    private static func registrationPayload(from executable: Data) throws -> Data {
        var executableReader = CanonicalNoritoReader(data: executable)
        guard try executableReader.readUInt32LE() == 0 else {
            throw invalid("transaction executable")
        }
        let instructions = try compactField(&executableReader, "instructions")
        try finish(executableReader, "transaction executable")

        var instructionsReader = CanonicalNoritoReader(data: instructions)
        guard try instructionsReader.readUInt64LE() == 1 else {
            throw invalid("device registration instruction count")
        }
        let instruction = try compactField(
            &instructionsReader,
            "device registration instruction"
        )
        try finish(instructionsReader, "instructions")

        var instructionReader = CanonicalNoritoReader(data: instruction)
        let nameField = try compactField(&instructionReader, "instruction name")
        let archiveField = try compactField(&instructionReader, "instruction archive")
        try finish(instructionReader, "instruction")
        let name = try ToriiCanonicalTransactionDraft.decodeString(
            nameField,
            field: "instruction name"
        )
        guard name == KagemushaDeviceAttestationTransactionEncoder.instructionWireId else {
            throw invalid("instruction name")
        }
        let framedInstruction = try bytesVec(archiveField, field: "instruction archive")
        guard let frame = noritoDecodeFrame(framedInstruction),
              frame.header.schema == noritoSchemaHash(
                  forTypeName: KagemushaDeviceAttestationTransactionEncoder.instructionSchemaName
              ),
              frame.header.compression == .none,
              frame.header.flags == NoritoHeader.compactLen,
              frame.paddingLength == 0,
              noritoEncode(
                  typeName: KagemushaDeviceAttestationTransactionEncoder.instructionSchemaName,
                  payload: frame.payload,
                  flags: NoritoHeader.compactLen
              ) == framedInstruction else {
            throw invalid("instruction Norito frame")
        }
        var concreteReader = CanonicalNoritoReader(data: frame.payload)
        let registration = try compactField(&concreteReader, "registration")
        try finish(concreteReader, "registration instruction")
        guard !registration.isEmpty else {
            throw invalid("empty registration")
        }
        return registration
    }

    private static func bytesVec(_ data: Data, field name: String) throws -> Data {
        var reader = CanonicalNoritoReader(data: data)
        let length = try reader.readUInt64LE()
        guard length <= UInt64(Int.max) else {
            throw invalid(name)
        }
        let bytes = try reader.readBytes(Int(length))
        try finish(reader, name)
        return bytes
    }

    private static func compactField(
        _ reader: inout CanonicalNoritoReader,
        _ name: String
    ) throws -> Data {
        do {
            return try reader.readCompactField()
        } catch {
            throw invalid(name)
        }
    }

    private static func finish(_ reader: CanonicalNoritoReader, _ name: String) throws {
        guard reader.remaining() == 0 else {
            throw invalid("trailing bytes in \(name)")
        }
    }

    private static func invalid(
        _ reason: String
    ) -> KagemushaDeviceAttestationSignedTransactionError {
        .invalidCanonicalNorito(reason)
    }
}

private enum KagemushaDeviceAttestationTransactionEncoder {
    fileprivate static let signedTransactionWireVersion: UInt8 = 1
    fileprivate static let instructionWireId =
        "iroha.offline.device_attestation.register"
    fileprivate static let instructionSchemaName =
        "iroha_data_model::isi::offline::RegisterOfflineDeviceAttestation"

    static func encodeUnsigned(
        request: RegisterKagemushaDeviceAttestationRequest,
        creationTimeMs: UInt64
    ) throws -> KagemushaDeviceAttestationUnsignedTransaction {
        guard creationTimeMs > 0 else {
            throw KagemushaDeviceAttestationError.nonCanonicalField(
                field: "creation_time_ms"
            )
        }
        guard request.ttlMs > 0 else {
            throw KagemushaDeviceAttestationError.nonCanonicalField(field: "ttl_ms")
        }
        guard request.nonce.map({ $0 > 0 }) != false else {
            throw KagemushaDeviceAttestationError.nonCanonicalField(field: "nonce")
        }
        let ids = try TransactionInputValidator.validate(
            networkId: request.networkId,
            authorityId: request.authority
        )
        do {
            _ = try ToriiCanonicalTransactionDraft.decodeEd25519Authority(
                CanonicalNorito.encodeCompactAccountId(ids.authorityId),
                context: "Kagemusha device-registration authority"
            )
        } catch {
            throw KagemushaDeviceAttestationError.nonCanonicalField(
                field: "authority_ed25519_single_key"
            )
        }
        let instruction = try encodeInstruction(registration: request.registration)
        let payload = try encodeTransactionPayload(
            networkId: ids.networkId,
            authority: ids.authorityId,
            creationTimeMs: creationTimeMs,
            ttlMs: request.ttlMs,
            nonce: request.nonce,
            feePayment: request.feePayment,
            instructionPayload: instruction,
            metadata: request.metadata
        )
        do {
            try ToriiCanonicalTransactionDraft.validateTransactionPayload(
                payload,
                context: "Kagemusha device-registration transaction"
            )
        } catch {
            throw KagemushaDeviceAttestationError.nonCanonicalField(
                field: "transaction_payload_v1"
            )
        }
        return KagemushaDeviceAttestationUnsignedTransaction(
            signingHash: IrohaHash.hash(payload),
            transactionPayload: payload
        )
    }

    private static func encodeInstruction(
        registration: KagemushaDeviceAttestationRegistration
    ) throws -> Data {
        var concrete = CompactNoritoWriter()
        concrete.writeField(
            try KagemushaDeviceAttestationEncoding.encodeDeviceAttestationRegistration(registration)
        )
        let framed = noritoEncode(
            typeName: instructionSchemaName,
            payload: concrete.data,
            flags: NoritoHeader.compactLen
        )
        var boxed = CompactNoritoWriter()
        boxed.writeField(CompactNorito.encodeString(instructionWireId))
        boxed.writeField(CompactNorito.encodeBytesVec(framed))
        return boxed.data
    }

    private static func encodeTransactionPayload(
        networkId: NetworkId,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64,
        nonce: UInt32?,
        feePayment: FeePaymentIntent,
        instructionPayload: Data,
        metadata: [String: ToriiJSONValue]
    ) throws -> Data {
        var domain = CompactNoritoWriter()
        domain.writeUInt32LE(0)
        domain.writeField(networkId.bytes)
        var payload = CompactNoritoWriter()
        payload.writeField(domain.data)
        payload.writeField(try CanonicalNorito.encodeCompactAccountId(authority))
        payload.writeField(CompactNorito.encodeUInt64(creationTimeMs))
        payload.writeField(encodeExecutable(instructionPayload))
        payload.writeField(
            try CompactNorito.encodeOption(Optional(ttlMs), encode: CompactNorito.encodeUInt64)
        )
        payload.writeField(try CompactNorito.encodeOption(nonce, encode: CompactNorito.encodeUInt32))
        payload.writeField(try feePayment.compactNorito())
        payload.writeField(TransactionAdmissionIntentV1.queuePlanSynced.norito)
        payload.writeField(try ToriiCanonicalTransactionDraft.compactMetadata(metadata))
        payload.writeField(Data([0]))
        return payload.data
    }

    private static func encodeExecutable(_ instructionPayload: Data) -> Data {
        var instructions = CompactNoritoWriter()
        instructions.writeUInt64LE(1)
        instructions.writeField(instructionPayload)
        var executable = CompactNoritoWriter()
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
        let transactionHash = IrohaHash.hash(encodeTransactionEntrypoint(transactionPayload))
        var norito = Data([signedTransactionWireVersion])
        norito.append(signedTransaction)
        do {
            _ = try ToriiCanonicalTransactionDraft.inspectVersionedSignedTransaction(
                norito,
                context: "Kagemusha device-registration transaction"
            )
        } catch {
            throw KagemushaDeviceAttestationError.nonCanonicalField(
                field: "transaction_signature"
            )
        }
        guard NoritoNativeBridge.shared.decodeSignedTransaction(norito) != nil else {
            throw KagemushaDeviceAttestationError.nonCanonicalField(
                field: "signed_transaction_v1"
            )
        }
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
        var transactionSignature = CompactNoritoWriter()
        transactionSignature.writeField(CompactNorito.encodeConstVec(signature))
        var signed = CompactNoritoWriter()
        signed.writeField(transactionSignature.data)
        signed.writeField(transactionPayload)
        signed.writeField(Data([0]))
        return signed.data
    }

    fileprivate static func encodeTransactionEntrypoint(_ transactionPayload: Data) -> Data {
        var entrypoint = CompactNoritoWriter()
        entrypoint.writeUInt32LE(0)
        entrypoint.writeField(transactionPayload)
        return entrypoint.data
    }
}

public extension IrohaSDK {
    func buildUnsignedRegisterKagemushaDeviceAttestation(
        request: RegisterKagemushaDeviceAttestationRequest
    ) throws -> KagemushaDeviceAttestationUnsignedTransaction {
        try KagemushaDeviceAttestationTransactionEncoder.encodeUnsigned(
            request: request,
            creationTimeMs: creationTimeProvider()
        )
    }
}
