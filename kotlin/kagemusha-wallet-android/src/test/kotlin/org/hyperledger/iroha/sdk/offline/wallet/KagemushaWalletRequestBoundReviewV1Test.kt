// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

/** Pure managed route/custody checks; these frames confer no Native financial authority. */
class KagemushaWalletRequestBoundReviewV1Test {
    private fun id() = ByteArray(32) { 1 }
    private fun projection() = ByteArray(496).also { bytes ->
        byteArrayOf(75, 87, 79, 82, 86, 49, 0, 0).copyInto(bytes)
        bytes[8] = 1; bytes[9] = 1; bytes[73] = 1; bytes[74] = 1; bytes[138] = 1
        for (at in listOf(106, 202, 234, 266, 298, 330, 362, 394)) bytes[at] = 1
        bytes[426] = 4; bytes[427] = 1; bytes[491] = 1; bytes[495] = 7
    }
    private fun reply(kind: Int, bytes: ByteArray = byteArrayOf(), low: Long = 0,
        high: Long = 0, detail: Int = 0) = KagemushaWalletCallV1(kind, -1, 0, low, high, detail, bytes)
    private fun decision(kind: Int, bytes: ByteArray = byteArrayOf(), low: Long = 0,
        high: Long = 0, detail: Int = 0, owner: Any = Any(), identity: ByteArray = id()) =
        KagemushaWalletReviewReplyV1(kind, -1, 0, low, high, detail, bytes)
            .decision(owner, identity, KagemushaWalletReviewProjectionV1.Kind.SEND)
    private fun ready(owner: Any, identity: ByteArray = id()) =
        (decision(18, projection(), 7, owner = owner, identity = identity) as KagemushaWalletReviewDecisionV1.Ready).review
    private fun nested(kind: Int, payload: ByteArray = byteArrayOf(), detail: Int = 0,
        low: Long = 0, high: Long = 0) = ByteArray(28 + payload.size).also { bytes ->
        fun word(at: Int, value: Long, width: Int) { repeat(width) { bytes[at + it] = (value ushr (8 * it)).toByte() } }
        word(0, kind.toLong(), 4); word(4, detail.toLong(), 4); word(8, low, 8); word(16, high, 8)
        word(24, payload.size.toLong(), 4); payload.copyInto(bytes, 28)
    }
    private fun refuses(action: () -> Unit) { assertThrows(KagemushaWalletExceptionV1::class.java) { action() } }

    @Test fun originalIdentityAndExpectationAreCopiedAtIssuance() {
        val originalId = id(); val bytes = projection(); val expected = KagemushaWalletReviewExpectationV1.OriginalUnstarted(bytes)
        val input = KagemushaWalletReviewInputV1(originalId, 1, expected, first = byteArrayOf(2), second = byteArrayOf(3))
        val owner = Any(); val review = ready(owner, originalId)
        originalId.fill(0); bytes.fill(0); input.requestId().fill(0); review.requestId().fill(0); expected.bytes().fill(0)
        assertArrayEquals(id(), input.requestId()); assertArrayEquals(id(), review.requestId())
        assertArrayEquals(projection(), expected.bytes()); assertEquals(1, expected.tag)
        assertEquals(0, KagemushaWalletReviewExpectationV1.Fresh.tag)
        assertEquals(0, KagemushaWalletReviewExpectationV1.Fresh.bytes().size)
        assertEquals(7L, review.consume(owner))
        for (invalid in listOf(ByteArray(31), ByteArray(32), ByteArray(33))) {
            assertThrows(IllegalArgumentException::class.java) {
                KagemushaWalletReviewInputV1(invalid, 1, expected, first = byteArrayOf(2), second = byteArrayOf(3))
            }
        }
    }
    @Test fun oldHashAndJsonFenceBodiesCannotBecomeConditionalApproval() {
        for (old in listOf(ByteArray(64) { 'a'.code.toByte() },
            "{\"kind\":\"SEND\",\"reviewSHA256\":\"abc\"}".toByteArray())) {
            refuses { KagemushaWalletReviewExpectationV1.OriginalUnstarted(old) }
        }
        assertThrows(IllegalArgumentException::class.java) {
            KagemushaWalletReviewInputV1(id(), 8, KagemushaWalletReviewExpectationV1.OriginalUnstarted(projection()),
                KagemushaWalletUInt128V1(1, 0))
        }
    }
    @Test fun existingPreservesAllCurrentCompletionBitsAndExactPayload() {
        for (kind in listOf(1, 2, 3, 4, 5, 10)) {
            val payload = if (kind == 1 || kind == 10) byteArrayOf(3, 4) else byteArrayOf()
            val bytes = nested(kind, payload, low = -1, high = Long.MIN_VALUE)
            val result = decision(66, bytes) as KagemushaWalletReviewDecisionV1.Existing
            bytes.fill(0)
            val value = (result.status as KagemushaWalletBoundRequestStatusV1.Outcome).completion
            assertEquals(kind, value.status); assertEquals(-1L, value.sequenceLow); assertEquals(Long.MIN_VALUE, value.sequenceHigh)
            assertArrayEquals(payload, value.bytes()); value.bytes().fill(0); assertArrayEquals(payload, value.bytes())
        }
        val result = decision(66, nested(1, ByteArray(10_034) { 8 })) as KagemushaWalletReviewDecisionV1.Existing
        assertEquals(10_034, (result.status as KagemushaWalletBoundRequestStatusV1.Outcome).completion.bytes().size)
        assertSame(KagemushaWalletBoundRequestStatusV1.Preparing,
            (decision(66, nested(11)) as KagemushaWalletReviewDecisionV1.Existing).status)
    }
    @Test fun existingRejectsUnknownControlsNestingAndNonexactHeader() {
        for (kind in listOf(0, 6, 7, 8, 9, 18, 47, 64, 65, 66, 67, -1)) refuses { decision(66, nested(kind)) }
        for (bytes in listOf(ByteArray(27), ByteArray(10_029), nested(2) + byteArrayOf(0),
            nested(1, byteArrayOf(1)).copyOf(28), nested(2).also { it[27] = -1 },
            nested(11, low = 1), nested(11, high = 1), nested(11, detail = 1), nested(11, byteArrayOf(1)))) {
            refuses { decision(66, bytes) }
        }
        refuses { decision(66, nested(2), low = 1) }; refuses { decision(66, nested(2), high = 1) }
        refuses { decision(66, nested(2), detail = 1) }; refuses { reply(66, nested(2)).completion() }
    }
    @Test fun waitingIsTokenlessIssuanceOnlyAndNeverRestoresExecution() {
        for ((tag, wait) in listOf(1 to KagemushaWalletReviewMaintenanceWaitV1.SELECTED_PENDING,
            2 to KagemushaWalletReviewMaintenanceWaitV1.ARCHIVE_FOLD, 6 to KagemushaWalletReviewMaintenanceWaitV1.BUSY)) {
            assertEquals(wait, (decision(67, detail = tag) as KagemushaWalletReviewDecisionV1.Waiting).wait)
            val owner = Any(); val review = ready(owner); val token = review.consume(owner)
            refuses { reply(67, detail = tag).reviewedExecution(owner, review, token) }
            refuses { review.consume(owner) }
            refuses { decision(65, low = 7, detail = tag) }
            refuses { decision(67, low = 7, detail = tag) }; refuses { decision(67, high = 1, detail = tag) }
            refuses { decision(67, byteArrayOf(1), detail = tag) }
        }
        for (tag in listOf(-1, 0, 3, 4, 5, 7)) refuses { decision(67, detail = tag) }
    }
    @Test fun retainedExecutionKeepsSameActualReviewAndOneUseIdentity() {
        val owner = Any(); val review = ready(owner); val token = review.consume(owner)
        refuses { reply(65, low = 8, detail = 1).reviewedExecution(owner, review, token) }
        val retained = reply(65, low = token, detail = 2).reviewedExecution(owner, review, token) as KagemushaWalletReviewedExecutionV1.RetainedReview
        assertSame(review, retained.review); assertArrayEquals(id(), retained.review.requestId())
        assertEquals(KagemushaWalletReviewMaintenanceWaitV1.ARCHIVE_FOLD, retained.wait)
        assertThrows(IllegalStateException::class.java) { review.consume(Any()) }
        assertEquals(token, review.consume(owner))
        val existing = reply(66, nested(11)).reviewedExecution(owner, review, token) as KagemushaWalletReviewedExecutionV1.Existing
        assertSame(KagemushaWalletBoundRequestStatusV1.Preparing, existing.status)
        refuses { review.consume(owner) }
    }
    @Test fun approvedLiveRetryUsesTheSameCapabilityWithoutNewConfirmationOrPublication() {
        val owner = Any(); val review = ready(owner); val phase = KagemushaProbeReviewPhase()
        var confirmations = 0; var publications = 0; var fence = byteArrayOf()
        phase.beginConfirmation(); confirmations++
        phase.publishFence { publications++; fence = review.projection.bytes() }
        val token = review.consume(owner)
        val retained = reply(65, low = token, detail = 6).reviewedExecution(owner, review, token) as KagemushaWalletReviewedExecutionV1.RetainedReview
        assertSame(review, retained.review); phase.retained()
        assertThrows(IllegalStateException::class.java) { phase.beginConfirmation() }
        phase.retry { assertArrayEquals(review.projection.bytes(), fence) }
        assertEquals(token, review.consume(owner))
        val result = reply(66, nested(2)).reviewedExecution(owner, review, token) as KagemushaWalletReviewedExecutionV1.Existing
        assertEquals(2, (result.status as KagemushaWalletBoundRequestStatusV1.Outcome).completion.status)
        phase.finish()
        assertEquals(1, confirmations); assertEquals(1, publications)
        assertThrows(IllegalStateException::class.java) { phase.retry {} }
        refuses { review.consume(owner) }
    }
    @Test fun ordinaryExecutionNeverAcceptsUnknownAndPreservesOperationalFailures() {
        val owner = Any(); val review = ready(owner); val token = review.consume(owner)
        for (kind in listOf(0, 6, 11)) refuses { reply(kind).reviewedExecution(owner, review, token) }
        val outcome = reply(1, byteArrayOf(9)).reviewedExecution(owner, review, token) as KagemushaWalletReviewedExecutionV1.Executed
        assertArrayEquals(byteArrayOf(9), outcome.completion.bytes())
        val failure = assertThrows(KagemushaWalletExceptionV1::class.java) {
            KagemushaWalletCallV1(-4, 3, 7, 0, 0, 0, byteArrayOf()).reviewedExecution(owner, review, token)
        }
        assertEquals(-4, failure.status); assertEquals(3, failure.reason); assertEquals(7, failure.platformCode)
        refuses { review.consume(owner) }
    }
}
