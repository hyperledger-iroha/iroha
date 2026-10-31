// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import java.io.Closeable

/** One Native logical exchange across UI, network and worker gaps.
 * Admit synchronously before enqueue; finish each accepted task after its last result.
 * End on a worker after finishing this caller's tasks. Native owns Archive admission. */
class KagemushaWalletExchangeV1 internal constructor(
    private val wallet: KagemushaWalletV1,
    internal val token: Long,
    endNative: () -> KagemushaWalletExchangeEndV1,
) : Closeable, KagemushaWalletCleanupResourceV1 {
    private val lifetime = KagemushaWalletExchangeLifetimeV1(endNative)
    init { require(token > 0) }

    /** Acquire local task custody before an actor, coroutine, worker or UI enqueue. */
    fun admit(): KagemushaWalletExchangeCallV1 {
        wallet.exchangeHandle()
        return KagemushaWalletExchangeCallV1(wallet, this, lifetime.admit())
    }

    /** Fence synchronously before enqueueing an off-UI end/join task. */
    fun requestEnd() = lifetime.requestEnd()

    /** Fence new work, join accepted tasks, then end this exact Native exchange. */
    fun end(): KagemushaWalletExchangeEndV1 = try {
        lifetime.end().also { result ->
            if (result == KagemushaWalletExchangeEndV1.ENDING) {
                KagemushaWalletInstalledRuntimeV1.retainFailure(this,
                    IllegalStateException("Native exchange cleanup remains pending"))
            }
        }
    } catch (failure: Throwable) {
        KagemushaWalletInstalledRuntimeV1.retainFailure(this, failure)
        throw failure
    }
    override fun close() {
        check(end() != KagemushaWalletExchangeEndV1.ENDING) { "Native exchange cleanup remains pending" }
    }
    /** Retry the exact same Native exchange; this never creates replacement admission. */
    override fun retryCleanup() { close() }
    override fun cleanupReleased(): Boolean = lifetime.released() || wallet.cleanupReleased()

    /** Discard an unused exact-origin review, including after the end fence. */
    fun discardReview(review: KagemushaWalletReviewV1) {
        val owner = wallet.exchangeCleanupHandle()
        val reviewToken = review.consume(this)
        KagemushaWalletExchangeCleanupV1.run(wallet::close) {
            val result = KagemushaWalletNativeCallsV1.discardReview(owner, token, reviewToken)
            if (result != 0) throw KagemushaWalletExceptionV1(result)
        }
    }

    /** Discard an unanswered challenge; no new work is admitted after the end fence. */
    fun cancelTimeExchange(exchange: KagemushaWalletTimeExchangeV1) {
        val input = KagemushaWalletSetupInputV1(6, token = exchange.tokenFor(this))
        wallet.exchangeCleanupHandle()
        exchange.consume(this)
        KagemushaWalletExchangeCleanupV1.run(wallet::close) { wallet.deletionSetup(input, token).idle() }
    }

    /** Discard an unused deletion review belonging to this exact exchange. */
    fun discardCustodyDeletionReview(review: KagemushaWalletDeletionReviewV1) =
        wallet.deletionGate.discard(review, this) { reviewToken ->
            KagemushaWalletExchangeCleanupV1.run(wallet::close) {
                wallet.deletionSetup(KagemushaWalletSetupInputV1(51, token = reviewToken), token).also {
                    if (it.status != KagemushaWalletCallV1.DELETION_REVIEW_DISCARDED) {
                        throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
                    }
                }
            }
        }
    override fun toString(): String = "KagemushaWalletExchangeV1(owner=[REDACTED])"
}

/** After local consumption an ambiguous cleanup cannot restore a one-use capability.
 * Retire the exact wallet instead; its existing failed-close quarantine retains custody. */
internal object KagemushaWalletExchangeCleanupV1 {
    fun <T> run(close: () -> Unit, action: () -> T): T = try { action() }
    catch (failure: Throwable) {
        try { close() } catch (cleanup: Throwable) { if (cleanup !== failure) failure.addSuppressed(cleanup) }
        throw failure
    }
}
