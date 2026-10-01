// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * Fresh dual-signature authentication of the independently retained enrolled native owner.
 * These transport projections never restore authority from an application cache. Native Core
 * checks its original enrollment, checkpoint, hardware, journal and continuous deadline before
 * granting its lease. Recovery does not invoke initial enrollment or select a replacement owner.
 */
class KagemushaNativeRecoveredEnrollmentV1 internal constructor(
    private val bridge: KagemushaCoreCoordinatorBridgeV1,
) {
    class Challenge internal constructor(fields: List<ByteArray>) {
        private val original = fields.map(ByteArray::copyOf)
        val accountChallenge: KagemushaEnrolledOpenAccountChallengeV1 =
            KagemushaEnrolledOpenChallengeCodecV1.decodeAccountChallengeShapeExact(original[1])
        init {
            val value = accountChallenge
            val message = KagemushaEnrolledOpenChallengeCodecV1.accountSigningMessageShape(value,
                KagemushaEnrolledOpenSelectorV1(1, value.owner, value.enrollmentId()), value.nonce(),
                value.releaseId(), value.hardwarePolicyDigest(), value.coreAuthorizationKeyReference())
            require(message.contentEquals(original[2])) { "Native recovery signing message differs from its exact account challenge" }
            require(value.nonce().contentEquals(original[4])) { "Native recovery device nonce differs from its account challenge" }
            val command = KagemushaDeviceOperationCodecV1.encodeControlCommand(
                KagemushaDeviceControlCommandV1.ReadActiveHardwareCredential)
            require(command.contentEquals(original[3])) {
                "Native recovery command is not the original operation-1 read"
            }
        }
        fun attemptId(): ByteArray = original[0].copyOf()
        fun canonicalAccountChallenge(): ByteArray = original[1].copyOf()
        fun accountSigningMessage(): ByteArray = original[2].copyOf()
        fun canonicalDeviceCommand(): ByteArray = original[3].copyOf()
        fun deviceRequestId(): ByteArray = original[4].copyOf()
        internal fun same(fields: List<ByteArray>): Boolean = original.size == fields.size &&
            original.indices.all { original[it].contentEquals(fields[it]) }
    }

    private var original: Challenge? = null
    private var proofRequest: List<ByteArray>? = null
    private var authenticated = false
    private var revoked = false

    /** Recheck the same fresh native challenge. Exact retry never renews its deadline. */
    @Synchronized
    fun begin(): Challenge {
        check(!revoked && !authenticated) { "Recovered enrollment attempt is unavailable" }
        val fields = bridge.invoke(METHOD, listOf(u32(9)))
        original?.let {
            if (!it.same(fields)) {
                revoked = true
                bridge.close()
                error("The retained native recovery challenge changed")
            }
            return it
        }
        return try { Challenge(fields).also { original = it } }
        catch (error: Throwable) {
            revoked = true
            runCatching { bridge.close() }
            throw error
        }
    }

    /** Grant only native's original lease after its account and actual device verification. */
    @Synchronized
    fun authenticate(challenge: Challenge, accountSignature: ByteArray, completeDeviceResponse: ByteArray) {
        check(!revoked && original === challenge) { "Recovered enrollment challenge belongs to another owner" }
        val fields = listOf(u32(10), challenge.attemptId(), accountSignature.copyOf(), completeDeviceResponse.copyOf())
        proofRequest?.let { retained ->
            require(retained.indices.all { retained[it].contentEquals(fields[it]) }) {
                "The original recovery proof changed"
            }
        }
        // Strict framing rejects invalid requests before retaining or dispatching them.
        KagemushaCoreCoordinatorFrameV1.encodeRequest(METHOD, fields)
        if (proofRequest == null) proofRequest = fields.map(ByteArray::copyOf)
        bridge.invoke(METHOD, fields)
        authenticated = true
    }

    /** Revoke before asking native to cancel; an uncertain response cannot revive this scope. */
    @Synchronized
    fun cancel(challenge: Challenge) {
        check(!revoked && original === challenge) { "Recovered enrollment challenge belongs to another owner" }
        revoked = true
        // Phase11 cancels only an outstanding challenge. A completed observation lease
        // is retired through its original owning handle, never a new cancellation ticket.
        if (authenticated) bridge.close()
        else bridge.invoke(METHOD, listOf(u32(11), challenge.attemptId()))
    }

    @Synchronized
    internal fun revokeLocal() {
        revoked = true
        original = null
        proofRequest = null
        authenticated = false
    }

    private companion object {
        val METHOD = KagemushaCoreCoordinatorMethodV1.INITIAL_ENROLLMENT
        fun u32(value: Int): ByteArray = ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN).putInt(value).array()
    }
}
