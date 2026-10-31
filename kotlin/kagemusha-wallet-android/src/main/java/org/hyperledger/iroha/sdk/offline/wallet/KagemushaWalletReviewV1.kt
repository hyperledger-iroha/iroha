// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import java.util.concurrent.atomic.AtomicLong

/** Exact authenticated Native review DATA. This projection cannot construct an operation. */
class KagemushaWalletReviewProjectionV1 internal constructor(original: ByteArray) {
    private val retained = original.takeIf { it.size in FIXED_BYTES..MAXIMUM_BYTES }?.copyOf()
        ?: throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
    val kind: Kind
    val amount: KagemushaWalletUInt128V1
    val fee: KagemushaWalletUInt128V1
    val grossDebit: KagemushaWalletUInt128V1
    val netDestinationAmount: KagemushaWalletUInt128V1
    enum class Kind { SEND, UNLOAD }
    internal companion object {
        const val FIXED_BYTES = 495
        const val MAXIMUM_BYTES = FIXED_BYTES + 4_096
    }
    init {
        fun invalid(): Nothing = throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        if (!retained.copyOfRange(0, 8).contentEquals(byteArrayOf(75,87,79,82,86,49,0,0))) invalid()
        kind = when (retained[8].toInt()) { 1 -> Kind.SEND; 8 -> Kind.UNLOAD; else -> invalid() }
        var accountLength = 0L
        for (i in 0..3) accountLength = accountLength or ((retained[491+i].toLong() and 255) shl (8*i))
        if (accountLength > 4_096 || retained.size.toLong() != FIXED_BYTES + accountLength ||
            (if (kind == Kind.SEND) accountLength == 0L else accountLength != 0L)) invalid()
        amount = scalar(9); fee = scalar(25); grossDebit = scalar(41); netDestinationAmount = scalar(57)
        fun word(offset: Int) = retained.copyOfRange(offset, offset + 32).any { it != 0.toByte() }
        if (amount == KagemushaWalletUInt128V1(0, 0) || retained[426] != 4.toByte() ||
            retained.copyOfRange(427, 491).all { it == 0.toByte() } ||
            listOf(106, 202, 234, 266, 298, 330, 362, 394).any { !word(it) }) invalid()
        if (kind == Kind.SEND) {
            if (retained[73] != 1.toByte() || !word(74) || !word(138) || word(170)) invalid()
        } else if (retained[73] != 0.toByte() || word(74) || word(138)) invalid()
    }
    private fun scalar(offset: Int): KagemushaWalletUInt128V1 {
        fun limb(at: Int): Long { var value=0L; for (i in 0..7) value = value or ((retained[at+i].toLong() and 255) shl (8*i)); return value }
        return KagemushaWalletUInt128V1(limb(offset), limb(offset + 8))
    }
    private fun word(index: Int) = retained.copyOfRange(106 + index * 32, 138 + index * 32)
    /** Exact bytes bind a fresh hardware confirmation; no local frame is reassembled. */
    fun bytes(): ByteArray = retained.copyOf()
    fun receiverWalletId(): ByteArray? = if (kind == Kind.SEND) retained.copyOfRange(74,106) else null
    fun destinationAccountDigest() = word(0)
    /** Exact Native-authenticated canonical AccountId original; null for Unload.
     * Render only under independently authenticated installed network presentation selection. */
    fun destinationAccountOriginal(): ByteArray? =
        if (kind == Kind.SEND) retained.copyOfRange(FIXED_BYTES, retained.size) else null
    fun requestDigest() = word(1)
    fun chargeQuoteDigest() = word(2)
    fun schemeId() = word(3)
    fun walletId() = word(4)
    fun currentHead() = word(5)
    fun sourceStateCommitment() = word(6)
    fun sourceCapsuleDigest() = word(7)
    fun credentialDigest() = word(8)
    fun artifactManifestDigest() = word(9)
    fun paymentPublicKey() = retained.copyOfRange(426,491)
    override fun toString() = "KagemushaWalletReviewProjectionV1(kind=$kind, source=[REDACTED])"
}

/** Comparison DATA for a new Native review; never a reconstructed approval or capability. */
sealed class KagemushaWalletReviewExpectationV1 {
    object Fresh : KagemushaWalletReviewExpectationV1()
    class OriginalUnstarted(originalProjection: ByteArray) : KagemushaWalletReviewExpectationV1() {
        private val retained = KagemushaWalletReviewProjectionV1(originalProjection)
        internal fun projection() = retained
    }
    internal fun bytes(): ByteArray = when (this) {
        Fresh -> byteArrayOf()
        is OriginalUnstarted -> projection().bytes()
    }
    internal val tag: Int get() = if (this === Fresh) 0 else 1
}

/** Only genuine preflight maintenance reasons, without permission to dispatch. */
enum class KagemushaWalletReviewMaintenanceWaitV1 {
    SELECTED_PENDING, ARCHIVE_FOLD, BUSY;
    internal companion object {
        fun from(tag: Int): KagemushaWalletReviewMaintenanceWaitV1 = when (tag) {
            1 -> SELECTED_PENDING; 2 -> ARCHIVE_FOLD; 6 -> BUSY
            else -> invalidReviewOutput()
        }
    }
}

/** Exact already-bound request observation. There is deliberately no Unknown case. */
sealed class KagemushaWalletBoundRequestStatusV1 {
    object Preparing : KagemushaWalletBoundRequestStatusV1()
    class Outcome internal constructor(val completion: KagemushaWalletCallV1) : KagemushaWalletBoundRequestStatusV1()
    internal companion object {
        fun from(reply: KagemushaWalletCallV1): KagemushaWalletBoundRequestStatusV1 {
            if (reply.status != KagemushaWalletCallV1.REVIEW_EXISTING) invalidReviewOutput()
            val bytes = reply.bytes()
            fun u32(at: Int): Long = (0..3).fold(0L) { value, i -> value or ((bytes[at + i].toLong() and 255) shl (8 * i)) }
            fun u64(at: Int): Long = (0..7).fold(0L) { value, i -> value or ((bytes[at + i].toLong() and 255) shl (8 * i)) }
            if (bytes.size !in 28..10_062) invalidReviewOutput()
            val kind = u32(0); val detail = u32(4); val low = u64(8); val high = u64(16); val length = u32(24)
            if (kind !in listOf(1L, 2L, 3L, 4L, 5L, 10L, 11L) || length > 10_034 || bytes.size.toLong() != 28 + length) invalidReviewOutput()
            if (kind == 11L) {
                if (detail != 0L || low != 0L || high != 0L || length != 0L) invalidReviewOutput()
                return Preparing
            }
            return Outcome(KagemushaWalletCallV1(kind.toInt(), -1, 0, low, high, detail.toInt(),
                bytes.copyOfRange(28, bytes.size)).completion())
        }
    }
}

/** Only Ready carries a newly authenticated one-use review. */
sealed class KagemushaWalletReviewDecisionV1 {
    class Ready internal constructor(val review: KagemushaWalletReviewV1) : KagemushaWalletReviewDecisionV1()
    class Existing internal constructor(val status: KagemushaWalletBoundRequestStatusV1) : KagemushaWalletReviewDecisionV1()
    class Waiting internal constructor(val wait: KagemushaWalletReviewMaintenanceWaitV1) : KagemushaWalletReviewDecisionV1()
}

/** Existing work and retained approval are distinct from a newly executed operation. */
sealed class KagemushaWalletReviewedExecutionV1 {
    class Executed internal constructor(val completion: KagemushaWalletCallV1) : KagemushaWalletReviewedExecutionV1()
    class Existing internal constructor(val status: KagemushaWalletBoundRequestStatusV1) : KagemushaWalletReviewedExecutionV1()
    class RetainedReview internal constructor(val review: KagemushaWalletReviewV1,
        val wait: KagemushaWalletReviewMaintenanceWaitV1) : KagemushaWalletReviewedExecutionV1()
}

/** Route-specific parser shared by the actual executor and pure contract regression tests. */
internal fun KagemushaWalletCallV1.reviewedExecution(owner: Any, review: KagemushaWalletReviewV1,
    consumedToken: Long): KagemushaWalletReviewedExecutionV1 {
    if (status < 0) throw KagemushaWalletExceptionV1(status, reason, platformCode)
    return when (status) {
        KagemushaWalletCallV1.REVIEW_RETAINED -> KagemushaWalletReviewedExecutionV1.RetainedReview(
            review, review.restoreRetained(owner, consumedToken, this))
        KagemushaWalletCallV1.REVIEW_EXISTING -> KagemushaWalletReviewedExecutionV1.Existing(
            KagemushaWalletBoundRequestStatusV1.from(this))
        1, 2, 3, 4, 5, 10 -> KagemushaWalletReviewedExecutionV1.Executed(completion())
        else -> invalidReviewOutput()
    }
}

/** Dedicated JNI issuance parser: ordinary monetary kind18 has another ABI meaning. */
internal class KagemushaWalletReviewReplyV1(
    @JvmField val status: Int, @JvmField val reason: Int, @JvmField val platformCode: Int,
    private val sequenceLow: Long, private val sequenceHigh: Long, private val detail: Int, original: ByteArray,
) {
    private val retained = original.copyOf()
    internal fun decision(owner: Any, requestId: ByteArray, expected: KagemushaWalletReviewProjectionV1.Kind): KagemushaWalletReviewDecisionV1 {
        requireReviewRequestId(requestId)
        if (status < 0) {
            if (sequenceLow != 0L || sequenceHigh != 0L || detail != 0 || retained.isNotEmpty()) invalidReviewOutput()
            throw KagemushaWalletExceptionV1(status, reason, platformCode)
        }
        if (reason != -1 || platformCode != 0) invalidReviewOutput()
        return when (status) {
            18 -> {
                if (sequenceLow <= 0 || sequenceHigh != 0L || detail != 0) invalidReviewOutput()
                val projection = KagemushaWalletReviewProjectionV1(retained)
                if (projection.kind != expected) invalidReviewOutput()
                KagemushaWalletReviewDecisionV1.Ready(KagemushaWalletReviewV1.fromNative(owner, sequenceLow, requestId, projection))
            }
            KagemushaWalletCallV1.REVIEW_EXISTING -> KagemushaWalletReviewDecisionV1.Existing(
                KagemushaWalletBoundRequestStatusV1.from(KagemushaWalletCallV1(status, reason, platformCode,
                    sequenceLow, sequenceHigh, detail, retained)))
            KagemushaWalletCallV1.REVIEW_WAITING -> {
                KagemushaWalletCallV1(status, reason, platformCode, sequenceLow, sequenceHigh, detail, retained)
                KagemushaWalletReviewDecisionV1.Waiting(KagemushaWalletReviewMaintenanceWaitV1.from(detail))
            }
            else -> invalidReviewOutput()
        }
    }
    internal fun cleanupToken(): Long? = sequenceLow.takeIf { status == 18 && it > 0 }
    override fun toString() = "KagemushaWalletReviewReplyV1(status=$status, source=[REDACTED])"
}

/** Owner-local move-only capability, bound to its original request at Native issuance. */
class KagemushaWalletReviewV1 private constructor(
    private val origin: Any, token: Long, requestId: ByteArray, val projection: KagemushaWalletReviewProjectionV1,
) {
    private val identity = requestId.copyOf()
    private val selected = AtomicLong(token)
    /** Original identity, for exact recovery only; execution never accepts a replacement ID. */
    fun requestId(): ByteArray = identity.copyOf()
    internal fun consume(owner: Any): Long {
        check(owner === origin) { "review belongs to another exchange" }
        return selected.getAndSet(0).takeIf { it > 0 } ?: throw KagemushaWalletExceptionV1(-2)
    }
    /** Restore only the exact token returned by Native's typed absent-ID preflight refusal. */
    internal fun restoreRetained(owner: Any, expectedToken: Long, reply: KagemushaWalletCallV1): KagemushaWalletReviewMaintenanceWaitV1 {
        check(owner === origin)
        if (reply.status != KagemushaWalletCallV1.REVIEW_RETAINED || reply.sequenceLow != expectedToken ||
            expectedToken <= 0 || !selected.compareAndSet(0, expectedToken)) invalidReviewOutput()
        return KagemushaWalletReviewMaintenanceWaitV1.from(reply.detail)
    }
    internal companion object {
        internal fun fromNative(owner: Any, token: Long, requestId: ByteArray, projection: KagemushaWalletReviewProjectionV1): KagemushaWalletReviewV1 {
            requireReviewRequestId(requestId)
            return KagemushaWalletReviewV1(owner, token, requestId, projection)
        }
    }
    override fun toString() = "KagemushaWalletReviewV1(owner=[REDACTED])"
}

/** Copy/bounds only. Native authenticates originals, request absence and comparison DATA. */
internal class KagemushaWalletReviewInputV1(
    requestId: ByteArray, val selector: Int, val expectation: KagemushaWalletReviewExpectationV1,
    val amount: KagemushaWalletUInt128V1 = KagemushaWalletUInt128V1(0, 0),
    first: ByteArray = byteArrayOf(), second: ByteArray = byteArrayOf(),
) {
    private val identity = requestId.copyOf()
    private val originals: List<ByteArray>
    init {
        requireReviewRequestId(requestId)
        val nonzero = amount.low != 0L || amount.high != 0L
        when (selector) {
            1 -> require(!nonzero && first.isNotEmpty() && first.size <= 10_034 &&
                second.isNotEmpty() && second.size <= 4_096)
            8 -> {
                require(nonzero && first.size <= 1_024 && first.isEmpty() == second.isEmpty())
                if (second.isNotEmpty()) KagemushaWalletUnloadChargeReviewV1.validate(second)
            }
            else -> throw IllegalArgumentException("unknown review operation")
        }
        if (expectation is KagemushaWalletReviewExpectationV1.OriginalUnstarted) {
            require(expectation.projection().kind == if (selector == 1) KagemushaWalletReviewProjectionV1.Kind.SEND else KagemushaWalletReviewProjectionV1.Kind.UNLOAD)
        }
        originals = listOf(first.copyOf(), second.copyOf())
    }
    fun requestId(): ByteArray = identity.copyOf()
    fun first(): ByteArray = originals[0].copyOf()
    fun second(): ByteArray = originals[1].copyOf()
    override fun toString(): String = "KagemushaWalletReviewInputV1(originals=[REDACTED])"
}

internal fun requireReviewRequestId(value: ByteArray) {
    require(value.size == 32 && value.any { it != 0.toByte() }) { "nonzero32-byte request identity required" }
}
private fun invalidReviewOutput(): Nothing = throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
