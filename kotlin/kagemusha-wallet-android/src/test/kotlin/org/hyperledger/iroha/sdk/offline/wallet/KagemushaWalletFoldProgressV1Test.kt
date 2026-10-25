package org.hyperledger.iroha.sdk.offline.wallet

import java.math.BigInteger
import kotlin.test.*
import org.junit.jupiter.api.Test

/** Fold telemetry readings only; no owner, proof or readiness is involved. */
class KagemushaWalletFoldProgressV1Test {
    private class Row(val started: Long, val ended: Long, val sequence: Long, val operation: String?,
        val stage: String?, val ordinal: Long, val stages: Long?, val outcome: String)

    private val second = 1_000_000L

    private fun row(startedSeconds: Long, endedSeconds: Long, sequence: Long, operation: String?, stage: String?,
        ordinal: Long, stages: Long?, outcome: String) = Row(startedSeconds * second, endedSeconds * second, sequence,
        operation, stage, ordinal, stages, outcome)

    private fun name(value: String?) = value?.let { "\"$it\"" } ?: "null"

    // The shape Native's FoldMetricsV1::to_json writes, including the fields a progress reading ignores.
    private fun telemetry(sampledSeconds: Long, vararg rows: Row, schema: String = KagemushaWalletFoldProgressV1.SCHEMA) =
        "{\"schema\":\"$schema\",\"sampledNativeMicros\":${sampledSeconds * second},\"counts\":{\"subProofs\":${rows.size}}," +
            "\"processScratch\":{\"limitBytes\":1,\"inUseBytes\":0,\"peakBytes\":0},\"processPeakRssBytes\":null,\"records\":[" +
            rows.joinToString(",") {
                "{\"startedNativeMicros\":${it.started},\"endedNativeMicros\":${it.ended},\"sequence\":${it.sequence}," +
                    "\"operation\":${name(it.operation)},\"stage\":${name(it.stage)},\"ordinal\":${it.ordinal}," +
                    "\"stages\":${it.stages ?: "null"},\"outcome\":\"${it.outcome}\",\"cause\":null,\"ingests\":0," +
                    "\"peakRssBytes\":null,\"micros\":{\"prove\":${it.ended - it.started}}}"
            } + "],\"nativeWorkCount\":0,\"nativeWork\":[]}"

    private fun progress(sampledSeconds: Long, vararg rows: Row) =
        KagemushaWalletFoldProgressV1.parse(telemetry(sampledSeconds, *rows))

    private val loadThenReceive = arrayOf(
        row(0, 20, 1, "load", "q", 0, 3, "checkpoint"),
        row(20, 40, 1, "load", "a", 1, 3, "checkpoint"),
        row(40, 60, 1, "load", "omega", 2, 3, "folded"),
        row(60, 90, 2, "receive", "q", 0, 12, "checkpoint"),
    )

    private fun head(value: Long) = BigInteger.valueOf(value)

    @Test fun learnsTheHeadSchedulesAndPaceFromItsTurns() {
        val reading = progress(100, *loadThenReceive)
        assertEquals(100 * second, reading.sampledNativeMicros)
        assertEquals(head(2), reading.head)
        assertEquals(mapOf("load" to 3, "receive" to 12), reading.schedules)
        assertEquals(22_500_000L, reading.subProofMicros)
        assertNull(progress(10).head)
    }

    @Test fun countsTheCurrentHeadAndEveryHeadFoldedSinceTheWaitBegan() {
        val reading = progress(100, *loadThenReceive)
        // The Receive head's own turn says it needs twelve; no schedule passed in names receive.
        val schedules = mapOf("load" to 3)
        val whole = assertNotNull(reading.estimate(backlog = 1, running = true, firstHead = head(1), schedules = schedules))
        assertEquals(4L, whole.proved)
        assertEquals(11L, whole.remaining)
        assertEquals(4.0 / 9.0, whole.runningShare, 1e-9)
        assertEquals((4 + 4.0 / 9.0) / 15, whole.fraction, 1e-9)
        assertEquals(237_500_000L, whole.remainingMicros)
        // A wait that began after the Load folded counts the Receive head alone.
        val later = assertNotNull(reading.estimate(backlog = 1, running = true, firstHead = head(2), schedules = schedules))
        assertEquals(1L, later.proved)
        assertEquals((1 + 4.0 / 9.0) / 12, later.fraction, 1e-9)
        // A parked worker spends nothing on a sub-proof.
        val parked = assertNotNull(reading.estimate(backlog = 1, running = false, firstHead = head(1), schedules = schedules))
        assertEquals(0.0, parked.runningShare)
        assertEquals(4.0 / 15, parked.fraction, 1e-9)
    }

    @Test fun aWaitThatBeganMidHeadNeverDropsThatHeadsEarlierSubProofs() {
        val schedules = mapOf("load" to 3, "send" to 5)
        val loading = arrayOf(row(0, 20, 1, "load", "q", 0, 3, "checkpoint"), row(20, 40, 1, "load", "a", 1, 3, "checkpoint"))
        // The wait begins with two of the Load head's three sub-proofs done and a Send head behind it.
        val start = assertNotNull(progress(50, *loading).estimate(2, false, head(1), schedules))
        assertEquals(0.25, start.fraction, 1e-9)
        val sending = progress(80, *loading, row(40, 60, 1, "load", "omega", 2, 3, "folded"),
            row(60, 80, 2, "send", "q", 0, 5, "checkpoint"))
        val after = assertNotNull(sending.estimate(1, false, head(1), schedules))
        assertEquals(4L, after.proved)
        assertEquals(0.5, after.fraction, 1e-9)
    }

    @Test fun aFoldedHeadMovesToTheNextHeadAtTheLargestSchedule() {
        val reading = progress(55,
            row(0, 10, 5, "send", "q", 0, 5, "checkpoint"),
            row(10, 20, 5, "send", "a", 1, 5, "checkpoint"),
            row(20, 30, 5, "send", "w", 2, 5, "checkpoint"),
            row(30, 40, 5, "send", "a", 3, 5, "checkpoint"),
            row(40, 50, 5, "send", "omega", 4, 5, "folded"))
        assertEquals(mapOf("send" to 5), reading.schedules)
        assertEquals(head(6), reading.head)
        val estimate = assertNotNull(reading.estimate(2, false, head(5), mapOf("send" to 5, "archive_sent" to 20)))
        assertEquals(5L, estimate.proved)
        assertEquals(40L, estimate.remaining)
        assertEquals(5.0 / 45, estimate.fraction, 1e-9)
        assertEquals(400 * second, estimate.remainingMicros)
    }

    @Test fun aHeadsOwnScheduleIsExactWhenItsOperationRanLongerBefore() {
        // ArchiveSent folds in 17 or 20 sub-proofs; the head's own turn says which.
        val reading = progress(100, row(0, 10, 8, "archive_sent", "a", 8, 17, "checkpoint"))
        val estimate = assertNotNull(reading.estimate(2, false, head(8), mapOf("archive_sent" to 20)))
        assertEquals(9L, estimate.proved)
        assertEquals(8L + 20, estimate.remaining) // this head's other eight, the next head at the largest
        assertEquals(9.0 / 37, estimate.fraction, 1e-9)
    }

    @Test fun aCancelledTurnStillSaysWhereItsHeadStands() {
        val reading = progress(80,
            row(0, 20, 7, "receive", "q", 0, 12, "checkpoint"),
            row(20, 40, 7, "receive", "q", 1, 12, "checkpoint"),
            row(40, 60, 7, "receive", "a", 2, 12, "checkpoint"),
            row(60, 70, 7, "receive", "w", 3, 12, "cancelled"),
            // Cancelled before its released step was read: the head's earlier turns name it and its schedule.
            row(70, 70, 7, null, null, 3, null, "cancelled"))
        val estimate = assertNotNull(reading.estimate(1, false, head(7), emptyMap()))
        assertEquals(3L, estimate.proved)
        assertEquals(9L, estimate.remaining)
        assertEquals(0.25, estimate.fraction, 1e-9)
    }

    @Test fun aHeadPastItsKnownScheduleIsNeverCountedDone() {
        val reading = progress(30, row(0, 20, 9, "receive", "a", 12, null, "checkpoint"))
        val estimate = assertNotNull(reading.estimate(1, false, head(9), mapOf("receive" to 12)))
        assertEquals(13L, estimate.proved)
        assertEquals(1L, estimate.remaining)
        assertEquals(13.0 / 14, estimate.fraction, 1e-9)
    }

    @Test fun onlyTheSnapshotCompletesAWait() {
        val reading = progress(1000, row(0, 10, 3, "x", "a", 998, 1000, "checkpoint"))
        val estimate = assertNotNull(reading.estimate(1, true, head(3), emptyMap()))
        assertEquals(KagemushaWalletFoldProgressV1.RUNNING_SHARE_MAX, estimate.runningShare)
        assertEquals(KagemushaWalletFoldProgressV1.FRACTION_MAX, estimate.fraction)
    }

    @Test fun adoptedTurnsAreDurableButOnlyAnOmegaFoldsAHead() {
        val reading = progress(50,
            row(0, 1, 4, "load", null, 0, 3, "adopted"),
            row(1, 2, 4, "load", "omega", 2, 3, "adopted"),
            row(2, 3, 5, "unload", null, 1, 3, "adopted"))
        // Adopted bytes were never proved here: they set no pace.
        assertNull(reading.subProofMicros)
        assertEquals(mapOf("load" to 3, "unload" to 3), reading.schedules)
        val estimate = assertNotNull(reading.estimate(1, true, head(4), emptyMap()))
        assertEquals(0.0, estimate.runningShare)
        assertNull(estimate.remainingMicros)
        assertEquals(5L, estimate.proved) // load's three sub-proofs, from its Ω, and unload's first two
        assertEquals(1L, estimate.remaining)
    }

    @Test fun nothingKnownIsNoEstimate() {
        assertNull(progress(10).estimate(1, true, head(1), mapOf("load" to 3)))
        // Cancelled after its step was read but before its schedule: no size for the head.
        val unsized = progress(30, row(0, 20, 1, "load", null, 0, null, "cancelled"))
        assertNull(unsized.estimate(1, true, head(1), emptyMap()))
        assertNull(unsized.estimate(1, true, head(1), mapOf("load" to 0)))
        // A sized head says nothing about the heads behind it.
        val sized = progress(30, row(0, 20, 1, "load", "q", 0, 3, "checkpoint"))
        assertNull(sized.estimate(2, true, head(1), emptyMap()))
        assertNull(sized.estimate(0, true, head(1), mapOf("load" to 3)))
    }

    @Test fun manyLaterHeadsStayExact() {
        val reading = progress(30, row(0, 20, 1, "receive", "q", 0, 12, "checkpoint"))
        val estimate = assertNotNull(reading.estimate(Long.MAX_VALUE, false, head(1), mapOf("receive" to 12)))
        assertTrue(estimate.remaining > 0)
        assertTrue(estimate.fraction in 0.0..KagemushaWalletFoldProgressV1.FRACTION_MAX)
    }

    @Test fun refusesMalformedOrForeignTelemetry() {
        val good = row(0, 20, 1, "load", "q", 0, 3, "checkpoint")
        val inputs = listOf(
            "", "[]", "{}",
            // A Native without schedules in its records.
            telemetry(30, good, schema = "iroha.kagemusha.fold-metrics.v2").replace(",\"stages\":3", ""),
            telemetry(30, good, schema = "iroha.kagemusha.fold-metrics.v2"),
            telemetry(10, good), // ended after the sample
            telemetry(30, row(20, 10, 1, "load", "q", 0, 3, "checkpoint")),
            telemetry(30, row(0, 20, 1, "load", "q", 0, 3, "finished")),
            telemetry(30, row(0, 20, 1, "load", "q", 4_294_967_296, 3, "checkpoint")),
            telemetry(30, row(0, 20, 1, "load", "q", 0, 0, "checkpoint")),
            telemetry(30, row(0, 20, 1, "load", "q", 0, 4_294_967_296, "checkpoint")),
            telemetry(30, good).replace(",\"stages\":3", ""),
            telemetry(30, good).replace("\"stages\":3", "\"stages\":\"3\""),
            telemetry(30, good).replace("\"stages\":3", "\"stages\":-3"),
            telemetry(30, good).replace("\"sequence\":1", "\"sequence\":\"1\""),
            telemetry(30, good).replace("\"sequence\":1", "\"sequence\":-1"),
            telemetry(30, good).replace("\"operation\":\"load\"", "\"operation\":7"),
            telemetry(30, good).replace("\"records\":[", "\"records\":{\"x\":["),
            telemetry(30, good).replace(",\"outcome\":\"checkpoint\"", ""),
        )
        for (input in inputs) {
            val failure = assertFailsWith<KagemushaWalletExceptionV1>(input) { KagemushaWalletFoldProgressV1.parse(input) }
            assertEquals(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT, failure.status)
        }
    }

    @Test fun theLargestScheduleNativeCanReportStillCounts() {
        val reading = progress(30, row(0, 20, 1, "load", "q", 0, 4_294_967_295, "checkpoint"))
        val estimate = assertNotNull(reading.estimate(1, false, head(1), emptyMap()))
        assertEquals(1L, estimate.proved)
        assertEquals(4_294_967_294L, estimate.remaining)
    }
}
