package org.hyperledger.iroha.sdk.offline

import java.net.URI
import java.nio.charset.StandardCharsets
import java.util.ArrayList
import java.util.Collections
import java.util.concurrent.CompletableFuture
import java.util.concurrent.locks.ReentrantLock
import org.hyperledger.iroha.sdk.client.CanonicalRequestSigner
import org.hyperledger.iroha.sdk.client.transport.TransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.client.LocalSigningContext
import org.hyperledger.iroha.sdk.client.ToriiCanonicalRequestAuth
import org.hyperledger.iroha.sdk.client.ZkMerklePathEntry
import org.hyperledger.iroha.sdk.client.ZkMerklePathResponse
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash

private const val DRAIN_ONLY_REDEEM_WIRE_ID_V4 =
    "iroha_data_model::isi::offline::RedeemKagemushaRecursiveV4"

/**
 * Native bridge ABI 22 for Kagemusha ABI-21/V4 artifact streaming and capabilities.
 *
 * This is the sole first-release offline-cash surface. It authenticates the opaque eight-file proof
 * artifact set and validates exact typed request/payment/acknowledgement and proof-bound membership
 * archives. Proof execution remains fail-closed while the native backend reports unavailable.
 * Every recursive lifecycle result is projected only through an ABI-21/V4 native decoder.
 */
class KagemushaRecursiveSpendProver private constructor() {
    /** Retryable contention signal raised before a second proof request is copied. */
    class ProofWorkerBusyException internal constructor(
        message: String,
        cause: Throwable? = null,
    ) : IllegalStateException(message, cause)

    /** Caller-owned trust for the finalized device-policy BridgeFinalityProof. */
    class OfflineDeviceFinalityTrustAnchorV1(
        @JvmField val networkId: NetworkId,
        trustedHeightContextId: ByteArray,
    ) {
        private val contextId = trustedHeightContextId.copyOf()

        init {
            require(contextId.size == 32 && (contextId[31].toInt() and 1) == 1) {
                "trustedHeightContextId must be one exact marked 32-byte Iroha hash"
            }
        }

        fun trustedHeightContextId(): ByteArray = contextId.copyOf()

        override fun equals(other: Any?): Boolean =
            this === other || other is OfflineDeviceFinalityTrustAnchorV1 &&
                networkId == other.networkId && contextId.contentEquals(other.contextId)

        override fun hashCode(): Int = 31 * networkId.hashCode() + contextId.contentHashCode()
    }

    /** Caller-owned durable checkpoint for one device-policy proof page. */
    class OfflineDevicePolicyCheckpointV1(
        @JvmField val networkId: NetworkId,
        @JvmField val height: Long,
        heightContextId: ByteArray,
    ) {
        private val contextId = heightContextId.copyOf()

        init {
            require(height > 0) { "height must be positive" }
            require(contextId.size == 32 && (contextId[31].toInt() and 1) == 1) {
                "heightContextId must be one exact marked 32-byte Iroha hash"
            }
        }

        fun heightContextId(): ByteArray = contextId.copyOf()

        override fun equals(other: Any?): Boolean =
            this === other || other is OfflineDevicePolicyCheckpointV1 &&
                networkId == other.networkId && height == other.height &&
                contextId.contentEquals(other.contextId)

        override fun hashCode(): Int =
            31 * (31 * networkId.hashCode() + height.hashCode()) + contextId.contentHashCode()
    }

    /** Natively verified page plus the exact checkpoint eligible for durable promotion. */
    class OfflineDevicePolicyVerifiedPageV1 internal constructor(
        projection: ByteArray,
        expectedNetworkId: NetworkId,
    ) {
        @JvmField val evaluatedCheckpoint: OfflineDevicePolicyCheckpointV1
        @JvmField val moreAvailable: Boolean
        @JvmField val terminalPolicyView: DeviceAttestationPolicyViewV1?

        init {
            require(
                projection.size in VERIFIED_POLICY_PAGE_FIXED_BYTES_V1..
                    MAX_DEVICE_POLICY_VERIFIED_PAGE_BYTES_V1,
            ) { "verified policy page projection has an invalid size" }
            require(
                projection.copyOfRange(0, VERIFIED_POLICY_PAGE_MAGIC_V1.size)
                    .contentEquals(VERIFIED_POLICY_PAGE_MAGIC_V1),
            ) { "verified policy page projection has an invalid discriminator" }
            require(projection[8].toInt() and 0x80 == 0) {
                "evaluated policy height exceeds the maintained SDK range"
            }
            var height = 0L
            for (index in 8 until 16) {
                height = (height shl 8) or (projection[index].toLong() and 0xff)
            }
            val context = projection.copyOfRange(16, 48)
            val moreByte = projection[48].toInt() and 0xff
            require(moreByte in 0..1 && projection.sliceArray(49 until 52).all { it == 0.toByte() }) {
                "verified policy page projection has invalid flags"
            }
            var policyLength = 0L
            for (index in 52 until 56) {
                policyLength = (policyLength shl 8) or (projection[index].toLong() and 0xff)
            }
            require(
                policyLength <= MAX_DEVICE_ATTESTATION_POLICY_VIEW_ARCHIVE_BYTES_V1.toLong() &&
                    VERIFIED_POLICY_PAGE_FIXED_BYTES_V1.toLong() + policyLength ==
                    projection.size.toLong(),
            ) { "verified policy page projection has an invalid terminal-policy length" }
            moreAvailable = moreByte == 1
            require(moreAvailable == (policyLength == 0L)) {
                "verified policy page pagination and terminal policy disagree"
            }
            evaluatedCheckpoint = OfflineDevicePolicyCheckpointV1(
                expectedNetworkId,
                height,
                context,
            )
            terminalPolicyView = if (policyLength == 0L) {
                null
            } else {
                DeviceAttestationPolicyViewV1(
                    projection.copyOfRange(VERIFIED_POLICY_PAGE_FIXED_BYTES_V1, projection.size),
                    OfflineDeviceFinalityTrustAnchorV1(expectedNetworkId, context),
                )
            }
        }
    }

    /** Exact protected-state selector; canonical request auth supplies the account identity. */
    class OfflineDeviceEligibilityRequestV1(
        registrationHash: ByteArray,
        @JvmField val deviceId: String,
        @JvmField val attestationKeyId: String,
        @JvmField val requestedTtlMilliseconds: Long,
    ) {
        private val registrationHashValue = registrationHash.copyOf()

        init {
            val deviceBytes = deviceId.toByteArray(StandardCharsets.UTF_8)
            val keyBytes = attestationKeyId.toByteArray(StandardCharsets.UTF_8)
            require(registrationHashValue.size == 32 && registrationHashValue.any { it != 0.toByte() }) {
                "registrationHash must be one non-zero 32-byte protected-state key"
            }
            require(
                deviceBytes.isNotEmpty() && deviceBytes.size <= MAX_DEVICE_ELIGIBILITY_DEVICE_ID_BYTES_V1 &&
                    deviceId == deviceId.trim() && deviceId.none(Character::isISOControl),
            ) { "deviceId must be canonical bounded UTF-8" }
            require(
                keyBytes.isNotEmpty() &&
                    keyBytes.size <= MAX_DEVICE_ELIGIBILITY_ATTESTATION_KEY_ID_BYTES_V1 &&
                    attestationKeyId == attestationKeyId.trim() &&
                    attestationKeyId.none(Character::isISOControl),
            ) { "attestationKeyId must be canonical bounded UTF-8" }
            require(requestedTtlMilliseconds in 1..MAX_DEVICE_ELIGIBILITY_CREDENTIAL_TTL_MS_V1) {
                "requestedTtlMilliseconds must be within the 24-hour credential limit"
            }
        }

        fun registrationHash(): ByteArray = registrationHashValue.copyOf()

        override fun equals(other: Any?): Boolean =
            this === other || other is OfflineDeviceEligibilityRequestV1 &&
                registrationHashValue.contentEquals(other.registrationHashValue) &&
                deviceId == other.deviceId && attestationKeyId == other.attestationKeyId &&
                requestedTtlMilliseconds == other.requestedTtlMilliseconds

        override fun hashCode(): Int =
            31 * (
                31 * (
                    31 * registrationHashValue.contentHashCode() + deviceId.hashCode()
                ) + attestationKeyId.hashCode()
            ) + requestedTtlMilliseconds.hashCode()
    }

    enum class OfflineDeviceEligibilityOutcomeV1 { ELIGIBLE, DRAIN_ONLY, CRYPTOGRAPHICALLY_REJECTED }

    enum class OfflineDeviceEligibilityReasonV1 {
        POLICY_SATISFIED,
        CRYPTOGRAPHIC_ATTESTATION_REJECTED,
        POLICY_NOT_FRESH,
        INCOMPLETE_ATTESTED_PROPERTIES,
        UNSUPPORTED_PRE_ANDROID_12_TEE,
        VULNERABLE_FIRMWARE,
        PERMANENTLY_BLOCKED_DEVICE,
    }

    class OfflineDeviceEligibilityDecisionV1 internal constructor(
        @JvmField val outcome: OfflineDeviceEligibilityOutcomeV1,
        @JvmField val reason: OfflineDeviceEligibilityReasonV1,
        matchedRuleIds: List<String>,
    ) {
        @JvmField val matchedRuleIds: List<String> =
            Collections.unmodifiableList(ArrayList(matchedRuleIds))
    }

    class OfflineDeviceEligibilityAdmissionProvenanceV1 internal constructor(
        registrationHash: ByteArray,
        admissionPolicyHash: ByteArray,
        @JvmField val admissionHeight: Long,
        admissionTransactionHash: ByteArray,
    ) {
        private val registrationHashValue = registrationHash.copyOf()
        private val admissionPolicyHashValue = admissionPolicyHash.copyOf()
        private val admissionTransactionHashValue = admissionTransactionHash.copyOf()

        fun registrationHash(): ByteArray = registrationHashValue.copyOf()
        fun admissionPolicyHash(): ByteArray = admissionPolicyHashValue.copyOf()
        fun admissionTransactionHash(): ByteArray = admissionTransactionHashValue.copyOf()
    }

    class OfflineDevicePolicyFinalityClaimsV1 internal constructor(
        @JvmField val finalizedBlockHeight: Long,
        finalizedBlockHash: ByteArray,
        @JvmField val finalizedBlockTimestampMilliseconds: Long,
        finalityEvidenceHash: ByteArray,
    ) {
        private val finalizedBlockHashValue = finalizedBlockHash.copyOf()
        private val finalityEvidenceHashValue = finalityEvidenceHash.copyOf()
        fun finalizedBlockHash(): ByteArray = finalizedBlockHashValue.copyOf()
        fun finalityEvidenceHash(): ByteArray = finalityEvidenceHashValue.copyOf()
    }

    class OfflineDeviceEligibilityPolicyClaimsV1 internal constructor(
        @JvmField val policyEpoch: Long,
        policyHash: ByteArray,
        @JvmField val freshnessDeadlineMilliseconds: Long,
        @JvmField val finality: OfflineDevicePolicyFinalityClaimsV1,
    ) {
        private val policyHashValue = policyHash.copyOf()
        fun policyHash(): ByteArray = policyHashValue.copyOf()
    }

    /**
     * Public claims projected from one natively verified, finalized device-policy view.
     *
     * The fixed ABI-22 projection is deliberately smaller than the policy archive: callers can
     * use these claims as a registration trust context only after the native bridge has verified
     * the canonical policy, its exact block binding, and the Sumeragi finality proof against a
     * caller-owned network and height-context anchor.
     */
    class OfflineDeviceAttestationPolicyViewClaimsV1 internal constructor(projection: ByteArray) {
        @JvmField val policyEpoch: Long
        @JvmField val freshnessDeadlineMilliseconds: Long
        @JvmField val finalizedBlockHeight: Long
        @JvmField val finalizedBlockTimestampMilliseconds: Long
        private val policyHashValue: ByteArray
        private val finalizedBlockHashValue: ByteArray
        private val finalityEvidenceHashValue: ByteArray

        init {
            require(
                projection.size == OFFLINE_DEVICE_POLICY_VIEW_CLAIMS_BYTES_V1 &&
                    projection.copyOfRange(0, 8)
                        .contentEquals(OFFLINE_DEVICE_POLICY_VIEW_CLAIMS_MAGIC_V1),
            ) { "offline device policy claims projection has invalid framing" }
            policyEpoch = readProjectionUInt64(projection, 8)
            policyHashValue = projection.copyOfRange(16, 48)
            freshnessDeadlineMilliseconds = readProjectionUInt64(projection, 48)
            finalizedBlockHeight = readProjectionUInt64(projection, 56)
            finalizedBlockHashValue = projection.copyOfRange(64, 96)
            finalizedBlockTimestampMilliseconds = readProjectionUInt64(projection, 96)
            finalityEvidenceHashValue = projection.copyOfRange(104, 136)
            require(
                policyEpoch > 0 && policyHashValue.any { it != 0.toByte() } &&
                    freshnessDeadlineMilliseconds > finalizedBlockTimestampMilliseconds &&
                    finalizedBlockHeight > 0 &&
                    finalizedBlockHashValue.any { it != 0.toByte() } &&
                    finalizedBlockTimestampMilliseconds > 0 &&
                    finalityEvidenceHashValue.any { it != 0.toByte() },
            ) { "offline device policy claims projection is invalid" }
        }

        fun policyHash(): ByteArray = policyHashValue.copyOf()

        fun finalizedBlockHash(): ByteArray = finalizedBlockHashValue.copyOf()

        fun finalityEvidenceHash(): ByteArray = finalityEvidenceHashValue.copyOf()
    }

    class OfflineDeviceEligibilityCredentialClaimsV1 internal constructor(
        @JvmField val accountId: String,
        @JvmField val deviceId: String,
        @JvmField val attestationKeyId: String,
        devicePublicKey: ByteArray,
        assertionPublicKey: ByteArray,
        @JvmField val issuedAtMilliseconds: Long,
        @JvmField val expiresAtMilliseconds: Long,
    ) {
        private val devicePublicKeyValue = devicePublicKey.copyOf()
        private val assertionPublicKeyValue = assertionPublicKey.copyOf()
        fun devicePublicKey(): ByteArray = devicePublicKeyValue.copyOf()
        fun assertionPublicKey(): ByteArray = assertionPublicKeyValue.copyOf()
    }

    /** Native-verified public decision, issuer, optional credential, policy, and provenance. */
    class OfflineDeviceEligibilityResponseV1 internal constructor(
        projection: ByteArray,
        responseArchive: ByteArray,
        expectedRegistrationHash: ByteArray,
        trustAnchor: OfflineDeviceFinalityTrustAnchorV1,
    ) {
        @JvmField val decision: OfflineDeviceEligibilityDecisionV1
        @JvmField val issuer: EligibilityIssuerPublicKeyV1
        @JvmField val credential: EligibilityCredentialV1?
        @JvmField val finalizedPolicy: DeviceAttestationPolicyViewV1
        @JvmField val policyClaims: OfflineDeviceEligibilityPolicyClaimsV1
        @JvmField val credentialClaims: OfflineDeviceEligibilityCredentialClaimsV1?
        @JvmField val admission: OfflineDeviceEligibilityAdmissionProvenanceV1
        private val responseArchiveValue = responseArchive.copyOf()

        init {
            require(
                responseArchiveValue.isNotEmpty() &&
                    responseArchiveValue.size <= MAX_DEVICE_ELIGIBILITY_RESPONSE_ARCHIVE_BYTES_V1 &&
                projection.size in VERIFIED_ELIGIBILITY_RESPONSE_FIXED_BYTES_V1..
                    MAX_DEVICE_ELIGIBILITY_VERIFIED_RESPONSE_BYTES_V1,
            ) { "verified eligibility response projection has an invalid size" }
            require(
                projection.copyOfRange(0, 8).contentEquals(VERIFIED_ELIGIBILITY_RESPONSE_MAGIC_V1) &&
                    projection[11] == 0.toByte() &&
                    projection.copyOfRange(118, 120).all { it == 0.toByte() } &&
                    projection.copyOfRange(290, 292).all { it == 0.toByte() },
            ) { "verified eligibility response projection has invalid framing" }
            val outcome = OfflineDeviceEligibilityOutcomeV1.entries.getOrNull(
                projection[8].toInt() and 0xff,
            ) ?: throw IllegalArgumentException("eligibility outcome is invalid")
            val reason = OfflineDeviceEligibilityReasonV1.entries.getOrNull(
                projection[9].toInt() and 0xff,
            ) ?: throw IllegalArgumentException("eligibility reason is invalid")
            val credentialFlag = projection[10].toInt() and 0xff
            require(credentialFlag in 0..1) { "eligibility credential flag is invalid" }
            val admissionHeight = readProjectionUInt64(projection, 12)
            val registrationHash = projection.copyOfRange(20, 52)
            val admissionPolicyHash = projection.copyOfRange(52, 84)
            val admissionTransactionHash = projection.copyOfRange(84, 116)
            require(
                admissionHeight > 0 && registrationHash.contentEquals(expectedRegistrationHash) &&
                    registrationHash.any { it != 0.toByte() } &&
                    admissionPolicyHash.any { it != 0.toByte() } &&
                    admissionTransactionHash.any { it != 0.toByte() },
            ) { "eligibility admission provenance is invalid" }
            val matchedCount = readProjectionUInt16(projection, 116)
            val matchedLength = readProjectionUInt32(projection, 120)
            val issuerLength = readProjectionUInt32(projection, 124)
            val credentialLength = readProjectionUInt32(projection, 128)
            val policyLength = readProjectionUInt32(projection, 132)
            val policyEpoch = readProjectionUInt64(projection, 136)
            val policyHash = projection.copyOfRange(144, 176)
            val freshnessDeadline = readProjectionUInt64(projection, 176)
            val finalizedBlockHeight = readProjectionUInt64(projection, 184)
            val finalizedBlockHash = projection.copyOfRange(192, 224)
            val finalizedBlockTimestamp = readProjectionUInt64(projection, 224)
            val finalityEvidenceHash = projection.copyOfRange(232, 264)
            val credentialIssuedAt = readProjectionUInt64(projection, 264)
            val credentialExpiresAt = readProjectionUInt64(projection, 272)
            val claimLengths = listOf(
                readProjectionUInt16(projection, 280),
                readProjectionUInt16(projection, 282),
                readProjectionUInt16(projection, 284),
                readProjectionUInt16(projection, 286),
                readProjectionUInt16(projection, 288),
            )
            val claimsLength = readProjectionUInt32(projection, 292)
            require(
                policyEpoch > 0 && policyHash.any { it != 0.toByte() } &&
                    freshnessDeadline > finalizedBlockTimestamp &&
                    finalizedBlockHeight >= admissionHeight &&
                    finalizedBlockHash.any { it != 0.toByte() } &&
                    finalizedBlockTimestamp > 0 && finalityEvidenceHash.any { it != 0.toByte() } &&
                    claimLengths.sumOf { it.toLong() } == claimsLength,
            ) { "eligibility response claims are invalid" }
            var expectedLength = VERIFIED_ELIGIBILITY_RESPONSE_FIXED_BYTES_V1.toLong()
            for (
                length in listOf(
                    matchedLength,
                    issuerLength,
                    credentialLength,
                    policyLength,
                    claimsLength,
                )
            ) {
                expectedLength = Math.addExact(expectedLength, length)
            }
            require(
                expectedLength == projection.size.toLong() &&
                    issuerLength in 1..MAX_ELIGIBILITY_ISSUER_ARCHIVE_BYTES_V1.toLong() &&
                    credentialLength in 0..MAX_ELIGIBILITY_CREDENTIAL_ARCHIVE_BYTES_V1.toLong() &&
                    policyLength in 1..MAX_DEVICE_ATTESTATION_POLICY_VIEW_ARCHIVE_BYTES_V1.toLong() &&
                    (credentialFlag == 1) == (credentialLength > 0) &&
                    (outcome == OfflineDeviceEligibilityOutcomeV1.ELIGIBLE) ==
                    (credentialLength > 0) &&
                    (credentialLength == 0L) ==
                    (credentialIssuedAt == 0L && credentialExpiresAt == 0L &&
                        claimLengths.all { it == 0 }),
            ) { "eligibility response sections or credential presence are invalid" }

            var cursor = VERIFIED_ELIGIBILITY_RESPONSE_FIXED_BYTES_V1
            val matchedEnd = Math.addExact(cursor, matchedLength.toInt())
            val matchedRuleIds = decodeMatchedRuleProjection(
                projection.copyOfRange(cursor, matchedEnd),
                matchedCount,
            )
            cursor = matchedEnd
            val issuerEnd = Math.addExact(cursor, issuerLength.toInt())
            val issuerArchive = projection.copyOfRange(cursor, issuerEnd)
            cursor = issuerEnd
            val credentialEnd = Math.addExact(cursor, credentialLength.toInt())
            val credentialArchive = projection.copyOfRange(cursor, credentialEnd)
            cursor = credentialEnd
            val policyEnd = Math.addExact(cursor, policyLength.toInt())
            val policyArchive = projection.copyOfRange(cursor, policyEnd)
            cursor = policyEnd
            val claimsBytes = projection.copyOfRange(cursor, projection.size)
            requireEligibilityDecisionProjection(outcome, reason, matchedRuleIds)

            decision = OfflineDeviceEligibilityDecisionV1(outcome, reason, matchedRuleIds)
            issuer = EligibilityIssuerPublicKeyV1(issuerArchive)
            if (credentialArchive.isEmpty()) {
                credential = null
                credentialClaims = null
            } else {
                require(
                    credentialIssuedAt > 0 && credentialExpiresAt > credentialIssuedAt &&
                        claimLengths[3] == 65 && claimLengths[4] == 65,
                ) { "eligibility credential claims are invalid" }
                val sections = ArrayList<ByteArray>(claimLengths.size)
                var claimCursor = 0
                for (length in claimLengths) {
                    val end = Math.addExact(claimCursor, length)
                    require(end <= claimsBytes.size) { "eligibility credential claims are truncated" }
                    sections.add(claimsBytes.copyOfRange(claimCursor, end))
                    claimCursor = end
                }
                require(
                    claimCursor == claimsBytes.size && sections[3].firstOrNull() == 0x04.toByte() &&
                        sections[4].firstOrNull() == 0x04.toByte(),
                ) { "eligibility credential public-key claims are invalid" }
                credential = EligibilityCredentialV1(credentialArchive, trustAnchor)
                credentialClaims = OfflineDeviceEligibilityCredentialClaimsV1(
                    decodeCanonicalProjectionString(sections[0], "accountId"),
                    decodeCanonicalProjectionString(sections[1], "deviceId"),
                    decodeCanonicalProjectionString(sections[2], "attestationKeyId"),
                    sections[3],
                    sections[4],
                    credentialIssuedAt,
                    credentialExpiresAt,
                )
            }
            finalizedPolicy = DeviceAttestationPolicyViewV1(policyArchive, trustAnchor)
            policyClaims = OfflineDeviceEligibilityPolicyClaimsV1(
                policyEpoch,
                policyHash,
                freshnessDeadline,
                OfflineDevicePolicyFinalityClaimsV1(
                    finalizedBlockHeight,
                    finalizedBlockHash,
                    finalizedBlockTimestamp,
                    finalityEvidenceHash,
                ),
            )
            admission = OfflineDeviceEligibilityAdmissionProvenanceV1(
                registrationHash,
                admissionPolicyHash,
                admissionHeight,
                admissionTransactionHash,
            )
        }

        /** Exact canonical Torii response, retained only after native verification. */
        fun noritoEncoded(): ByteArray = responseArchiveValue.copyOf()
    }

    /** Canonical ABI-21 artifact roles. Declaration order is part of the native contract. */
    enum class ArtifactRoleV4(val fileName: String) {
        STEP_EQ_PARAMS_IPA("step-eq.params-ipa.krv4"),
        STEP_EQ_PROVING_KEY("step-eq.proving-key.krv4"),
        STEP_EQ_VERIFYING_KEY("step-eq.verifying-key.krv4"),
        STEP_EQ_BOOTSTRAP_WITNESS("step-eq.bootstrap-witness.krv4"),
        STEP_EP_PARAMS_IPA("step-ep.params-ipa.krv4"),
        STEP_EP_PROVING_KEY("step-ep.proving-key.krv4"),
        STEP_EP_VERIFYING_KEY("step-ep.verifying-key.krv4"),
        STEP_EP_BOOTSTRAP_WITNESS("step-ep.bootstrap-witness.krv4"),
    }

    /** Closed first-release hardware assertion profiles for online operations. */
    enum class OnlineHardwareAssertionPlatform(val wireName: String) {
        ANDROID_KEYMINT(DeviceAttestationRegistration.ANDROID_KEYMINT_PLATFORM),
        IOS_APP_ATTEST(DeviceAttestationRegistration.IOS_APP_ATTEST_PLATFORM),
    }

    companion object {
        const val V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION: Int = 22
        const val REQUIRED_NATIVE_BRIDGE_ABI_VERSION: Int = V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION
        /** Frozen recursive proof/artifact ABI carried unchanged through the ABI-22 bridge. */
        const val REQUIRED_RECURSIVE_PROOF_ARTIFACT_ABI_VERSION: Int = 21
        /** Mandatory sender-final peer-cash handoff/finality contract. */
        const val CASH_HANDOFF_CAPABILITY_V1: String = "cash_handoff_v1"
        /** Eligibility-gated handoff advertised separately from the legacy capability. */
        const val CASH_HANDOFF_ELIGIBILITY_CAPABILITY_V1: String =
            "cash_handoff_eligibility_v1"
        const val V4_ARTIFACT_MANIFEST_VERSION: Int = 4
        const val V4_ARTIFACT_MANIFEST_SCHEMA: String =
            "kagemusha.offline.recursive_spend.artifact_manifest.v4"
        const val ARTIFACT_MANIFEST_SCHEMA: String = V4_ARTIFACT_MANIFEST_SCHEMA
        val V4_ARTIFACT_FILES: List<String> = listOf(
            "step-eq.params-ipa.krv4",
            "step-eq.proving-key.krv4",
            "step-eq.verifying-key.krv4",
            "step-eq.bootstrap-witness.krv4",
            "step-ep.params-ipa.krv4",
            "step-ep.proving-key.krv4",
            "step-ep.verifying-key.krv4",
            "step-ep.bootstrap-witness.krv4",
        )
        val ARTIFACT_FILES: List<String> = V4_ARTIFACT_FILES
        const val V4_ARTIFACT_COUNT: Int = 8
        const val ARTIFACT_COUNT: Int = V4_ARTIFACT_COUNT
        const val MAX_MANIFEST_BYTES: Int = 1024 * 1024
        const val MAX_ARTIFACT_CHUNK_BYTES: Int = 1024 * 1024
        const val MAX_TRUSTED_RELEASE_POLICY_BYTES: Int = 64 * 1024
        const val MAX_RELEASE_ATTESTATION_BYTES: Int = 1024 * 1024
        const val MAX_RELEASE_EVIDENCE_BYTES: Int = 16 * 1024 * 1024
        const val MAX_PROMOTION_RECORD_BYTES: Int = 1024 * 1024
        const val MAX_PEER_TEXT_ENVELOPE_BYTES: Int = 12 * 1024
        const val MAX_PEER_TEXT_ARCHIVE_BYTES: Int =
            (MAX_PEER_TEXT_ENVELOPE_BYTES - 6) * 3 / 4
        const val MAX_PEER_ARCHIVE_BYTES_V2: Int = 32 * 1024
        const val MAX_RECIPIENT_RECEIVE_OFFER_BYTES_V2: Int = 24_576
        const val MAX_PUBLISHER_CHECKPOINT_ENVELOPE_BYTES_V1: Int = 2 * 1024
        const val PROMOTED_FINALITY_CHECKPOINT_BYTES_V2: Int = 40
        /** Consensus ceiling for one canonical recipient-only ABI-21 peer archive. */
        const val MAX_PEER_ARCHIVE_BYTES_V4: Int = 32 * 1024 * 1024
        const val MAX_PEER_ARCHIVE_BYTES: Int = MAX_PEER_ARCHIVE_BYTES_V4
        const val MAX_ELIGIBILITY_CREDENTIAL_ARCHIVE_BYTES_V1: Int = 64 * 1024
        const val MAX_ELIGIBILITY_PAYMENT_ENVELOPE_ARCHIVE_BYTES_V1: Int =
            MAX_PEER_ARCHIVE_BYTES_V4 + MAX_ELIGIBILITY_CREDENTIAL_ARCHIVE_BYTES_V1 +
                64 * 1024
        const val MAX_DEVICE_ATTESTATION_POLICY_VIEW_ARCHIVE_BYTES_V1: Int =
            8 * 1024 * 1024 + 256 * 1024 + 64 * 1024
        const val MAX_DEVICE_POLICY_PROOF_PAGE_ARCHIVE_BYTES_V1: Int =
            MAX_DEVICE_ATTESTATION_POLICY_VIEW_ARCHIVE_BYTES_V1 + 3 * 1024 * 1024 + 64 * 1024
        const val MAX_DEVICE_POLICY_VERIFIED_PAGE_BYTES_V1: Int =
            MAX_DEVICE_ATTESTATION_POLICY_VIEW_ARCHIVE_BYTES_V1 + 56
        const val MAX_DEVICE_ELIGIBILITY_REQUEST_ARCHIVE_BYTES_V1: Int = 8 * 1024
        const val MAX_DEVICE_ELIGIBILITY_RESPONSE_ARCHIVE_BYTES_V1: Int =
            MAX_DEVICE_ATTESTATION_POLICY_VIEW_ARCHIVE_BYTES_V1 +
                MAX_ELIGIBILITY_CREDENTIAL_ARCHIVE_BYTES_V1 + 64 * 1024
        const val MAX_DEVICE_ELIGIBILITY_VERIFIED_RESPONSE_BYTES_V1: Int =
            MAX_DEVICE_ELIGIBILITY_RESPONSE_ARCHIVE_BYTES_V1 + 64 * 1024
        const val MAX_DEVICE_ELIGIBILITY_CREDENTIAL_TTL_MS_V1: Long = 24L * 60 * 60 * 1000
        const val MAX_DEVICE_ELIGIBILITY_DEVICE_ID_BYTES_V1: Int = 128
        const val MAX_DEVICE_ELIGIBILITY_ATTESTATION_KEY_ID_BYTES_V1: Int = 64
        private const val VERIFIED_POLICY_PAGE_FIXED_BYTES_V1: Int = 56
        private val VERIFIED_POLICY_PAGE_MAGIC_V1: ByteArray =
            byteArrayOf(0x49, 0x44, 0x50, 0x50, 0x56, 0x31, 0, 0)
        private const val OFFLINE_DEVICE_POLICY_VIEW_CLAIMS_BYTES_V1: Int = 136
        private val OFFLINE_DEVICE_POLICY_VIEW_CLAIMS_MAGIC_V1: ByteArray =
            byteArrayOf(0x49, 0x44, 0x50, 0x56, 0x43, 0x4c, 0x31, 0)
        private const val VERIFIED_ELIGIBILITY_RESPONSE_FIXED_BYTES_V1: Int = 296
        private val VERIFIED_ELIGIBILITY_RESPONSE_MAGIC_V1: ByteArray =
            byteArrayOf(0x49, 0x44, 0x45, 0x52, 0x53, 0x50, 0x31, 0)
        private const val MAX_ELIGIBILITY_ISSUER_ARCHIVE_BYTES_V1: Int = 4 * 1024
        /** Consensus-derived ceiling for one canonical ABI-21 top-up provenance archive. */
        const val MAX_TOP_UP_PROVENANCE_ARCHIVE_BYTES_V4: Int = 6_488_064
        /** Largest V4 local verify carrier accepted by native, plus framing headroom. */
        const val MAX_LOCAL_REQUEST_ARCHIVE_BYTES_V4: Int = 64 * 1024 * 1024 + 64
        const val MAX_LOCAL_RESULT_ARCHIVE_BYTES_V4: Int = 64 * 1024 * 1024 + 64
        const val MAX_LOCAL_REQUEST_ARCHIVE_BYTES: Int = MAX_LOCAL_REQUEST_ARCHIVE_BYTES_V4
        const val MAX_LOCAL_RESULT_ARCHIVE_BYTES: Int = MAX_LOCAL_RESULT_ARCHIVE_BYTES_V4
        /** Exact Torii body ceiling for the ABI-21/V4 top-up route. */
        const val MAX_TORII_TOP_UP_REQUEST_BYTES_V4: Int = 512 * 1024

        /** Exact Torii body ceiling for the ABI-21/V4 redemption route. */
        const val MAX_TORII_REDEEM_REQUEST_BYTES_V4: Int = 48 * 1024 * 1024

        private const val MAX_REQUEST_AUTHORIZATION_BYTES: Int = 512 * 1024
        private const val IOS_APP_ATTEST_ASSERTION_OBJECT_MAX_BYTES: Int = 8 * 1024
        private const val IOS_APP_ATTEST_AUTHENTICATOR_DATA_FIXED_HEADER_BYTES: Int = 37
        private const val IOS_APP_ATTEST_AUTHENTICATOR_DATA_MIN_BYTES: Int =
            IOS_APP_ATTEST_AUTHENTICATOR_DATA_FIXED_HEADER_BYTES + 1
        private const val IOS_APP_ATTEST_AUTHENTICATOR_DATA_MAX_BYTES: Int = 4 * 1024
        private const val IOS_APP_ATTEST_EXTENSION_DATA_FLAG: Int = 0x80
        const val MAX_TORII_RESPONSE_BYTES: Int = 4 * 1024 * 1024
        const val MAXIMUM_INPUTS_PER_TRANSITION: Int = 2
        const val MAXIMUM_LOCAL_APPEND_BUILDER_INPUTS: Int = MAXIMUM_INPUTS_PER_TRANSITION
        const val MAXIMUM_BRANCH_CLAIMS: Int = 2
        const val MAXIMUM_PEER_HOPS: Int = 8
        const val MAXIMUM_PROOF_STEPS: Int = 128
        const val MAXIMUM_RECURSIVE_PROOF_PAIR_BYTES_V4: Int = 384 * 1024
        const val CONFIDENTIAL_TREE_DEPTH: Int = 16
        const val MAX_OUTPUT_MEMBERSHIP_FRONTIER_ARCHIVE_BYTES_V4: Int = 4 * 1024
        const val MAX_OUTPUT_MEMBERSHIP_PATHS_ARCHIVE_BYTES_V4: Int = 16 * 1024

        private const val EXACT_STATE_PROJECTION_VERSION: Int = 1

        private const val LIBRARY_NAME = "connect_norito_bridge"
        private val artifactBridgeAvailable = loadArtifactBridge()
        private val heavyProofPermit = ReentrantLock()
        private const val NATIVE_BUSY_MESSAGE = " is busy; retry after the active proof completes"

        private inline fun <T> withHeavyProofPermit(label: String, action: () -> T): T {
            if (!heavyProofPermit.tryLock()) {
                throw ProofWorkerBusyException(
                    "Kagemusha $label is busy; retry after the active proof completes",
                )
            }
            try {
                return action()
            } catch (failure: ProofWorkerBusyException) {
                throw failure
            } catch (failure: IllegalStateException) {
                if (failure.message.orEmpty().contains(NATIVE_BUSY_MESSAGE)) {
                    throw ProofWorkerBusyException(
                        "Kagemusha $label is busy; retry after the active proof completes",
                        failure,
                    )
                }
                throw failure
            } finally {
                heavyProofPermit.unlock()
            }
        }

        internal fun withHeavyProofPermitForTest(action: () -> Unit) {
            withHeavyProofPermit("test", action)
        }

        private inline fun <T> transferChangeOpeningOwnership(
            changeOpening: NoteOpening?,
            transfer: (NoteOpening?) -> T,
        ): T {
            var locallyOwned = changeOpening
            return try {
                transfer(locallyOwned).also { locallyOwned = null }
            } finally {
                locallyOwned?.destroy()
            }
        }

        internal fun requireCanonicalV4ArtifactRoleInventory(roles: List<ArtifactRoleV4>) {
            require(roles.size == ArtifactRoleV4.entries.size) {
                "artifact roles must contain exactly eight entries"
            }
            require(roles == ArtifactRoleV4.entries) {
                "artifact roles are not in canonical V4 order"
            }
        }

        @JvmStatic
        fun isArtifactStreamingAvailable(): Boolean = artifactBridgeAvailable

        /**
         * True only when the linked bridge was compiled with the non-default production
         * Kagemusha capability. Unlike [isProofBackendAvailable], this remains true before an
         * authenticated artifact set is installed and is therefore safe for setup bootstrapping.
         */
        @JvmStatic
        fun isProductionProofBackendCompiled(): Boolean =
            artifactBridgeAvailable && detectProductionProofBackendCompilation {
                nativeArtifactBeginV4(byteArrayOf(0), ByteArray(32), ByteArray(32))
            }

        @JvmStatic
        fun isProofBackendAvailable(): Boolean =
            artifactBridgeAvailable && runCatching { nativePastaCycleV4BackendAvailable() }
                .getOrDefault(false)

        /** SHA-256 of the exact authenticated release held by native, or null when absent. */
        @JvmStatic
        fun installedArtifactManifestSha256V4(): ByteArray? =
            if (!artifactBridgeAvailable) {
                null
            } else {
                runCatching {
                    requireDigest(nativeInstalledManifestSha256V4(), "installedManifestSha256")
                }.getOrNull()
            }

        @JvmStatic
        fun beginArtifactIngest(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
            expectedArtifactSha256: ByteArray,
        ): ArtifactIngest {
            requireArtifactBridge()
            val manifest = requireManifest(manifestNorito)
            val manifestDigest = requireDigest(manifestSha256, "manifestSha256")
            val artifactDigest = requireDigest(expectedArtifactSha256, "expectedArtifactSha256")
            val handle = nativeArtifactBeginV4(manifest, manifestDigest, artifactDigest)
            check(handle > 0) { "native Kagemusha artifact ingest returned no handle" }
            return ArtifactIngest(handle)
        }

        @JvmStatic
        fun beginArtifactInstallSession(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
            releaseAuthentication: ReleaseAuthentication,
        ): ArtifactInstallSession {
            requireArtifactBridge()
            return ArtifactInstallSession(
                requireManifest(manifestNorito),
                requireDigest(manifestSha256, "manifestSha256"),
                releaseAuthentication,
            )
        }

        @JvmStatic
        fun decodeRecipientPaymentRequest(archive: ByteArray): RecipientPaymentRequest =
            RecipientPaymentRequest(archive)

        @JvmStatic
        fun decodeRecipientRegistrationLineageV2(
            archive: ByteArray,
        ): RecipientRegistrationLineage = RecipientRegistrationLineage(archive)

        @JvmStatic
        fun decodeRecipientReceiveOfferV2(archive: ByteArray): RecipientReceiveOfferV2 =
            RecipientReceiveOfferV2(archive).also(::projectRecipientReceiveOfferV2)

        /** Restore an exact persisted readiness archive and force native canonical validation. */
        @JvmStatic
        fun decodeReadiness(archive: ByteArray): Readiness =
            Readiness(archive).also(::projectReadiness)

        @JvmStatic
        fun decodePeerPayment(archive: ByteArray): PeerPayment = PeerPayment(archive)

        @JvmStatic
        fun decodeEligibilityCredentialV1(archive: ByteArray): EligibilityCredentialV1 =
            EligibilityCredentialV1(archive)

        @JvmStatic
        fun decodeEligibilityIssuerPublicKeyV1(
            archive: ByteArray,
        ): EligibilityIssuerPublicKeyV1 = EligibilityIssuerPublicKeyV1(archive)

        @JvmStatic
        fun decodeDeviceAttestationPolicyViewV1(
            archive: ByteArray,
        ): DeviceAttestationPolicyViewV1 = DeviceAttestationPolicyViewV1(archive)

        /**
         * Canonically decode and authenticate a finalized V2 policy view.
         * The context anchor is caller-owned and is never learned from the archive.
         */
        @JvmStatic
        fun verifyDeviceAttestationPolicyViewV1(
            archive: ByteArray,
            trustAnchor: OfflineDeviceFinalityTrustAnchorV1,
            evaluationTimeMilliseconds: Long,
        ): DeviceAttestationPolicyViewV1 {
            require(evaluationTimeMilliseconds > 0) {
                "evaluationTimeMilliseconds must be positive"
            }
            requireArtifactBridge()
            val canonical = nativeVerifyOfflineDeviceAttestationPolicyViewV1(
                archive.copyOf(),
                trustAnchor.networkId.bytes(),
                trustAnchor.trustedHeightContextId(),
                evaluationTimeMilliseconds,
            )
            require(canonical.contentEquals(archive)) {
                "native policy verifier changed canonical policy-view bytes"
            }
            return DeviceAttestationPolicyViewV1(canonical, trustAnchor)
        }

        /**
         * Reverify [policyView] and project its exact finalized policy/block binding.
         *
         * A decoded but unverified policy view is rejected because it carries no caller-owned
         * trust anchor. Native code performs the finality verification again before returning the
         * fixed public projection; no Torii status field participates in this trust decision.
         */
        @JvmStatic
        fun projectDeviceAttestationPolicyViewClaimsV1(
            policyView: DeviceAttestationPolicyViewV1,
            evaluationTimeMilliseconds: Long,
        ): OfflineDeviceAttestationPolicyViewClaimsV1 {
            require(evaluationTimeMilliseconds > 0) {
                "evaluationTimeMilliseconds must be positive"
            }
            val trustAnchor = checkNotNull(policyView.verificationTrustAnchor) {
                "policy view must be returned by the native finalized-policy verifier"
            }
            requireArtifactBridge()
            return OfflineDeviceAttestationPolicyViewClaimsV1(
                nativeProjectOfflineDeviceAttestationPolicyViewClaimsV1(
                    policyView.noritoEncoded(),
                    trustAnchor.networkId.bytes(),
                    trustAnchor.trustedHeightContextId(),
                    evaluationTimeMilliseconds,
                ),
            )
        }

        private fun readProjectionUInt16(bytes: ByteArray, offset: Int): Int {
            require(offset >= 0 && offset + 2 <= bytes.size)
            return ((bytes[offset].toInt() and 0xff) shl 8) or
                (bytes[offset + 1].toInt() and 0xff)
        }

        private fun readProjectionUInt32(bytes: ByteArray, offset: Int): Long {
            require(offset >= 0 && offset + 4 <= bytes.size)
            var result = 0L
            for (index in offset until offset + 4) {
                result = (result shl 8) or (bytes[index].toLong() and 0xff)
            }
            return result
        }

        private fun readProjectionUInt64(bytes: ByteArray, offset: Int): Long {
            require(offset >= 0 && offset + 8 <= bytes.size)
            require((bytes[offset].toInt() and 0x80) == 0) {
                "projection value exceeds the maintained signed Long range"
            }
            var result = 0L
            for (index in offset until offset + 8) {
                result = (result shl 8) or (bytes[index].toLong() and 0xff)
            }
            return result
        }

        private fun decodeMatchedRuleProjection(bytes: ByteArray, expectedCount: Int): List<String> {
            val rules = ArrayList<String>(expectedCount)
            var cursor = 0
            while (cursor < bytes.size) {
                require(cursor + 2 <= bytes.size) { "matched-rule projection is truncated" }
                val length = readProjectionUInt16(bytes, cursor)
                cursor += 2
                require(length > 0 && cursor + length <= bytes.size) {
                    "matched-rule projection has an invalid length"
                }
                val rule = decodeCanonicalProjectionString(
                    bytes.copyOfRange(cursor, cursor + length),
                    "matchedRuleId",
                )
                rules.add(rule)
                cursor += length
            }
            require(rules.size == expectedCount && rules.zipWithNext().all { it.first < it.second }) {
                "matched-rule projection count or ordering is invalid"
            }
            return Collections.unmodifiableList(rules)
        }

        private fun decodeCanonicalProjectionString(bytes: ByteArray, field: String): String {
            require(bytes.isNotEmpty()) { "$field must not be empty" }
            val value = bytes.toString(StandardCharsets.UTF_8)
            require(
                value.toByteArray(StandardCharsets.UTF_8).contentEquals(bytes) &&
                    value == value.trim() && value.none(Character::isISOControl),
            ) { "$field is not canonical UTF-8" }
            return value
        }

        private fun requireEligibilityDecisionProjection(
            outcome: OfflineDeviceEligibilityOutcomeV1,
            reason: OfflineDeviceEligibilityReasonV1,
            matchedRuleIds: List<String>,
        ) {
            val valid = when (Triple(outcome, reason, matchedRuleIds.isEmpty())) {
                Triple(
                    OfflineDeviceEligibilityOutcomeV1.ELIGIBLE,
                    OfflineDeviceEligibilityReasonV1.POLICY_SATISFIED,
                    true,
                ),
                Triple(
                    OfflineDeviceEligibilityOutcomeV1.CRYPTOGRAPHICALLY_REJECTED,
                    OfflineDeviceEligibilityReasonV1.CRYPTOGRAPHIC_ATTESTATION_REJECTED,
                    true,
                ),
                Triple(
                    OfflineDeviceEligibilityOutcomeV1.DRAIN_ONLY,
                    OfflineDeviceEligibilityReasonV1.POLICY_NOT_FRESH,
                    true,
                ),
                Triple(
                    OfflineDeviceEligibilityOutcomeV1.DRAIN_ONLY,
                    OfflineDeviceEligibilityReasonV1.INCOMPLETE_ATTESTED_PROPERTIES,
                    true,
                ),
                Triple(
                    OfflineDeviceEligibilityOutcomeV1.DRAIN_ONLY,
                    OfflineDeviceEligibilityReasonV1.UNSUPPORTED_PRE_ANDROID_12_TEE,
                    true,
                ),
                Triple(
                    OfflineDeviceEligibilityOutcomeV1.DRAIN_ONLY,
                    OfflineDeviceEligibilityReasonV1.VULNERABLE_FIRMWARE,
                    false,
                ),
                Triple(
                    OfflineDeviceEligibilityOutcomeV1.DRAIN_ONLY,
                    OfflineDeviceEligibilityReasonV1.PERMANENTLY_BLOCKED_DEVICE,
                    false,
                ),
                -> true
                else -> false
            }
            require(valid) { "eligibility decision projection is inconsistent" }
        }

        /** Encode one exact typed policy-proof request from caller-owned trust. */
        @JvmStatic
        fun makeOfflineDevicePolicyProofRequestV1(
            checkpoint: OfflineDevicePolicyCheckpointV1,
        ): ByteArray {
            requireArtifactBridge()
            return nativeEncodeOfflineDevicePolicyProofRequestV1(
                checkpoint.height,
                checkpoint.heightContextId(),
            ).also { require(it.isNotEmpty()) { "native policy proof request is empty" } }.copyOf()
        }

        /**
         * Verify one proof page. Persist [OfflineDevicePolicyVerifiedPageV1.evaluatedCheckpoint]
         * atomically before using it for another page; native and this SDK own no checkpoint store.
         */
        @JvmStatic
        fun verifyOfflineDevicePolicyProofPageV1(
            archive: ByteArray,
            checkpoint: OfflineDevicePolicyCheckpointV1,
            evaluationTimeMilliseconds: Long,
        ): OfflineDevicePolicyVerifiedPageV1 {
            require(
                archive.isNotEmpty() && archive.size <= MAX_DEVICE_POLICY_PROOF_PAGE_ARCHIVE_BYTES_V1,
            ) { "policy proof page exceeds its canonical response bound" }
            require(evaluationTimeMilliseconds > 0) {
                "evaluationTimeMilliseconds must be positive"
            }
            requireArtifactBridge()
            val projection = nativeVerifyOfflineDevicePolicyProofV1(
                archive.copyOf(),
                checkpoint.networkId.bytes(),
                checkpoint.height,
                checkpoint.heightContextId(),
                evaluationTimeMilliseconds,
            )
            return OfflineDevicePolicyVerifiedPageV1(projection, checkpoint.networkId)
        }

        /** Encode the exact authenticated POST body without duplicating the caller account. */
        @JvmStatic
        fun makeOfflineDeviceEligibilityRequestV1(
            request: OfflineDeviceEligibilityRequestV1,
        ): ByteArray {
            requireArtifactBridge()
            return nativeEncodeOfflineDeviceEligibilityRequestV1(
                request.registrationHash(),
                request.deviceId.toByteArray(StandardCharsets.UTF_8),
                request.attestationKeyId.toByteArray(StandardCharsets.UTF_8),
                request.requestedTtlMilliseconds,
            ).also {
                require(it.isNotEmpty() && it.size <= MAX_DEVICE_ELIGIBILITY_REQUEST_ARCHIVE_BYTES_V1) {
                    "native device eligibility request is empty or oversized"
                }
            }.copyOf()
        }

        /**
         * Canonically decode and authenticate one issuance response against the
         * exact request registration, independently pinned issuer, network,
         * finality context, and wall-clock evaluation time.
         */
        @JvmStatic
        fun verifyOfflineDeviceEligibilityResponseV1(
            archive: ByteArray,
            request: OfflineDeviceEligibilityRequestV1,
            expectedIssuer: EligibilityIssuerPublicKeyV1,
            trustAnchor: OfflineDeviceFinalityTrustAnchorV1,
            evaluationTimeMilliseconds: Long,
        ): OfflineDeviceEligibilityResponseV1 {
            require(
                archive.isNotEmpty() && archive.size <= MAX_DEVICE_ELIGIBILITY_RESPONSE_ARCHIVE_BYTES_V1,
            ) { "device eligibility response exceeds its canonical response bound" }
            require(evaluationTimeMilliseconds > 0) {
                "evaluationTimeMilliseconds must be positive"
            }
            requireArtifactBridge()
            val projection = nativeVerifyOfflineDeviceEligibilityResponseV1(
                archive.copyOf(),
                request.registrationHash(),
                expectedIssuer.noritoEncoded(),
                trustAnchor.networkId.bytes(),
                trustAnchor.trustedHeightContextId(),
                evaluationTimeMilliseconds,
            )
            return OfflineDeviceEligibilityResponseV1(
                projection,
                archive,
                request.registrationHash(),
                trustAnchor,
            )
        }

        /** Verify issuer, credential claims, policy binding, freshness, and finality together. */
        @JvmStatic
        fun verifyEligibilityCredentialV1(
            archive: ByteArray,
            expectedIssuer: EligibilityIssuerPublicKeyV1,
            currentPolicyView: DeviceAttestationPolicyViewV1,
            evaluationTimeMilliseconds: Long,
        ): EligibilityCredentialV1 {
            require(evaluationTimeMilliseconds > 0) {
                "evaluationTimeMilliseconds must be positive"
            }
            val trustAnchor = checkNotNull(currentPolicyView.verificationTrustAnchor) {
                "currentPolicyView has not passed native finality verification"
            }
            requireArtifactBridge()
            val canonical = nativeVerifyOfflineDeviceEligibilityCredentialV1(
                archive.copyOf(),
                expectedIssuer.noritoEncoded(),
                currentPolicyView.noritoEncoded(),
                trustAnchor.networkId.bytes(),
                trustAnchor.trustedHeightContextId(),
                evaluationTimeMilliseconds,
            )
            require(canonical.contentEquals(archive)) {
                "native credential verifier changed canonical credential bytes"
            }
            return EligibilityCredentialV1(canonical, trustAnchor)
        }

        /**
         * Authenticate an IPN1 peer certificate as a live governed eligibility
         * credential and return the registered device key authorized to sign
         * that peer transcript. Raw public keys are never accepted here.
         */
        @JvmStatic
        fun verifyEligibilityPeerCertificateV1(
            certificate: ByteArray,
            expectedIssuer: EligibilityIssuerPublicKeyV1,
            currentPolicyView: DeviceAttestationPolicyViewV1,
            evaluationTimeMilliseconds: Long,
        ): KagemushaDevicePublicKeyV2 {
            require(evaluationTimeMilliseconds > 0) {
                "evaluationTimeMilliseconds must be positive"
            }
            val trustAnchor = checkNotNull(currentPolicyView.verificationTrustAnchor) {
                "currentPolicyView has not passed native finality verification"
            }
            requireArtifactBridge()
            val publicKey = nativeVerifyOfflineDeviceEligibilityPeerCertificateV1(
                certificate.copyOf(),
                expectedIssuer.noritoEncoded(),
                currentPolicyView.noritoEncoded(),
                trustAnchor.networkId.bytes(),
                trustAnchor.trustedHeightContextId(),
                evaluationTimeMilliseconds,
            )
            return KagemushaDevicePublicKeyV2(publicKey)
        }

        /** Restore only after the ABI-22 static validator accepts the exact envelope. */
        @JvmStatic
        fun decodeEligibilityPaymentEnvelopeV1(
            archive: ByteArray,
        ): EligibilityPaymentEnvelopeV1 {
            requireArtifactBridge()
            val canonical = nativeValidateEligibilityPaymentStaticV1(archive.copyOf())
            require(canonical.contentEquals(archive)) {
                "native eligibility validator changed canonical envelope bytes"
            }
            return EligibilityPaymentEnvelopeV1(canonical)
        }

        @JvmStatic
        fun decodeReceiverAcknowledgement(archive: ByteArray): ReceiverAcknowledgement =
            ReceiverAcknowledgement(archive)

        @JvmStatic
        fun decodeNoteMembershipWitness(archive: ByteArray): NoteMembershipWitness =
            NoteMembershipWitness(archive)

        /** Restore the opaque note opening retained for a finalized top-up or staged output. */
        @JvmStatic
        fun decodeNoteOpening(archive: ByteArray): NoteOpening = NoteOpening(archive)

        @JvmStatic
        fun decodeInitRequestV4(archive: ByteArray): InitRequestV4 = InitRequestV4(archive)

        @JvmStatic
        fun decodeAppendRequestV4(
            archive: ByteArray,
            changeOpening: NoteOpening?,
        ): AppendRequestV4 = transferChangeOpeningOwnership(changeOpening) {
            AppendRequestV4(archive, it)
        }

        @JvmStatic
        fun decodeVerifyRequestV4(archive: ByteArray): VerifyRequestV4 = VerifyRequestV4(archive)

        @JvmStatic
        fun decodeTopUpAnchorV4(archive: ByteArray): TopUpAnchorV4 = TopUpAnchorV4(archive)

        @JvmStatic
        fun decodeBundleV4(archive: ByteArray): BundleV4 = BundleV4(archive)

        @JvmStatic
        fun decodeTopUpFinalityEvidenceV4(archive: ByteArray): TopUpFinalityEvidenceV4 =
            TopUpFinalityEvidenceV4(archive)

        @JvmStatic
        fun decodeTopUpProvenanceV4(archive: ByteArray): TopUpProvenanceV4 =
            TopUpProvenanceV4(archive)

        /** Restores canonical persisted frontier bytes without making a branch spendable. */
        @JvmStatic
        fun decodeOutputMembershipFrontierV4(archive: ByteArray): OutputMembershipFrontierV4 =
            OutputMembershipFrontierV4(archive)

        /** Builds the canonical next-zero frontier persisted atomically with a branch. */
        @JvmStatic
        fun buildOutputMembershipFrontierV4(
            zeroPath: OutputMembershipPath,
        ): OutputMembershipFrontierV4 {
            requireArtifactBridge()
            val siblings = zeroPath.flattenedSiblings()
            val directions = zeroPath.directions()
            val root = zeroPath.root()
            return try {
                OutputMembershipFrontierV4(nativeBuildOutputMembershipFrontierV4(
                    zeroPath.leafIndex,
                    siblings,
                    directions,
                    root,
                ))
            } finally {
                siblings.fill(0)
                directions.fill(0)
                root.fill(0)
            }
        }

        /** Derives the only valid consecutive output paths from one authenticated frontier. */
        @JvmStatic
        fun deriveOutputMembershipPathsV4(
            frontier: OutputMembershipFrontierV4,
            recipientCommitment: ByteArray?,
            changeCommitment: ByteArray?,
        ): OutputMembershipPaths {
            require(recipientCommitment != null || changeCommitment != null) {
                "recipientCommitment or changeCommitment must be present"
            }
            requireArtifactBridge()
            val frontierArchive = frontier.noritoEncoded()
            val recipient = recipientCommitment?.let {
                requireDigest(it, "recipientCommitment")
            } ?: byteArrayOf()
            val change = changeCommitment?.let {
                requireDigest(it, "changeCommitment")
            } ?: byteArrayOf()
            return try {
                outputMembershipPathsFromNativeProjection(
                    nativeDeriveOutputMembershipPathsV4(frontierArchive, recipient, change),
                )
            } finally {
                frontierArchive.fill(0)
                recipient.fill(0)
                change.fill(0)
            }
        }

        /**
         * Restore one secret-bearing V4 branch only after native revalidates its provenance
         * against the bundle and the release installed at the current block height.
         *
         * Ownership of [opening] transfers at call entry. A failed restore destroys it; a
         * successful restore transfers it to the returned [SpendableBranchV4], which the caller
         * must close after the local builder has consumed the branch.
         */
        @JvmStatic
        fun restoreSpendableBranchV4(
            bundle: BundleV4,
            membershipWitness: NoteMembershipWitness,
            opening: NoteOpening,
            topUpProvenance: TopUpProvenanceV4,
            blockHeight: Long,
        ): SpendableBranchV4 = transferChangeOpeningOwnership(opening) { ownedOpening ->
            restoreSpendableBranchV4Owned(
                bundle,
                membershipWitness,
                checkNotNull(ownedOpening),
                topUpProvenance,
                blockHeight,
            )
        }

        private fun restoreSpendableBranchV4Owned(
            bundle: BundleV4,
            membershipWitness: NoteMembershipWitness,
            opening: NoteOpening,
            topUpProvenance: TopUpProvenanceV4,
            blockHeight: Long,
        ): SpendableBranchV4 {
            require(blockHeight > 0) { "blockHeight must be positive" }
            requireArtifactBridge()
            requireV4ProofBackend()
            val bundleArchive = bundle.noritoEncoded()
            val provenanceArchive = topUpProvenance.noritoEncoded()
            val witnessArchive = membershipWitness.noritoEncoded()
            val openingArchive = opening.noritoEncoded()
            return try {
                val frontier = OutputMembershipFrontierV4(nativeValidateSpendableBranchV4(
                    bundleArchive,
                    provenanceArchive,
                    witnessArchive,
                    openingArchive,
                    blockHeight,
                ))
                SpendableBranchV4(
                    bundle,
                    membershipWitness,
                    opening,
                    topUpProvenance,
                    frontier,
                )
            } finally {
                bundleArchive.fill(0)
                provenanceArchive.fill(0)
                witnessArchive.fill(0)
                openingArchive.fill(0)
            }
        }

        /** Restore finalized top-up state with its caller-retained, local-only note opening. */
        @JvmStatic
        fun restoreInitBranchV4(
            result: InitResultV4,
            opening: NoteOpening,
            blockHeight: Long,
        ): SpendableBranchV4 = transferChangeOpeningOwnership(opening) { ownedOpening ->
            require(blockHeight > 0) { "blockHeight must be positive" }
            val projection = projectInitResultV4(result)
            restoreSpendableBranchV4Owned(
                projection.branch.bundle,
                projection.branch.membershipWitness,
                checkNotNull(ownedOpening),
                projection.topUpProvenance,
                blockHeight,
            )
        }

        /** Restore a received offline payment with the receiver's local-only note opening. */
        @JvmStatic
        fun restorePeerPaymentBranchV4(
            payment: PeerPayment,
            opening: NoteOpening,
            blockHeight: Long,
        ): SpendableBranchV4 = transferChangeOpeningOwnership(opening) { ownedOpening ->
            require(blockHeight > 0) { "blockHeight must be positive" }
            val projection = projectPeerPayment(payment)
            restoreSpendableBranchV4Owned(
                projection.branch.bundle,
                projection.branch.membershipWitness,
                checkNotNull(ownedOpening),
                projection.topUpProvenance,
                blockHeight,
            )
        }

        /** Restore sender change retained locally after a successful offline split. */
        @JvmStatic
        fun restoreSplitChangeBranchV4(
            result: SplitResultV4,
            blockHeight: Long,
        ): SpendableBranchV4 {
            require(blockHeight > 0) { "blockHeight must be positive" }
            val opening = checkNotNull(result.takeChangeOpening()) {
                "split result has no local change opening"
            }
            return transferChangeOpeningOwnership(opening) { ownedOpening ->
                val projection = projectSplitResultV4(result)
                val change = checkNotNull(projection.change) {
                    "split result has no spendable change branch"
                }
                val provenance = checkNotNull(projection.changeTopUpProvenance) {
                    "split result has no spendable change provenance"
                }
                restoreSpendableBranchV4Owned(
                    change.bundle,
                    change.membershipWitness,
                    checkNotNull(ownedOpening),
                    provenance,
                    blockHeight,
                )
            }
        }

        /** Restore offline change retained locally after building a partial redemption. */
        @JvmStatic
        fun restoreRedeemChangeBranchV4(
            result: RedeemBuildResultV4,
            blockHeight: Long,
        ): SpendableBranchV4 {
            require(blockHeight > 0) { "blockHeight must be positive" }
            val opening = checkNotNull(result.takeChangeOpening()) {
                "redeem result has no local change opening"
            }
            return transferChangeOpeningOwnership(opening) { ownedOpening ->
                val projection = projectRedeemBuildResultV4(result)
                val change = checkNotNull(projection.change) {
                    "redeem result has no spendable change branch"
                }
                val provenance = checkNotNull(projection.changeTopUpProvenance) {
                    "redeem result has no spendable change provenance"
                }
                restoreSpendableBranchV4Owned(
                    change.bundle,
                    change.membershipWitness,
                    checkNotNull(ownedOpening),
                    provenance,
                    blockHeight,
                )
            }
        }

        @JvmStatic
        fun decodeRedeemRequestV4(
            archive: ByteArray,
            changeOpening: NoteOpening?,
        ): RedeemRequestV4 = transferChangeOpeningOwnership(changeOpening) {
            RedeemRequestV4(archive, it)
        }

        @JvmStatic
        fun decodeInitResultV4(archive: ByteArray): InitResultV4 = InitResultV4(archive)

        @JvmStatic
        fun decodeSplitResultV4(
            archive: ByteArray,
            changeOpening: NoteOpening?,
        ): SplitResultV4 = transferChangeOpeningOwnership(changeOpening) {
            SplitResultV4(archive, it)
        }

        @JvmStatic
        fun decodeVerifyResultV4(archive: ByteArray): VerifyResultV4 = VerifyResultV4(archive)

        @JvmStatic
        fun decodeRedeemBuildResultV4(
            archive: ByteArray,
            changeOpening: NoteOpening?,
        ): RedeemBuildResultV4 = transferChangeOpeningOwnership(changeOpening) {
            RedeemBuildResultV4(archive, it)
        }

        @JvmStatic
        fun decodeTopUpFinalityRosterArtifact(archive: ByteArray): TopUpFinalityRosterArtifact =
            TopUpFinalityRosterArtifact(archive)

        /** Decode exact compact-finality proof bytes without interpreting proof-controlled fields. */
        @JvmStatic
        fun decodeTopUpFinalityProof(archive: ByteArray): TopUpFinalityProof =
            TopUpFinalityProof(archive)

        /** Restore the exact canonical Torii request retained for an idempotent top-up retry. */
        @JvmStatic
        fun decodeTopUpRequest(archive: ByteArray): TopUpRequest = TopUpRequest(archive)

        /** Restore the exact canonical Torii request retained for an idempotent redemption retry. */
        @JvmStatic
        fun decodeRedeemSubmissionRequest(archive: ByteArray): RedeemSubmissionRequest =
            RedeemSubmissionRequest(archive)

        /**
         * Natively decode the HTTP-202 operation reference and bind it to the exact retained
         * request identity. HTTP acceptance is not finality, but its transaction hash is the only
         * hash later authenticated through committed transaction details and therefore must be
         * persisted before polling.
         */
        @JvmStatic
        fun projectOperationReference(
            reference: OperationReference,
            expectedOperationId: String,
            expectedKind: OperationKind,
            expectedSubmittedAtMilliseconds: Long,
        ): OperationReferenceProjection {
            requireArtifactBridge()
            val kindText = when (expectedKind) {
                OperationKind.TOP_UP -> "top_up"
                OperationKind.REDEEM -> "redeem"
            }
            val fields = nativeProjectOperationReferenceV1(
                reference.noritoEncoded(),
                expectedOperationId.toByteArray(Charsets.UTF_8),
                kindText.toByteArray(Charsets.UTF_8),
                expectedSubmittedAtMilliseconds,
            )
            requireFieldCount(fields, 6, "operation reference projection")
            check(canonicalText(fields[0], "operationState") == "pending") {
                "native Kagemusha operation reference is not Pending"
            }
            check(canonicalText(fields[1], "operationKind") == kindText) {
                "native Kagemusha operation reference changed kind"
            }
            val submittedAt = longInteger(fields[5], "submittedAtMilliseconds")
            check(submittedAt == expectedSubmittedAtMilliseconds) {
                "native Kagemusha operation reference changed submitted time"
            }
            return OperationReferenceProjection(
                state = OperationState.PENDING,
                kind = expectedKind,
                operationId = requireDigest(fields[2], "operationId"),
                transactionHash = requireDigest(fields[3], "transactionHash"),
                statusUri = canonicalText(fields[4], "statusUri"),
                submittedAtMilliseconds = submittedAt,
            )
        }

        @JvmStatic
        fun projectOperationStatus(status: OperationStatus): OperationStatusProjection {
            requireArtifactBridge()
            val fields = nativeProjectOperationStatusV4(status.noritoEncoded())
            requireFieldCount(fields, 10, "operation status projection")
            val state = when (canonicalText(fields[0], "operationState")) {
                "pending" -> OperationState.PENDING
                "applied" -> OperationState.APPLIED
                "rejected" -> OperationState.REJECTED
                else -> error("native Kagemusha operation state is invalid")
            }
            val kind = when (canonicalText(fields[1], "operationKind")) {
                "top_up" -> OperationKind.TOP_UP
                "redeem" -> OperationKind.REDEEM
                else -> error("native Kagemusha operation kind is invalid")
            }
            val heightOrSubmittedAt = fields[4].takeIf { it.isNotEmpty() }
                ?.let { longInteger(it, "operationHeightOrSubmittedAt") }
            val serverTime = fields[5].takeIf { it.isNotEmpty() }
                ?.let { longInteger(it, "serverTimeMilliseconds") }
            val finalizedTopUp = if (fields[6].isNotEmpty() || fields[7].isNotEmpty()) {
                check(state == OperationState.APPLIED && kind == OperationKind.TOP_UP &&
                    fields[6].isNotEmpty() && fields[7].isNotEmpty() &&
                    heightOrSubmittedAt != null && serverTime != null) {
                    "native Kagemusha finalized top-up fields are invalid"
                }
                FinalizedTopUp(
                    TopUpAnchorV4(fields[6]),
                    TopUpFinalityProof(fields[7]),
                    heightOrSubmittedAt,
                    serverTime,
                )
            } else {
                null
            }
            val rejection = if (fields[8].isNotEmpty() || fields[9].isNotEmpty()) {
                check(state == OperationState.REJECTED && fields[8].isNotEmpty() && fields[9].isNotEmpty()) {
                    "native Kagemusha rejection fields are invalid"
                }
                OperationRejection(
                    canonicalText(fields[8], "rejectionCode"),
                    canonicalText(fields[9], "rejectionMessage"),
                )
            } else {
                null
            }
            return OperationStatusProjection(
                state,
                kind,
                requireDigest(fields[2], "operationId"),
                requireDigest(fields[3], "transactionHash"),
                if (state == OperationState.PENDING) heightOrSubmittedAt else null,
                if (state == OperationState.APPLIED) heightOrSubmittedAt else null,
                serverTime,
                finalizedTopUp,
                rejection,
            )
        }

        /** Decode the authoritative, snapshot-bound Torii Kagemusha capability response. */
        @JvmStatic
        fun projectReadiness(readiness: Readiness): ReadinessProjection {
            requireArtifactBridge()
            val fields = nativeProjectReadinessV4(readiness.noritoEncoded())
            check(fields.size >= 18) { "native Kagemusha readiness projection returned invalid fields" }
            val blockerCount = integer(fields[17], "blockerCount")
            check(blockerCount >= 0 && fields.size == 18 + blockerCount * 2) {
                "native Kagemusha readiness projection returned invalid blockers"
            }
            val blockers = ArrayList<ReadinessBlocker>(blockerCount)
            repeat(blockerCount) { index ->
                blockers.add(
                    ReadinessBlocker(
                        canonicalText(fields[18 + index * 2], "blockerCode"),
                        canonicalText(fields[19 + index * 2], "blockerMessage"),
                    ),
                )
            }
            return ReadinessProjection(
                cashHandoffCapability = canonicalText(fields[0], "cashHandoffCapability"),
                eligibilityCashHandoffCapability = canonicalText(
                    fields[1],
                    "eligibilityCashHandoffCapability",
                ),
                requiredBridgeAbiVersion = integer(fields[2], "requiredBridgeAbiVersion"),
                maximumHops = integer(fields[3], "maximumHops"),
                assetDefinitionId = canonicalText(fields[4], "assetDefinitionId"),
                assetScale = fields[5].takeIf { it.isNotEmpty() }?.let { integer(it, "assetScale") },
                evaluatedBlockHeight = longInteger(fields[6], "evaluatedBlockHeight"),
                evaluatedBlockHash = requireDigest(fields[7], "evaluatedBlockHash"),
                proofBackendAvailable = bool(fields[8], "proofBackendAvailable"),
                recursiveLineageSupported = bool(fields[9], "recursiveLineageSupported"),
                ready = bool(fields[10], "ready"),
                transferVerifier = activeVerifier(fields[11]),
                topUpShieldVerifier = activeVerifier(fields[12]),
                unshieldVerifier = activeVerifier(fields[13]),
                recursiveStepEqVerifier = activeVerifier(fields[14]),
                recursiveStepEpVerifier = activeVerifier(fields[15]),
                artifactSet = authenticatedArtifactSet(fields[16]),
                blockers = blockers,
            )
        }

        @JvmStatic
        fun prepareRequestAuthorization(
            authority: String,
            chainDiscriminant: Int,
            deviceId: String,
            assetDefinitionId: String,
            operationId: ByteArray,
            issuedAtMilliseconds: Long,
            expiresAtMilliseconds: Long,
            nonce: ByteArray,
            payloadDigest: ByteArray,
            registrationHash: ByteArray,
            platform: OnlineHardwareAssertionPlatform,
        ): RequestAuthorizationPreparation {
            requireArtifactBridge()
            val fields = nativePrepareAuthorizationV2(
                utf8(authority, "authority"),
                requireChainDiscriminant(chainDiscriminant),
                utf8(deviceId, "deviceId"),
                utf8(assetDefinitionId, "assetDefinitionId"),
                requireDigest(operationId, "operationId"),
                issuedAtMilliseconds,
                expiresAtMilliseconds,
                requireDigest(nonce, "nonce"),
                requireDigest(payloadDigest, "payloadDigest"),
                requireDigest(registrationHash, "registrationHash"),
                utf8(platform.wireName, "hardwareAssertionPlatform"),
            )
            requireFieldCount(fields, 5, "authorization preparation")
            return RequestAuthorizationPreparation(
                RequestAuthorizationPreparationArchive(fields[0]),
                fields[1],
                fields[2],
                fields[3],
                fields[4],
            )
        }

        /**
         * Creates the closed same-account drain-only redemption authorization.
         *
         * [finalizedPolicy] must come from the native finalized-policy verifier. Native code
         * verifies its canonical policy bytes, exact block binding, Sumeragi finality proof,
         * caller-owned NetworkId/context anchor, and freshness again before encoding. The result
         * carries no eligibility credential, device registration hash, or P-256 assertion and is
         * rejected by consensus outside a redemption submitted by the exact Ed25519 recipient.
         */
        @JvmStatic
        fun createDrainOnlySameAccountRedemptionAuthorizationV1(
            authority: String,
            chainDiscriminant: Int,
            deviceId: String,
            assetDefinitionId: String,
            operationId: ByteArray,
            issuedAtMilliseconds: Long,
            expiresAtMilliseconds: Long,
            nonce: ByteArray,
            payloadDigest: ByteArray,
            finalizedPolicy: DeviceAttestationPolicyViewV1,
        ): RequestAuthorization {
            requireArtifactBridge()
            val trustAnchor = checkNotNull(finalizedPolicy.verificationTrustAnchor) {
                "drain-only redemption requires a native-verified finalized policy view"
            }
            return RequestAuthorization(
                nativeFinalizeDrainOnlyRedemptionAuthorizationV1(
                    utf8(authority, "authority"),
                    requireChainDiscriminant(chainDiscriminant),
                    utf8(deviceId, "deviceId"),
                    utf8(assetDefinitionId, "assetDefinitionId"),
                    requireDigest(operationId, "operationId"),
                    issuedAtMilliseconds,
                    expiresAtMilliseconds,
                    requireDigest(nonce, "nonce"),
                    requireDigest(payloadDigest, "payloadDigest"),
                    finalizedPolicy.noritoEncoded(),
                    trustAnchor.networkId.bytes(),
                    trustAnchor.trustedHeightContextId(),
                ),
            )
        }

        /**
         * Registry-decode the exact drain-only request and return its one canonical native
         * `RedeemKagemushaRecursiveV4` instruction. Native code rejects another recipient,
         * authority, authorization variant, wire id, or non-canonical request archive.
         */
        @JvmStatic
        fun buildDrainOnlyRedeemInstructionV4(
            request: RedeemSubmissionRequest,
            authority: String,
            chainDiscriminant: Int,
        ): DrainOnlyRedeemInstructionV4 {
            requireArtifactBridge()
            val fields = nativeBuildDrainOnlyRedeemInstructionV4(
                request.noritoEncoded(),
                utf8(authority, "authority"),
                requireChainDiscriminant(chainDiscriminant),
            )
            requireFieldCount(fields, 5, "drain-only redemption instruction")
            return DrainOnlyRedeemInstructionV4(
                wireId = canonicalText(fields[0], "drainOnlyRedeemWireId"),
                framedPayload = fields[1],
                operationId = requireDigest(fields[2], "operationId"),
                issuedAtMilliseconds = longInteger(fields[3], "issuedAtMilliseconds"),
                expiresAtMilliseconds = longInteger(fields[4], "expiresAtMilliseconds"),
            )
        }

        @JvmStatic
        fun finalizeRequestAuthorization(
            preparation: RequestAuthorizationPreparation,
            platformSignatureDer: ByteArray,
            authenticatorData: ByteArray? = null,
        ): RequestAuthorization {
            requireArtifactBridge()
            val expectedRawSignature =
                KagemushaP256Codec.rawLowSFromStrictDer(platformSignatureDer)
            val fields = nativeFinalizeHardwareAuthorizationV2(
                preparation.archive.noritoEncoded(),
                authenticatorData?.copyOf() ?: ByteArray(0),
                platformSignatureDer.copyOf(),
            )
            requireFieldCount(fields, 2, "authorization finalization")
            check(fields[1].contentEquals(expectedRawSignature)) {
                "native authorization signature normalization drifted from the SDK"
            }
            return RequestAuthorization(
                fields[0],
            )
        }

        /** Finalize directly from the CBOR returned by DCAppAttestService.generateAssertion. */
        @JvmStatic
        fun finalizeIosAppAttest(
            preparation: RequestAuthorizationPreparation,
            assertionObject: ByteArray,
        ): RequestAuthorization {
            requireArtifactBridge()
            val boundedAssertionObject = requiredBytes(assertionObject, "assertionObject")
            require(boundedAssertionObject.size <= IOS_APP_ATTEST_ASSERTION_OBJECT_MAX_BYTES) {
                "assertionObject exceeds the App Attest response bound"
            }
            val fields = nativeFinalizeIosAppAttestAuthorizationV2(
                preparation.archive.noritoEncoded(),
                boundedAssertionObject,
            )
            return requestAuthorizationFromIosAppAttestNativeProjection(fields)
        }

        internal fun requestAuthorizationFromIosAppAttestNativeProjection(
            fields: Array<ByteArray>?,
        ): RequestAuthorization {
            requireFieldCount(fields, 3, "App Attest authorization finalization")
            val projection = checkNotNull(fields)
            try {
                KagemushaP256Codec.requireRawLowSSignature(projection[1])
            } catch (failure: IllegalArgumentException) {
                throw IllegalStateException(
                    "native Kagemusha App Attest finalization returned an invalid raw signature",
                    failure,
                )
            }
            requireIosAppAttestAuthenticatorDataProjection(projection[2])
            return RequestAuthorization(projection[0])
        }

        @JvmStatic
        fun finalizeTopUp(
            unsigned: TopUpUnsigned,
            authorization: RequestAuthorization,
        ): TopUpRequest {
            requireArtifactBridge()
            return TopUpRequest(
                nativeFinalizeTopUpV4(unsigned.noritoEncoded(), authorization.noritoEncoded()),
            )
        }

        @JvmStatic
        fun finalizeTopUp(
            preparation: TopUpPreparation,
            authorization: RequestAuthorization,
        ): TopUpRequest = finalizeTopUp(preparation.unsigned, authorization)

        @JvmStatic
        fun prepareTopUp(
            networkId: NetworkId,
            chainDiscriminant: Int,
            assetDefinitionId: String,
            payerAccountId: String,
            amount: KagemushaScaledAmount,
            operationId: ByteArray,
            openingSpendKey: ByteArray,
            openingRho: ByteArray,
            openingDiversifier: ByteArray,
            zeroPath: TopUpZeroPath,
            shieldVerifierCommitment: ByteArray,
            artifactBinding: ArtifactBindingV4,
        ): TopUpPreparation {
            requireArtifactBridge()
            return SecretArchiveWiper.withOpeningDigests(
                openingSpendKey,
                "openingSpendKey",
                openingRho,
                "openingRho",
                openingDiversifier,
                "openingDiversifier",
            ) { spendKeyCopy, rhoCopy, diversifierCopy ->
                var fields: Array<ByteArray>? = null
                var locallyOwnedOpening: NoteOpening? = null
                try {
                    val nativeFields = nativePrepareTopUpV4(
                        networkId.bytes(),
                        requireChainDiscriminant(chainDiscriminant),
                        utf8(assetDefinitionId, "assetDefinitionId"),
                        utf8(payerAccountId, "payerAccountId"),
                        utf8(amount.atomicUnits, "atomicUnits"),
                        amount.scale,
                        requireDigest(operationId, "operationId"),
                        spendKeyCopy,
                        rhoCopy,
                        diversifierCopy,
                        zeroPath.leafIndex,
                        zeroPath.flattenedSiblings(),
                        zeroPath.directions(),
                        zeroPath.root(),
                        requireDigest(shieldVerifierCommitment, "shieldVerifierCommitment"),
                        artifactBinding.noritoEncoded(),
                    ).also { fields = it }
                    requireFieldCount(nativeFields, 11, "top-up preparation")
                    val opening = NoteOpening(nativeFields[2]).also {
                        locallyOwnedOpening = it
                    }
                    val preparation = TopUpPreparation(
                        TopUpUnsigned(nativeFields[0]),
                        nativeFields[1],
                        opening,
                        nativeFields[3],
                        nativeFields[4],
                        nativeFields[5],
                        nativeFields[6],
                        nativeFields[7],
                        amount(nativeFields[8], nativeFields[9]),
                        integer(nativeFields[10], "leafIndex"),
                    )
                    locallyOwnedOpening = null
                    preparation
                } finally {
                    locallyOwnedOpening?.close()
                    SecretArchiveWiper.wipeAll(fields)
                }
            }
        }

        @JvmStatic
        fun finalizeRedeemV4(
            buildResult: RedeemBuildResultV4,
            authorization: RequestAuthorization,
        ): RedeemFinalization {
            requireArtifactBridge()
            val fields = nativeFinalizeRedeemV4(
                buildResult.noritoEncoded(),
                authorization.noritoEncoded(),
            )
            requireFieldCount(fields, 2, "V4 redeem finalization")
            return RedeemFinalization(
                RedeemSubmissionRequest(fields[0]),
                requireDigest(fields[1], "operationId"),
            )
        }

        @JvmStatic
        fun prepareRecipientPaymentRequest(
            networkId: NetworkId,
            chainDiscriminant: Int,
            assetDefinitionId: String,
            amount: KagemushaScaledAmount,
            recipientAccountId: String,
            receiverDeviceId: String,
            receiverPublicKey: KagemushaDevicePublicKeyV2,
            requestId: ByteArray,
            issuedAtMilliseconds: Long,
            expiresAtMilliseconds: Long,
            spendKey: ByteArray,
            rho: ByteArray,
            diversifier: ByteArray,
        ): RecipientRequestPreparation {
            requireArtifactBridge()
            return SecretArchiveWiper.withOpeningDigests(
                spendKey,
                "spendKey",
                rho,
                "rho",
                diversifier,
                "diversifier",
            ) { spendKeyCopy, rhoCopy, diversifierCopy ->
                var fields: Array<ByteArray>? = null
                var locallyOwnedOpening: NoteOpening? = null
                try {
                    val nativeFields = nativePrepareRecipientRequestV2(
                        networkId.bytes(),
                        requireChainDiscriminant(chainDiscriminant),
                        utf8(assetDefinitionId, "assetDefinitionId"),
                        utf8(amount.atomicUnits, "atomicUnits"),
                        amount.scale,
                        utf8(recipientAccountId, "recipientAccountId"),
                        utf8(receiverDeviceId, "receiverDeviceId"),
                        receiverPublicKey.sec1Bytes(),
                        requireDigest(requestId, "requestId"),
                        issuedAtMilliseconds,
                        expiresAtMilliseconds,
                        spendKeyCopy,
                        rhoCopy,
                        diversifierCopy,
                    ).also { fields = it }
                    requireFieldCount(nativeFields, 5, "recipient request preparation")
                    val opening = NoteOpening(nativeFields[2]).also {
                        locallyOwnedOpening = it
                    }
                    val preparation = RecipientRequestPreparation(
                        RecipientRequestPayload(nativeFields[0]),
                        nativeFields[1],
                        opening,
                        nativeFields[3],
                        nativeFields[4],
                        amount,
                    )
                    locallyOwnedOpening = null
                    preparation
                } finally {
                    locallyOwnedOpening?.close()
                    SecretArchiveWiper.wipeAll(fields)
                }
            }
        }

        /** Prepare one local-only opening for sender change or partial redemption change. */
        @JvmStatic
        fun prepareNoteOpening(
            spendKey: ByteArray,
            rho: ByteArray,
            diversifier: ByteArray,
        ): NoteOpening {
            requireArtifactBridge()
            return SecretArchiveWiper.withOpeningDigests(
                spendKey,
                "spendKey",
                rho,
                "rho",
                diversifier,
                "diversifier",
            ) { spendKeyCopy, rhoCopy, diversifierCopy ->
                var nativeArchive: ByteArray? = null
                try {
                    NoteOpening(
                        nativePrepareNoteOpeningV2(spendKeyCopy, rhoCopy, diversifierCopy)
                            .also { nativeArchive = it },
                    )
                } finally {
                    SecretArchiveWiper.wipe(nativeArchive)
                }
            }
        }

        /**
         * Prepare partial-redemption change inside the native secret boundary.
         *
         * Native code revalidates the exact input note/opening and derives a fresh opening from a
         * domain-separated binding over that input, [changeAmount], [operationId], and caller entropy.
         * The authoritative confidential diversifier is selected natively; wallet code never
         * fabricates it. Returned coordinates exist only so encrypted wallet state can restore the
         * proof-bound change after finality.
         */
        @JvmStatic
        fun prepareRedemptionChangeV4(
            input: SpendableBranchV4,
            changeAmount: KagemushaScaledAmount,
            operationId: ByteArray,
            entropy: ByteArray,
        ): RedemptionChangePreparationV4 {
            requireArtifactBridge()
            var operation: ByteArray? = null
            var freshEntropy: ByteArray? = null
            var bundleArchive: ByteArray? = null
            var openingArchive: ByteArray? = null
            var atomicUnits: ByteArray? = null
            var fields: Array<ByteArray>? = null
            var opening: NoteOpening? = null
            return try {
                val operationCopy = requireDigest(operationId, "operationId")
                    .also { operation = it }
                val entropyCopy = requireDigest(entropy, "entropy")
                    .also { freshEntropy = it }
                require(!operationCopy.contentEquals(entropyCopy)) {
                    "entropy must be distinct from operationId"
                }
                val bundleBytes = input.bundle.noritoEncoded().also { bundleArchive = it }
                val openingBytes = input.opening.noritoEncoded().also { openingArchive = it }
                val amountBytes = utf8(changeAmount.atomicUnits, "atomicUnits")
                    .also { atomicUnits = it }
                val nativeFields = nativePrepareRedemptionChangeV4(
                    bundleBytes,
                    openingBytes,
                    amountBytes,
                    changeAmount.scale,
                    operationCopy,
                    entropyCopy,
                ).also { fields = it }
                requireFieldCount(nativeFields, 7, "V4 redemption change preparation")
                for ((index, name) in listOf(
                    1 to "rho",
                    2 to "diversifier",
                    3 to "commitment",
                    4 to "spendNullifier",
                )) {
                    require(
                        nativeFields[index].size == 32 &&
                            nativeFields[index].any { it.toInt() != 0 },
                    ) { "$name must be a non-zero 32-byte native field" }
                }
                require(!nativeFields[1].contentEquals(nativeFields[2])) {
                    "native Kagemusha redemption opening coordinates collide"
                }
                val projectedAmount = amount(nativeFields[5], nativeFields[6])
                check(projectedAmount == changeAmount) {
                    "native Kagemusha redemption change amount changed"
                }
                val preparedOpening = NoteOpening(nativeFields[0]).also { opening = it }
                opening = null
                RedemptionChangePreparationV4(
                    preparedOpening,
                    nativeFields[1],
                    nativeFields[2],
                    nativeFields[3],
                    nativeFields[4],
                    projectedAmount,
                )
            } finally {
                opening?.destroy()
                fields?.forEach { field ->
                    @Suppress("SENSELESS_COMPARISON")
                    if (field != null) field.fill(0)
                }
                atomicUnits?.fill(0)
                openingArchive?.fill(0)
                bundleArchive?.fill(0)
                freshEntropy?.fill(0)
                operation?.fill(0)
            }
        }

        /**
         * Prepare sender change for an ordinary one- or two-input peer split.
         *
         * Native reauthenticates every ordered bundle/opening pair, enforces shared
         * chain/asset/root/artifact context and exact value conservation against [recipientRequest],
         * then derives an owned opening under a peer-split-only domain.
         */
        @JvmStatic
        fun preparePeerSplitChangeV4(
            inputs: List<SpendableBranchV4>,
            recipientRequest: VerifiedRecipientPaymentRequest,
            changeAmount: KagemushaScaledAmount,
            operationId: ByteArray,
            entropy: ByteArray,
        ): PeerSplitChangePreparationV4 {
            requireArtifactBridge()
            require(inputs.size in 1..MAXIMUM_INPUTS_PER_TRANSITION) {
                "inputs must contain one or two spendable branches"
            }
            val operation = requireDigest(operationId, "operationId")
            val freshEntropy = requireDigest(entropy, "entropy")
            require(!operation.contentEquals(freshEntropy)) {
                "entropy must be distinct from operationId"
            }
            val bundles = inputs.map { it.bundle.noritoEncoded() }.toTypedArray()
            val openings = inputs.map { it.opening.noritoEncoded() }.toTypedArray()
            val signedRequest = recipientRequest.request.noritoEncoded()
            val atomicUnits = utf8(changeAmount.atomicUnits, "atomicUnits")
            var fields: Array<ByteArray>? = null
            var opening: NoteOpening? = null
            return try {
                val nativeFields = nativePreparePeerSplitChangeV4(
                    bundles,
                    openings,
                    signedRequest,
                    atomicUnits,
                    changeAmount.scale,
                    operation,
                    freshEntropy,
                ).also { fields = it }
                requireFieldCount(nativeFields, 7, "V4 peer-split change preparation")
                for ((index, name) in listOf(
                    1 to "rho",
                    2 to "diversifier",
                    3 to "commitment",
                    4 to "spendNullifier",
                )) {
                    require(nativeFields[index].size == 32 && nativeFields[index].any { it != 0.toByte() }) {
                        "$name must be a non-zero 32-byte native field"
                    }
                }
                val projectedAmount = amount(nativeFields[5], nativeFields[6])
                check(projectedAmount == changeAmount) {
                    "native Kagemusha peer-split change amount changed"
                }
                val preparedOpening = NoteOpening(nativeFields[0]).also { opening = it }
                opening = null
                PeerSplitChangePreparationV4(
                    preparedOpening,
                    nativeFields[1],
                    nativeFields[2],
                    nativeFields[3],
                    nativeFields[4],
                    projectedAmount,
                )
            } finally {
                opening?.destroy()
                fields?.forEach { it.fill(0) }
                atomicUnits.fill(0)
                signedRequest.fill(0)
                openings.forEach { it.fill(0) }
                bundles.forEach { it.fill(0) }
                freshEntropy.fill(0)
                operation.fill(0)
            }
        }

        @JvmStatic
        fun signRecipientPaymentRequest(
            preparation: RecipientRequestPreparation,
            signature: KagemushaDeviceSignatureV2,
        ): RecipientPaymentRequest {
            requireArtifactBridge()
            return RecipientPaymentRequest(
                nativeCreateRecipientRequestV2(
                    preparation.payload.noritoEncoded(),
                    signature.rawBytes(),
                ),
            )
        }

        @JvmStatic
        fun verifyRecipientPaymentRequest(
            request: RecipientPaymentRequest,
            verifiedAtMilliseconds: Long,
        ): VerifiedRecipientPaymentRequest {
            requireArtifactBridge()
            require(verifiedAtMilliseconds > 0) { "verifiedAtMilliseconds must be positive" }
            val projection = projectRecipientPaymentRequest(request)
            return VerifiedRecipientPaymentRequest(
                request,
                requireDigest(
                    nativeVerifyRecipientRequestV2(request.noritoEncoded(), verifiedAtMilliseconds),
                    "requestDigest",
                ),
                verifiedAtMilliseconds,
                projection,
            )
        }

        /** Create the request-independent selector used to prefetch portable receiver lineage. */
        @JvmStatic
        fun createRecipientLineageQueryV2(
            networkId: NetworkId,
            chainDiscriminant: Int,
            recipientAccountId: String,
            receiverDeviceId: String,
            assetDefinitionId: String,
            trustedCheckpointHeight: Long,
        ): RecipientLineageQueryV2 {
            requireArtifactBridge()
            require(trustedCheckpointHeight > 0) { "trustedCheckpointHeight must be positive" }
            return RecipientLineageQueryV2(
                nativeCreateRecipientLineageQueryV2(
                    networkId.bytes(),
                    requireChainDiscriminant(chainDiscriminant),
                    utf8(recipientAccountId, "recipientAccountId"),
                    utf8(receiverDeviceId, "receiverDeviceId"),
                    utf8(assetDefinitionId, "assetDefinitionId"),
                    trustedCheckpointHeight,
                ),
            )
        }

        /** Verify signed request, active-state lineage and a bounded finality suffix locally. */
        @JvmStatic
        fun verifyRecipientRegistrationLineageV2(
            request: RecipientPaymentRequest,
            lineage: RecipientRegistrationLineage,
            verifiedAtMilliseconds: Long,
            trustedCheckpointHeight: Long,
            trustedCheckpointContextId: ByteArray,
        ): VerifiedRecipientRegistrationLineageV2 {
            requireArtifactBridge()
            require(verifiedAtMilliseconds > 0) { "verifiedAtMilliseconds must be positive" }
            require(trustedCheckpointHeight > 0) {
                "trustedCheckpointHeight must be positive"
            }
            val trustedContext = requireFinalityCheckpointContext(
                trustedCheckpointContextId,
                "trustedCheckpointContextId",
            )
            return try {
                val fields = nativeVerifyRecipientRegistrationLineageV2(
                    request.noritoEncoded(),
                    lineage.noritoEncoded(),
                    verifiedAtMilliseconds,
                    trustedCheckpointHeight,
                    trustedContext,
                )
                requireFieldCount(fields, 2, "verified recipient lineage")
                VerifiedRecipientRegistrationLineageV2(
                    RecipientRegistrationLineage(fields[0]),
                    FinalityCheckpointPromotionV2(fields[1]),
                )
            } finally {
                trustedContext.fill(0)
            }
        }

        /** Build one canonical receive offer carrying request, lineage and publisher envelope. */
        @JvmStatic
        fun createRecipientReceiveOfferV2(
            request: RecipientPaymentRequest,
            lineage: RecipientRegistrationLineage,
            publisherCheckpointEnvelope: ByteArray,
        ): RecipientReceiveOfferV2 {
            requireArtifactBridge()
            val envelope = requireBoundedBytes(
                publisherCheckpointEnvelope,
                "publisherCheckpointEnvelope",
                MAX_PUBLISHER_CHECKPOINT_ENVELOPE_BYTES_V1,
            )
            return try {
                RecipientReceiveOfferV2(
                    nativeCreateRecipientReceiveOfferV2(
                        request.noritoEncoded(),
                        lineage.noritoEncoded(),
                        envelope,
                    ),
                )
            } finally {
                envelope.fill(0)
            }
        }

        @JvmStatic
        fun projectRecipientReceiveOfferV2(
            offer: RecipientReceiveOfferV2,
        ): RecipientReceiveOfferProjectionV2 {
            requireArtifactBridge()
            val fields = nativeProjectRecipientReceiveOfferV2(offer.noritoEncoded())
            requireFieldCount(fields, 3, "recipient receive offer projection")
            return RecipientReceiveOfferProjectionV2(
                request = RecipientPaymentRequest(fields[0]),
                lineage = RecipientRegistrationLineage(fields[1]),
                publisherCheckpointEnvelope = fields[2],
            )
        }

        /** Verify the exact whole offer locally against one durable trusted checkpoint. */
        @JvmStatic
        fun verifyRecipientReceiveOfferV2(
            offer: RecipientReceiveOfferV2,
            verifiedAtMilliseconds: Long,
            trustedCheckpointHeight: Long,
            trustedCheckpointContextId: ByteArray,
        ): VerifiedRecipientReceiveOfferV2 {
            requireArtifactBridge()
            require(verifiedAtMilliseconds > 0) { "verifiedAtMilliseconds must be positive" }
            require(trustedCheckpointHeight > 0) {
                "trustedCheckpointHeight must be positive"
            }
            val trustedContext = requireFinalityCheckpointContext(
                trustedCheckpointContextId,
                "trustedCheckpointContextId",
            )
            return try {
                val fields = nativeVerifyRecipientReceiveOfferV2(
                    offer.noritoEncoded(),
                    verifiedAtMilliseconds,
                    trustedCheckpointHeight,
                    trustedContext,
                )
                requireFieldCount(fields, 4, "verified recipient receive offer")
                VerifiedRecipientReceiveOfferV2(
                    request = RecipientPaymentRequest(fields[0]),
                    lineage = RecipientRegistrationLineage(fields[1]),
                    publisherCheckpointEnvelope = fields[2],
                    promotedCheckpoint = FinalityCheckpointPromotionV2(fields[3]),
                    verifiedAtMilliseconds = verifiedAtMilliseconds,
                )
            } finally {
                trustedContext.fill(0)
            }
        }

        @JvmStatic
        fun projectRecipientPaymentRequest(
            request: RecipientPaymentRequest,
        ): RecipientRequestProjection {
            requireArtifactBridge()
            val fields = nativeProjectRecipientRequestV2(request.noritoEncoded())
            requireFieldCount(fields, 14, "recipient request projection")
            return RecipientRequestProjection(
                networkId = NetworkId.fromBytes(requireDigest(fields[0], "networkId")),
                assetDefinitionId = canonicalText(fields[1], "assetDefinitionId"),
                amount = amount(fields[2], fields[3]),
                recipientAccountId = canonicalText(fields[4], "recipientAccountId"),
                receiverDeviceId = canonicalText(fields[5], "receiverDeviceId"),
                requestId = fields[6],
                issuedAtMilliseconds = longInteger(fields[7], "issuedAtMilliseconds"),
                expiresAtMilliseconds = longInteger(fields[8], "expiresAtMilliseconds"),
                outputCommitment = fields[9],
                outputNullifier = fields[10],
                receiverKeyReference = fields[11],
                receiverPublicKey = fields[12],
                digest = fields[13],
            )
        }

        @JvmStatic
        fun buildInitRequestV4(
            topUpAnchor: TopUpAnchorV4,
            topUpFinalityProof: TopUpFinalityProof,
            topUpFinalityRosterArtifact: TopUpFinalityRosterArtifact,
            opening: NoteOpening,
            outputMembershipPaths: OutputMembershipPaths,
        ): InitRequestV4 {
            requireArtifactBridge()
            requireV4ProofBackend()
            require(outputMembershipPaths.recipient != null && outputMembershipPaths.change == null) {
                "initialization requires exactly one recipient output path"
            }
            var openingArchive: ByteArray? = null
            var membershipArchive: ByteArray? = null
            var nativeArchive: ByteArray? = null
            return try {
                val openingBytes = opening.noritoEncoded().also { openingArchive = it }
                val membershipBytes = outputMembershipPaths.nativeArchive()
                    .also { membershipArchive = it }
                InitRequestV4(
                    nativeBuildInitRequestV4(
                        topUpAnchor.noritoEncoded(),
                        topUpFinalityProof.noritoEncoded(),
                        topUpFinalityRosterArtifact.noritoEncoded(),
                        openingBytes,
                        membershipBytes,
                    ).also { nativeArchive = it },
                )
            } finally {
                SecretArchiveWiper.wipe(nativeArchive)
                SecretArchiveWiper.wipe(membershipArchive)
                SecretArchiveWiper.wipe(openingArchive)
            }
        }

        /**
         * Verify the compact top-up proof and expose only its native-authenticated block identity
         * for agreement with the uniform finalized issuer outcome.
         */
        @JvmStatic
        fun projectVerifiedTopUpFinalityV4(
            topUpAnchor: TopUpAnchorV4,
            topUpFinalityProof: TopUpFinalityProof,
            topUpFinalityRosterArtifact: TopUpFinalityRosterArtifact,
        ): VerifiedTopUpFinalityV4 {
            requireArtifactBridge()
            requireV4ProofBackend()
            val fields = nativeProjectVerifiedTopUpFinalityV4(
                topUpAnchor.noritoEncoded(),
                topUpFinalityProof.noritoEncoded(),
                topUpFinalityRosterArtifact.noritoEncoded(),
            )
            requireFieldCount(fields, 5, "verified V4 top-up finality projection")
            return VerifiedTopUpFinalityV4(
                operationId = requireDigest(fields[0], "operationId"),
                exactTransactionHashHex = canonicalText(fields[1], "transactionHashHex"),
                exactHeight = longInteger(fields[2], "height"),
                exactBlockHashHex = canonicalText(fields[3], "blockHashHex"),
                heightContextId = requireFinalityCheckpointContext(
                    fields[4],
                    "heightContextId",
                ),
            )
        }

        /** Build and validate the complete origin-finality inventory for one V4 bundle. */
        @JvmStatic
        fun buildTopUpProvenanceV4(
            bundle: BundleV4,
            topUpFinalityRosterArtifact: TopUpFinalityRosterArtifact,
            topUpAnchors: List<TopUpAnchorV4>,
            topUpFinalityProofs: List<TopUpFinalityProof>,
            blockHeight: Long,
        ): TopUpProvenanceV4 {
            require(topUpAnchors.size in 1..MAXIMUM_INPUTS_PER_TRANSITION &&
                topUpFinalityProofs.size == topUpAnchors.size) {
                "topUpAnchors and topUpFinalityProofs must have the same 1..2 count"
            }
            requireArtifactBridge()
            val anchors = topUpAnchors.map { it.noritoEncoded() }.toTypedArray()
            val proofs = topUpFinalityProofs.map { it.noritoEncoded() }.toTypedArray()
            return try {
                TopUpProvenanceV4(nativeBuildTopUpProvenanceV4(
                    bundle.noritoEncoded(),
                    topUpFinalityRosterArtifact.noritoEncoded(),
                    anchors,
                    proofs,
                    blockHeight,
                ))
            } finally {
                anchors.forEach { it.fill(0) }
                proofs.forEach { it.fill(0) }
            }
        }

        /** Revalidate persisted provenance against the bundle and current installed release. */
        @JvmStatic
        fun validateTopUpProvenanceV4(
            bundle: BundleV4,
            topUpProvenance: TopUpProvenanceV4,
            blockHeight: Long,
        ): TopUpProvenanceV4 {
            requireArtifactBridge()
            return TopUpProvenanceV4(nativeValidateTopUpProvenanceV4(
                bundle.noritoEncoded(),
                topUpProvenance.noritoEncoded(),
                blockHeight,
            ))
        }

        /** Build one canonical append request from one or two independently spendable inputs. */
        @JvmStatic
        fun buildAppendRequestV4(
            inputs: List<SpendableBranchV4>,
            changeOpening: NoteOpening?,
            outputMembershipPaths: OutputMembershipPaths,
            transferVerifierCommitment: ByteArray,
            operationId: ByteArray,
            blockHeight: Long,
        ): AppendRequestV4 = transferChangeOpeningOwnership(changeOpening) { ownedChangeOpening ->
            require(inputs.size in 1..MAXIMUM_LOCAL_APPEND_BUILDER_INPUTS) {
                "inputs must contain one or two spendable branches"
            }
            require(inputs.map { it.bundle }.distinct().size == inputs.size) {
                "inputs must refer to distinct V4 bundles"
            }
            require(outputMembershipPaths.recipient != null) {
                "append requires a recipient output path"
            }
            require((outputMembershipPaths.change != null) == (ownedChangeOpening != null)) {
                "change output membership must be present exactly when changeOpening is present"
            }
            requireArtifactBridge()
            requireV4ProofBackend()
            var bundles: Array<ByteArray?>? = null
            var topUpProvenances: Array<ByteArray?>? = null
            var openings: Array<ByteArray?>? = null
            var witnesses: Array<ByteArray?>? = null
            var change: ByteArray? = null
            var outputMembership: ByteArray? = null
            var verifier: ByteArray? = null
            var operation: ByteArray? = null
            var archive: ByteArray? = null
            try {
                val bundleCopies = arrayOfNulls<ByteArray>(inputs.size)
                val provenanceCopies = arrayOfNulls<ByteArray>(inputs.size)
                val openingCopies = arrayOfNulls<ByteArray>(inputs.size)
                val witnessCopies = arrayOfNulls<ByteArray>(inputs.size)
                bundles = bundleCopies
                topUpProvenances = provenanceCopies
                openings = openingCopies
                witnesses = witnessCopies
                for (index in inputs.indices) {
                    val input = inputs[index]
                    bundleCopies[index] = input.bundle.noritoEncoded()
                    provenanceCopies[index] = input.topUpProvenance.noritoEncoded()
                    openingCopies[index] = input.opening.noritoEncoded()
                    witnessCopies[index] = input.membershipWitness.noritoEncoded()
                }
                change = ownedChangeOpening?.noritoEncoded() ?: byteArrayOf()
                outputMembership = outputMembershipPaths.nativeArchive()
                verifier = requireDigest(
                    transferVerifierCommitment,
                    "transferVerifierCommitment",
                )
                operation = requireDigest(operationId, "operationId")
                archive = nativeBuildAppendRequestV4(
                    Array(inputs.size) { checkNotNull(bundleCopies[it]) },
                    Array(inputs.size) { checkNotNull(provenanceCopies[it]) },
                    Array(inputs.size) { checkNotNull(openingCopies[it]) },
                    Array(inputs.size) { checkNotNull(witnessCopies[it]) },
                    checkNotNull(change),
                    checkNotNull(outputMembership),
                    checkNotNull(verifier),
                    checkNotNull(operation),
                    blockHeight,
                )
                return@transferChangeOpeningOwnership AppendRequestV4(
                    checkNotNull(archive),
                    ownedChangeOpening,
                )
            } finally {
                SecretArchiveWiper.wipeAll(bundles)
                SecretArchiveWiper.wipeAll(topUpProvenances)
                SecretArchiveWiper.wipeAll(openings)
                SecretArchiveWiper.wipeAll(witnesses)
                SecretArchiveWiper.wipe(change)
                SecretArchiveWiper.wipe(outputMembership)
                SecretArchiveWiper.wipe(verifier)
                SecretArchiveWiper.wipe(operation)
                SecretArchiveWiper.wipe(archive)
            }
        }

        @JvmStatic
        fun projectPeerPayment(payment: PeerPayment): PeerPaymentProjection {
            requireArtifactBridge()
            val fields = nativeProjectPeerPaymentV4(payment.noritoEncoded())
            val cursor = ProjectionCursor(fields, "peer payment projection")
            projectionVersion(cursor.next("version"), "peer payment projection")
            val operationId = requireDigest(cursor.next("operationId"), "operationId")
            val requestDigest = requireDigest(cursor.next("requestDigest"), "requestDigest")
            val topUpProvenance = TopUpProvenanceV4(cursor.next("topUpProvenance"))
            val projection = branchProjection(cursor)
            cursor.finish()
            val result = PeerPaymentProjection(
                projection,
                topUpProvenance,
                operationId,
                requestDigest,
            )
            operationId.fill(0)
            requestDigest.fill(0)
            return result
        }

        @JvmStatic
        fun prepareEligibilityPaymentV1(
            payment: PeerPayment,
            credential: EligibilityCredentialV1,
            request: RecipientPaymentRequest,
        ): EligibilityPaymentPayloadV1 {
            checkNotNull(credential.verificationTrustAnchor) {
                "credential has not passed native issuer/policy/finality verification"
            }
            requireArtifactBridge()
            val paymentArchive = payment.noritoEncoded()
            val credentialArchive = credential.noritoEncoded()
            val requestArchive = request.noritoEncoded()
            return try {
                EligibilityPaymentPayloadV1(nativePrepareEligibilityPaymentV1(
                    paymentArchive,
                    credentialArchive,
                    requestArchive,
                ))
            } finally {
                paymentArchive.fill(0)
                credentialArchive.fill(0)
                requestArchive.fill(0)
            }
        }

        @JvmStatic
        fun eligibilityPaymentSigningBytesV1(
            payload: EligibilityPaymentPayloadV1,
        ): ByteArray {
            requireArtifactBridge()
            val archive = payload.noritoEncoded()
            return try {
                nativeEligibilityPaymentSigningBytesV1(archive).also { bytes ->
                    require(bytes.isNotEmpty() && bytes.size <= MAX_PEER_ARCHIVE_BYTES_V2) {
                        "native eligibility signing preimage exceeds its bound"
                    }
                }
            } finally {
                archive.fill(0)
            }
        }

        @JvmStatic
        fun finalizeEligibilityPaymentV1(
            payload: EligibilityPaymentPayloadV1,
            signature: KagemushaDeviceSignatureV2,
        ): EligibilityPaymentEnvelopeV1 {
            requireArtifactBridge()
            val archive = payload.noritoEncoded()
            val rawSignature = signature.rawBytes()
            return try {
                EligibilityPaymentEnvelopeV1(nativeFinalizeEligibilityPaymentV1(
                    archive,
                    rawSignature,
                ))
            } finally {
                archive.fill(0)
                rawSignature.fill(0)
            }
        }

        @JvmStatic
        fun validateEligibilityPaymentStaticV1(
            envelope: EligibilityPaymentEnvelopeV1,
        ): EligibilityPaymentEnvelopeV1 {
            requireArtifactBridge()
            val archive = envelope.noritoEncoded()
            return try {
                val canonical = nativeValidateEligibilityPaymentStaticV1(archive)
                require(canonical.contentEquals(archive)) {
                    "native eligibility validator changed canonical envelope bytes"
                }
                EligibilityPaymentEnvelopeV1(canonical)
            } finally {
                archive.fill(0)
            }
        }

        @JvmStatic
        fun validateEligibilityPaymentFirstDeliveryV1(
            envelope: EligibilityPaymentEnvelopeV1,
            request: RecipientPaymentRequest,
            expectedIssuer: EligibilityIssuerPublicKeyV1,
            currentPolicyView: DeviceAttestationPolicyViewV1,
            receivedAtMilliseconds: Long,
        ): PeerPayment {
            require(receivedAtMilliseconds > 0) {
                "receivedAtMilliseconds must be positive"
            }
            val trustAnchor = checkNotNull(currentPolicyView.verificationTrustAnchor) {
                "currentPolicyView has not passed native finality verification"
            }
            requireArtifactBridge()
            val envelopeArchive = envelope.noritoEncoded()
            val requestArchive = request.noritoEncoded()
            val issuerArchive = expectedIssuer.noritoEncoded()
            val policyArchive = currentPolicyView.noritoEncoded()
            return try {
                PeerPayment(nativeValidateEligibilityPaymentFirstDeliveryFinalizedV1(
                    envelopeArchive,
                    requestArchive,
                    issuerArchive,
                    policyArchive,
                    trustAnchor.networkId.bytes(),
                    trustAnchor.trustedHeightContextId(),
                    receivedAtMilliseconds,
                ))
            } finally {
                envelopeArchive.fill(0)
                requestArchive.fill(0)
                issuerArchive.fill(0)
                policyArchive.fill(0)
            }
        }

        @JvmStatic
        fun projectInitResultV4(result: InitResultV4): InitProjectionV4 {
            requireArtifactBridge()
            val cursor = ProjectionCursor(
                nativeProjectInitResultV4(result.noritoEncoded()),
                "V4 init result projection",
            )
            projectionVersion(cursor.next("version"), "V4 init result projection")
            val topUpProvenance = TopUpProvenanceV4(cursor.next("topUpProvenance"))
            val branch = branchProjection(cursor)
            val publicStatementDigest =
                requireDigest(cursor.next("publicStatementDigest"), "publicStatementDigest")
            cursor.finish()
            return InitProjectionV4(
                branch,
                topUpProvenance,
                publicStatementDigest,
            )
        }

        /** Decode every wallet-safe field of an ABI-21 append result. */
        @JvmStatic
        fun projectSplitResultV4(result: SplitResultV4): SplitProjection {
            requireArtifactBridge()
            val cursor = ProjectionCursor(
                nativeProjectSplitResultV4(result.noritoEncoded()),
                "V4 split result projection",
            )
            projectionVersion(cursor.next("version"), "V4 split result projection")
            val payment = PeerPayment(cursor.next("peerPayment"))
            val operationId = requireDigest(cursor.next("operationId"), "operationId")
            val requestDigest = requireDigest(cursor.next("requestDigest"), "requestDigest")
            val splitBindingDigest =
                requireDigest(cursor.next("splitBindingDigest"), "splitBindingDigest")
            val recipientTopUpProvenance =
                TopUpProvenanceV4(cursor.next("recipientTopUpProvenance"))
            val recipient = branchProjection(cursor)
            val change = if (bool(cursor.next("changePresent"), "changePresent")) {
                Pair(
                    TopUpProvenanceV4(cursor.next("changeTopUpProvenance")),
                    branchProjection(cursor),
                )
            } else {
                null
            }
            cursor.finish()
            return SplitProjection(
                payment,
                recipient,
                change?.second,
                recipientTopUpProvenance,
                change?.first,
                operationId,
                requestDigest,
                splitBindingDigest,
            )
        }

        /** Decode the terminal decision and exact verified ABI-21 state. */
        @JvmStatic
        fun projectVerifyResultV4(result: VerifyResultV4): VerifyProjection {
            requireArtifactBridge()
            val cursor = ProjectionCursor(
                nativeProjectVerifyResultV4(result.noritoEncoded()),
                "V4 verify result projection",
            )
            projectionVersion(cursor.next("version"), "V4 verify result projection")
            val valid = bool(cursor.next("valid"), "valid")
            val chainAdmissible = bool(cursor.next("chainAdmissible"), "chainAdmissible")
            val lineageRedeemable = bool(cursor.next("lineageRedeemable"), "lineageRedeemable")
            val witnesslessRedemptionSupported = bool(
                cursor.next("witnesslessRedemptionSupported"),
                "witnesslessRedemptionSupported",
            )
            val commitment = cursor.next("commitment")
            val spendNullifier = cursor.next("spendNullifier")
            val amount = amount(cursor.next("atomicUnits"), cursor.next("scale"))
            val hopCount = integer(cursor.next("hopCount"), "hopCount")
            val proofStepCount = integer(cursor.next("proofStepCount"), "proofStepCount")
            val bundleDigest = cursor.next("bundleDigest")
            val assetDefinitionId = canonicalText(
                cursor.next("assetDefinitionId"),
                "assetDefinitionId",
            )
            val artifactBinding = ArtifactBindingV4(cursor.next("artifactBinding"))
            val requestDigest = cursor.next("requestDigest")
            val outputBindingDigest = cursor.next("outputBindingDigest")
            val verifierBackend = canonicalText(cursor.next("verifierBackend"), "verifierBackend")
            val verifierName = canonicalText(cursor.next("verifierName"), "verifierName")
            val verifierCircuitId =
                canonicalText(cursor.next("verifierCircuitId"), "verifierCircuitId")
            val activation = cursor.next("verifierActivationHeight").takeIf { it.isNotEmpty() }
                ?.let { longInteger(it, "verifierActivationHeight") }
            val withdrawal = cursor.next("verifierWithdrawalHeight").takeIf { it.isNotEmpty() }
                ?.let { longInteger(it, "verifierWithdrawalHeight") }
            val verifiedAtBlockHeight =
                longInteger(cursor.next("verifiedAtBlockHeight"), "verifiedAtBlockHeight")
            val verifiedAtMilliseconds =
                longInteger(cursor.next("verifiedAtMilliseconds"), "verifiedAtMilliseconds")
            val claimCount = projectionCount(cursor.next("branchClaimCount"), "branchClaim")
            val claims = List(claimCount) { BranchClaim(cursor.next("branchClaim[$it]")) }
            cursor.finish()
            return VerifyProjection(
                valid,
                chainAdmissible,
                lineageRedeemable,
                witnesslessRedemptionSupported,
                commitment,
                spendNullifier,
                amount,
                hopCount,
                proofStepCount,
                bundleDigest,
                assetDefinitionId,
                artifactBinding,
                requestDigest,
                outputBindingDigest,
                verifierBackend,
                verifierName,
                verifierCircuitId,
                activation,
                withdrawal,
                verifiedAtBlockHeight,
                verifiedAtMilliseconds,
                claims,
            )
        }

        /** Decode the authorization payload and optional spendable change of a V4 redemption. */
        @JvmStatic
        fun projectRedeemBuildResultV4(result: RedeemBuildResultV4): RedeemBuildProjection {
            requireArtifactBridge()
            val cursor = ProjectionCursor(
                nativeProjectRedeemBuildResultV4(result.noritoEncoded()),
                "V4 redeem build projection",
            )
            projectionVersion(cursor.next("version"), "V4 redeem build projection")
            val unsigned = RedeemUnsignedV4(cursor.next("unsigned"))
            val authorizationDigest = cursor.next("authorizationDigest")
            val operationId = cursor.next("operationId")
            val change = if (bool(cursor.next("changePresent"), "changePresent")) {
                Pair(
                    TopUpProvenanceV4(cursor.next("changeTopUpProvenance")),
                    branchProjection(cursor),
                )
            } else {
                null
            }
            cursor.finish()
            return RedeemBuildProjection(
                unsigned,
                authorizationDigest,
                change?.second,
                change?.first,
                operationId,
            )
        }

        @JvmStatic
        fun buildVerifyRequestV4(
            bundle: BundleV4,
            recipientRequest: RecipientPaymentRequest,
            topUpProvenance: TopUpProvenanceV4,
            maximumHops: Int,
            blockHeight: Long,
            verifiedAtMilliseconds: Long,
        ): VerifyRequestV4 {
            requireArtifactBridge()
            requireV4ProofBackend()
            return VerifyRequestV4(nativeBuildVerifyRequestV4(
                bundle.noritoEncoded(),
                recipientRequest.noritoEncoded(),
                topUpProvenance.noritoEncoded(),
                maximumHops,
                blockHeight,
                verifiedAtMilliseconds,
            ))
        }

        @JvmStatic
        fun buildRedeemRequestV4(
            input: SpendableBranchV4,
            recipientAccountId: String,
            chainDiscriminant: Int,
            amount: KagemushaScaledAmount,
            changeOpening: NoteOpening?,
            changeOutputMembershipPaths: OutputMembershipPaths?,
            unshieldVerifierCommitment: ByteArray,
            operationId: ByteArray,
            blockHeight: Long,
        ): RedeemRequestV4 = transferChangeOpeningOwnership(changeOpening) { ownedChangeOpening ->
            requireArtifactBridge()
            requireV4ProofBackend()
            require((ownedChangeOpening != null) == (changeOutputMembershipPaths != null)) {
                "change output membership must be present exactly when changeOpening is present"
            }
            changeOutputMembershipPaths?.let {
                require(it.recipient == null && it.change != null) {
                    "redemption change requires exactly one change output path"
                }
            }
            var change: ByteArray? = null
            var outputMembership: ByteArray? = null
            var verifier: ByteArray? = null
            var operation: ByteArray? = null
            var bundleArchive: ByteArray? = null
            var topUpProvenanceArchive: ByteArray? = null
            var openingArchive: ByteArray? = null
            var witnessArchive: ByteArray? = null
            var recipient: ByteArray? = null
            var atomicUnits: ByteArray? = null
            var archive: ByteArray? = null
            try {
                change = ownedChangeOpening?.noritoEncoded() ?: byteArrayOf()
                outputMembership = changeOutputMembershipPaths?.nativeArchive() ?: byteArrayOf()
                verifier = requireDigest(
                    unshieldVerifierCommitment,
                    "unshieldVerifierCommitment",
                )
                operation = requireDigest(operationId, "operationId")
                bundleArchive = input.bundle.noritoEncoded()
                topUpProvenanceArchive = input.topUpProvenance.noritoEncoded()
                openingArchive = input.opening.noritoEncoded()
                witnessArchive = input.membershipWitness.noritoEncoded()
                recipient = utf8(recipientAccountId, "recipientAccountId")
                atomicUnits = utf8(amount.atomicUnits, "atomicUnits")
                archive = nativeBuildRedeemRequestV4(
                    checkNotNull(bundleArchive),
                    checkNotNull(topUpProvenanceArchive),
                    checkNotNull(openingArchive),
                    checkNotNull(witnessArchive),
                    checkNotNull(recipient),
                    requireChainDiscriminant(chainDiscriminant),
                    checkNotNull(atomicUnits),
                    amount.scale,
                    checkNotNull(change),
                    checkNotNull(outputMembership),
                    checkNotNull(verifier),
                    checkNotNull(operation),
                    blockHeight,
                )
                return@transferChangeOpeningOwnership RedeemRequestV4(
                    checkNotNull(archive),
                    ownedChangeOpening,
                )
            } finally {
                SecretArchiveWiper.wipe(change)
                SecretArchiveWiper.wipe(outputMembership)
                SecretArchiveWiper.wipe(verifier)
                SecretArchiveWiper.wipe(operation)
                SecretArchiveWiper.wipe(bundleArchive)
                SecretArchiveWiper.wipe(topUpProvenanceArchive)
                SecretArchiveWiper.wipe(openingArchive)
                SecretArchiveWiper.wipe(witnessArchive)
                SecretArchiveWiper.wipe(recipient)
                SecretArchiveWiper.wipe(atomicUnits)
                SecretArchiveWiper.wipe(archive)
            }
        }

        @JvmStatic
        fun prepareAcknowledgement(
            request: RecipientPaymentRequest,
            payment: PeerPayment,
            acceptedAtMilliseconds: Long,
        ): AcknowledgementPreparation {
            requireArtifactBridge()
            val fields = nativePrepareAcknowledgementV2(
                request.noritoEncoded(), payment.noritoEncoded(), acceptedAtMilliseconds,
            )
            requireFieldCount(fields, 6, "acknowledgement preparation")
            return AcknowledgementPreparation(
                AcknowledgementPayload(fields[0]), fields[1], fields[2], fields[3], fields[4], fields[5],
            )
        }

        @JvmStatic
        fun signAcknowledgement(
            preparation: AcknowledgementPreparation,
            signature: KagemushaDeviceSignatureV2,
            request: RecipientPaymentRequest,
            payment: PeerPayment,
        ): ReceiverAcknowledgement {
            requireArtifactBridge()
            return ReceiverAcknowledgement(
                nativeCreateAcknowledgementV2(
                    preparation.payload.noritoEncoded(), signature.rawBytes(),
                    request.noritoEncoded(), payment.noritoEncoded(),
                ),
            )
        }

        /**
         * Verifies a receiver-signed delivery receipt. Under cash_handoff_v1
         * this is never a sender commit, acceptance, rollback, or clawback gate.
         */
        @JvmStatic
        fun verifyAcknowledgement(
            acknowledgement: ReceiverAcknowledgement,
            request: RecipientPaymentRequest,
            payment: PeerPayment,
        ): AcknowledgementVerification {
            requireArtifactBridge()
            val fields = nativeVerifyAcknowledgementV2(
                acknowledgement.noritoEncoded(), request.noritoEncoded(), payment.noritoEncoded(),
            )
            requireFieldCount(fields, 5, "acknowledgement verification")
            return AcknowledgementVerification(
                bool(fields[0], "valid"), fields[1], fields[2], fields[3], fields[4],
            )
        }

        /** Build the first spendable branch from a finalized top-up anchor. */
        @JvmStatic
        fun initSpendV4(request: InitRequestV4): InitResultV4 {
            return withHeavyProofPermit("init spend") {
                var secretArchive: ByteArray? = null
                var terminal = true
                try {
                    requireProofBackend()
                    val borrowed = request.borrowForNative().also { secretArchive = it }
                    InitResultV4(callNativeLifecycle("init spend") {
                        nativeInitSpendV4(borrowed)
                    })
                } catch (failure: ProofWorkerBusyException) {
                    terminal = false
                    throw failure
                } finally {
                    SecretArchiveWiper.wipe(secretArchive)
                    if (terminal) request.close()
                }
            }
        }

        /** Prove one exact recipient output and optional independently spendable sender change. */
        @JvmStatic
        fun appendSpendV4(
            request: AppendRequestV4,
            recipientRequest: RecipientPaymentRequest,
            verifiedAtMilliseconds: Long,
        ): SplitResultV4 = withHeavyProofPermit("append spend") {
            var secretArchive: ByteArray? = null
            var terminal = true
            try {
                require(verifiedAtMilliseconds > 0) {
                    "verifiedAtMilliseconds must be positive"
                }
                requireProofBackend()
                val borrowed = request.borrowForNative().also { secretArchive = it }
                val resultArchive = callNativeLifecycle("append spend") {
                    nativeAppendSpendV4(
                        borrowed,
                        recipientRequest.noritoEncoded(),
                        verifiedAtMilliseconds,
                    )
                }
                transferChangeOpeningOwnership(request.takeChangeOpening()) { opening ->
                    SplitResultV4(resultArchive, opening)
                }
            } catch (failure: ProofWorkerBusyException) {
                terminal = false
                throw failure
            } finally {
                SecretArchiveWiper.wipe(secretArchive)
                if (terminal) request.close()
            }
        }

        /** Verify the recursive proof, exact split bindings, membership, and hop limit. */
        @JvmStatic
        fun verifySpendV4(request: VerifyRequestV4): VerifyResultV4 {
            return withHeavyProofPermit("verify spend") {
                requireProofBackend()
                VerifyResultV4(callNativeLifecycle("verify spend") {
                    nativeVerifySpendV4(request.borrowForNative())
                })
            }
        }

        /** Build a full or partial redemption and its optional proof-bound offline change. */
        @JvmStatic
        fun buildRedeemV4(request: RedeemRequestV4): RedeemBuildResultV4 =
            withHeavyProofPermit("build redeem") {
                var secretArchive: ByteArray? = null
                var terminal = true
                try {
                    requireProofBackend()
                    val borrowed = request.borrowForNative().also { secretArchive = it }
                    val resultArchive = callNativeLifecycle("build redeem") {
                        nativeBuildRedeemV4(borrowed)
                    }
                    transferChangeOpeningOwnership(request.takeChangeOpening()) { opening ->
                        RedeemBuildResultV4(resultArchive, opening)
                    }
                } catch (failure: ProofWorkerBusyException) {
                    terminal = false
                    throw failure
                } finally {
                    SecretArchiveWiper.wipe(secretArchive)
                    if (terminal) request.close()
                }
            }

        @JvmStatic
        fun newToriiClient(
            baseUri: URI,
            transport: TransportExecutor,
            localSigningContext: LocalSigningContext,
        ): ToriiClient = ToriiClient(baseUri, transport, localSigningContext)

        internal fun isExactBridgeAbi(abiVersion: Int): Boolean =
            abiVersion == REQUIRED_NATIVE_BRIDGE_ABI_VERSION

        internal fun detectExactNativeAvailability(
            loadLibrary: () -> Unit,
            abiVersion: () -> Int,
            symbolProbe: () -> Boolean,
        ): Boolean = try {
            loadLibrary()
            isExactBridgeAbi(abiVersion()) && symbolProbe()
        } catch (_: UnsatisfiedLinkError) {
            false
        } catch (_: SecurityException) {
            false
        } catch (_: RuntimeException) {
            false
        }

        private fun loadArtifactBridge(): Boolean =
            detectExactNativeAvailability(
                loadLibrary = { System.loadLibrary(LIBRARY_NAME) },
                abiVersion = { nativeBridgeAbiVersion() },
                symbolProbe = {
                    expectRejectedSymbolProbe {
                        nativeArtifactBeginV4(byteArrayOf(0), ByteArray(32), ByteArray(32))
                    } && expectRejectedSymbolProbe {
                        nativeVerifyOfflineDeviceAttestationPolicyViewV1(
                            byteArrayOf(0),
                            ByteArray(32),
                            ByteArray(32),
                            1,
                        )
                    } && expectRejectedSymbolProbe {
                        nativeVerifyOfflineDeviceEligibilityCredentialV1(
                            byteArrayOf(0),
                            byteArrayOf(0),
                            byteArrayOf(0),
                            ByteArray(32),
                            ByteArray(32),
                            1,
                        )
                    } && expectRejectedSymbolProbe {
                        nativeVerifyOfflineDeviceEligibilityPeerCertificateV1(
                            byteArrayOf(0),
                            byteArrayOf(0),
                            byteArrayOf(0),
                            ByteArray(32),
                            ByteArray(32),
                            1,
                        )
                    } && expectRejectedSymbolProbe {
                        nativeValidateEligibilityPaymentFirstDeliveryFinalizedV1(
                            byteArrayOf(0),
                            byteArrayOf(0),
                            byteArrayOf(0),
                            byteArrayOf(0),
                            ByteArray(32),
                            ByteArray(32),
                            1,
                        )
                    }
                },
            )

        /*
         * The symbol exists in both lifecycle states. Before production promotion native rejects
         * the probe as unavailable; after promotion it rejects the malformed manifest. Either
         * typed rejection proves linkage. Only an absent JNI symbol must make the bridge false.
         */
        private fun expectRejectedSymbolProbe(probe: () -> Unit): Boolean = try {
            probe()
            false
        } catch (_: IllegalArgumentException) {
            true
        } catch (_: IllegalStateException) {
            true
        }

        internal fun detectProductionProofBackendCompilation(probe: () -> Unit): Boolean = try {
            probe()
            false
        } catch (_: IllegalArgumentException) {
            // Production promotion passed and native reached malformed-artifact validation.
            true
        } catch (_: IllegalStateException) {
            // The symbol is linked, but the default/candidate build rejected production use.
            false
        } catch (_: UnsatisfiedLinkError) {
            false
        } catch (_: SecurityException) {
            false
        }

        private fun requireArtifactBridge() {
            requireV4ArtifactBridge()
        }

        private fun requireV4ArtifactBridge() {
            check(artifactBridgeAvailable) {
                "$LIBRARY_NAME ABI $REQUIRED_NATIVE_BRIDGE_ABI_VERSION artifact streaming is unavailable"
            }
        }

        private fun requireProofBackend() {
            requireV4ProofBackend()
        }

        private fun requireV4ProofBackend() {
            check(isProofBackendAvailable()) {
                "$LIBRARY_NAME ABI $REQUIRED_NATIVE_BRIDGE_ABI_VERSION Kagemusha proof backend is unavailable"
            }
        }

        private inline fun callNativeLifecycle(label: String, call: () -> ByteArray?): ByteArray =
            try {
                checkNotNull(call()) { "native Kagemusha $label returned no archive" }
            } catch (failure: IllegalStateException) {
                if (failure.message.orEmpty().contains(NATIVE_BUSY_MESSAGE)) {
                    throw ProofWorkerBusyException(
                        "Kagemusha $label is busy; retry after the active proof completes",
                        failure,
                    )
                }
                throw failure
            } catch (failure: UnsatisfiedLinkError) {
                throw IllegalStateException("native Kagemusha $label entrypoint is unavailable", failure)
            }

        private fun utf8(value: String?, field: String): ByteArray {
            require(value != null && value.isNotEmpty() && value == value.trim()) {
                "$field must be canonical non-empty text"
            }
            return value.toByteArray(Charsets.UTF_8)
        }

        private fun requireChainDiscriminant(value: Int): Int {
            require(value in 0..0xffff) { "chainDiscriminant must fit in u16" }
            return value
        }

        private fun requiredBytes(value: ByteArray?, field: String): ByteArray {
            require(value != null && value.isNotEmpty()) { "$field must not be empty" }
            return value.copyOf()
        }

        private fun requireFieldCount(fields: Array<ByteArray>?, expected: Int, label: String) {
            check(fields != null && fields.size == expected) {
                "native Kagemusha $label returned invalid fields"
            }
        }

        private fun requireIosAppAttestAuthenticatorDataProjection(authenticatorData: ByteArray) {
            check(
                authenticatorData.size in IOS_APP_ATTEST_AUTHENTICATOR_DATA_MIN_BYTES..
                    IOS_APP_ATTEST_AUTHENTICATOR_DATA_MAX_BYTES,
            ) {
                "native Kagemusha App Attest finalization returned invalid authenticator data"
            }
            val flags = authenticatorData[32].toInt() and 0xff
            check(flags == IOS_APP_ATTEST_EXTENSION_DATA_FLAG) {
                "native Kagemusha App Attest finalization must return extension-bearing authenticator data"
            }
        }

        private fun amount(atomic: ByteArray, scale: ByteArray): KagemushaScaledAmount =
            KagemushaScaledAmount.fromAtomicUnits(
                atomic.toString(Charsets.US_ASCII),
                integer(scale, "scale"),
            )

        private fun integer(value: ByteArray, field: String): Int =
            value.toString(Charsets.US_ASCII).toIntOrNull()
                ?: error("native Kagemusha $field is invalid")

        private fun longInteger(value: ByteArray, field: String): Long =
            value.toString(Charsets.US_ASCII).toLongOrNull()
                ?: error("native Kagemusha $field is invalid")

        private fun outputMembershipSiblings(
            flattened: ByteArray,
            field: String,
        ): List<ByteArray> {
            check(flattened.size == CONFIDENTIAL_TREE_DEPTH * 32) {
                "native Kagemusha $field has an invalid sibling count"
            }
            return List(CONFIDENTIAL_TREE_DEPTH) { index ->
                flattened.copyOfRange(index * 32, (index + 1) * 32)
            }
        }

        private fun outputMembershipPathFromNativeProjection(
            fields: Array<ByteArray>,
            leafIndex: Int,
            siblingsIndex: Int,
            directionsIndex: Int,
            rootIndex: Int,
            field: String,
        ): OutputMembershipPath = try {
            OutputMembershipPath(
                leafIndex,
                outputMembershipSiblings(fields[siblingsIndex], "$field.siblings"),
                fields[directionsIndex],
                fields[rootIndex],
            )
        } catch (failure: IllegalArgumentException) {
            throw IllegalStateException("native Kagemusha $field is invalid", failure)
        }

        private fun outputMembershipLeafFromNativeProjection(
            fields: Array<ByteArray>,
            offset: Int,
            field: String,
        ): OutputMembershipLeafPaths? {
            val values = fields.sliceArray(offset until offset + 7)
            if (values.all { it.isEmpty() }) return null
            check(values.all { it.isNotEmpty() }) {
                "native Kagemusha $field is only partially present"
            }
            val leafIndex = integer(fields[offset], "$field.leafIndex")
            return OutputMembershipLeafPaths(
                outputMembershipPathFromNativeProjection(
                    fields,
                    leafIndex,
                    offset + 1,
                    offset + 2,
                    offset + 3,
                    "$field.updatePath",
                ),
                outputMembershipPathFromNativeProjection(
                    fields,
                    leafIndex,
                    offset + 4,
                    offset + 5,
                    offset + 6,
                    "$field.membershipPath",
                ),
            )
        }

        private fun outputMembershipPathsFromNativeProjection(
            fields: Array<ByteArray>,
        ): OutputMembershipPaths {
            requireFieldCount(fields, 21, "V4 output membership derivation")
            val recipient = outputMembershipLeafFromNativeProjection(fields, 3, "recipient")
            val change = outputMembershipLeafFromNativeProjection(fields, 10, "change")
            check((17..20).all { fields[it].isNotEmpty() }) {
                "native Kagemusha dummy output membership path is absent"
            }
            val dummyLeafIndex = integer(fields[17], "dummy.leafIndex")
            val dummy = outputMembershipPathFromNativeProjection(
                fields,
                dummyLeafIndex,
                18,
                19,
                20,
                "dummy.path",
            )
            return try {
                OutputMembershipPaths(
                    fields[1],
                    fields[2],
                    recipient,
                    change,
                    dummy,
                    fields[0],
                )
            } catch (failure: IllegalArgumentException) {
                throw IllegalStateException(
                    "native Kagemusha V4 output membership derivation is invalid",
                    failure,
                )
            }
        }

        private fun canonicalText(value: ByteArray, field: String): String {
            val text = value.toString(Charsets.UTF_8)
            check(text.isNotEmpty() && text == text.trim() && text.none(Char::isISOControl)) {
                "native Kagemusha $field is invalid"
            }
            return text
        }

        private fun activeVerifier(archive: ByteArray): ActiveVerifier? {
            if (archive.isEmpty()) return null
            val fields = nativeProjectActiveVerifierV2(archive)
            requireFieldCount(fields, 9, "active verifier projection")
            return ActiveVerifier(
                backend = canonicalText(fields[0], "verifierBackend"),
                name = canonicalText(fields[1], "verifierName"),
                version = integer(fields[2], "verifierVersion"),
                circuitId = canonicalText(fields[3], "verifierCircuitId"),
                commitment = requireDigest(fields[4], "verifierCommitment"),
                publicInputsSchemaHash = requireDigest(fields[5], "publicInputsSchemaHash"),
                maximumProofBytes = integer(fields[6], "maximumProofBytes"),
                activationHeight = longInteger(fields[7], "activationHeight"),
                withdrawalHeight = fields[8].takeIf { it.isNotEmpty() }
                    ?.let { longInteger(it, "withdrawalHeight") },
            )
        }

        private fun authenticatedArtifactSet(archive: ByteArray): AuthenticatedArtifactSet? {
            if (archive.isEmpty()) return null
            val fields = nativeProjectAuthenticatedArtifactSetV4(archive)
            requireFieldCount(fields, 8, "authenticated artifact-set projection")
            return AuthenticatedArtifactSet(
                generation = canonicalText(fields[0], "artifactGeneration"),
                manifestSha256 = requireDigest(fields[1], "artifactManifestSha256"),
                releasePolicySha256 = requireDigest(fields[2], "artifactReleasePolicySha256"),
                releaseAttestationSha256 =
                    requireDigest(fields[3], "artifactReleaseAttestationSha256"),
                activationHeight = longInteger(fields[4], "artifactActivationHeight"),
                withdrawalHeight = longInteger(fields[5], "artifactWithdrawalHeight"),
                maximumProofBytes = integer(fields[6], "artifactMaximumProofBytes"),
                assetScale = integer(fields[7], "artifactAssetScale"),
            )
        }

        private fun bool(value: ByteArray, field: String): Boolean {
            check(value.size == 1 && (value[0] == 0.toByte() || value[0] == 1.toByte())) {
                "native Kagemusha $field is invalid"
            }
            return value[0] == 1.toByte()
        }

        private fun projectionVersion(value: ByteArray, field: String) {
            check(
                value.size == 4 &&
                    value[0] == 0.toByte() && value[1] == 0.toByte() &&
                    value[2] == 0.toByte() &&
                    (value[3].toInt() and 0xff) == EXACT_STATE_PROJECTION_VERSION,
            ) { "native Kagemusha $field version is unsupported" }
        }

        private fun projectionCount(value: ByteArray, field: String): Int {
            check(value.size == 4) { "native Kagemusha $field count is invalid" }
            val count = value.fold(0L) { result, octet ->
                (result shl 8) or (octet.toLong() and 0xff)
            }
            check(count in 1..MAXIMUM_BRANCH_CLAIMS.toLong()) {
                "native Kagemusha $field count is outside the exact-state limit"
            }
            return count.toInt()
        }

        private class ProjectionCursor(
            private val fields: Array<ByteArray>,
            private val label: String,
            start: Int = 0,
        ) {
            var index: Int = start
                private set

            fun next(field: String): ByteArray {
                check(index < fields.size) { "native Kagemusha $label omitted $field" }
                return fields[index++]
            }

            fun finish() {
                check(index == fields.size) { "native Kagemusha $label has trailing fields" }
            }
        }

        private fun branchProjection(cursor: ProjectionCursor): BranchProjection {
            val bundle = BundleV4(cursor.next("bundle"))
            val witness = NoteMembershipWitness(cursor.next("membershipWitness"))
            val commitment = cursor.next("commitment")
            val spendNullifier = cursor.next("spendNullifier")
            val amount = amount(cursor.next("atomicUnits"), cursor.next("scale"))
            val hopCount = integer(cursor.next("hopCount"), "hopCount")
            val proofStepCount = integer(cursor.next("proofStepCount"), "proofStepCount")
            val bundleDigest = cursor.next("bundleDigest")
            val artifactBinding = ArtifactBindingV4(cursor.next("artifactBinding"))
            val claimCount = projectionCount(cursor.next("branchClaimCount"), "branchClaim")
            val claims = List(claimCount) { BranchClaim(cursor.next("branchClaim[$it]")) }
            return BranchProjection(
                bundle, witness, commitment, spendNullifier, amount, hopCount,
                proofStepCount, bundleDigest, artifactBinding, claims,
            )
        }

        private fun requireManifest(value: ByteArray?): ByteArray {
            require(value != null && value.isNotEmpty() && value.size <= MAX_MANIFEST_BYTES) {
                "manifestNorito must contain 1..$MAX_MANIFEST_BYTES bytes"
            }
            return value.copyOf()
        }

        private fun requireDigest(value: ByteArray?, name: String): ByteArray {
            require(value != null && value.size == 32) { "$name must contain exactly 32 bytes" }
            require(value.any { it.toInt() != 0 }) { "$name must be non-zero" }
            return value.copyOf()
        }

        private fun requireFinalityCheckpointContext(value: ByteArray?, name: String): ByteArray =
            requireDigest(value, name).also { context ->
                if (context.last().toInt() and 1 != 1) {
                    context.fill(0)
                    throw IllegalArgumentException("$name must preserve the Iroha hash marker")
                }
            }

        private fun requireBoundedBytes(
            value: ByteArray?,
            name: String,
            maximumBytes: Int,
        ): ByteArray {
            require(value != null && value.isNotEmpty() && value.size <= maximumBytes) {
                "$name must contain 1..$maximumBytes bytes"
            }
            return value.copyOf()
        }

        internal fun requireChunk(value: ByteArray?): ByteArray {
            require(value != null && value.isNotEmpty() && value.size <= MAX_ARTIFACT_CHUNK_BYTES) {
                "chunk must contain 1..$MAX_ARTIFACT_CHUNK_BYTES bytes"
            }
            return value.copyOf()
        }

        private fun requireCanonicalArchive(
            value: ByteArray?,
            schema: String,
            field: String,
            maximumBytes: Int,
        ): ByteArray {
            require(value != null && value.isNotEmpty() && value.size <= maximumBytes) {
                "$field must contain 1..$maximumBytes bytes"
            }
            val archive = value.copyOf()
            return try {
                val decoded = try {
                    NoritoHeader.decode(archive, SchemaHash.hash16(schema))
                } catch (failure: RuntimeException) {
                    throw IllegalArgumentException("$field must contain canonical $schema", failure)
                }
                val header = decoded.header
                require(
                    header.compression == NoritoHeader.COMPRESSION_NONE &&
                        header.flags == NoritoHeader.COMPACT_LEN &&
                        decoded.payload.isNotEmpty() &&
                        archive.size == NoritoHeader.HEADER_LENGTH +
                            peerArchivePadding(schema) + decoded.payload.size &&
                        header.encode().contentEquals(
                            archive.copyOfRange(0, NoritoHeader.HEADER_LENGTH),
                        ),
                ) { "$field must use canonical compact Norito framing" }
                header.validateChecksum(decoded.payload)
                archive
            } catch (failure: Throwable) {
                archive.fill(0)
                throw failure
            }
        }

        private fun peerArchivePadding(schema: String): Int = when (schema) {
            "iroha_data_model::offline::model::KagemushaRecipientPaymentRequestV2",
            "iroha_torii_shared::offline_api::OfflineRecipientReceiveOfferV2",
            "iroha_data_model::offline::model::KagemushaRecursiveSpendPeerPaymentV4",
            "iroha_data_model::offline::model::KagemushaEligibilityPaymentEnvelopePayloadV1",
            "iroha_data_model::offline::model::KagemushaEligibilityPaymentEnvelopeV1" -> 8
            "iroha_data_model::offline::model::KagemushaReceiverAcknowledgementV2" -> 0
            else -> 0
        }

        private fun hex(digest: ByteArray): String = buildString(64) {
            for (octet in digest) append("%02x".format(octet.toInt() and 0xff))
        }

        @JvmStatic
        private external fun nativeBridgeAbiVersion(): Int

        @JvmStatic
        private external fun nativePastaCycleV4BackendAvailable(): Boolean

        @JvmStatic
        private external fun nativeArtifactBeginV4(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
            expectedArtifactSha256: ByteArray,
        ): Long

        @JvmStatic
        private external fun nativeArtifactWriteV4(handle: Long, chunk: ByteArray)

        @JvmStatic
        private external fun nativeArtifactFinalizeV4(handle: Long)

        @JvmStatic
        private external fun nativeArtifactCancelV4(handle: Long)

        @JvmStatic
        private external fun nativeArtifactSetInstallV4(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
            trustedPolicyNorito: ByteArray,
            releaseAttestationNorito: ByteArray,
            benchmarkEvidence: ByteArray,
            cryptographicReview: ByteArray,
            promotionRecordNorito: ByteArray,
            artifactHandles: LongArray,
        )

        @JvmStatic
        private external fun nativeArtifactSetIsInstalledV4(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
        ): Boolean

        @JvmStatic
        private external fun nativeInstalledManifestSha256V4(): ByteArray

        @JvmStatic
        private external fun nativeBuildArtifactBindingV4(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
        ): ByteArray

        @JvmStatic
        private external fun nativeArtifactSetUninstallV4(manifestSha256: ByteArray)

        @JvmStatic
        private external fun nativeInitSpendV4(requestNorito: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeAppendSpendV4(
            requestNorito: ByteArray,
            recipientRequestNorito: ByteArray,
            verifiedAtMilliseconds: Long,
        ): ByteArray?

        @JvmStatic
        private external fun nativeVerifySpendV4(requestNorito: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeBuildRedeemV4(requestNorito: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativePrepareRecipientRequestV2(
            networkId: ByteArray,
            chainDiscriminant: Int,
            asset: ByteArray,
            atomicUnits: ByteArray,
            scale: Int,
            recipient: ByteArray,
            receiverDeviceId: ByteArray,
            receiverPublicKey: ByteArray,
            requestId: ByteArray,
            issuedAtMilliseconds: Long,
            expiresAtMilliseconds: Long,
            spendKey: ByteArray,
            rho: ByteArray,
            diversifier: ByteArray,
        ): Array<ByteArray>

        @JvmStatic private external fun nativeCreateRecipientRequestV2(payload: ByteArray, signature: ByteArray): ByteArray
        @JvmStatic private external fun nativeVerifyRecipientRequestV2(request: ByteArray, verifiedAtMilliseconds: Long): ByteArray
        @JvmStatic private external fun nativeCreateRecipientLineageQueryV2(networkId: ByteArray, chainDiscriminant: Int, recipient: ByteArray, receiverDeviceId: ByteArray, asset: ByteArray, trustedCheckpointHeight: Long): ByteArray
        @JvmStatic private external fun nativeVerifyRecipientRegistrationLineageV2(request: ByteArray, lineage: ByteArray, verifiedAtMilliseconds: Long, trustedCheckpointHeight: Long, trustedCheckpointContextId: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeCreateRecipientReceiveOfferV2(request: ByteArray, lineage: ByteArray, publisherCheckpointEnvelope: ByteArray): ByteArray
        @JvmStatic private external fun nativeProjectRecipientReceiveOfferV2(offer: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeVerifyRecipientReceiveOfferV2(offer: ByteArray, verifiedAtMilliseconds: Long, trustedCheckpointHeight: Long, trustedCheckpointContextId: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeBuildOutputMembershipFrontierV4(leafIndex: Int, flattenedSiblings: ByteArray, directions: ByteArray, root: ByteArray): ByteArray
        @JvmStatic private external fun nativeDeriveOutputMembershipPathsV4(frontier: ByteArray, recipientCommitment: ByteArray, changeCommitment: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeValidateSpendableBranchV4(bundle: ByteArray, provenance: ByteArray, membershipWitness: ByteArray, opening: ByteArray, blockHeight: Long): ByteArray
        @JvmStatic private external fun nativeBuildOutputMembershipPathsV4(initialRoot: ByteArray, finalRoot: ByteArray, recipientFields: Array<ByteArray>, changeFields: Array<ByteArray>, dummyFields: Array<ByteArray>): ByteArray
        @JvmStatic private external fun nativeBuildInitRequestV4(anchor: ByteArray, proof: ByteArray, roster: ByteArray, opening: ByteArray, outputMembership: ByteArray): ByteArray
        @JvmStatic private external fun nativeProjectVerifiedTopUpFinalityV4(anchor: ByteArray, finalityProof: ByteArray, rosterArtifact: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeBuildTopUpProvenanceV4(bundle: ByteArray, roster: ByteArray, anchors: Array<ByteArray>, finalityProofs: Array<ByteArray>, blockHeight: Long): ByteArray
        @JvmStatic private external fun nativeValidateTopUpProvenanceV4(bundle: ByteArray, provenance: ByteArray, blockHeight: Long): ByteArray
        @JvmStatic private external fun nativeBuildAppendRequestV4(bundles: Array<ByteArray>, topUpProvenances: Array<ByteArray>, openings: Array<ByteArray>, witnesses: Array<ByteArray>, changeOpening: ByteArray, outputMembership: ByteArray, verifierCommitment: ByteArray, operationId: ByteArray, blockHeight: Long): ByteArray
        @JvmStatic private external fun nativeProjectPeerPaymentV4(payment: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeEncodeOfflineDevicePolicyProofRequestV1(trustedCheckpointHeight: Long, trustedCheckpointContextId: ByteArray): ByteArray
        @JvmStatic private external fun nativeVerifyOfflineDevicePolicyProofV1(proofPage: ByteArray, expectedNetworkId: ByteArray, trustedCheckpointHeight: Long, trustedCheckpointContextId: ByteArray, evaluationTimeMilliseconds: Long): ByteArray
        @JvmStatic private external fun nativeEncodeOfflineDeviceEligibilityRequestV1(registrationHash: ByteArray, deviceId: ByteArray, attestationKeyId: ByteArray, requestedTtlMilliseconds: Long): ByteArray
        @JvmStatic private external fun nativeVerifyOfflineDeviceEligibilityResponseV1(response: ByteArray, expectedRegistrationHash: ByteArray, expectedIssuer: ByteArray, expectedNetworkId: ByteArray, trustedContextId: ByteArray, evaluationTimeMilliseconds: Long): ByteArray
        @JvmStatic private external fun nativeVerifyOfflineDeviceAttestationPolicyViewV1(policyView: ByteArray, expectedNetworkId: ByteArray, trustedContextId: ByteArray, evaluationTimeMilliseconds: Long): ByteArray
        @JvmStatic private external fun nativeProjectOfflineDeviceAttestationPolicyViewClaimsV1(policyView: ByteArray, expectedNetworkId: ByteArray, trustedContextId: ByteArray, evaluationTimeMilliseconds: Long): ByteArray
        @JvmStatic private external fun nativeVerifyOfflineDeviceEligibilityCredentialV1(credential: ByteArray, expectedIssuer: ByteArray, policyView: ByteArray, expectedNetworkId: ByteArray, trustedContextId: ByteArray, evaluationTimeMilliseconds: Long): ByteArray
        @JvmStatic private external fun nativeVerifyOfflineDeviceEligibilityPeerCertificateV1(credential: ByteArray, expectedIssuer: ByteArray, policyView: ByteArray, expectedNetworkId: ByteArray, trustedContextId: ByteArray, evaluationTimeMilliseconds: Long): ByteArray
        @JvmStatic private external fun nativePrepareEligibilityPaymentV1(payment: ByteArray, credential: ByteArray, request: ByteArray): ByteArray
        @JvmStatic private external fun nativeEligibilityPaymentSigningBytesV1(payload: ByteArray): ByteArray
        @JvmStatic private external fun nativeFinalizeEligibilityPaymentV1(payload: ByteArray, signature: ByteArray): ByteArray
        @JvmStatic private external fun nativeValidateEligibilityPaymentStaticV1(envelope: ByteArray): ByteArray
        @JvmStatic private external fun nativeValidateEligibilityPaymentFirstDeliveryV1(envelope: ByteArray, request: ByteArray, expectedIssuer: ByteArray, policyView: ByteArray, receivedAtMilliseconds: Long): ByteArray
        @JvmStatic private external fun nativeValidateEligibilityPaymentFirstDeliveryFinalizedV1(envelope: ByteArray, request: ByteArray, expectedIssuer: ByteArray, policyView: ByteArray, expectedNetworkId: ByteArray, trustedContextId: ByteArray, receivedAtMilliseconds: Long): ByteArray
        @JvmStatic private external fun nativeProjectInitResultV4(result: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeProjectSplitResultV4(result: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeBuildVerifyRequestV4(bundle: ByteArray, recipientRequest: ByteArray, topUpProvenance: ByteArray, maximumHops: Int, blockHeight: Long, verifiedAtMilliseconds: Long): ByteArray
        @JvmStatic private external fun nativeProjectVerifyResultV4(result: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeBuildRedeemRequestV4(bundle: ByteArray, topUpProvenance: ByteArray, opening: ByteArray, membershipWitness: ByteArray, recipient: ByteArray, chainDiscriminant: Int, atomicUnits: ByteArray, scale: Int, changeOpening: ByteArray, changeOutputMembership: ByteArray, verifierCommitment: ByteArray, operationId: ByteArray, blockHeight: Long): ByteArray
        @JvmStatic private external fun nativeProjectRedeemBuildResultV4(result: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativePrepareAcknowledgementV2(request: ByteArray, payment: ByteArray, acceptedAtMilliseconds: Long): Array<ByteArray>
        @JvmStatic private external fun nativeCreateAcknowledgementV2(payload: ByteArray, signature: ByteArray, request: ByteArray, payment: ByteArray): ByteArray
        @JvmStatic private external fun nativeVerifyAcknowledgementV2(acknowledgement: ByteArray, request: ByteArray, payment: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeProjectReadinessV4(readiness: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeProjectAuthenticatedArtifactSetV4(artifactSet: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeProjectActiveVerifierV2(verifier: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativePrepareAuthorizationV2(authority: ByteArray, chainDiscriminant: Int, deviceId: ByteArray, assetDefinitionId: ByteArray, operationId: ByteArray, issuedAtMilliseconds: Long, expiresAtMilliseconds: Long, nonce: ByteArray, payloadDigest: ByteArray, registrationHash: ByteArray, hardwareAssertionPlatform: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeFinalizeDrainOnlyRedemptionAuthorizationV1(authority: ByteArray, chainDiscriminant: Int, deviceId: ByteArray, assetDefinitionId: ByteArray, operationId: ByteArray, issuedAtMilliseconds: Long, expiresAtMilliseconds: Long, nonce: ByteArray, payloadDigest: ByteArray, policyView: ByteArray, expectedNetworkId: ByteArray, trustedContextId: ByteArray): ByteArray
        @JvmStatic private external fun nativeBuildDrainOnlyRedeemInstructionV4(request: ByteArray, authority: ByteArray, chainDiscriminant: Int): Array<ByteArray>
        @JvmStatic private external fun nativeFinalizeHardwareAuthorizationV2(preparation: ByteArray, authenticatorData: ByteArray, signatureDer: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeFinalizeIosAppAttestAuthorizationV2(preparation: ByteArray, assertionObject: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeFinalizeTopUpV4(unsigned: ByteArray, authorization: ByteArray): ByteArray
        @JvmStatic private external fun nativeFinalizeRedeemV4(buildResult: ByteArray, authorization: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativePrepareTopUpV4(networkId: ByteArray, chainDiscriminant: Int, assetDefinition: ByteArray, payer: ByteArray, atomicUnits: ByteArray, scale: Int, operationId: ByteArray, spendKey: ByteArray, rho: ByteArray, diversifier: ByteArray, leafIndex: Int, flattenedSiblings: ByteArray, directions: ByteArray, root: ByteArray, shieldVerifierCommitment: ByteArray, artifactBinding: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeProjectOperationReferenceV1(reference: ByteArray, expectedOperationId: ByteArray, expectedKind: ByteArray, expectedSubmittedAtMilliseconds: Long): Array<ByteArray>
        @JvmStatic private external fun nativeProjectOperationStatusV4(status: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeBranchClaimsConflictV2(left: ByteArray, right: ByteArray): Boolean
        @JvmStatic private external fun nativePrepareRedemptionChangeV4(bundle: ByteArray, inputOpening: ByteArray, atomicUnits: ByteArray, scale: Int, operationId: ByteArray, entropy: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativePreparePeerSplitChangeV4(bundles: Array<ByteArray>, inputOpenings: Array<ByteArray>, recipientRequest: ByteArray, atomicUnits: ByteArray, scale: Int, operationId: ByteArray, entropy: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativePrepareNoteOpeningV2(spendKey: ByteArray, rho: ByteArray, diversifier: ByteArray): ByteArray
        @JvmStatic private external fun nativeProjectRecipientRequestV2(request: ByteArray): Array<ByteArray>
    }

    /** Null-safe zeroization shared by secret-bearing native request builders. */
    internal object SecretArchiveWiper {
        fun wipe(archive: ByteArray?) {
            archive?.fill(0)
        }

        fun wipeAll(archives: Array<out ByteArray?>?) {
            archives?.forEach(::wipe)
        }

        fun <T> withOpeningDigests(
            spendKey: ByteArray,
            spendKeyName: String,
            rho: ByteArray,
            rhoName: String,
            diversifier: ByteArray,
            diversifierName: String,
            observer: (ByteArray) -> Unit = {},
            action: (ByteArray, ByteArray, ByteArray) -> T,
        ): T {
            var spendKeyCopy: ByteArray? = null
            var rhoCopy: ByteArray? = null
            var diversifierCopy: ByteArray? = null
            return try {
                val ownedSpendKey = requireDigest(spendKey, spendKeyName)
                    .also {
                        spendKeyCopy = it
                        observer(it)
                    }
                val ownedRho = requireDigest(rho, rhoName)
                    .also {
                        rhoCopy = it
                        observer(it)
                    }
                val ownedDiversifier = requireDigest(diversifier, diversifierName)
                    .also {
                        diversifierCopy = it
                        observer(it)
                    }
                action(ownedSpendKey, ownedRho, ownedDiversifier)
            } finally {
                wipe(diversifierCopy)
                wipe(rhoCopy)
                wipe(spendKeyCopy)
            }
        }
    }

    /** Immutable canonical Norito archive; proof and accumulator bytes remain opaque. */
    abstract class CanonicalArchive internal constructor(
        archive: ByteArray,
        schema: String,
        field: String,
        maximumBytes: Int,
    ) {
        private val bytes = requireCanonicalArchive(archive, schema, field, maximumBytes)
        // Retain only the 32-bit collection bucket after zeroization, never a secret digest.
        private val equalityHashCode = bytes.contentHashCode()
        private var destroyed = false

        @Synchronized
        fun noritoEncoded(): ByteArray {
            check(!destroyed) { "canonical archive has been destroyed" }
            return bytes.copyOf()
        }

        /** Borrow one synchronized native-call copy without changing ownership. */
        @Synchronized
        internal fun borrowForNative(): ByteArray {
            check(!destroyed) { "canonical archive has been destroyed" }
            return bytes.copyOf()
        }

        @Synchronized
        internal fun consumeAndDestroy(): ByteArray {
            check(!destroyed) { "canonical archive has already been consumed" }
            val consumed = bytes.copyOf()
            bytes.fill(0)
            destroyed = true
            return consumed
        }

        @Synchronized
        internal fun destroy() {
            bytes.fill(0)
            destroyed = true
        }

        @Synchronized
        fun isDestroyed(): Boolean = destroyed

        final override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other == null || this::class != other::class) return false
            other as CanonicalArchive
            val identity = System.identityHashCode(this)
            val otherIdentity = System.identityHashCode(other)
            return when {
                identity < otherIdentity -> synchronized(this) {
                    synchronized(other) { liveContentEquals(other) }
                }
                identity > otherIdentity -> synchronized(other) {
                    synchronized(this) { liveContentEquals(other) }
                }
                else -> synchronized(EQUALITY_TIE_LOCK) {
                    synchronized(this) {
                        synchronized(other) { liveContentEquals(other) }
                    }
                }
            }
        }

        final override fun hashCode(): Int = equalityHashCode

        private fun liveContentEquals(other: CanonicalArchive): Boolean =
            !destroyed && !other.destroyed && bytes.contentEquals(other.bytes)

        private companion object {
            val EQUALITY_TIE_LOCK = Any()
        }
    }

    /** Nominal bounded native input; the ABI-22 entrypoint performs exact typed decoding. */
    abstract class EligibilityNativeInputArchive internal constructor(
        archive: ByteArray,
        field: String,
        maximumBytes: Int,
    ) {
        private val bytes = requireBoundedBytes(archive, field, maximumBytes)

        fun noritoEncoded(): ByteArray = bytes.copyOf()

        final override fun equals(other: Any?): Boolean =
            other != null && this::class == other::class &&
                other is EligibilityNativeInputArchive && bytes.contentEquals(other.bytes)

        final override fun hashCode(): Int = bytes.contentHashCode()
    }

    class EligibilityCredentialV1 internal constructor(
        archive: ByteArray,
        internal val verificationTrustAnchor: OfflineDeviceFinalityTrustAnchorV1? = null,
    ) :
        EligibilityNativeInputArchive(
            archive,
            "eligibilityCredentialV1",
            MAX_ELIGIBILITY_CREDENTIAL_ARCHIVE_BYTES_V1,
        )

    class EligibilityIssuerPublicKeyV1 internal constructor(archive: ByteArray) :
        EligibilityNativeInputArchive(
            archive,
            "eligibilityIssuerPublicKeyV1",
            MAX_ELIGIBILITY_ISSUER_ARCHIVE_BYTES_V1,
        )

    class DeviceAttestationPolicyViewV1 internal constructor(
        archive: ByteArray,
        internal val verificationTrustAnchor: OfflineDeviceFinalityTrustAnchorV1? = null,
    ) :
        EligibilityNativeInputArchive(
            archive,
            "deviceAttestationPolicyViewV1",
            MAX_DEVICE_ATTESTATION_POLICY_VIEW_ARCHIVE_BYTES_V1,
        )

    class RecipientPaymentRequest internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "iroha_data_model::offline::model::KagemushaRecipientPaymentRequestV2",
        "recipientPaymentRequest",
        MAX_PEER_ARCHIVE_BYTES_V2,
    )

    class RecipientLineageQueryV2 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "iroha_torii_shared::offline_api::OfflineRecipientLineageRequest",
        "recipientLineageQuery",
        MAX_PEER_ARCHIVE_BYTES_V2,
    )

    /** Portable proof material; it becomes trusted only through a V2 native verifier result. */
    class RecipientRegistrationLineage internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "iroha_torii_shared::offline_api::OfflineRecipientRegistrationLineage",
        "recipientRegistrationLineage",
        MAX_TORII_RESPONSE_BYTES,
    )

    class RecipientReceiveOfferV2 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "iroha_torii_shared::offline_api::OfflineRecipientReceiveOfferV2",
        "recipientReceiveOffer",
        MAX_RECIPIENT_RECEIVE_OFFER_BYTES_V2,
    )

    class PeerPayment internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "iroha_data_model::offline::model::KagemushaRecursiveSpendPeerPaymentV4",
        "peerPayment",
        MAX_PEER_ARCHIVE_BYTES_V4,
    )

    class EligibilityPaymentPayloadV1 internal constructor(archive: ByteArray) :
        CanonicalArchive(
            archive,
            "iroha_data_model::offline::model::KagemushaEligibilityPaymentEnvelopePayloadV1",
            "eligibilityPaymentPayloadV1",
            MAX_ELIGIBILITY_PAYMENT_ENVELOPE_ARCHIVE_BYTES_V1,
        )

    class EligibilityPaymentEnvelopeV1 internal constructor(archive: ByteArray) :
        CanonicalArchive(
            archive,
            "iroha_data_model::offline::model::KagemushaEligibilityPaymentEnvelopeV1",
            "eligibilityPaymentEnvelopeV1",
            MAX_ELIGIBILITY_PAYMENT_ENVELOPE_ARCHIVE_BYTES_V1,
        )

    class ReceiverAcknowledgement internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "iroha_data_model::offline::model::KagemushaReceiverAcknowledgementV2",
        "receiverAcknowledgement",
        MAX_PEER_ARCHIVE_BYTES_V2,
    )

    /** Proof-bound output membership state carried atomically with an accepted branch. */
    class NoteMembershipWitness internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaNoteMembershipWitnessV2",
        "noteMembershipWitness",
        MAX_PEER_ARCHIVE_BYTES_V2,
    )

    /**
     * Encrypted local note opening; never send this archive to Torii or a peer.
     *
     * Use [close] (normally through `use`) as soon as ownership ends so the secret archive is
     * zeroized deterministically.
     */
    class NoteOpening internal constructor(archive: ByteArray) : CanonicalArchive(
            archive,
            "KagemushaNoteOpeningV2",
            "noteOpening",
            MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
        ), AutoCloseable {
        /** Zeroize this opening. Repeated closes are harmless. */
        override fun close() {
            destroy()
        }
    }

    private class ChangeOpeningOwner(changeOpening: NoteOpening?) : AutoCloseable {
        private var opening = changeOpening
        private var transferred = false
        private var closed = false

        @Synchronized
        fun take(): NoteOpening? {
            check(!closed) { "change-opening owner has been closed" }
            check(!transferred) { "change opening has already been transferred" }
            transferred = true
            val ownedOpening = opening
            opening = null
            return ownedOpening
        }

        @Synchronized
        override fun close() {
            if (closed) return
            opening?.destroy()
            opening = null
            closed = true
        }
    }

    /** Owns native-derived redemption change secrets until [takeOpening] moves the opening. */
    class RedemptionChangePreparationV4 internal constructor(
        opening: NoteOpening?,
        rho: ByteArray?,
        diversifier: ByteArray?,
        commitment: ByteArray?,
        spendNullifier: ByteArray?,
        amount: KagemushaScaledAmount?,
    ) : AutoCloseable {
        private var openingValue: NoteOpening? = null
        private val rhoValue: ByteArray
        private val diversifierValue: ByteArray
        private val commitmentValue: ByteArray
        private val spendNullifierValue: ByteArray
        private val amountValue: KagemushaScaledAmount
        private var closed = false

        init {
            val ownedOpening = requireNotNull(opening) { "opening must not be null" }
            var rhoCopy: ByteArray? = null
            var diversifierCopy: ByteArray? = null
            var commitmentCopy: ByteArray? = null
            var spendNullifierCopy: ByteArray? = null
            try {
                check(!ownedOpening.isDestroyed()) { "opening has already been destroyed" }
                val requiredAmount = requireNotNull(amount) { "amount must not be null" }
                val checkedRho = requireDigest(rho, "rho").also { rhoCopy = it }
                val checkedDiversifier = requireDigest(diversifier, "diversifier")
                    .also { diversifierCopy = it }
                val checkedCommitment = requireDigest(commitment, "commitment")
                    .also { commitmentCopy = it }
                val checkedSpendNullifier = requireDigest(spendNullifier, "spendNullifier")
                    .also { spendNullifierCopy = it }
                check(!checkedRho.contentEquals(checkedDiversifier)) {
                    "native Kagemusha redemption opening coordinates collide"
                }
                openingValue = ownedOpening
                rhoValue = checkedRho
                diversifierValue = checkedDiversifier
                commitmentValue = checkedCommitment
                spendNullifierValue = checkedSpendNullifier
                amountValue = requiredAmount
            } catch (failure: Throwable) {
                spendNullifierCopy?.fill(0)
                commitmentCopy?.fill(0)
                diversifierCopy?.fill(0)
                rhoCopy?.fill(0)
                ownedOpening.destroy()
                throw failure
            }
        }

        val amount: KagemushaScaledAmount
            @Synchronized get() {
                requireOpen()
                return amountValue
            }

        /** Move the opening to a request/result owner. This succeeds exactly once. */
        @Synchronized
        fun takeOpening(): NoteOpening {
            requireOpen()
            val ownedOpening = checkNotNull(openingValue) {
                "redemption change opening has already been transferred"
            }
            openingValue = null
            return ownedOpening
        }

        @Synchronized
        fun rho(): ByteArray = openCopy(rhoValue)

        @Synchronized
        fun diversifier(): ByteArray = openCopy(diversifierValue)

        @Synchronized
        fun commitment(): ByteArray = openCopy(commitmentValue)

        @Synchronized
        fun spendNullifier(): ByteArray = openCopy(spendNullifierValue)

        @Synchronized
        override fun close() {
            if (closed) return
            openingValue?.destroy()
            openingValue = null
            rhoValue.fill(0)
            diversifierValue.fill(0)
            commitmentValue.fill(0)
            spendNullifierValue.fill(0)
            closed = true
        }

        private fun openCopy(value: ByteArray): ByteArray {
            requireOpen()
            return value.copyOf()
        }

        private fun requireOpen() {
            check(!closed) { "redemption change preparation has been destroyed" }
        }
    }

    /** Owns native-derived ordinary peer-split change until [takeOpening] transfers it. */
    class PeerSplitChangePreparationV4 internal constructor(
        opening: NoteOpening,
        rho: ByteArray,
        diversifier: ByteArray,
        commitment: ByteArray,
        spendNullifier: ByteArray,
        val amount: KagemushaScaledAmount,
    ) : AutoCloseable {
        private var openingValue: NoteOpening? = null
        private val rhoValue: ByteArray
        private val diversifierValue: ByteArray
        private val commitmentValue: ByteArray
        private val spendNullifierValue: ByteArray
        private var closed = false

        init {
            var rhoCopy: ByteArray? = null
            var diversifierCopy: ByteArray? = null
            var commitmentCopy: ByteArray? = null
            var nullifierCopy: ByteArray? = null
            try {
                check(!opening.isDestroyed()) { "opening has already been destroyed" }
                val checkedRho = requireDigest(rho, "rho").also { rhoCopy = it }
                val checkedDiversifier = requireDigest(diversifier, "diversifier")
                    .also { diversifierCopy = it }
                val checkedCommitment = requireDigest(commitment, "commitment")
                    .also { commitmentCopy = it }
                val checkedNullifier = requireDigest(spendNullifier, "spendNullifier")
                    .also { nullifierCopy = it }
                check(!checkedRho.contentEquals(checkedDiversifier)) {
                    "native Kagemusha peer-split opening coordinates collide"
                }
                openingValue = opening
                rhoValue = checkedRho
                diversifierValue = checkedDiversifier
                commitmentValue = checkedCommitment
                spendNullifierValue = checkedNullifier
            } catch (failure: Throwable) {
                nullifierCopy?.fill(0)
                commitmentCopy?.fill(0)
                diversifierCopy?.fill(0)
                rhoCopy?.fill(0)
                opening.destroy()
                throw failure
            }
        }

        @Synchronized fun takeOpening(): NoteOpening {
            requireOpen()
            val owned = checkNotNull(openingValue) {
                "peer-split change opening has already been transferred"
            }
            openingValue = null
            return owned
        }

        @Synchronized fun rho(): ByteArray = copyOpen(rhoValue)
        @Synchronized fun diversifier(): ByteArray = copyOpen(diversifierValue)
        @Synchronized fun commitment(): ByteArray = copyOpen(commitmentValue)
        @Synchronized fun spendNullifier(): ByteArray = copyOpen(spendNullifierValue)

        @Synchronized override fun close() {
            if (closed) return
            openingValue?.destroy()
            openingValue = null
            rhoValue.fill(0)
            diversifierValue.fill(0)
            commitmentValue.fill(0)
            spendNullifierValue.fill(0)
            closed = true
        }

        private fun copyOpen(value: ByteArray): ByteArray {
            requireOpen()
            return value.copyOf()
        }

        private fun requireOpen() {
            check(!closed) { "peer-split change preparation has been destroyed" }
        }
    }

    class RecipientRequestPayload internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecipientPaymentRequestSigningPayloadV2",
        "recipientRequestPayload",
        MAX_PEER_ARCHIVE_BYTES_V2,
    )

    /** Opaque ABI-21 recursive state. */
    class BundleV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendBundleV4",
        "bundleV4",
        MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
    )

    /** Opaque current lineage claim; native comparison implements all overlap rules. */
    class BranchClaim internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendBranchClaimV2",
        "branchClaim",
        MAX_PEER_ARCHIVE_BYTES_V2,
    ) {
        fun conflictsWith(other: BranchClaim): Boolean {
            requireArtifactBridge()
            return nativeBranchClaimsConflictV2(noritoEncoded(), other.noritoEncoded())
        }
    }

    class ArtifactBindingV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendArtifactBindingV4",
        "artifactBinding",
        MAX_MANIFEST_BYTES,
    )

    class TopUpUnsigned internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendTopUpUnsignedV4",
        "topUpUnsigned",
        MAX_TORII_TOP_UP_REQUEST_BYTES_V4,
    )

    class TopUpRequest internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "iroha.torii.v1.offline.top_up.request",
        "topUpRequest",
        MAX_TORII_TOP_UP_REQUEST_BYTES_V4,
    )

    /** Finalized ABI-21 top-up receipt with a V4 artifact binding. */
    class TopUpAnchorV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendTopUpAnchorV4",
        "topUpAnchorV4",
        MAX_TORII_RESPONSE_BYTES,
    )

    class TopUpFinalityProof internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaTopUpFinalityProofV2",
        "topUpFinalityProof",
        MAX_TORII_RESPONSE_BYTES,
    )

    class TopUpFinalityRosterArtifact internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaTopUpFinalityRosterArtifactV2",
        "topUpFinalityRosterArtifact",
        MAX_TORII_RESPONSE_BYTES,
    )

    /** Complete V4 origin plus its stable compact-finality proof. */
    class TopUpFinalityEvidenceV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendTopUpFinalityEvidenceV4",
        "topUpFinalityEvidenceV4",
        MAX_TORII_RESPONSE_BYTES,
    )

    /** Exact block identity returned only after the native compact-finality verifier succeeds. */
    class VerifiedTopUpFinalityV4 internal constructor(
        operationId: ByteArray,
        private val exactTransactionHashHex: String,
        private val exactHeight: Long,
        private val exactBlockHashHex: String,
        heightContextId: ByteArray,
    ) {
        private val exactOperationId = requireDigest(operationId, "operationId")
        private val exactHeightContextId = requireFinalityCheckpointContext(
            heightContextId,
            "heightContextId",
        )

        init {
            require(isMarkedIrohaHashHex(exactTransactionHashHex)) {
                "transactionHashHex must be an exact lowercase marked 32-byte Iroha hash"
            }
            require(exactHeight > 0) { "height must be positive" }
            require(isMarkedIrohaHashHex(exactBlockHashHex)) {
                "blockHashHex must be an exact lowercase marked 32-byte Iroha hash"
            }
        }

        fun operationId(): ByteArray = exactOperationId.copyOf()
        fun transactionHashHex(): String = exactTransactionHashHex
        fun height(): Long = exactHeight
        fun blockHashHex(): String = exactBlockHashHex
        fun heightContextId(): ByteArray = exactHeightContextId.copyOf()

        private fun isMarkedIrohaHashHex(value: String): Boolean =
            value.matches(Regex("[0-9a-f]{64}")) &&
                (Character.digit(value.last(), 16) and 1) == 1
    }

    /** Complete bounded origin-finality inventory required to spend or verify one V4 bundle. */
    class TopUpProvenanceV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendTopUpProvenanceV4",
        "topUpProvenanceV4",
        MAX_TOP_UP_PROVENANCE_ARCHIVE_BYTES_V4,
    )

    class RedeemSubmissionRequest internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "iroha.torii.v1.offline.redeem.request",
        "redeemSubmissionRequest",
        MAX_TORII_REDEEM_REQUEST_BYTES_V4,
    )

    class DrainOnlyRedeemInstructionV4 internal constructor(
        val wireId: String,
        framedPayload: ByteArray,
        operationId: ByteArray,
        val issuedAtMilliseconds: Long,
        val expiresAtMilliseconds: Long,
    ) {
        private val framedPayloadValue = requiredBytes(framedPayload, "framedPayload")
        private val operationIdValue = requireDigest(operationId, "operationId")

        init {
            require(wireId == DRAIN_ONLY_REDEEM_WIRE_ID_V4) {
                "native drain-only redemption instruction returned another wire id"
            }
            require(
                issuedAtMilliseconds > 0 &&
                    expiresAtMilliseconds > issuedAtMilliseconds,
            ) { "native drain-only redemption instruction returned an invalid lifetime" }
        }

        fun framedPayload(): ByteArray = framedPayloadValue.copyOf()

        fun operationId(): ByteArray = operationIdValue.copyOf()
    }

    class RedeemUnsignedV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendRedeemUnsignedV4",
        "redeemUnsignedV4",
        MAX_TORII_REDEEM_REQUEST_BYTES_V4,
    )

    class RequestAuthorizationPreparationArchive internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRequestAuthorizationPreparationV2",
        "requestAuthorizationPreparation",
        MAX_REQUEST_AUTHORIZATION_BYTES,
    )

    class RequestAuthorization internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRequestAuthorizationV2",
        "requestAuthorization",
        MAX_REQUEST_AUTHORIZATION_BYTES,
    )

    class TopUpZeroPath(
        val leafIndex: Int,
        siblings: List<ByteArray>,
        directions: ByteArray,
        root: ByteArray,
    ) {
        private val siblingsValue: List<ByteArray>
        private val directionsValue: ByteArray
        private val rootValue: ByteArray

        init {
            require(leafIndex in 0 until (1 shl CONFIDENTIAL_TREE_DEPTH)) {
                "leafIndex is outside the confidential tree"
            }
            require(siblings.size == CONFIDENTIAL_TREE_DEPTH && siblings.all { it.size == 32 }) {
                "siblings must contain exactly $CONFIDENTIAL_TREE_DEPTH 32-byte nodes"
            }
            require(directions.size == CONFIDENTIAL_TREE_DEPTH && directions.all { it.toInt() in 0..1 }) {
                "directions must contain exactly $CONFIDENTIAL_TREE_DEPTH binary values"
            }
            val encodedLeaf = directions.withIndex().fold(0) { value, (level, direction) ->
                value or (direction.toInt() shl level)
            }
            require(encodedLeaf == leafIndex) { "directions do not encode leafIndex" }
            siblingsValue = siblings.map(ByteArray::copyOf)
            directionsValue = directions.copyOf()
            rootValue = requireDigest(root, "root")
        }

        fun siblings(): List<ByteArray> = siblingsValue.map(ByteArray::copyOf)

        fun directions(): ByteArray = directionsValue.copyOf()

        fun root(): ByteArray = rootValue.copyOf()

        internal fun flattenedSiblings(): ByteArray =
            ByteArray(CONFIDENTIAL_TREE_DEPTH * 32).also { flattened ->
                siblingsValue.forEachIndexed { index, sibling ->
                    sibling.copyInto(flattened, index * 32)
                }
            }

        companion object {
            /** Convert only Torii's authoritative next-zero path; ordinary inclusion paths fail. */
            @JvmStatic
            fun from(response: ZkMerklePathResponse): TopUpZeroPath {
                require(response.treeDepth == CONFIDENTIAL_TREE_DEPTH) {
                    "Torii confidential tree depth does not match Kagemusha"
                }
                val path = response.requireNextZeroPath()
                return TopUpZeroPath(
                    path.leafIndex,
                    path.siblingBytes(),
                    path.directions,
                    path.rootBytes(),
                )
            }
        }
    }

    /** Canonical next-zero cursor persisted atomically with every restored ABI-21 branch. */
    class OutputMembershipFrontierV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "connect_norito_bridge::KagemushaOutputMembershipFrontierV4",
        "outputMembershipFrontierV4",
        MAX_OUTPUT_MEMBERSHIP_FRONTIER_ARCHIVE_BYTES_V4,
    )

    /** One authenticated confidential-tree path used by the V4 output-update witness. */
    class OutputMembershipPath(
        val leafIndex: Int,
        siblings: List<ByteArray>,
        directions: ByteArray,
        root: ByteArray,
    ) {
        private val siblingsValue: List<ByteArray>
        private val directionsValue: ByteArray
        private val rootValue: ByteArray

        init {
            require(leafIndex in 0 until (1 shl CONFIDENTIAL_TREE_DEPTH)) {
                "leafIndex is outside the confidential tree"
            }
            require(siblings.size == CONFIDENTIAL_TREE_DEPTH && siblings.all { it.size == 32 }) {
                "siblings must contain exactly $CONFIDENTIAL_TREE_DEPTH 32-byte nodes"
            }
            require(directions.size == CONFIDENTIAL_TREE_DEPTH && directions.all { it.toInt() in 0..1 }) {
                "directions must contain exactly $CONFIDENTIAL_TREE_DEPTH binary values"
            }
            val encodedLeaf = directions.withIndex().fold(0) { value, (level, direction) ->
                value or (direction.toInt() shl level)
            }
            require(encodedLeaf == leafIndex) { "directions do not encode leafIndex" }
            siblingsValue = siblings.map(ByteArray::copyOf)
            directionsValue = directions.copyOf()
            rootValue = requireDigest(root, "root")
        }

        fun siblings(): List<ByteArray> = siblingsValue.map(ByteArray::copyOf)

        fun directions(): ByteArray = directionsValue.copyOf()

        fun root(): ByteArray = rootValue.copyOf()

        internal fun flattenedSiblings(): ByteArray =
            ByteArray(CONFIDENTIAL_TREE_DEPTH * 32).also { flattened ->
                siblingsValue.forEachIndexed { index, sibling ->
                    sibling.copyInto(flattened, index * 32)
                }
            }

        companion object {
            /** Convert one validated Torii path entry without weakening its root/index binding. */
            @JvmStatic
            fun from(entry: ZkMerklePathEntry): OutputMembershipPath = OutputMembershipPath(
                entry.leafIndex,
                entry.siblingBytes(),
                entry.directions,
                entry.rootBytes(),
            )
        }
    }

    /** Insertion path plus membership path for one output at the operation's final root. */
    class OutputMembershipLeafPaths(
        val updatePath: OutputMembershipPath,
        val membershipPath: OutputMembershipPath,
    ) {
        val leafIndex: Int = updatePath.leafIndex

        init {
            require(membershipPath.leafIndex == leafIndex) {
                "updatePath and membershipPath must address the same leaf"
            }
        }

        internal fun nativeFields(): Array<ByteArray> = arrayOf(
            leafIndex.toString().toByteArray(StandardCharsets.UTF_8),
            updatePath.flattenedSiblings(),
            updatePath.directions(),
            updatePath.root(),
            membershipPath.flattenedSiblings(),
            membershipPath.directions(),
            membershipPath.root(),
        )
    }

    /** Complete V4 output-update witness; commitments are derived and bound by native code. */
    class OutputMembershipPaths internal constructor(
        initialRoot: ByteArray,
        finalRoot: ByteArray,
        val recipient: OutputMembershipLeafPaths?,
        val change: OutputMembershipLeafPaths?,
        val dummyPath: OutputMembershipPath,
        canonicalArchive: ByteArray?,
    ) {
        private val initialRootValue = requireDigest(initialRoot, "initialRoot")
        private val finalRootValue = requireDigest(finalRoot, "finalRoot")
        private val canonicalArchiveValue = canonicalArchive?.let {
            requireCanonicalArchive(
                it,
                "connect_norito_bridge::KagemushaOutputMembershipPathsV4",
                "outputMembershipPathsV4",
                MAX_OUTPUT_MEMBERSHIP_PATHS_ARCHIVE_BYTES_V4,
            )
        }

        constructor(
            initialRoot: ByteArray,
            finalRoot: ByteArray,
            recipient: OutputMembershipLeafPaths?,
            change: OutputMembershipLeafPaths?,
            dummyPath: OutputMembershipPath,
        ) : this(initialRoot, finalRoot, recipient, change, dummyPath, null)

        init {
            require(!initialRootValue.contentEquals(finalRootValue)) {
                "initialRoot and finalRoot must differ"
            }
            require(recipient != null || change != null) {
                "at least one output membership leaf is required"
            }
            require(dummyPath.root().contentEquals(finalRootValue)) {
                "dummyPath must be rooted at finalRoot"
            }
            recipient?.let {
                require(it.membershipPath.root().contentEquals(finalRootValue)) {
                    "recipient membershipPath must be rooted at finalRoot"
                }
            }
            change?.let {
                require(it.membershipPath.root().contentEquals(finalRootValue)) {
                    "change membershipPath must be rooted at finalRoot"
                }
            }
            if (recipient != null) {
                require(recipient.updatePath.root().contentEquals(initialRootValue)) {
                    "recipient updatePath must be rooted at initialRoot"
                }
            } else {
                require(change!!.updatePath.root().contentEquals(initialRootValue)) {
                    "change updatePath must be rooted at initialRoot"
                }
            }
            if (recipient != null && change != null) {
                require(recipient.leafIndex + 1 == change.leafIndex) {
                    "change output must immediately follow the recipient output"
                }
            }
            val lastOutputLeafIndex = change?.leafIndex ?: recipient!!.leafIndex
            require(dummyPath.leafIndex == lastOutputLeafIndex + 1) {
                "dummyPath must immediately follow the final output"
            }
            val occupied = listOfNotNull(recipient?.leafIndex, change?.leafIndex) + dummyPath.leafIndex
            require(occupied.distinct().size == occupied.size) {
                "output and dummy paths must address distinct leaves"
            }
        }

        fun initialRoot(): ByteArray = initialRootValue.copyOf()

        fun finalRoot(): ByteArray = finalRootValue.copyOf()

        internal fun nativeArchive(): ByteArray {
            canonicalArchiveValue?.let { return it.copyOf() }
            requireArtifactBridge()
            val recipientFields = recipient?.nativeFields() ?: emptyArray()
            val changeFields = change?.nativeFields() ?: emptyArray()
            val dummyFields = arrayOf(
                dummyPath.leafIndex.toString().toByteArray(StandardCharsets.UTF_8),
                dummyPath.flattenedSiblings(),
                dummyPath.directions(),
                dummyPath.root(),
            )
            return try {
                nativeBuildOutputMembershipPathsV4(
                    initialRootValue,
                    finalRootValue,
                    recipientFields,
                    changeFields,
                    dummyFields,
                )
            } finally {
                recipientFields.forEach { it.fill(0) }
                changeFields.forEach { it.fill(0) }
                dummyFields.forEach { it.fill(0) }
            }
        }
    }

    /** Local secret-bearing initialization input. Close it if it is not submitted. */
    class InitRequestV4 internal constructor(archive: ByteArray) : CanonicalArchive(
            archive,
            "KagemushaRecursiveSpendInitLocalRequestV4",
            "initRequest",
            MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
        ), AutoCloseable {
        /** Zeroize an unconsumed initialization request. Repeated closes are harmless. */
        override fun close() {
            destroy()
        }
    }

    /** Local secret-bearing append input. Native code consumes and wipes its openings. */
    class AppendRequestV4 internal constructor(
        archive: ByteArray,
        changeOpening: NoteOpening? = null,
    ) : CanonicalArchive(
            archive,
            "KagemushaRecursiveSpendAppendLocalRequestV4",
            "appendRequest",
            MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
        ), AutoCloseable {
        private val changeOpeningOwner = ChangeOpeningOwner(changeOpening)

        @Synchronized
        internal fun takeChangeOpening(): NoteOpening? {
            check(!isDestroyed()) { "append request has been closed" }
            return changeOpeningOwner.take()
        }

        @Synchronized
        override fun close() {
            destroy()
            changeOpeningOwner.close()
        }
    }

    class VerifyRequestV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendVerifyLocalRequestV4",
        "verifyRequest",
        MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
    )

    /** Local secret-bearing redemption input. Native code consumes and wipes its openings. */
    class RedeemRequestV4 internal constructor(
        archive: ByteArray,
        changeOpening: NoteOpening? = null,
    ) : CanonicalArchive(
            archive,
            "KagemushaRecursiveSpendRedeemLocalRequestV4",
            "redeemRequest",
            MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
        ), AutoCloseable {
        private val changeOpeningOwner = ChangeOpeningOwner(changeOpening)

        @Synchronized
        internal fun takeChangeOpening(): NoteOpening? {
            check(!isDestroyed()) { "redeem request has been closed" }
            return changeOpeningOwner.take()
        }

        @Synchronized
        override fun close() {
            destroy()
            changeOpeningOwner.close()
        }
    }

    class InitResultV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendInitResultV4",
        "initResult",
        MAX_LOCAL_RESULT_ARCHIVE_BYTES,
    )

    class SplitResultV4 internal constructor(
        archive: ByteArray,
        changeOpening: NoteOpening? = null,
    ) : CanonicalArchive(
            archive,
        "KagemushaRecursiveSpendSplitResultV4",
            "splitResult",
            MAX_LOCAL_RESULT_ARCHIVE_BYTES,
        ), AutoCloseable {
        private val changeOpeningOwner = ChangeOpeningOwner(changeOpening)

        @Synchronized
        internal fun takeChangeOpening(): NoteOpening? {
            check(!isDestroyed()) { "split result has been closed" }
            return changeOpeningOwner.take()
        }

        @Synchronized
        override fun close() {
            destroy()
            changeOpeningOwner.close()
        }
    }

    class VerifyResultV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendVerifyResultV4",
        "verifyResult",
        MAX_LOCAL_RESULT_ARCHIVE_BYTES,
    )

    class RedeemBuildResultV4 internal constructor(
        archive: ByteArray,
        changeOpening: NoteOpening? = null,
    ) : CanonicalArchive(
            archive,
        "KagemushaRecursiveSpendRedeemBuildResultV4",
            "redeemBuildResult",
            MAX_LOCAL_RESULT_ARCHIVE_BYTES,
        ), AutoCloseable {
        private val changeOpeningOwner = ChangeOpeningOwner(changeOpening)

        @Synchronized
        internal fun takeChangeOpening(): NoteOpening? {
            check(!isDestroyed()) { "redeem result has been closed" }
            return changeOpeningOwner.take()
        }

        @Synchronized
        override fun close() {
            destroy()
            changeOpeningOwner.close()
        }
    }

    class RecipientRequestPreparation internal constructor(
        payload: RecipientRequestPayload,
        signingBytes: ByteArray,
        opening: NoteOpening,
        commitment: ByteArray,
        nullifier: ByteArray,
        amount: KagemushaScaledAmount,
    ) {
        internal val payload: RecipientRequestPayload
        private val signingBytesValue: ByteArray
        val opening: NoteOpening
        private val commitmentValue: ByteArray
        private val nullifierValue: ByteArray
        val amount: KagemushaScaledAmount

        init {
            var signingBytesCopy: ByteArray? = null
            var commitmentCopy: ByteArray? = null
            var nullifierCopy: ByteArray? = null
            try {
                val ownedSigningBytes = requiredBytes(signingBytes, "signingBytes")
                    .also { signingBytesCopy = it }
                val ownedCommitment = requireDigest(commitment, "commitment")
                    .also { commitmentCopy = it }
                val ownedNullifier = requireDigest(nullifier, "nullifier")
                    .also { nullifierCopy = it }
                this.payload = payload
                signingBytesValue = ownedSigningBytes
                this.opening = opening
                commitmentValue = ownedCommitment
                nullifierValue = ownedNullifier
                this.amount = amount
            } catch (failure: Throwable) {
                SecretArchiveWiper.wipe(nullifierCopy)
                SecretArchiveWiper.wipe(commitmentCopy)
                SecretArchiveWiper.wipe(signingBytesCopy)
                opening.close()
                throw failure
            }
        }

        fun signingBytes(): ByteArray = signingBytesValue.copyOf()
        fun commitment(): ByteArray = commitmentValue.copyOf()
        fun nullifier(): ByteArray = nullifierValue.copyOf()
    }

    class RequestAuthorizationPreparation internal constructor(
        internal val archive: RequestAuthorizationPreparationArchive,
        signingBytes: ByteArray,
        operationId: ByteArray,
        payloadDigest: ByteArray,
        registrationHash: ByteArray,
    ) {
        private val signingBytesValue = requiredBytes(signingBytes, "signingBytes")
        private val operationIdValue = requireDigest(operationId, "operationId")
        private val payloadDigestValue = requireDigest(payloadDigest, "payloadDigest")
        private val registrationHashValue = requireDigest(registrationHash, "registrationHash")

        fun signingBytes(): ByteArray = signingBytesValue.copyOf()

        fun operationId(): ByteArray = operationIdValue.copyOf()

        fun payloadDigest(): ByteArray = payloadDigestValue.copyOf()

        fun registrationHash(): ByteArray = registrationHashValue.copyOf()
    }

    class TopUpPreparation internal constructor(
        unsigned: TopUpUnsigned,
        authorizationDigest: ByteArray,
        opening: NoteOpening,
        noteCommitment: ByteArray,
        spendNullifier: ByteArray,
        initialRoot: ByteArray,
        finalizedRoot: ByteArray,
        operationId: ByteArray,
        amount: KagemushaScaledAmount,
        leafIndex: Int,
    ) {
        val unsigned: TopUpUnsigned
        private val authorizationDigestValue: ByteArray
        val opening: NoteOpening
        private val noteCommitmentValue: ByteArray
        private val spendNullifierValue: ByteArray
        private val initialRootValue: ByteArray
        private val finalizedRootValue: ByteArray
        private val operationIdValue: ByteArray
        val amount: KagemushaScaledAmount
        val leafIndex: Int

        init {
            var authorizationDigestCopy: ByteArray? = null
            var noteCommitmentCopy: ByteArray? = null
            var spendNullifierCopy: ByteArray? = null
            var initialRootCopy: ByteArray? = null
            var finalizedRootCopy: ByteArray? = null
            var operationIdCopy: ByteArray? = null
            try {
                val ownedAuthorizationDigest = requireDigest(
                    authorizationDigest,
                    "authorizationDigest",
                ).also { authorizationDigestCopy = it }
                val ownedNoteCommitment = requireDigest(noteCommitment, "noteCommitment")
                    .also { noteCommitmentCopy = it }
                val ownedSpendNullifier = requireDigest(spendNullifier, "spendNullifier")
                    .also { spendNullifierCopy = it }
                val ownedInitialRoot = requireDigest(initialRoot, "initialRoot")
                    .also { initialRootCopy = it }
                val ownedFinalizedRoot = requireDigest(finalizedRoot, "finalizedRoot")
                    .also { finalizedRootCopy = it }
                val ownedOperationId = requireDigest(operationId, "operationId")
                    .also { operationIdCopy = it }
                this.unsigned = unsigned
                authorizationDigestValue = ownedAuthorizationDigest
                this.opening = opening
                noteCommitmentValue = ownedNoteCommitment
                spendNullifierValue = ownedSpendNullifier
                initialRootValue = ownedInitialRoot
                finalizedRootValue = ownedFinalizedRoot
                operationIdValue = ownedOperationId
                this.amount = amount
                this.leafIndex = leafIndex
            } catch (failure: Throwable) {
                SecretArchiveWiper.wipe(operationIdCopy)
                SecretArchiveWiper.wipe(finalizedRootCopy)
                SecretArchiveWiper.wipe(initialRootCopy)
                SecretArchiveWiper.wipe(spendNullifierCopy)
                SecretArchiveWiper.wipe(noteCommitmentCopy)
                SecretArchiveWiper.wipe(authorizationDigestCopy)
                opening.close()
                throw failure
            }
        }

        fun authorizationDigest(): ByteArray = authorizationDigestValue.copyOf()
        fun noteCommitment(): ByteArray = noteCommitmentValue.copyOf()
        fun spendNullifier(): ByteArray = spendNullifierValue.copyOf()
        fun initialRoot(): ByteArray = initialRootValue.copyOf()
        fun finalizedRoot(): ByteArray = finalizedRootValue.copyOf()
        fun operationId(): ByteArray = operationIdValue.copyOf()
    }

    class RedeemFinalization internal constructor(
        val request: RedeemSubmissionRequest,
        operationId: ByteArray,
    ) {
        private val operationIdValue = requireDigest(operationId, "operationId")

        fun operationId(): ByteArray = operationIdValue.copyOf()
    }

    class VerifiedRecipientPaymentRequest internal constructor(
        val request: RecipientPaymentRequest,
        digest: ByteArray,
        val verifiedAtMilliseconds: Long,
        val projection: RecipientRequestProjection,
    ) {
        private val digestValue = requireDigest(digest, "requestDigest")
        init {
            check(digestValue.contentEquals(projection.digest())) {
                "verified request digest does not match its projection"
            }
        }
        fun digest(): ByteArray = digestValue.copyOf()
    }

    class FinalityCheckpointPromotionV2 internal constructor(bytes: ByteArray) {
        private val encodedValue: ByteArray
        val height: Long

        init {
            require(bytes.size == PROMOTED_FINALITY_CHECKPOINT_BYTES_V2) {
                "promoted checkpoint must contain exactly 40 bytes"
            }
            require(bytes[0].toInt() and 0x80 == 0) {
                "promoted checkpoint height exceeds the signed-64-bit client bound"
            }
            var parsedHeight = 0L
            repeat(8) { index ->
                parsedHeight = (parsedHeight shl 8) or (bytes[index].toLong() and 0xffL)
            }
            require(parsedHeight > 0) { "promoted checkpoint height must be positive" }
            val context = bytes.copyOfRange(8, bytes.size)
            try {
                requireFinalityCheckpointContext(context, "promotedCheckpointContextId")
                    .fill(0)
            } finally {
                context.fill(0)
            }
            encodedValue = bytes.copyOf()
            height = parsedHeight
        }

        fun encoded(): ByteArray = encodedValue.copyOf()

        fun contextId(): ByteArray = encodedValue.copyOfRange(8, encodedValue.size)
    }

    class VerifiedRecipientRegistrationLineageV2 internal constructor(
        val lineage: RecipientRegistrationLineage,
        val promotedCheckpoint: FinalityCheckpointPromotionV2,
    )

    class RecipientReceiveOfferProjectionV2 internal constructor(
        val request: RecipientPaymentRequest,
        val lineage: RecipientRegistrationLineage,
        publisherCheckpointEnvelope: ByteArray,
    ) {
        private val publisherEnvelopeValue = requireBoundedBytes(
            publisherCheckpointEnvelope,
            "publisherCheckpointEnvelope",
            MAX_PUBLISHER_CHECKPOINT_ENVELOPE_BYTES_V1,
        )

        fun publisherCheckpointEnvelope(): ByteArray = publisherEnvelopeValue.copyOf()
    }

    class VerifiedRecipientReceiveOfferV2 internal constructor(
        val request: RecipientPaymentRequest,
        val lineage: RecipientRegistrationLineage,
        publisherCheckpointEnvelope: ByteArray,
        val promotedCheckpoint: FinalityCheckpointPromotionV2,
        val verifiedAtMilliseconds: Long,
    ) {
        private val publisherEnvelopeValue = requireBoundedBytes(
            publisherCheckpointEnvelope,
            "publisherCheckpointEnvelope",
            MAX_PUBLISHER_CHECKPOINT_ENVELOPE_BYTES_V1,
        )

        init {
            require(verifiedAtMilliseconds > 0)
        }

        fun publisherCheckpointEnvelope(): ByteArray = publisherEnvelopeValue.copyOf()
    }

    class RecipientRequestProjection internal constructor(
        networkId: NetworkId,
        val assetDefinitionId: String,
        val amount: KagemushaScaledAmount,
        val recipientAccountId: String,
        val receiverDeviceId: String,
        requestId: ByteArray,
        val issuedAtMilliseconds: Long,
        val expiresAtMilliseconds: Long,
        outputCommitment: ByteArray,
        outputNullifier: ByteArray,
        receiverKeyReference: ByteArray,
        receiverPublicKey: ByteArray,
        digest: ByteArray,
    ) {
        private val networkIdValue = networkId
        private val requestIdValue = requireDigest(requestId, "requestId")
        private val outputCommitmentValue = requireDigest(outputCommitment, "outputCommitment")
        private val outputNullifierValue = requireDigest(outputNullifier, "outputNullifier")
        private val receiverKeyReferenceValue = requireDigest(receiverKeyReference, "receiverKeyReference")
        private val receiverPublicKeyValue = KagemushaDevicePublicKeyV2(receiverPublicKey)
        private val digestValue = requireDigest(digest, "requestDigest")

        fun networkId(): NetworkId = networkIdValue
        fun requestId(): ByteArray = requestIdValue.copyOf()
        fun outputCommitment(): ByteArray = outputCommitmentValue.copyOf()
        fun outputNullifier(): ByteArray = outputNullifierValue.copyOf()
        fun receiverKeyReference(): ByteArray = receiverKeyReferenceValue.copyOf()
        fun receiverPublicKey(): KagemushaDevicePublicKeyV2 = receiverPublicKeyValue
        fun digest(): ByteArray = digestValue.copyOf()
    }

    open class BranchProjection internal constructor(
        val bundle: BundleV4,
        val membershipWitness: NoteMembershipWitness,
        commitment: ByteArray,
        spendNullifier: ByteArray,
        val amount: KagemushaScaledAmount,
        val hopCount: Int,
        val proofStepCount: Int,
        bundleDigest: ByteArray,
        val artifactBinding: ArtifactBindingV4,
        branchClaims: List<BranchClaim>,
    ) {
        private val commitmentValue = requireDigest(commitment, "commitment")
        private val spendNullifierValue = requireDigest(spendNullifier, "spendNullifier")
        private val bundleDigestValue = requireDigest(bundleDigest, "bundleDigest")
        val branchClaims: List<BranchClaim> = Collections.unmodifiableList(ArrayList(branchClaims))

        init {
            check(hopCount in 0..MAXIMUM_PEER_HOPS) { "native Kagemusha hop count is invalid" }
            check(proofStepCount in 1..MAXIMUM_PROOF_STEPS) {
                "native Kagemusha proof-step count is invalid"
            }
            check(this.branchClaims.size in 1..MAXIMUM_BRANCH_CLAIMS) {
                "native Kagemusha exact-state claims are invalid"
            }
        }

        fun commitment(): ByteArray = commitmentValue.copyOf()
        fun spendNullifier(): ByteArray = spendNullifierValue.copyOf()
        fun bundleDigest(): ByteArray = bundleDigestValue.copyOf()
        fun branchClaims(): List<BranchClaim> = branchClaims.toList()

        fun conflictsWith(other: BranchProjection): Boolean = branchClaims.any { left ->
            other.branchClaims.any { right -> left.conflictsWith(right) }
        }
    }

    /** Secret-bearing local state used only by the genuine ABI-21 builders. */
    class SpendableBranchV4 internal constructor(
        val bundle: BundleV4,
        val membershipWitness: NoteMembershipWitness,
        val opening: NoteOpening,
        val topUpProvenance: TopUpProvenanceV4,
        val frontier: OutputMembershipFrontierV4,
    ) : AutoCloseable {
        /** Destroy the locally held secret opening; public proof artifacts remain immutable. */
        override fun close() {
            opening.destroy()
        }
    }

    class PeerPaymentProjection internal constructor(
        val branch: BranchProjection,
        val topUpProvenance: TopUpProvenanceV4,
        operationId: ByteArray,
        requestDigest: ByteArray,
    ) {
        private val operationIdValue = requireDigest(operationId, "operationId")
        private val requestDigestValue = requireDigest(requestDigest, "requestDigest")

        fun operationId(): ByteArray = operationIdValue.copyOf()
        fun requestDigest(): ByteArray = requestDigestValue.copyOf()
    }

    class InitProjectionV4 internal constructor(
        val branch: BranchProjection,
        val topUpProvenance: TopUpProvenanceV4,
        publicStatementDigest: ByteArray,
    ) {
        private val publicStatementDigestValue =
            requireDigest(publicStatementDigest, "publicStatementDigest")

        val bundle: BundleV4 get() = branch.bundle
        fun publicStatementDigest(): ByteArray = publicStatementDigestValue.copyOf()
    }

    class SplitProjection internal constructor(
        val peerPayment: PeerPayment,
        val recipient: BranchProjection,
        val change: BranchProjection?,
        val recipientTopUpProvenance: TopUpProvenanceV4,
        val changeTopUpProvenance: TopUpProvenanceV4?,
        operationId: ByteArray,
        requestDigest: ByteArray,
        splitBindingDigest: ByteArray,
    ) {
        private val operationIdValue = requireDigest(operationId, "operationId")
        private val requestDigestValue = requireDigest(requestDigest, "requestDigest")
        private val splitBindingDigestValue = requireDigest(splitBindingDigest, "splitBindingDigest")

        fun operationId(): ByteArray = operationIdValue.copyOf()
        fun requestDigest(): ByteArray = requestDigestValue.copyOf()
        fun splitBindingDigest(): ByteArray = splitBindingDigestValue.copyOf()
    }

    class VerifyProjection internal constructor(
        val valid: Boolean,
        val chainAdmissible: Boolean,
        val lineageRedeemable: Boolean,
        val witnesslessRedemptionSupported: Boolean,
        commitment: ByteArray,
        spendNullifier: ByteArray,
        val amount: KagemushaScaledAmount,
        val hopCount: Int,
        val proofStepCount: Int,
        bundleDigest: ByteArray,
        val assetDefinitionId: String,
        val artifactBinding: ArtifactBindingV4,
        requestDigest: ByteArray,
        outputBindingDigest: ByteArray,
        val verifierBackend: String,
        val verifierName: String,
        val verifierCircuitId: String,
        val verifierActivationHeight: Long?,
        val verifierWithdrawalHeight: Long?,
        val verifiedAtBlockHeight: Long,
        val verifiedAtMilliseconds: Long,
        branchClaims: List<BranchClaim>,
    ) {
        private val commitmentValue = requireDigest(commitment, "commitment")
        private val spendNullifierValue = requireDigest(spendNullifier, "spendNullifier")
        private val bundleDigestValue = requireDigest(bundleDigest, "bundleDigest")
        private val requestDigestValue = requireDigest(requestDigest, "requestDigest")
        private val outputBindingDigestValue = requireDigest(outputBindingDigest, "outputBindingDigest")
        val branchClaims: List<BranchClaim> = Collections.unmodifiableList(ArrayList(branchClaims))

        init {
            check(hopCount in 0..MAXIMUM_PEER_HOPS && proofStepCount in 1..MAXIMUM_PROOF_STEPS) {
                "native Kagemusha verified state counters are invalid"
            }
            check(verifiedAtBlockHeight > 0 && verifiedAtMilliseconds > 0) {
                "native Kagemusha verification snapshot is invalid"
            }
            check(this.branchClaims.size in 1..MAXIMUM_BRANCH_CLAIMS) {
                "native Kagemusha verified branch claim vector is invalid"
            }
        }

        fun commitment(): ByteArray = commitmentValue.copyOf()
        fun spendNullifier(): ByteArray = spendNullifierValue.copyOf()
        fun bundleDigest(): ByteArray = bundleDigestValue.copyOf()
        fun requestDigest(): ByteArray = requestDigestValue.copyOf()
        fun outputBindingDigest(): ByteArray = outputBindingDigestValue.copyOf()
        fun branchClaims(): List<BranchClaim> = branchClaims.toList()
    }

    class RedeemBuildProjection internal constructor(
        val unsigned: RedeemUnsignedV4,
        authorizationDigest: ByteArray,
        val change: BranchProjection?,
        val changeTopUpProvenance: TopUpProvenanceV4?,
        operationId: ByteArray,
    ) {
        private val authorizationDigestValue = requireDigest(authorizationDigest, "authorizationDigest")
        private val operationIdValue = requireDigest(operationId, "operationId")

        init {
            check((change == null) == (changeTopUpProvenance == null)) {
                "native Kagemusha redemption change provenance does not match change projection"
            }
        }

        fun authorizationDigest(): ByteArray = authorizationDigestValue.copyOf()
        fun operationId(): ByteArray = operationIdValue.copyOf()
    }

    class AcknowledgementPayload internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaReceiverAcknowledgementPayloadV2",
        "acknowledgementPayload",
        MAX_PEER_ARCHIVE_BYTES,
    )

    class AcknowledgementPreparation internal constructor(
        internal val payload: AcknowledgementPayload,
        signingBytes: ByteArray,
        operationId: ByteArray,
        requestDigest: ByteArray,
        bundleDigest: ByteArray,
        commitment: ByteArray,
    ) {
        private val signingBytesValue = requiredBytes(signingBytes, "signingBytes")
        private val operationIdValue = requireDigest(operationId, "operationId")
        private val requestDigestValue = requireDigest(requestDigest, "requestDigest")
        private val bundleDigestValue = requireDigest(bundleDigest, "bundleDigest")
        private val commitmentValue = requireDigest(commitment, "commitment")

        fun signingBytes(): ByteArray = signingBytesValue.copyOf()
        fun operationId(): ByteArray = operationIdValue.copyOf()
        fun requestDigest(): ByteArray = requestDigestValue.copyOf()
        fun bundleDigest(): ByteArray = bundleDigestValue.copyOf()
        fun commitment(): ByteArray = commitmentValue.copyOf()
    }

    /** Delivery-receipt evidence for an already-final sender cash handoff. */
    class AcknowledgementVerification internal constructor(
        val valid: Boolean,
        operationId: ByteArray,
        requestDigest: ByteArray,
        bundleDigest: ByteArray,
        acknowledgementDigest: ByteArray,
    ) {
        private val operationIdValue = requireDigest(operationId, "operationId")
        private val requestDigestValue = requireDigest(requestDigest, "requestDigest")
        private val bundleDigestValue = requireDigest(bundleDigest, "bundleDigest")
        private val acknowledgementDigestValue = requireDigest(acknowledgementDigest, "acknowledgementDigest")

        fun operationId(): ByteArray = operationIdValue.copyOf()
        fun requestDigest(): ByteArray = requestDigestValue.copyOf()
        fun bundleDigest(): ByteArray = bundleDigestValue.copyOf()
        fun acknowledgementDigest(): ByteArray = acknowledgementDigestValue.copyOf()
    }

    /** Asset-neutral offline protocol capability implemented by every app-api node. */
    class OfflineStatus internal constructor(
        val mandatory: Boolean,
        val cashHandoffCapability: String,
        val eligibilityCashHandoffCapability: String,
        val requiredBridgeAbiVersion: Int,
        val maximumHops: Int,
        val ready: Boolean,
        val assets: List<Any?>,
        val blockers: List<ReadinessBlocker>,
    ) {
        init {
            require(!mandatory) { "mandatory must be false for universal offline capability" }
            require(cashHandoffCapability == CASH_HANDOFF_CAPABILITY_V1) {
                "cashHandoffCapability must be the exact cash_handoff_v1 contract"
            }
            require(
                eligibilityCashHandoffCapability == CASH_HANDOFF_ELIGIBILITY_CAPABILITY_V1,
            ) {
                "eligibilityCashHandoffCapability must be the exact cash_handoff_eligibility_v1 contract"
            }
            require(requiredBridgeAbiVersion == REQUIRED_NATIVE_BRIDGE_ABI_VERSION) {
                "requiredBridgeAbiVersion must be 22"
            }
            require(maximumHops == MAXIMUM_PEER_HOPS) {
                "maximumHops must match the cash_handoff_v1 bound"
            }
            require(ready) { "ready must be true for universal offline capability" }
            require(assets.isEmpty()) { "assets must be empty because capability is asset-neutral" }
            require(blockers.isEmpty()) { "blockers must be empty for universal offline capability" }
        }

        internal companion object {
            private val fields = setOf(
                "mandatory",
                "cash_handoff_capability",
                "eligibility_cash_handoff_capability",
                "required_bridge_abi_version",
                "max_hops",
                "ready",
                "assets",
                "blockers",
            )

            fun decode(payload: ByteArray): OfflineStatus {
                val parsed = JsonParser.parse(String(payload, StandardCharsets.UTF_8))
                check(parsed is Map<*, *>) { "offline capability response must be a JSON object" }
                check(parsed.keys == fields) {
                    "offline capability response must contain exactly the universal fields"
                }
                val mandatory = parsed["mandatory"] as? Boolean
                    ?: error("offline capability mandatory must be a boolean")
                val capability = parsed["cash_handoff_capability"] as? String
                    ?: error("offline capability cash_handoff_capability must be a string")
                val eligibilityCapability = parsed["eligibility_cash_handoff_capability"] as? String
                    ?: error(
                        "offline capability eligibility_cash_handoff_capability must be a string",
                    )
                val abi = exactInt(parsed["required_bridge_abi_version"], "required_bridge_abi_version")
                val hops = exactInt(parsed["max_hops"], "max_hops")
                val ready = parsed["ready"] as? Boolean
                    ?: error("offline capability ready must be a boolean")
                val assets = parsed["assets"] as? List<*>
                    ?: error("offline capability assets must be an array")
                val blockers = parsed["blockers"] as? List<*>
                    ?: error("offline capability blockers must be an array")
                check(assets.isEmpty()) { "offline capability assets must be empty" }
                check(blockers.isEmpty()) { "offline capability blockers must be empty" }
                return OfflineStatus(
                    mandatory = mandatory,
                    cashHandoffCapability = capability,
                    eligibilityCashHandoffCapability = eligibilityCapability,
                    requiredBridgeAbiVersion = abi,
                    maximumHops = hops,
                    ready = ready,
                    assets = emptyList(),
                    blockers = emptyList(),
                )
            }

            private fun exactInt(value: Any?, field: String): Int {
                check(value is Long && value in Int.MIN_VALUE.toLong()..Int.MAX_VALUE.toLong()) {
                    "offline capability $field must be a signed 32-bit JSON integer"
                }
                return value.toInt()
            }
        }
    }

    /** Legacy command-specific artifact diagnostics; not a discovery response. */
    class Readiness internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "OfflineReadiness",
        "readiness",
        MAX_TORII_RESPONSE_BYTES,
    )

    class ActiveVerifier internal constructor(
        val backend: String,
        val name: String,
        val version: Int,
        val circuitId: String,
        commitment: ByteArray,
        publicInputsSchemaHash: ByteArray,
        val maximumProofBytes: Int,
        val activationHeight: Long,
        val withdrawalHeight: Long?,
    ) {
        private val commitmentValue = requireDigest(commitment, "verifierCommitment")
        private val publicInputsSchemaHashValue =
            requireDigest(publicInputsSchemaHash, "publicInputsSchemaHash")

        fun commitment(): ByteArray = commitmentValue.copyOf()

        fun publicInputsSchemaHash(): ByteArray = publicInputsSchemaHashValue.copyOf()

        fun isActiveAt(blockHeight: Long): Boolean =
            blockHeight >= activationHeight &&
                (withdrawalHeight == null || blockHeight < withdrawalHeight)
    }

    /** Authenticated ABI-21 V4 release identity selected at the readiness snapshot. */
    class AuthenticatedArtifactSet internal constructor(
        val generation: String,
        manifestSha256: ByteArray,
        releasePolicySha256: ByteArray,
        releaseAttestationSha256: ByteArray,
        val activationHeight: Long,
        val withdrawalHeight: Long,
        val maximumProofBytes: Int,
        val assetScale: Int,
    ) {
        private val manifestSha256Value = requireDigest(manifestSha256, "artifactManifestSha256")
        private val releasePolicySha256Value =
            requireDigest(releasePolicySha256, "artifactReleasePolicySha256")
        private val releaseAttestationSha256Value =
            requireDigest(releaseAttestationSha256, "artifactReleaseAttestationSha256")

        init {
            val asciiAlphanumeric: (Char) -> Boolean = { character ->
                character in 'a'..'z' || character in 'A'..'Z' || character in '0'..'9'
            }
            require(
                generation.length in 1..128 &&
                    asciiAlphanumeric(generation.first()) &&
                    asciiAlphanumeric(generation.last()) &&
                    generation.all { character ->
                        asciiAlphanumeric(character) || character == '.' ||
                            character == '_' || character == '-'
                    },
            ) { "artifactGeneration must be a portable V4 identifier" }
            val basename = generation.substringBefore('.').lowercase()
            require(
                basename !in setOf("con", "prn", "aux", "nul") &&
                    !(basename.length == 4 &&
                        basename.substring(0, 3) in setOf("com", "lpt") &&
                        basename[3] in '1'..'9'),
            ) { "artifactGeneration must not use a Windows reserved basename" }
            require(
                !manifestSha256Value.contentEquals(releasePolicySha256Value) &&
                    !manifestSha256Value.contentEquals(releaseAttestationSha256Value) &&
                    !releasePolicySha256Value.contentEquals(releaseAttestationSha256Value),
            ) { "authenticated artifact digests must be pairwise distinct" }
            require(activationHeight > 0 && withdrawalHeight > activationHeight) {
                "authenticated artifact activation window is invalid"
            }
            require(maximumProofBytes in 1..MAXIMUM_RECURSIVE_PROOF_PAIR_BYTES_V4) {
                "artifactMaximumProofBytes exceeds the ABI-21 V4 release limit"
            }
            require(assetScale in 0..KagemushaScaledAmount.MAXIMUM_SCALE) {
                "artifactAssetScale exceeds the offline payment limit"
            }
        }

        fun manifestSha256(): ByteArray = manifestSha256Value.copyOf()

        fun releasePolicySha256(): ByteArray = releasePolicySha256Value.copyOf()

        fun releaseAttestationSha256(): ByteArray = releaseAttestationSha256Value.copyOf()

        fun isActiveAt(blockHeight: Long): Boolean =
            blockHeight >= activationHeight && blockHeight < withdrawalHeight
    }

    class ReadinessBlocker(val code: String, val message: String)

    class ReadinessProjection internal constructor(
        val cashHandoffCapability: String,
        val eligibilityCashHandoffCapability: String,
        val requiredBridgeAbiVersion: Int,
        val maximumHops: Int,
        val assetDefinitionId: String,
        val assetScale: Int?,
        val evaluatedBlockHeight: Long,
        evaluatedBlockHash: ByteArray,
        val proofBackendAvailable: Boolean,
        val recursiveLineageSupported: Boolean,
        val ready: Boolean,
        val transferVerifier: ActiveVerifier?,
        val topUpShieldVerifier: ActiveVerifier?,
        val unshieldVerifier: ActiveVerifier?,
        val recursiveStepEqVerifier: ActiveVerifier?,
        val recursiveStepEpVerifier: ActiveVerifier?,
        val artifactSet: AuthenticatedArtifactSet?,
        val blockers: List<ReadinessBlocker>,
    ) {
        private val evaluatedBlockHashValue = requireDigest(evaluatedBlockHash, "evaluatedBlockHash")

        init {
            require(cashHandoffCapability == CASH_HANDOFF_CAPABILITY_V1) {
                "cashHandoffCapability must be the exact cash_handoff_v1 contract"
            }
            require(
                eligibilityCashHandoffCapability == CASH_HANDOFF_ELIGIBILITY_CAPABILITY_V1,
            ) {
                "eligibilityCashHandoffCapability must be the exact cash_handoff_eligibility_v1 contract"
            }
        }

        val bridgeCompatible: Boolean
            get() = requiredBridgeAbiVersion == REQUIRED_NATIVE_BRIDGE_ABI_VERSION

        /** Every role-specific verifier is present and active at the same committed snapshot. */
        val allVerifiersActive: Boolean
            get() = listOf(
                transferVerifier,
                topUpShieldVerifier,
                unshieldVerifier,
                recursiveStepEqVerifier,
                recursiveStepEpVerifier,
            ).all { verifier -> verifier?.isActiveAt(evaluatedBlockHeight) == true }

        /** Chain-side recursive artifact/verifier set readiness at the evaluated snapshot. */
        val chainArtifactSetReady: Boolean
            get() = proofBackendAvailable && artifactSet?.isActiveAt(evaluatedBlockHeight) == true &&
                recursiveStepEqVerifier?.isActiveAt(evaluatedBlockHeight) == true &&
                recursiveStepEpVerifier?.isActiveAt(evaluatedBlockHeight) == true

        /** Whether native holds the exact manifest authenticated by this Torii snapshot. */
        val localArtifactSetMatches: Boolean
            get() {
                val expected = artifactSet?.manifestSha256() ?: return false
                val installed = installedArtifactManifestSha256V4() ?: return false
                return installed.contentEquals(expected)
            }

        /** Complete fail-closed wallet decision for the exact Torii-authenticated release. */
        val offlineReady: Boolean
            get() = ready && cashHandoffCapability == CASH_HANDOFF_CAPABILITY_V1 &&
                eligibilityCashHandoffCapability == CASH_HANDOFF_ELIGIBILITY_CAPABILITY_V1 &&
                recursiveLineageSupported && bridgeCompatible &&
                chainArtifactSetReady && allVerifiersActive && assetScale != null &&
                assetScale in 0..KagemushaScaledAmount.MAXIMUM_SCALE &&
                evaluatedBlockHeight > 0 && maximumHops == MAXIMUM_PEER_HOPS &&
                isProofBackendAvailable() && localArtifactSetMatches && blockers.isEmpty()

        fun evaluatedBlockHash(): ByteArray = evaluatedBlockHashValue.copyOf()
    }

    class OperationReference internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "OfflineOperationReference",
        "operationReference",
        MAX_TORII_RESPONSE_BYTES,
    )

    class OperationReferenceProjection internal constructor(
        val state: OperationState,
        val kind: OperationKind,
        operationId: ByteArray,
        transactionHash: ByteArray,
        val statusUri: String,
        val submittedAtMilliseconds: Long,
    ) {
        private val operationIdValue = requireDigest(operationId, "operationId")
        private val transactionHashValue = requireDigest(transactionHash, "transactionHash")

        init {
            require(state == OperationState.PENDING)
            require(statusUri == "/v1/offline/operations/${hex(operationIdValue)}") {
                "statusUri is not canonical for operationId"
            }
            require(submittedAtMilliseconds > 0)
        }

        fun operationId(): ByteArray = operationIdValue.copyOf()

        fun transactionHash(): ByteArray = transactionHashValue.copyOf()
    }

    class OperationStatus internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "OfflineOperationStatus",
        "operationStatus",
        MAX_TORII_RESPONSE_BYTES,
    )

    enum class OperationState { PENDING, APPLIED, REJECTED }

    enum class OperationKind { TOP_UP, REDEEM }

    class OperationRejection(val code: String, val message: String)

    class FinalizedTopUp internal constructor(
        val anchor: TopUpAnchorV4,
        val finalityProof: TopUpFinalityProof,
        val finalizedBlockHeight: Long,
        val serverTimeMilliseconds: Long,
    )

    class OperationStatusProjection internal constructor(
        val state: OperationState,
        val kind: OperationKind,
        operationId: ByteArray,
        transactionHash: ByteArray,
        val submittedAtMilliseconds: Long?,
        val finalizedBlockHeight: Long?,
        val serverTimeMilliseconds: Long?,
        val finalizedTopUp: FinalizedTopUp?,
        val rejection: OperationRejection?,
    ) {
        private val operationIdValue = requireDigest(operationId, "operationId")
        private val transactionHashValue = requireDigest(transactionHash, "transactionHash")

        fun operationId(): ByteArray = operationIdValue.copyOf()

        fun transactionHash(): ByteArray = transactionHashValue.copyOf()
    }

    /** Strict typed client for the governed Kagemusha Torii routes. */
    class ToriiClient internal constructor(
        baseUri: URI,
        private val transport: TransportExecutor,
        private val localSigningContext: LocalSigningContext,
    ) {
        companion object {
            const val READINESS_PATH: String = "/v1/offline/readiness"
            const val TOP_UP_PATH: String = "/v1/offline/top-up"
            const val REDEEM_PATH: String = "/v1/offline/redeem"
            const val OPERATIONS_PATH: String = "/v1/offline/operations"
            const val RECEIVER_LINEAGE_PATH: String = "/v1/offline/receiver-lineage"
            const val DEVICE_ATTESTATION_POLICY_PATH: String =
                "/v1/offline/device-attestation-policy"
            const val DEVICE_ATTESTATION_POLICY_PROOF_PATH: String =
                "/v1/offline/device-attestation-policy/proof"
            const val DEVICE_ELIGIBILITY_PATH: String = "/v1/offline/device-eligibility"
            const val JSON_MEDIA_TYPE: String = "application/json"
            const val NORITO_MEDIA_TYPE: String = "application/x-norito"

            private fun requireOperationId(value: String?): String {
                require(
                    value != null &&
                        value.length == 64 &&
                        value != "0".repeat(64) &&
                        value.all { it in '0'..'9' || it in 'a'..'f' },
                ) { "operationId must be non-zero lowercase 32-byte hex" }
                return value
            }

            private fun stripTrailingSlash(value: String): String = value.trimEnd('/')
        }

        private val baseUri: String

        init {
            require(
                baseUri.isAbsolute &&
                    !baseUri.isOpaque &&
                    !baseUri.host.isNullOrEmpty() &&
                    baseUri.rawQuery == null &&
                    baseUri.rawFragment == null &&
                    baseUri.rawUserInfo == null,
            ) { "baseUri must be an absolute credential-free HTTP URI" }
            require(baseUri.scheme.equals("http", true) || baseUri.scheme.equals("https", true)) {
                "baseUri must use HTTP or HTTPS"
            }
            this.baseUri = stripTrailingSlash(baseUri.toString())
        }

        fun getOfflineCapability(): CompletableFuture<OfflineStatus> {
            return execute(
                TransportRequest.builder()
                    .setMethod("GET")
                    .setUri(URI.create("$baseUri$READINESS_PATH"))
                    .addHeader("Accept", JSON_MEDIA_TYPE)
                    .setMaximumResponseBytes(MAX_TORII_RESPONSE_BYTES.toLong())
                    .build(),
                200,
                JSON_MEDIA_TYPE,
            ).thenApply { OfflineStatus.decode(it.body) }
        }

        fun getDeviceAttestationPolicyViewV1(
            canonicalAuth: ToriiCanonicalRequestAuth,
            trustAnchor: OfflineDeviceFinalityTrustAnchorV1,
            evaluationTimeMilliseconds: Long = System.currentTimeMillis(),
        ): CompletableFuture<DeviceAttestationPolicyViewV1> {
            require(localSigningContext.networkId() == trustAnchor.networkId) {
                "offline device policy trust must match LocalSigningContext.networkId"
            }
            require(evaluationTimeMilliseconds > 0) {
                "evaluationTimeMilliseconds must be positive"
            }
            val target = URI.create("$baseUri$DEVICE_ATTESTATION_POLICY_PATH")
            val timestampMs = canonicalAuth.timestampMs
            val nonce = canonicalAuth.nonce
            require((timestampMs == null) == (nonce == null)) {
                "timestampMs and nonce must be provided together"
            }
            val authHeaders = if (timestampMs == null) {
                CanonicalRequestSigner.buildHeaders(
                    localSigningContext.networkId(),
                    "GET",
                    target,
                    null,
                    canonicalAuth.accountId,
                    canonicalAuth.privateKey,
                )
            } else {
                CanonicalRequestSigner.buildHeaders(
                    localSigningContext.networkId(),
                    "GET",
                    target,
                    null,
                    canonicalAuth.accountId,
                    canonicalAuth.privateKey,
                    timestampMs,
                    nonce!!,
                )
            }
            val builder = TransportRequest.builder()
                .setMethod("GET")
                .setUri(target)
                .addHeader("Accept", NORITO_MEDIA_TYPE)
                .setMaximumResponseBytes(MAX_DEVICE_ATTESTATION_POLICY_VIEW_ARCHIVE_BYTES_V1.toLong())
            authHeaders.forEach { (name, value) -> builder.addHeader(name, value) }
            return execute(builder.build(), 200).thenApply { response ->
                verifyDeviceAttestationPolicyViewV1(
                    response.body,
                    trustAnchor,
                    evaluationTimeMilliseconds,
                )
            }
        }

        /**
         * Fetch and verify one policy proof page. Persist the evaluated checkpoint before another
         * call and provide fresh canonical authentication for every page.
         */
        fun getDeviceAttestationPolicyProofPageV1(
            canonicalAuth: ToriiCanonicalRequestAuth,
            checkpoint: OfflineDevicePolicyCheckpointV1,
            evaluationTimeMilliseconds: Long = System.currentTimeMillis(),
        ): CompletableFuture<OfflineDevicePolicyVerifiedPageV1> {
            require(localSigningContext.networkId() == checkpoint.networkId) {
                "offline device policy checkpoint must match LocalSigningContext.networkId"
            }
            require(evaluationTimeMilliseconds > 0) {
                "evaluationTimeMilliseconds must be positive"
            }
            val target = URI.create("$baseUri$DEVICE_ATTESTATION_POLICY_PROOF_PATH")
            val body = makeOfflineDevicePolicyProofRequestV1(checkpoint)
            val timestampMs = canonicalAuth.timestampMs
            val nonce = canonicalAuth.nonce
            require((timestampMs == null) == (nonce == null)) {
                "timestampMs and nonce must be provided together"
            }
            val authHeaders = if (timestampMs == null) {
                CanonicalRequestSigner.buildHeaders(
                    localSigningContext.networkId(),
                    "POST",
                    target,
                    body,
                    canonicalAuth.accountId,
                    canonicalAuth.privateKey,
                )
            } else {
                CanonicalRequestSigner.buildHeaders(
                    localSigningContext.networkId(),
                    "POST",
                    target,
                    body,
                    canonicalAuth.accountId,
                    canonicalAuth.privateKey,
                    timestampMs,
                    nonce!!,
                )
            }
            val builder = TransportRequest.builder()
                .setMethod("POST")
                .setUri(target)
                .addHeader("Accept", NORITO_MEDIA_TYPE)
                .addHeader("Content-Type", NORITO_MEDIA_TYPE)
                .setBody(body)
                .setMaximumResponseBytes(MAX_DEVICE_POLICY_PROOF_PAGE_ARCHIVE_BYTES_V1.toLong())
            authHeaders.forEach { (name, value) -> builder.addHeader(name, value) }
            return execute(builder.build(), 200).thenApply { response ->
                verifyOfflineDevicePolicyProofPageV1(
                    response.body,
                    checkpoint,
                    evaluationTimeMilliseconds,
                )
            }
        }

        /**
         * Evaluate an exact native-protected registration and return a credential only for an
         * eligible finalized decision. Canonical auth supplies the account and is never replayed
         * through redirects or retries by the maintained transport contract.
         */
        fun postOfflineDeviceEligibilityV1(
            request: OfflineDeviceEligibilityRequestV1,
            canonicalAuth: ToriiCanonicalRequestAuth,
            expectedIssuer: EligibilityIssuerPublicKeyV1,
            trustAnchor: OfflineDeviceFinalityTrustAnchorV1,
            evaluationTimeMilliseconds: Long = System.currentTimeMillis(),
        ): CompletableFuture<OfflineDeviceEligibilityResponseV1> {
            require(localSigningContext.networkId() == trustAnchor.networkId) {
                "offline device eligibility trust must match LocalSigningContext.networkId"
            }
            require(evaluationTimeMilliseconds > 0) {
                "evaluationTimeMilliseconds must be positive"
            }
            val target = URI.create("$baseUri$DEVICE_ELIGIBILITY_PATH")
            val body = makeOfflineDeviceEligibilityRequestV1(request)
            val timestampMs = canonicalAuth.timestampMs
            val nonce = canonicalAuth.nonce
            require((timestampMs == null) == (nonce == null)) {
                "timestampMs and nonce must be provided together"
            }
            val authHeaders = if (timestampMs == null) {
                CanonicalRequestSigner.buildHeaders(
                    localSigningContext.networkId(),
                    "POST",
                    target,
                    body,
                    canonicalAuth.accountId,
                    canonicalAuth.privateKey,
                )
            } else {
                CanonicalRequestSigner.buildHeaders(
                    localSigningContext.networkId(),
                    "POST",
                    target,
                    body,
                    canonicalAuth.accountId,
                    canonicalAuth.privateKey,
                    timestampMs,
                    nonce!!,
                )
            }
            val builder = TransportRequest.builder()
                .setMethod("POST")
                .setUri(target)
                .addHeader("Accept", NORITO_MEDIA_TYPE)
                .addHeader("Content-Type", NORITO_MEDIA_TYPE)
                .setBody(body)
                .setMaximumResponseBytes(MAX_DEVICE_ELIGIBILITY_RESPONSE_ARCHIVE_BYTES_V1.toLong())
            authHeaders.forEach { (name, value) -> builder.addHeader(name, value) }
            return execute(builder.build(), 200).thenApply { response ->
                verifyOfflineDeviceEligibilityResponseV1(
                    response.body,
                    request,
                    expectedIssuer,
                    trustAnchor,
                    evaluationTimeMilliseconds,
                )
            }
        }

        fun getRecipientRegistrationLineage(
            query: RecipientLineageQueryV2,
            canonicalAuth: ToriiCanonicalRequestAuth,
        ): CompletableFuture<RecipientRegistrationLineage> {
            val target = URI.create("$baseUri$RECEIVER_LINEAGE_PATH")
            val body = query.noritoEncoded()
            val timestampMs = canonicalAuth.timestampMs
            val nonce = canonicalAuth.nonce
            require((timestampMs == null) == (nonce == null)) {
                "timestampMs and nonce must be provided together"
            }
            val authHeaders = if (timestampMs == null) {
                CanonicalRequestSigner.buildHeaders(
                    localSigningContext.networkId(),
                    "POST",
                    target,
                    body,
                    canonicalAuth.accountId,
                    canonicalAuth.privateKey,
                )
            } else {
                CanonicalRequestSigner.buildHeaders(
                    localSigningContext.networkId(),
                    "POST",
                    target,
                    body,
                    canonicalAuth.accountId,
                    canonicalAuth.privateKey,
                    timestampMs,
                    nonce!!,
                )
            }
            val builder = TransportRequest.builder()
                .setMethod("POST")
                .setUri(target)
                .addHeader("Accept", NORITO_MEDIA_TYPE)
                .addHeader("Content-Type", NORITO_MEDIA_TYPE)
                .setBody(body)
                .setMaximumResponseBytes(MAX_TORII_RESPONSE_BYTES.toLong())
            authHeaders.forEach { (name, value) -> builder.addHeader(name, value) }
            return execute(
                builder.build(),
                200,
            ).thenApply { RecipientRegistrationLineage(it.body) }
        }

        fun submitTopUp(
            request: TopUpRequest,
            operationId: String,
        ): CompletableFuture<OperationReference> = submitCommand(
            TOP_UP_PATH,
            request.noritoEncoded(),
            operationId,
        )

        fun submitRedeem(
            request: RedeemSubmissionRequest,
            operationId: String,
        ): CompletableFuture<OperationReference> = submitCommand(
            REDEEM_PATH,
            request.noritoEncoded(),
            operationId,
        )

        fun getOperation(operationId: String): CompletableFuture<OperationStatus> {
            val id = requireOperationId(operationId)
            return execute(
                TransportRequest.builder()
                    .setMethod("GET")
                    .setUri(URI.create("$baseUri$OPERATIONS_PATH/$id"))
                    .addHeader("Accept", NORITO_MEDIA_TYPE)
                    .setMaximumResponseBytes(MAX_TORII_RESPONSE_BYTES.toLong())
                    .build(),
                200,
            ).thenApply { OperationStatus(it.body) }
        }

        private fun submitCommand(
            path: String,
            request: ByteArray,
            operationId: String,
        ): CompletableFuture<OperationReference> {
            val id = requireOperationId(operationId)
            return execute(
                TransportRequest.builder()
                    .setMethod("POST")
                    .setUri(URI.create("$baseUri$path"))
                    .addHeader("Accept", NORITO_MEDIA_TYPE)
                    .addHeader("Content-Type", NORITO_MEDIA_TYPE)
                    .addHeader("Idempotency-Key", id)
                    .setBody(request)
                    .setMaximumResponseBytes(MAX_TORII_RESPONSE_BYTES.toLong())
                    .build(),
                202,
            ).thenApply { OperationReference(it.body) }
        }

        private fun execute(
            request: TransportRequest,
            expectedStatus: Int,
            expectedMediaType: String = NORITO_MEDIA_TYPE,
        ): CompletableFuture<TransportResponse> = transport.execute(request).thenApply { response ->
            check(response.statusCode == expectedStatus) {
                "Kagemusha Torii request failed with HTTP ${response.statusCode}"
            }
            val contentTypes = response.headers["Content-Type"].orEmpty()
            check(contentTypes.size == 1 && contentTypes.single().equals(expectedMediaType, true)) {
                "Kagemusha Torii response must use $expectedMediaType"
            }
            response
        }
    }

    /** Owns one native artifact spool until installation or cancellation. */
    class ArtifactIngest internal constructor(initialHandle: Long) : AutoCloseable {
        private var handle = initialHandle
        private var finalized = false
        private var installClaimed = false

        @Synchronized
        fun write(chunk: ByteArray) {
            requireOpen(allowFinalized = false)
            nativeArtifactWriteV4(handle, requireChunk(chunk))
        }

        fun finish() {
            withHeavyProofPermit("artifact finalization") {
                synchronized(this) {
                    requireOpen(allowFinalized = false)
                    nativeArtifactFinalizeV4(handle)
                    finalized = true
                }
            }
        }

        @Synchronized
        fun isFinalized(): Boolean = finalized

        @Synchronized
        override fun close() {
            if (handle == 0L) return
            check(!installClaimed) { "artifact ingest is being installed" }
            nativeArtifactCancelV4(handle)
            handle = 0
            finalized = false
        }

        @Synchronized
        internal fun claimFinalizedHandle(): Long {
            check(handle != 0L && finalized && !installClaimed) {
                "artifact ingest is not installable"
            }
            installClaimed = true
            return handle
        }

        @Synchronized
        internal fun releaseInstallClaim(expectedHandle: Long) {
            if (handle == expectedHandle) installClaimed = false
        }

        @Synchronized
        internal fun relinquishInstalledHandle(expectedHandle: Long) {
            check(handle == expectedHandle && finalized && installClaimed) {
                "artifact install ownership mismatch"
            }
            handle = 0
            finalized = false
            installClaimed = false
        }

        private fun requireOpen(allowFinalized: Boolean) {
            check(handle != 0L) { "artifact ingest is closed" }
            check(allowFinalized || !finalized) { "artifact ingest is already finalized" }
            check(!installClaimed) { "artifact ingest is being installed" }
        }
    }

    /**
     * Locally trusted material required to authenticate one published Kagemusha release.
     *
     * The policy must be provisioned from the deployment trust root, not copied from the
     * downloaded release. Native verifies signer-role thresholds and hashes both evidence files
     * before validating the candidate-bound promotion record and consuming any finalized artifact
     * handle.
     */
    class ReleaseAuthentication(
        trustedPolicyNorito: ByteArray,
        releaseAttestationNorito: ByteArray,
        benchmarkEvidence: ByteArray,
        cryptographicReview: ByteArray,
        promotionRecordNorito: ByteArray,
    ) {
        internal val trustedPolicyNorito = requireBoundedBytes(
            trustedPolicyNorito,
            "trustedPolicyNorito",
            MAX_TRUSTED_RELEASE_POLICY_BYTES,
        )
        internal val releaseAttestationNorito = requireBoundedBytes(
            releaseAttestationNorito,
            "releaseAttestationNorito",
            MAX_RELEASE_ATTESTATION_BYTES,
        )
        internal val benchmarkEvidence = requireBoundedBytes(
            benchmarkEvidence,
            "benchmarkEvidence",
            MAX_RELEASE_EVIDENCE_BYTES,
        )
        internal val cryptographicReview = requireBoundedBytes(
            cryptographicReview,
            "cryptographicReview",
            MAX_RELEASE_EVIDENCE_BYTES,
        )
        internal val promotionRecordNorito = requireBoundedBytes(
            promotionRecordNorito,
            "promotionRecordNorito",
            MAX_PROMOTION_RECORD_BYTES,
        )
    }

    /** Coordinates one authenticated, atomic eight-artifact generation install. */
    class ArtifactInstallSession internal constructor(
        manifest: ByteArray,
        manifestDigest: ByteArray,
        releaseAuthentication: ReleaseAuthentication,
    ) : AutoCloseable {
        private val manifestNorito = manifest.copyOf()
        private val manifestSha256 = manifestDigest.copyOf()
        private val trustedPolicyNorito = releaseAuthentication.trustedPolicyNorito.copyOf()
        private val releaseAttestationNorito =
            releaseAuthentication.releaseAttestationNorito.copyOf()
        private val benchmarkEvidence = releaseAuthentication.benchmarkEvidence.copyOf()
        private val cryptographicReview = releaseAuthentication.cryptographicReview.copyOf()
        private val promotionRecordNorito = releaseAuthentication.promotionRecordNorito.copyOf()
        private val artifacts = linkedMapOf<ArtifactRoleV4, ArtifactIngest>()
        private val artifactDigests = mutableListOf<String>()
        private var installed = false
        private var closed = false

        @Synchronized
        fun beginArtifact(
            role: ArtifactRoleV4,
            expectedArtifactSha256: ByteArray,
        ): ArtifactIngest {
            requirePending()
            check(artifacts.size < ARTIFACT_COUNT) { "artifact set already has eight streams" }
            require(role == ArtifactRoleV4.entries[artifacts.size]) {
                "artifact role is not in canonical V4 order"
            }
            val digest = requireDigest(expectedArtifactSha256, "expectedArtifactSha256")
            val key = hex(digest)
            require(!artifactDigests.contains(key)) { "expectedArtifactSha256 is duplicated" }
            return beginArtifactIngest(manifestNorito, manifestSha256, digest)
                .also {
                    artifacts[role] = it
                    artifactDigests += key
                }
        }

        fun install() {
            withHeavyProofPermit("artifact install") {
                synchronized(this) {
                    requirePending()
                    check(artifacts.size == ARTIFACT_COUNT) {
                        "artifact set must contain exactly eight streams"
                    }
                    requireCanonicalV4ArtifactRoleInventory(artifacts.keys.toList())
                    val ordered = artifacts.values.toList()
                    val handles = LongArray(ARTIFACT_COUNT)
                    var claimed = 0
                    try {
                        while (claimed < ordered.size) {
                            handles[claimed] = ordered[claimed].claimFinalizedHandle()
                            claimed += 1
                        }
                        nativeArtifactSetInstallV4(
                            manifestNorito,
                            manifestSha256,
                            trustedPolicyNorito,
                            releaseAttestationNorito,
                            benchmarkEvidence,
                            cryptographicReview,
                            promotionRecordNorito,
                            handles,
                        )
                    } catch (failure: Throwable) {
                        repeat(claimed) { index ->
                            ordered[index].releaseInstallClaim(handles[index])
                        }
                        throw failure
                    }
                    ordered.forEachIndexed { index, ingest ->
                        ingest.relinquishInstalledHandle(handles[index])
                    }
                    artifacts.clear()
                    artifactDigests.clear()
                    installed = true
                }
            }
        }

        @Synchronized
        fun isInstalled(): Boolean =
            !closed && nativeArtifactSetIsInstalledV4(manifestNorito, manifestSha256)

        @Synchronized
        fun artifactBinding(): ArtifactBindingV4 {
            check(installed && !closed && isInstalled()) { "artifact set is not installed" }
            return ArtifactBindingV4(
                nativeBuildArtifactBindingV4(manifestNorito, manifestSha256),
            )
        }

        fun uninstall() {
            withHeavyProofPermit("artifact uninstall") {
                synchronized(this) {
                    if (!installed || closed) return@withHeavyProofPermit
                    nativeArtifactSetUninstallV4(manifestSha256)
                    installed = false
                    closed = true
                }
            }
        }

        @Synchronized
        override fun close() {
            if (closed || installed) return
            var firstFailure: RuntimeException? = null
            artifacts.values.forEach { ingest ->
                try {
                    ingest.close()
                } catch (failure: RuntimeException) {
                    if (firstFailure == null) firstFailure = failure
                }
            }
            artifacts.clear()
            artifactDigests.clear()
            closed = true
            firstFailure?.let { throw it }
        }

        private fun requirePending() {
            check(!closed && !installed) { "artifact install session is not pending" }
        }
    }
}
