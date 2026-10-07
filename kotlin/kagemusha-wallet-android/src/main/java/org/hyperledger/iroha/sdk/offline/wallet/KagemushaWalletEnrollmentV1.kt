// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

/**
 * Exact E1, selected enrollment-policy preimage and existing canonical AccountId originals.
 * Obtain E1 through the authenticated issuer endpoint. Native checks these bytes against its
 * authenticated installation and derives the hardware profile; this carrier grants no freshness.
 */
class KagemushaWalletEnrollmentOriginalsV1(challenge: ByteArray, policy: ByteArray, account: ByteArray) {
    private val retained: List<ByteArray>
    init {
        val inputs = listOf(challenge, policy, account)
        require(inputs.zip(listOf(1024, 1024, 4096)).all { (bytes, bound) -> bytes.isNotEmpty() && bytes.size <= bound }) {
            "bounded original E1, enrollment policy and existing account required"
        }
        retained = inputs.map { it.copyOf() }
    }
    internal fun frames(): List<ByteArray> = retained.map { it.copyOf() }
    override fun toString(): String = "KagemushaWalletEnrollmentOriginalsV1(originals=[REDACTED])"
}

/** Durable Native enrollment progression; none of these outcomes admits monetary use. */
enum class KagemushaWalletEnrollmentStateV1 { ENROLLED, PENDING, SLOT_ABANDONED }

/** Exact slot and public marker DATA returned by the same Native custody provider. */
class KagemushaWalletEnrollmentProgressV1 internal constructor(reply: KagemushaWalletEnrollmentReplyV1) {
    @JvmField val state = when (reply.status) {
        0 -> KagemushaWalletEnrollmentStateV1.ENROLLED
        1 -> KagemushaWalletEnrollmentStateV1.PENDING
        2 -> KagemushaWalletEnrollmentStateV1.SLOT_ABANDONED
        else -> throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
    }
    private val retainedSlot = reply.slot()
    private val retainedKey = reply.paymentKey()
    private val retainedMarker = reply.bytes()
    /** Persist with the exact E1; restore uses resume and never a new generation grant. */
    fun slot(): ByteArray = retainedSlot.copyOf()
    /** SEC1 payment public key, present only for ENROLLED. No private key is exported. */
    fun paymentKey(): ByteArray = retainedKey.copyOf()
    /** Exact canonical generation-zero marker, present only for ENROLLED. */
    fun markerOriginal(): ByteArray = retainedMarker.copyOf()
    override fun toString(): String = "KagemushaWalletEnrollmentProgressV1(state=$state, originals=[REDACTED])"
}

/** Separate enrollment output bounds cannot broaden the monetary operation reply contract. */
internal class KagemushaWalletEnrollmentReplyV1(
    @JvmField val status: Int, @JvmField val reason: Int, @JvmField val platformCode: Int,
    slot: ByteArray, paymentKey: ByteArray, bytes: ByteArray,
) {
    private val retainedSlot: ByteArray
    private val retainedKey: ByteArray
    private val retainedBytes: ByteArray
    init {
        val valid = if (status < 0) slot.isEmpty() && paymentKey.isEmpty() && bytes.isEmpty() else {
            slot.size == 32 && slot.any { it != 0.toByte() } && when (status) {
                0 -> paymentKey.size == 65 && paymentKey[0] == 4.toByte() && bytes.size in 1..1024
                1, 2 -> paymentKey.isEmpty() && bytes.isEmpty()
                3 -> paymentKey.isEmpty() && bytes.size in 1..524288
                4 -> paymentKey.isEmpty() && bytes.size in 1..1024
                else -> false
            }
        }
        if (!valid) invalid()
        retainedSlot = slot.copyOf(); retainedKey = paymentKey.copyOf(); retainedBytes = bytes.copyOf()
    }
    fun checked(): KagemushaWalletEnrollmentReplyV1 {
        if (status < 0) throw KagemushaWalletExceptionV1(status, reason, platformCode)
        return this
    }
    fun progress(): KagemushaWalletEnrollmentProgressV1 = KagemushaWalletEnrollmentProgressV1(checked())
    fun original(expectedStatus: Int, expectedSlot: ByteArray): ByteArray {
        checked()
        if (status != expectedStatus || !retainedSlot.contentEquals(expectedSlot)) invalid()
        return bytes()
    }
    fun slot(): ByteArray = retainedSlot.copyOf()
    fun paymentKey(): ByteArray = retainedKey.copyOf()
    fun bytes(): ByteArray = retainedBytes.copyOf()
    private fun invalid(): Nothing = throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
}

/** Bounded DATA for the fixed original-only JNI request, never an enrollment authority. */
internal class KagemushaWalletEnrollmentInputV1(
    val selector: Int, originals: KagemushaWalletEnrollmentOriginalsV1,
    slot: ByteArray = byteArrayOf(), original: ByteArray = byteArrayOf(), certificates: ByteArray = byteArrayOf(),
) {
    private val retained: List<ByteArray>
    init {
        require(selector in 0..3) { "unknown enrollment operation" }
        require(if (selector == 0) slot.isEmpty() else slot.size == 32 && slot.any { it != 0.toByte() }) { "exact returned enrollment slot required" }
        require(when (selector) { 0, 1 -> original.isEmpty(); 2 -> original.size in 1..524288; else -> original.size in 1..1024 }) { "bounded operation original required" }
        require(if (selector == 3) certificates.size in 1..10000 else certificates.isEmpty()) { "only credential storage accepts issuer certificate originals" }
        retained = listOf(slot.copyOf()) + originals.frames() + listOf(original.copyOf(), certificates.copyOf())
    }
    fun frames(): List<ByteArray> = retained.map { it.copyOf() }
}

internal object KagemushaWalletEnrollmentNativeV1 {
    @JvmStatic external fun enroll(runtime: Long, selector: Int, slot: ByteArray, challenge: ByteArray,
        policy: ByteArray, account: ByteArray, original: ByteArray, certificates: ByteArray): KagemushaWalletEnrollmentReplyV1?
}
