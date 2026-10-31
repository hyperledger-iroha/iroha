// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertSame

/** Managed holder faults only; no Native, radio, payment key or monetary qualification. */
class KagemushaProbeReviewHolderTest {
    private fun retained(): Pair<KagemushaProbeReviewHolder, KagemushaProbeReviewAttempt> {
        val wallet = KagemushaWalletV1(1)
        val origin = KagemushaWalletExchangeV1(wallet, 1) { KagemushaWalletExchangeEndV1.ENDED }
        val bytes = ByteArray(496).also { value ->
            byteArrayOf(75, 87, 79, 82, 86, 49, 0, 0).copyInto(value)
            value[8] = 1; value[9] = 1; value[73] = 1; value[74] = 1; value[138] = 1
            for (at in listOf(106, 202, 234, 266, 298, 330, 362, 394)) value[at] = 1
            value[426] = 4; value[427] = 1; value[491] = 1; value[495] = 7
        }
        val review = KagemushaWalletReviewV1.fromNative(origin, 7, ByteArray(32) { 1 },
            KagemushaWalletReviewProjectionV1(bytes))
        val attempt = KagemushaProbeReviewAttempt(review, origin, wallet, 12, "original-fence", false)
        val holder = KagemushaProbeReviewHolder().also { it.retain(attempt) }
        attempt.phase.beginConfirmation(); attempt.phase.publishFence {}
        val token = review.consume(origin)
        val result = KagemushaWalletCallV1(65, -1, 0, token, 0, 2, byteArrayOf())
            .reviewedExecution(origin, review, token) as KagemushaWalletReviewedExecutionV1.RetainedReview
        assertSame(review, result.review)
        attempt.phase.retained()
        return holder to attempt
    }

    @Test fun radioTeardownFailureRetiresTheSameRetainedReviewBeforeTheNextCommand() {
        val (holder, attempt) = retained()
        val failure = IllegalStateException("radio teardown")
        var discards = 0; var retirements = 0
        val actual = assertFailsWith<IllegalStateException> {
            holder.command(attempt.origin, attempt.sequence,
                discard = { selected ->
                    assertSame(attempt, selected); assertNull(holder.current)
                    assertEquals(KagemushaProbeReviewPhase.State.FINISHED, selected.phase.state)
                    assertEquals(7L, selected.review.consume(selected.origin)); discards++
                }, retire = { retirements++ }) {
                try { "native-review-retained-ARCHIVE_FOLD" } finally { throw failure }
            }
        }
        assertSame(failure, actual); assertEquals(1, discards); assertEquals(0, retirements)
        assertNull(holder.current)
        assertFailsWith<IllegalStateException> { attempt.phase.retry {} }
        assertFailsWith<KagemushaWalletExceptionV1> { attempt.review.consume(attempt.origin) }
    }

    @Test fun cleanupUncertaintyPreservesPrimaryFailureAndCannotRestoreTheHolder() {
        val (holder, attempt) = retained()
        val primary = IllegalStateException("radio teardown")
        val discard = IllegalArgumentException("discard")
        val retirement = IllegalStateException("wallet cleanup")
        var retirements = 0
        val actual = assertFailsWith<IllegalStateException> {
            holder.command(attempt.origin, attempt.sequence,
                discard = { throw discard }, retire = { selected ->
                    assertSame(attempt, selected); assertNull(holder.current); retirements++; throw retirement
                }) { throw primary }
        }
        assertSame(primary, actual); assertEquals(listOf(discard, retirement), actual.suppressed.toList())
        assertEquals(1, retirements); assertNull(holder.current)
        assertEquals(KagemushaProbeReviewPhase.State.FINISHED, attempt.phase.state)
    }

    @Test fun successfulTeardownPreservesExplicitRetryWithoutConfirmationOrRepublication() {
        val (holder, attempt) = retained()
        val fence = attempt.review.projection.bytes()
        var discards = 0; var retirements = 0; var teardowns = 0
        val result = holder.command(attempt.origin, attempt.sequence,
            discard = { discards++ }, retire = { retirements++ }) {
            try { "native-review-retained-ARCHIVE_FOLD" } finally { teardowns++ }
        }
        assertEquals("native-review-retained-ARCHIVE_FOLD", result); assertEquals(1, teardowns)
        assertEquals(0, discards); assertEquals(0, retirements); assertSame(attempt, holder.current)
        assertEquals(KagemushaProbeReviewPhase.State.APPROVED_LIVE_RETAINED, attempt.phase.state)
        assertFailsWith<IllegalStateException> { attempt.phase.beginConfirmation() }
        attempt.phase.retry { assertContentEquals(fence, attempt.review.projection.bytes()) }
        assertEquals(7L, attempt.review.consume(attempt.origin))
    }

    @Test fun foreignOriginOrCommandFailureNeverConsumesAnotherAttempt() {
        for (foreignOrigin in listOf(false, true)) {
            val (holder, attempt) = retained()
            val origin = if (foreignOrigin) KagemushaWalletExchangeV1(attempt.wallet, 2) {
                KagemushaWalletExchangeEndV1.ENDED
            } else attempt.origin
            val sequence = if (foreignOrigin) attempt.sequence else attempt.sequence + 1
            var cleanups = 0
            assertFailsWith<IllegalStateException> {
                holder.command(origin, sequence, discard = { cleanups++ }, retire = { cleanups++ }) {
                    error("another command")
                }
            }
            assertEquals(0, cleanups); assertSame(attempt, holder.current)
            assertEquals(KagemushaProbeReviewPhase.State.APPROVED_LIVE_RETAINED, attempt.phase.state)
            assertEquals(7L, attempt.review.consume(attempt.origin))
        }
    }
}
