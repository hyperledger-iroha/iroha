// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.util.Base64
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.crypto.IrohaHash

/**
 * Strict first-release model for one finalized platform device attestation.
 *
 * This mirrors `OfflineDeviceAttestationRegistration` exactly and exposes only the canonical
 * Kagemusha model. Native attestation acquisition uses bridge ABI 22;
 * the on-chain registration format marker remains version 1.
 */
internal class DeviceAttestationRegistration(
    val version: Int,
    val platform: String,
    val keyId: String,
    val deviceId: String,
    val accountId: String,
    val assetDefinitionId: String?,
    val iosTeamId: String?,
    val iosBundleId: String?,
    val iosEnvironment: String?,
    val androidPackageName: String?,
    androidSigningCertificateSha256: ByteArray?,
    /** Hardware-authenticated Android properties; absent for iOS and synthetic codec fixtures. */
    val androidAttestedDeviceProperties: OfflineAndroidAttestedDevicePropertiesV2? = null,
    val publicKey: KagemushaDevicePublicKeyV2,
    val assertionScheme: String,
    val assertionKeyAlgorithm: String,
    assertionPublicKey: ByteArray,
    val assertionUsageCountLimit: Int?,
    val oneUse: Boolean,
    challengeHash: ByteArray?,
    attestationReportHash: ByteArray?,
    attestationReport: ByteArray,
    evidenceHash: ByteArray?,
    evidence: ByteArray?,
    val recentBlockHeight: Long,
    recentBlockHash: ByteArray,
    val expiresAtMs: Long,
) {
    private val _androidSigningCertificateSha256 = androidSigningCertificateSha256?.copyOf()
    private val _assertionPublicKey = assertionPublicKey.copyOf()
    private val _attestationReport = attestationReport.copyOf()
    private val _recentBlockHash = recentBlockHash.copyOf()
    private val _challengeHash: ByteArray
    private val _attestationReportHash: ByteArray
    private val _evidence: ByteArray
    private val _evidenceHash: ByteArray

    val androidSigningCertificateSha256: ByteArray?
        get() = _androidSigningCertificateSha256?.copyOf()
    val assertionPublicKey: ByteArray get() = _assertionPublicKey.copyOf()
    val challengeHash: ByteArray get() = _challengeHash.copyOf()
    val attestationReportHash: ByteArray get() = _attestationReportHash.copyOf()
    val attestationReport: ByteArray get() = _attestationReport.copyOf()
    val evidenceHash: ByteArray get() = _evidenceHash.copyOf()
    val evidence: ByteArray get() = _evidence.copyOf()
    val recentBlockHash: ByteArray get() = _recentBlockHash.copyOf()

    init {
        requireExactText(platform, "platform")
        requireExactText(keyId, "key_id")
        requireExactText(deviceId, "device_id")
        requireExactText(accountId, "account_id")
        requireOptionalExactText(iosTeamId, "ios_team_id")
        requireOptionalExactText(iosBundleId, "ios_bundle_id")
        requireOptionalExactText(iosEnvironment, "ios_environment")
        requireOptionalExactText(androidPackageName, "android_package_name")
        requireExactText(assertionScheme, "assertion_scheme")
        requireExactText(assertionKeyAlgorithm, "assertion_key_algorithm")
        requireCore()
        requirePlatformProfile()

        val expectedChallengeHash = OfflineDeviceAttestationCodec.canonicalChallengeHash(this)
        challengeHash?.let {
            requireHash(it, "challenge_hash")
            require(it.contentEquals(expectedChallengeHash)) {
                "challenge_hash does not match the canonical attestation preimage"
            }
        }
        _challengeHash = expectedChallengeHash

        val expectedReportHash = IrohaHash.prehash(_attestationReport)
        val resolvedReportHash = attestationReportHash?.copyOf() ?: expectedReportHash
        requireHash(resolvedReportHash, "attestation_report_hash")
        require(resolvedReportHash.contentEquals(expectedReportHash)) {
            "attestation_report_hash does not match attestation_report"
        }
        _attestationReportHash = resolvedReportHash

        val submittedEvidence = evidence?.copyOf() ?: ByteArray(0)
        _evidence = if (submittedEvidence.isEmpty() && evidenceHash == null) {
            evidenceEnvelope(resolvedReportHash)
        } else {
            submittedEvidence
        }
        requireEvidenceEnvelope(_evidence, resolvedReportHash)
        require(_evidence.size <= MAX_EVIDENCE_BYTES) {
            "evidence exceeds the on-chain size limit"
        }
        val expectedEvidenceHash = IrohaHash.prehash(_evidence)
        val resolvedEvidenceHash = evidenceHash?.copyOf() ?: expectedEvidenceHash
        requireHash(resolvedEvidenceHash, "evidence_hash")
        require(resolvedEvidenceHash.contentEquals(expectedEvidenceHash)) {
            "evidence_hash does not match evidence"
        }
        _evidenceHash = resolvedEvidenceHash
    }

    /** Encode this registration as the exact current framed Norito archive. */
    fun noritoEncoded(): ByteArray = OfflineDeviceAttestationCodec.encodeRegistration(this)

    /** Deterministic challenge hash that the platform report must bind. */
    fun canonicalChallengeHash(): ByteArray =
        OfflineDeviceAttestationCodec.canonicalChallengeHash(this)

    /** Canonical Iroha Hash/registration ID of the exact framed Norito registration archive. */
    fun canonicalRegistrationHash(): ByteArray = IrohaHash.prehash(noritoEncoded())

    private fun requireCore() {
        require(version == REGISTRATION_VERSION) { "registration version must be exactly 1" }
        require(oneUse) { "device attestation authority must be one-use" }
        OfflineDeviceAttestationCodec.validateAccountId(accountId)
        assetDefinitionId?.let(AssetDefinitionIdEncoder::parseAddressBytes)
        require(recentBlockHeight > 0) { "recent_block_height must be positive" }
        requireHash(_recentBlockHash, "recent_block_hash")
        require(expiresAtMs > 0) { "expires_at_ms must be positive" }
        require(_attestationReport.isNotEmpty() && _attestationReport.size <= MAX_REPORT_BYTES) {
            "attestation_report must be non-empty and within the on-chain size limit"
        }
    }

    private fun requirePlatformProfile() {
        KagemushaP256Codec.requireUncompressedPublicKey(_assertionPublicKey)
        when (platform) {
            ANDROID_KEYMINT_PLATFORM -> {
                require(
                    assertionScheme == ANDROID_KEYMINT_ASSERTION_SCHEME &&
                        assertionKeyAlgorithm == ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM &&
                        assertionUsageCountLimit == 1,
                ) { "Android KeyMint requires the canonical one-use P-256 assertion profile" }
                require(androidPackageName != null) {
                    "Android KeyMint requires android_package_name"
                }
                val signingDigest = _androidSigningCertificateSha256
                require(signingDigest != null && signingDigest.size == 32 && !allZero(signingDigest)) {
                    "Android KeyMint requires a non-zero 32-byte signing certificate SHA-256"
                }
                require(iosTeamId == null && iosBundleId == null && iosEnvironment == null) {
                    "Android KeyMint must not carry iOS app metadata"
                }
                require(keyId == hexLower(sha256(_assertionPublicKey))) {
                    "Android KeyMint key_id must be lowercase SHA-256 of assertion_public_key"
                }
            }
            IOS_APP_ATTEST_PLATFORM -> {
                require(
                    assertionScheme == IOS_APP_ATTEST_ASSERTION_SCHEME &&
                        assertionKeyAlgorithm == IOS_APP_ATTEST_ASSERTION_KEY_ALGORITHM &&
                        assertionUsageCountLimit == null,
                ) { "iOS App Attest requires the canonical P-256 assertion profile" }
                val decoded = try {
                    Base64.getDecoder().decode(keyId)
                } catch (ex: IllegalArgumentException) {
                    throw IllegalArgumentException("iOS App Attest key_id must be canonical base64", ex)
                }
                require(decoded.isNotEmpty() && Base64.getEncoder().encodeToString(decoded) == keyId) {
                    "iOS App Attest key_id must be canonical base64"
                }
                require(iosTeamId != null && iosBundleId != null && iosEnvironment != null) {
                    "iOS App Attest requires complete app metadata"
                }
                require(iosEnvironment == "production" || iosEnvironment == "development") {
                    "ios_environment must be production or development"
                }
                require(
                    androidPackageName == null &&
                        _androidSigningCertificateSha256 == null &&
                        androidAttestedDeviceProperties == null,
                ) {
                    "iOS App Attest must not carry Android app metadata"
                }
            }
            else -> throw IllegalArgumentException(
                "unsupported device attestation platform: $platform",
            )
        }
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is DeviceAttestationRegistration) return false
        return version == other.version &&
            platform == other.platform &&
            keyId == other.keyId &&
            deviceId == other.deviceId &&
            accountId == other.accountId &&
            assetDefinitionId == other.assetDefinitionId &&
            iosTeamId == other.iosTeamId &&
            iosBundleId == other.iosBundleId &&
            iosEnvironment == other.iosEnvironment &&
            androidPackageName == other.androidPackageName &&
            nullableBytesEqual(_androidSigningCertificateSha256, other._androidSigningCertificateSha256) &&
            androidAttestedDeviceProperties == other.androidAttestedDeviceProperties &&
            publicKey == other.publicKey &&
            assertionScheme == other.assertionScheme &&
            assertionKeyAlgorithm == other.assertionKeyAlgorithm &&
            _assertionPublicKey.contentEquals(other._assertionPublicKey) &&
            assertionUsageCountLimit == other.assertionUsageCountLimit &&
            oneUse == other.oneUse &&
            _challengeHash.contentEquals(other._challengeHash) &&
            _attestationReportHash.contentEquals(other._attestationReportHash) &&
            _attestationReport.contentEquals(other._attestationReport) &&
            _evidenceHash.contentEquals(other._evidenceHash) &&
            _evidence.contentEquals(other._evidence) &&
            recentBlockHeight == other.recentBlockHeight &&
            _recentBlockHash.contentEquals(other._recentBlockHash) &&
            expiresAtMs == other.expiresAtMs
    }

    override fun hashCode(): Int {
        var result = version
        for (value in listOf<Any?>(
            platform,
            keyId,
            deviceId,
            accountId,
            assetDefinitionId,
            iosTeamId,
            iosBundleId,
            iosEnvironment,
            androidPackageName,
            androidAttestedDeviceProperties,
            publicKey,
            assertionScheme,
            assertionKeyAlgorithm,
            assertionUsageCountLimit,
            oneUse,
            recentBlockHeight,
            expiresAtMs,
        )) {
            result = 31 * result + (value?.hashCode() ?: 0)
        }
        for (value in listOfNotNull(
            _androidSigningCertificateSha256,
            _assertionPublicKey,
            _challengeHash,
            _attestationReportHash,
            _attestationReport,
            _evidenceHash,
            _evidence,
            _recentBlockHash,
        )) {
            result = 31 * result + value.contentHashCode()
        }
        return result
    }

    companion object {
        /** Sole native bridge ABI supported by the first-release client. */
        const val REQUIRED_NATIVE_BRIDGE_ABI_VERSION: Int = 22
        /** Sole on-chain registration format marker. */
        const val REGISTRATION_VERSION: Int = 1
        const val ANDROID_KEYMINT_PLATFORM: String = "android-keymint"
        const val ANDROID_KEYMINT_ASSERTION_SCHEME: String =
            "android-keymint-ecdsa-p256-usage-limit-v1"
        const val ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM: String = "ecdsa-p256-sha256"
        const val IOS_APP_ATTEST_PLATFORM: String = "ios-appattest"
        const val IOS_APP_ATTEST_ASSERTION_SCHEME: String = "apple-appattest-counter-v1"
        const val IOS_APP_ATTEST_ASSERTION_KEY_ALGORITHM: String = "app-attest-p256"
        const val DEVICE_ATTESTATION_CHALLENGE_DOMAIN: String =
            "iroha:kagemusha:device-attestation-challenge:v1"
        const val DEVICE_ATTESTATION_EVIDENCE_PREFIX: String =
            "offline-device-attestation-evidence-v1"

        private const val MAX_REPORT_BYTES = 64 * 1024
        private const val MAX_EVIDENCE_BYTES = 128 * 1024
        private val EVIDENCE_PREFIX_BYTES =
            DEVICE_ATTESTATION_EVIDENCE_PREFIX.toByteArray(StandardCharsets.UTF_8)

        /** Decode a canonical framed registration and reject alternate representations. */
        @JvmStatic
        fun decodeCanonical(
            archive: ByteArray,
            chainDiscriminant: Int,
        ): DeviceAttestationRegistration =
            OfflineDeviceAttestationCodec.decodeRegistrationCanonical(
                archive,
                chainDiscriminant,
            )

        /** Build the canonical Android challenge before KeyMint creates the assertion key. */
        @JvmStatic
        fun androidPreKeyGenerationChallengeHash(
            version: Int,
            deviceId: String,
            accountId: String,
            assetDefinitionId: String?,
            androidPackageName: String,
            androidSigningCertificateSha256: ByteArray,
            publicKey: KagemushaDevicePublicKeyV2,
            recentBlockHeight: Long,
            recentBlockHash: ByteArray,
            expiresAtMs: Long,
        ): ByteArray = OfflineDeviceAttestationCodec.androidPreKeyGenerationChallengeHash(
            version,
            deviceId,
            accountId,
            assetDefinitionId,
            androidPackageName,
            androidSigningCertificateSha256,
            publicKey.sec1Bytes(),
            recentBlockHeight,
            recentBlockHash,
            expiresAtMs,
        )

        internal fun requireHash(value: ByteArray, field: String) {
            require(value.size == 32 && (value[31].toInt() and 1) == 1) {
                "$field must be a canonical 32-byte Iroha hash"
            }
        }

        private fun evidenceEnvelope(reportHash: ByteArray): ByteArray =
            EVIDENCE_PREFIX_BYTES + reportHash

        private fun requireEvidenceEnvelope(value: ByteArray, reportHash: ByteArray) {
            require(value.size == EVIDENCE_PREFIX_BYTES.size + 32) {
                "evidence must bind exactly one attestation report hash"
            }
            require(value.copyOfRange(0, EVIDENCE_PREFIX_BYTES.size)
                .contentEquals(EVIDENCE_PREFIX_BYTES)) {
                "evidence prefix is not canonical"
            }
            require(value.copyOfRange(EVIDENCE_PREFIX_BYTES.size, value.size)
                .contentEquals(reportHash)) {
                "evidence does not bind attestation_report_hash"
            }
        }

        private fun requireExactText(value: String, field: String) {
            require(value.isNotEmpty() && value == value.trim()) {
                "$field must be exact non-empty text"
            }
        }

        private fun requireOptionalExactText(value: String?, field: String) {
            value?.let { requireExactText(it, field) }
        }

        private fun sha256(value: ByteArray): ByteArray =
            MessageDigest.getInstance("SHA-256").digest(value)

        private fun hexLower(value: ByteArray): String = value.joinToString(separator = "") {
            "%02x".format(it.toInt() and 0xff)
        }

        private fun allZero(value: ByteArray): Boolean {
            var aggregate = 0
            for (item in value) aggregate = aggregate or item.toInt()
            return aggregate == 0
        }

        private fun nullableBytesEqual(left: ByteArray?, right: ByteArray?): Boolean =
            if (left == null || right == null) left == null && right == null
            else left.contentEquals(right)
    }
}
