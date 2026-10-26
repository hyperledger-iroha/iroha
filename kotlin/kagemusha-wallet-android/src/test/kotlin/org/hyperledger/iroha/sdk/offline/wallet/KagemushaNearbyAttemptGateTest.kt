// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import java.nio.ByteBuffer
import java.nio.ByteOrder
import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertSame

/** Component controls only: no native completion, radio, keystore or physical-device claim. */
class KagemushaNearbyAttemptGateTest {
    private fun delivery(
        evidence: KagemushaWalletCreditProjectionV1.Evidence = KagemushaWalletCreditProjectionV1.Evidence.UNFOLDED,
        archive: KagemushaWalletCreditProjectionV1.Archive = KagemushaWalletCreditProjectionV1.Archive.NOT_ARCHIVED,
        corePending: Boolean = true,
    ): KagemushaWalletCreditProjectionV1 {
        val original = if (archive == KagemushaWalletCreditProjectionV1.Archive.RECEIVER) byteArrayOf(9) else byteArrayOf()
        val bytes = ByteBuffer.allocate(92 + original.size).order(ByteOrder.LITTLE_ENDIAN)
            .putShort(1).put((evidence.ordinal + 1).toByte()).put(archive.ordinal.toByte())
            .put(if (corePending) 1.toByte() else 0.toByte()).put(ByteArray(3))
            .put(ByteArray(32) { 1 }).put(ByteArray(32) { 2 }).putLong(1).putLong(0)
            .putInt(original.size).put(original).array()
        return KagemushaWalletCreditProjectionV1(KagemushaWalletCallV1(47, -1, 0, 0, 0, 0, bytes))
    }

    @Test fun acknowledgmentNeedsNativeAdmissionAndPreservesSlowSamples() {
        val gate = KagemushaNearbyAttemptGate(false)
        assertFailsWith<IllegalStateException> { gate.dispatchMarked() }
        assertFailsWith<IllegalStateException> { gate.sendReleased() }
        gate.authorized(100); gate.dispatchMarked(); gate.sendReleased()
        assertFailsWith<IllegalStateException> { gate.creditedArrived(99) }
        gate.creditedArrived(3_000_000_100)
        assertNull(gate.elapsedNanos()); assertNull(gate.arrivalElapsedNanos())
        assertFailsWith<IllegalStateException> { gate.creditedAdmitted(200, delivery()) }
        gate.creditedAdmitted(4_000_000_100, delivery())
        assertEquals(4_000_000_000, gate.elapsedNanos()); assertEquals(3_000_000_000, gate.arrivalElapsedNanos())
        assertFailsWith<IllegalStateException> { gate.creditedArrived(5_000_000_100) }
        assertFailsWith<IllegalStateException> { gate.creditedAdmitted(5_000_000_100, delivery()) }
    }
    @Test fun recoveryCannotInventAuthorizationOrDispatchAgain() {
        val gate = KagemushaNearbyAttemptGate(true)
        assertFailsWith<IllegalStateException> { gate.authorized(1) }
        assertFailsWith<IllegalStateException> { gate.dispatchMarked() }
        gate.sendReleased(); gate.creditedArrived(10); gate.creditedAdmitted(20, delivery())
        assertNull(gate.elapsedNanos()); assertNull(gate.arrivalElapsedNanos())
    }
    @Test fun positiveDeliveryDoesNotWaitForArchiveOrCoreFold() {
        for (evidence in listOf(KagemushaWalletCreditProjectionV1.Evidence.UNFOLDED,
            KagemushaWalletCreditProjectionV1.Evidence.CREDITED)) {
            for (archive in KagemushaWalletCreditProjectionV1.Archive.values().filter {
                it != KagemushaWalletCreditProjectionV1.Archive.RECEIVER
            }) for (pending in listOf(false, true)) for (recovery in listOf(false, true)) {
                val gate = KagemushaNearbyAttemptGate(recovery)
                if (!recovery) { gate.authorized(100); gate.dispatchMarked() }
                gate.sendReleased(); gate.creditedArrived(200)
                gate.creditedAdmitted(300, delivery(evidence, archive, pending))
                assertEquals(if (recovery) null else 200L, gate.elapsedNanos())
                assertEquals(if (recovery) null else 100L, gate.arrivalElapsedNanos())
            }
        }
    }
    @Test fun authenticatedBurnedOrReceiverEvidenceNeverAdmitsPositiveDelivery() {
        val refused = KagemushaWalletCreditProjectionV1.Archive.values().filter {
            it != KagemushaWalletCreditProjectionV1.Archive.RECEIVER
        }.flatMap { archive -> listOf(false, true).map { pending ->
            delivery(KagemushaWalletCreditProjectionV1.Evidence.BURNED, archive, pending)
        }} + delivery(archive = KagemushaWalletCreditProjectionV1.Archive.RECEIVER, corePending = false)
        for (projection in refused) for (recovery in listOf(false, true)) {
            val gate = KagemushaNearbyAttemptGate(recovery)
            if (!recovery) { gate.authorized(100); gate.dispatchMarked() }
            gate.sendReleased(); gate.creditedArrived(200)
            assertFailsWith<IllegalStateException> { gate.creditedAdmitted(300, projection) }
            assertNull(gate.elapsedNanos()); assertNull(gate.arrivalElapsedNanos())
        }
    }
    @Test fun positiveProjectionCannotReplaceArrivalOrNativeSendRelease() {
        val gate = KagemushaNearbyAttemptGate(false)
        assertFailsWith<IllegalStateException> { gate.creditedAdmitted(300, delivery()) }
        gate.authorized(100); gate.dispatchMarked()
        assertFailsWith<IllegalStateException> { gate.creditedArrived(200) }
        assertFailsWith<IllegalStateException> { gate.creditedAdmitted(300, delivery()) }
        gate.sendReleased()
        assertFailsWith<IllegalStateException> { gate.creditedAdmitted(300, delivery()) }
        assertNull(gate.elapsedNanos()); assertNull(gate.arrivalElapsedNanos())
    }
    @Test fun receiveRejectsOtherStatusesBeforeReadingRetainedOrProjectedData() {
        val requestId = ByteArray(32) { 0x31 }
        val statuses = listOf(Int.MIN_VALUE, -15, -1) + (0..59).toList() + Int.MAX_VALUE
        for (recovery in listOf(false, true)) {
            val gate = KagemushaNearbyAttemptGate(recovery)
            for (status in statuses.filter { it != 1 && !(recovery && it == 10) }) {
                var callbacks = 0
                assertFailsWith<IllegalStateException>("status $status, recovery $recovery") {
                    gate.receivedCredit(status, requestId,
                        retainedOutput = { callbacks++ },
                        projection = {
                            callbacks++
                            KagemushaNearbyAttemptGate.ReceivedCredit(true, false, byteArrayOf(0x71))
                        })
                }
                assertEquals(0, callbacks, "status must be admitted before evidence lookup")
            }
        }
    }
    @Test fun collectedReceiveRecoverySkipsRetainedOutputAndReturnsExactFreshBytes() {
        val requestId = ByteArray(32) { 0x32 }
        val fresh = byteArrayOf(0, 0x7f, 0x80.toByte(), 0xff.toByte(), 0)
        val requests = mutableListOf<ByteArray>()
        val actual = KagemushaNearbyAttemptGate(true).receivedCredit(10, requestId,
            retainedOutput = { error("collected Receive no longer has retained output") },
            projection = { selected ->
                requests.add(selected.copyOf())
                KagemushaNearbyAttemptGate.ReceivedCredit(true, false, fresh)
            })
        assertEquals(1, requests.size)
        assertContentEquals(requestId, requests.single())
        assertContentEquals(fresh, actual)
    }
    @Test fun completeReceiveCapturesOriginalReleaseBeforeReadingFreshCredit() {
        val requestId = ByteArray(32) { 0x33 }
        val retainedReceipt = byteArrayOf(0x81.toByte(), 0x82.toByte())
        val fresh = byteArrayOf(0x91.toByte(), 0x92.toByte(), 0x93.toByte())
        for (recovery in listOf(false, true)) {
            val callbacks = mutableListOf<String>()
            var captured: ByteArray? = null
            val actual = KagemushaNearbyAttemptGate(recovery).receivedCredit(1, requestId,
                retainedOutput = { selected ->
                    assertContentEquals(requestId, selected)
                    callbacks.add("retained")
                    captured = retainedReceipt.copyOf()
                },
                projection = { selected ->
                    assertContentEquals(requestId, selected)
                    callbacks.add("projection")
                    KagemushaNearbyAttemptGate.ReceivedCredit(true, false, fresh)
                })
            assertEquals(listOf("retained", "projection"), callbacks)
            assertContentEquals(retainedReceipt, captured)
            assertContentEquals(fresh, actual)
            assertFalse(actual.contentEquals(requireNotNull(captured)), "acknowledgment must use fresh native evidence")
        }
    }
    @Test fun receiveRejectsMissingBurnedAndPayerEvidence() {
        val requestId = ByteArray(32) { 0x34 }
        val invalid = listOf(
            KagemushaNearbyAttemptGate.ReceivedCredit(true, false, null),
            KagemushaNearbyAttemptGate.ReceivedCredit(true, false, byteArrayOf()),
            KagemushaNearbyAttemptGate.ReceivedCredit(true, true, byteArrayOf(0x71)),
            KagemushaNearbyAttemptGate.ReceivedCredit(false, false, byteArrayOf(0x71)),
        )
        for ((status, recovery) in listOf(1 to false, 1 to true, 10 to true)) {
            for (value in invalid) {
                var retainedCalls = 0
                var projectionCalls = 0
                assertFailsWith<IllegalStateException> {
                    KagemushaNearbyAttemptGate(recovery).receivedCredit(status, requestId,
                        retainedOutput = { retainedCalls++ },
                        projection = { selected ->
                            assertContentEquals(requestId, selected)
                            projectionCalls++
                            value
                        })
                }
                assertEquals(if (status == 1) 1 else 0, retainedCalls)
                assertEquals(1, projectionCalls)
            }
        }
    }
    @Test fun receivePropagatesProjectionAndRetainedFailuresWithoutFallback() {
        val requestId = ByteArray(32) { 0x35 }
        val projectionFailure = IllegalArgumentException("projection")
        for ((status, recovery) in listOf(1 to false, 1 to true, 10 to true)) {
            var retainedCalls = 0
            var projectionCalls = 0
            val failure = assertFailsWith<IllegalArgumentException> {
                KagemushaNearbyAttemptGate(recovery).receivedCredit(status, requestId,
                    retainedOutput = { retainedCalls++ },
                    projection = { projectionCalls++; throw projectionFailure })
            }
            assertSame(projectionFailure, failure)
            assertEquals(if (status == 1) 1 else 0, retainedCalls)
            assertEquals(1, projectionCalls)
        }
        val retainedFailure = IllegalArgumentException("retained")
        var projectionCalled = false
        val failure = assertFailsWith<IllegalArgumentException> {
            KagemushaNearbyAttemptGate(false).receivedCredit(1, requestId,
                retainedOutput = { throw retainedFailure },
                projection = {
                    projectionCalled = true
                    KagemushaNearbyAttemptGate.ReceivedCredit(true, false, byteArrayOf(0x71))
                })
        }
        assertSame(retainedFailure, failure)
        assertFalse(projectionCalled, "failed release capture cannot become a successful diagnostic")
    }
    @Test fun receiveSelectionAndNativeOriginalRemainDefensiveCopies() {
        val requestId = ByteArray(32) { 0x36 }
        val expected = requestId.copyOf()
        val original = byteArrayOf(0, 1, 0xff.toByte())
        val expectedOriginal = original.copyOf()
        val evidence = KagemushaNearbyAttemptGate.ReceivedCredit(true, false, original)
        original.fill(0)
        evidence.original()!!.fill(0)
        val actual = KagemushaNearbyAttemptGate(false).receivedCredit(1, requestId,
            retainedOutput = { selected ->
                selected.fill(0)
                requestId.fill(0)
            },
            projection = { selected ->
                assertContentEquals(expected, selected)
                evidence
            })
        assertContentEquals(expectedOriginal, actual)
        actual.fill(0)
        assertContentEquals(expectedOriginal, evidence.original())
    }
    @Test fun conditionalContinuationNeverPublishesAnOriginalAttemptLatency() {
        val gate = KagemushaNearbyAttemptGate(recovery = false, continuation = true)
        gate.authorized(100); gate.dispatchMarked(); gate.sendReleased(); gate.creditedArrived(200)
        gate.creditedAdmitted(300, delivery(corePending = false))
        assertNull(gate.elapsedNanos()); assertNull(gate.arrivalElapsedNanos())
        assertFailsWith<IllegalArgumentException> { KagemushaNearbyAttemptGate(recovery = true, continuation = true) }
    }

}
