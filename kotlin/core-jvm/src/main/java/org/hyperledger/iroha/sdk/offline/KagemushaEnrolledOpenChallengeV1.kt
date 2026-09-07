// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import org.hyperledger.iroha.sdk.core.model.NetworkId

/** Complete Core recovery statement projection, without its hardware seal or authority. */
class KagemushaDurabilityAnchorStatementV1(
    @JvmField val metadataRevision: BigInteger,
    @JvmField val version: Int,
    @JvmField val lane: KagemushaDeviceLaneIdV1,
    stateCommitment: ByteArray,
    @JvmField val hardwareEpoch: KagemushaDeviceHardwareEpochV1,
    @JvmField val devicePolicyBinding: KagemushaDevicePolicyBindingV1,
    stateNonceCommitment: ByteArray,
    @JvmField val logicalSequence: BigInteger,
    @JvmField val journalRevision: BigInteger,
    @JvmField val inboxRevision: BigInteger,
    snapshotCommitment: ByteArray,
) {
    private val state = raw32(stateCommitment, "stateCommitment")
    private val nonce = fixed32(stateNonceCommitment, "stateNonceCommitment")
    private val snapshot = raw32(snapshotCommitment, "snapshotCommitment")

    init {
        require(version == 1) { "durability statement version must be 1" }
        requirePositiveU128(metadataRevision, "metadataRevision")
        requireUnsigned128(logicalSequence, "logicalSequence")
        requireUnsigned128(journalRevision, "journalRevision")
        requireUnsigned128(inboxRevision, "inboxRevision")
        NetworkId.fromBytes(lane.networkId())
    }

    fun stateCommitment(): ByteArray = state.copyOf()
    fun stateNonceCommitment(): ByteArray = nonce.copyOf()
    fun snapshotCommitment(): ByteArray = snapshot.copyOf()
}

/** Untrusted description of the source retained by native construction; never proof of it. */
sealed class KagemushaEnrolledOpenAuthoritySourceV1 {
    class InitialCertificate(certificateDigest: ByteArray) : KagemushaEnrolledOpenAuthoritySourceV1() {
        private val digest = raw32(certificateDigest, "certificateDigest")
        fun certificateDigest(): ByteArray = digest.copyOf()
    }

    class RecoveryCheckpoint(
        @JvmField val statement: KagemushaDurabilityAnchorStatementV1,
        terminalCertificateDigest: ByteArray,
    ) : KagemushaEnrolledOpenAuthoritySourceV1() {
        private val digest = raw32(terminalCertificateDigest, "terminalCertificateDigest")
        fun terminalCertificateDigest(): ByteArray = digest.copyOf()
    }
}

/**
 * Exact account signing projection. Matching it does not establish MiBank approval, enrollment,
 * a native deadline, hardware possession, checkpoint authenticity or monetary authority.
 */
class KagemushaEnrolledOpenAccountChallengeV1(
    @JvmField val version: Int,
    @JvmField val domain: String,
    enrollmentId: ByteArray,
    @JvmField val owner: KagemushaRetailEnrollmentOwnerV1,
    nonce: ByteArray,
    @JvmField val authoritySource: KagemushaEnrolledOpenAuthoritySourceV1,
    releaseId: ByteArray,
    hardwarePolicyDigest: ByteArray,
    coreAuthorizationKeyReference: ByteArray,
    @JvmField val lifetimeMs: Long,
) {
    private val enrollment = raw32(enrollmentId, "enrollmentId")
    private val request = fixed32(nonce, "nonce")
    private val release = fixed32(releaseId, "releaseId")
    private val policy = fixed32(hardwarePolicyDigest, "hardwarePolicyDigest")
    private val coreKey = fixed32(coreAuthorizationKeyReference, "coreAuthorizationKeyReference")

    init {
        require(version == 1) { "enrolled-open account challenge version must be 1" }
        require(domain == ACCOUNT_DOMAIN) { "enrolled-open account challenge domain mismatch" }
        require(lifetimeMs == LIFETIME_MS) { "enrolled-open account challenge lifetime must be 120000 ms" }
        KagemushaNoritoV1.encodeEnrolledOpenSelectorShape(KagemushaEnrolledOpenSelectorV1(1, owner, enrollment))
        if (authoritySource is KagemushaEnrolledOpenAuthoritySourceV1.RecoveryCheckpoint) {
            val lane = authoritySource.statement.lane
            require(lane.networkId().contentEquals(owner.runtime.networkId.bytes()) &&
                lane.deviceLaneId().contentEquals(owner.laneId()) &&
                lane.assetCanonicalPayload().contentEquals(owner.runtime.asset.canonicalPayload()) &&
                lane.scale == owner.runtime.scale) { "recovery statement belongs to a different owner lane" }
        }
    }

    fun enrollmentId(): ByteArray = enrollment.copyOf()
    fun nonce(): ByteArray = request.copyOf()
    fun releaseId(): ByteArray = release.copyOf()
    fun hardwarePolicyDigest(): ByteArray = policy.copyOf()
    fun coreAuthorizationKeyReference(): ByteArray = coreKey.copyOf()

    companion object {
        const val ACCOUNT_DOMAIN = "iroha:kagemusha:v1:enrolled-open-account-possession"
        const val LIFETIME_MS: Long = 120_000
    }
}
