// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import java.util.concurrent.atomic.AtomicLong

/** In-memory, one-use Native review, bound to the exact issuing exchange. */
class KagemushaWalletSuffixUnloadReviewV1 private constructor(
    private val origin: Any,
    token: Long,
    val projection: KagemushaWalletSuffixUnloadReviewProjectionV1,
) {
    private val selected = AtomicLong(token)
    fun requestId(): ByteArray = projection.requestId()
    internal fun consume(owner: Any): Long {
        require(owner === origin) { "suffix review belongs to another exchange" }
        return selected.getAndSet(0).takeIf { it > 0 } ?: throw KagemushaWalletExceptionV1(-2)
    }
    internal fun restore(owner: Any, consumed: Long, reply: KagemushaWalletSuffixUnloadReplyV1):
        KagemushaWalletReviewMaintenanceWaitV1 {
        requireSuffix(owner === origin && consumed > 0 && reply.status == 65 && reply.sequenceLow == consumed &&
            reply.detail in listOf(1, 6) && selected.compareAndSet(0, consumed))
        return KagemushaWalletReviewMaintenanceWaitV1.from(reply.detail)
    }
    internal companion object {
        fun fromNative(owner: Any, requestId: ByteArray, reply: KagemushaWalletSuffixUnloadReplyV1):
            KagemushaWalletSuffixUnloadReviewV1 {
            requireSuffix(reply.status == 73)
            val projection = KagemushaWalletSuffixUnloadReviewProjectionV1(reply.bytes())
            requireSuffix(projection.requestId().contentEquals(requestId))
            return KagemushaWalletSuffixUnloadReviewV1(owner, reply.sequenceLow, projection)
        }
    }
    override fun toString(): String = "KagemushaWalletSuffixUnloadReviewV1(owner=[REDACTED])"
}

/** A preparation turn never performs an irreversible debit. */
sealed class KagemushaWalletSuffixUnloadDecisionV1 {
    class Ready internal constructor(val review: KagemushaWalletSuffixUnloadReviewV1) : KagemushaWalletSuffixUnloadDecisionV1()
    class Verifying internal constructor(val progress: KagemushaWalletSuffixUnloadProgressV1) : KagemushaWalletSuffixUnloadDecisionV1()
    class Released internal constructor(val summary: KagemushaWalletSuffixUnloadReleasedV1) : KagemushaWalletSuffixUnloadDecisionV1()
    object Pending : KagemushaWalletSuffixUnloadDecisionV1()
    /** The selected request index positively contains no record for this exact named resume. */
    object NotStarted : KagemushaWalletSuffixUnloadDecisionV1()
    class NotApplicable internal constructor(val reason: Int) : KagemushaWalletSuffixUnloadDecisionV1()
    class Waiting internal constructor(val wait: KagemushaWalletReviewMaintenanceWaitV1) : KagemushaWalletSuffixUnloadDecisionV1()
}

/** Released is an offline debit, not online payout. Pending resumes the same request. */
sealed class KagemushaWalletSuffixUnloadExecutionV1 {
    class Released internal constructor(val summary: KagemushaWalletSuffixUnloadReleasedV1) : KagemushaWalletSuffixUnloadExecutionV1()
    object Pending : KagemushaWalletSuffixUnloadExecutionV1()
    class RetainedReview internal constructor(val review: KagemushaWalletSuffixUnloadReviewV1,
        val wait: KagemushaWalletReviewMaintenanceWaitV1) : KagemushaWalletSuffixUnloadExecutionV1()
}

internal fun KagemushaWalletSuffixUnloadReplyV1.decision(owner: Any, requestId: ByteArray,
    allowAbsent: Boolean = false): KagemushaWalletSuffixUnloadDecisionV1 {
    checked()
    return when (status) {
        2 -> KagemushaWalletSuffixUnloadDecisionV1.Pending
        67 -> KagemushaWalletSuffixUnloadDecisionV1.Waiting(KagemushaWalletReviewMaintenanceWaitV1.from(detail))
        71 -> KagemushaWalletSuffixUnloadDecisionV1.NotApplicable(detail)
        72 -> {
            val progress = KagemushaWalletSuffixUnloadProgressV1(bytes())
            requireSuffix(progress.requestId().contentEquals(requestId))
            KagemushaWalletSuffixUnloadDecisionV1.Verifying(progress)
        }
        73 -> KagemushaWalletSuffixUnloadDecisionV1.Ready(KagemushaWalletSuffixUnloadReviewV1.fromNative(owner, requestId, this))
        74 -> {
            val summary = KagemushaWalletSuffixUnloadReleasedV1(bytes())
            requireSuffix(summary.requestId().contentEquals(requestId))
            KagemushaWalletSuffixUnloadDecisionV1.Released(summary)
        }
        77 -> {
            requireSuffix(allowAbsent)
            requireSuffixRequestAbsent(bytes(), requestId)
            KagemushaWalletSuffixUnloadDecisionV1.NotStarted
        }
        else -> invalidSuffixOutput()
    }
}

internal fun KagemushaWalletSuffixUnloadReplyV1.execution(owner: Any, review: KagemushaWalletSuffixUnloadReviewV1,
    consumed: Long): KagemushaWalletSuffixUnloadExecutionV1 {
    checked()
    return when (status) {
        2 -> KagemushaWalletSuffixUnloadExecutionV1.Pending
        65 -> KagemushaWalletSuffixUnloadExecutionV1.RetainedReview(review, review.restore(owner, consumed, this))
        74 -> {
            val summary = KagemushaWalletSuffixUnloadReleasedV1(bytes())
            requireSuffix(summary.requestId().contentEquals(review.requestId()) &&
                summary.authorityDigest().contentEquals(review.projection.authorityDigest()) &&
                summary.amount == review.projection.amount && summary.stepCount == review.projection.stepCount &&
                summary.chunkCount == review.projection.chunkCount && summary.instructionCount == review.projection.instructionCount)
            KagemushaWalletSuffixUnloadExecutionV1.Released(summary)
        }
        else -> invalidSuffixOutput()
    }
}
