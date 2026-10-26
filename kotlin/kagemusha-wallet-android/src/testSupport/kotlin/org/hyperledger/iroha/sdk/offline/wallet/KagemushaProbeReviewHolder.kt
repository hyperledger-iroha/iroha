// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

/** One live actual capability shared by the existing monetary and carrier command owners. */
internal class KagemushaProbeReviewAttempt(
    val review: KagemushaWalletReviewV1, val origin: KagemushaWalletExchangeV1,
    val wallet: KagemushaWalletV1, val sequence: Int, val fence: String, val continuation: Boolean,
) {
    val requestId = review.requestId()
    val phase = KagemushaProbeReviewPhase()
}
internal class KagemushaProbeReviewHolder {
    var current: KagemushaProbeReviewAttempt? = null
        private set
    fun retain(value: KagemushaProbeReviewAttempt) { check(current == null); current = value }
    fun clear(value: KagemushaProbeReviewAttempt) { check(current === value); current = null }
    fun released() { current?.phase?.finish(); current = null }
    /** Failure anywhere in the command, including transport teardown, retires only its review.
     * Callback seams exercise the same holder without entering Native or device custody. */
    fun <T> command(
        origin: KagemushaWalletExchangeV1?, sequence: Int,
        discard: (KagemushaProbeReviewAttempt) -> Unit = { it.origin.discardReview(it.review) },
        retire: (KagemushaProbeReviewAttempt) -> Unit = { it.wallet.close() },
        body: () -> T,
    ): T = try { body() } catch (failure: Throwable) {
        val pending = current
        if (pending != null && pending.origin === origin && pending.sequence == sequence) {
            clear(pending)
            pending.phase.finish()
            try { discard(pending) } catch (cleanup: Throwable) {
                if (cleanup !is KagemushaWalletExceptionV1 || cleanup.status != -2) {
                    if (cleanup !== failure) failure.addSuppressed(cleanup)
                    try { retire(pending) } catch (retirement: Throwable) {
                        if (retirement !== failure) failure.addSuppressed(retirement)
                    }
                }
            }
        }
        throw failure
    }
    fun endRequested(origin: KagemushaWalletExchangeV1): KagemushaProbeReviewAttempt? {
        val selected = current ?: return null
        check(selected.origin === origin)
        selected.phase.endRequested(); selected.phase.finish(); current = null
        return selected
    }
}

