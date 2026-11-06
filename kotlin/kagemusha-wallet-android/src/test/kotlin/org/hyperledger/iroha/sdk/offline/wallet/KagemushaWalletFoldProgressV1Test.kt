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

    private fun micros(seconds: Double) = seconds * second

    @Test fun learnsTheHeadSchedulesAndCostsFromItsTurns() {
        val reading = progress(100, *loadThenReceive)
        assertEquals(100 * second, reading.sampledNativeMicros)
        assertEquals(head(2), reading.head)
        assertEquals(mapOf("load" to 3, "receive" to 12), reading.schedules)
        assertEquals(mapOf("load" to mapOf(0L to 20 * second, 1L to 20 * second, 2L to 20 * second),
            "receive" to mapOf(0L to 30 * second)), reading.costs)
        assertNull(progress(10).head)
        assertEquals(emptyMap(), progress(10).costs)
        // An ordinal proved twice costs its mean; adopted, cancelled and instant turns proved nothing here.
        val sends = progress(100,
            row(0, 10, 1, "send", "q", 0, 5, "checkpoint"),
            row(10, 30, 2, "send", "q", 0, 5, "checkpoint"),
            row(30, 31, 3, "send", "q", 0, 5, "adopted"),
            row(31, 40, 3, "send", "a", 1, 5, "cancelled"),
            row(40, 40, 3, "send", "a", 1, 5, "checkpoint"))
        assertEquals(mapOf("send" to mapOf(0L to 15 * second)), sends.costs)
    }

    @Test fun weighsTheCurrentHeadAndEveryHeadFoldedSinceTheWaitBeganByTime() {
        val reading = progress(100, *loadThenReceive)
        // The Receive head's own turn says it needs twelve; no schedule passed in names receive.
        val schedules = mapOf("load" to 3)
        val whole = assertNotNull(reading.estimate(backlog = 1, running = true, firstHead = head(1), schedules = schedules))
        assertEquals(4L, whole.proved)
        assertEquals(11L, whole.remaining)
        assertTrue(whole.timed)
        // The Load's three at 20 s and the Receive's first at 30 s are done; its other eleven at its 30 s mean remain.
        assertEquals(micros(90.0), whole.provedWeight, 1e-3)
        assertEquals(micros(330.0), whole.remainingWeight, 1e-3)
        // The running sub-proof began when the last turn ended, 10 s before this reading.
        assertEquals(micros(10.0), whole.runningWeight, 1e-3)
        assertEquals(100.0 / 420, whole.fraction, 1e-9)
        assertEquals(320 * second, whole.remainingMicros)
        // A wait that began after the Load folded counts the Receive head alone.
        val later = assertNotNull(reading.estimate(backlog = 1, running = true, firstHead = head(2), schedules = schedules))
        assertEquals(1L, later.proved)
        assertEquals(40.0 / 360, later.fraction, 1e-9)
        // A parked worker spends nothing on a sub-proof.
        val parked = assertNotNull(reading.estimate(backlog = 1, running = false, firstHead = head(1), schedules = schedules))
        assertEquals(0.0, parked.runningWeight)
        assertEquals(90.0 / 420, parked.fraction, 1e-9)
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

    @Test fun anOperationTheCallerNamesSizesAndWeighsAHeadBeforeItsFirstTurn() {
        // This phone's earlier folds: a Load is three sub-proofs of 24, 48 and 49 s, a Receive twelve.
        val schedules = mapOf("load" to 3, "receive" to 12)
        val costs = mapOf("load" to mapOf(0L to 24 * second, 1L to 48 * second, 2L to 49 * second),
            "receive" to mapOf(0L to 30 * second))
        // A new process: the app's Load released head 3 and its first reading at 5 s finds no turn yet.
        val operations = mapOf(head(3) to "load")
        val first = assertNotNull(progress(5).estimate(1, true, head(3), schedules, costs, operations, since = 5 * second))
        assertEquals(0L, first.proved)
        assertEquals(3L, first.remaining)
        assertEquals(0.0, first.fraction)
        assertEquals(121 * second, first.remainingMicros)
        // Twelve seconds into the first sub-proof the share moves by time.
        val running = assertNotNull(progress(17).estimate(1, true, head(3), schedules, costs, operations, since = 5 * second))
        assertEquals(12.0 / 121, running.fraction, 1e-9)
        assertEquals(109 * second, running.remainingMicros)
        // Native records the first sub-proof: the share goes on from it.
        val recorded = assertNotNull(progress(30, row(5, 29, 3, "load", "q", 0, 3, "checkpoint"))
            .estimate(1, true, head(3), schedules, costs, operations, since = 5 * second))
        assertEquals(1L, recorded.proved)
        assertEquals(25.0 / 121, recorded.fraction, 1e-9)
        // Unnamed, the head counts at the largest schedule and the mean cost.
        val unnamed = assertNotNull(progress(5).estimate(1, true, head(3), schedules, costs, since = 5 * second))
        assertEquals(12L, unnamed.remaining)
        assertEquals(12 * 37.75 * second, unnamed.remainingWeight, 1e-3)
        // Before this phone proved anything the weights count sub-proofs, with no time.
        val untimed = assertNotNull(progress(5).estimate(1, true, head(3), schedules, operations = operations))
        assertFalse(untimed.timed)
        assertEquals(3.0, untimed.remainingWeight)
        assertNull(untimed.remainingMicros)
    }

    @Test fun theRunningSubProofIsTimedFromTheLaterOfTheLastTurnAndSince() {
        // The last fold ended at 60 s; the caller's wait for a new Load head began with its reading at 990 s.
        val reading = progress(1000,
            row(0, 20, 1, "load", "q", 0, 3, "checkpoint"),
            row(20, 40, 1, "load", "a", 1, 3, "checkpoint"),
            row(40, 60, 1, "load", "omega", 2, 3, "folded"))
        val operations = mapOf(head(2) to "load")
        val timed = assertNotNull(reading.estimate(1, true, head(2), operations = operations, since = 990 * second))
        assertEquals(micros(10.0), timed.runningWeight, 1e-3)
        assertEquals(10.0 / 60, timed.fraction, 1e-9)
        // Without it, the running sub-proof would seem to have run since 60 s, capped at nine tenths of its cost.
        val unbounded = assertNotNull(reading.estimate(1, true, head(2), operations = operations))
        assertEquals(micros(18.0), unbounded.runningWeight, 1e-3)
    }

    @Test fun aReadingsOwnCostsWinOverTheCallersAndTheCallersFillTheRest() {
        val costs = mapOf("load" to mapOf(0L to 24 * second, 1L to 48 * second, 2L to 49 * second))
        val reading = progress(50, row(0, 30, 1, "load", "q", 0, 3, "checkpoint"))
        val estimate = assertNotNull(reading.estimate(1, true, head(1), costs = costs))
        assertEquals(micros(30.0), estimate.provedWeight, 1e-3)
        assertEquals(micros(97.0), estimate.remainingWeight, 1e-3)
        assertEquals(micros(20.0), estimate.runningWeight, 1e-3)
        assertEquals(50.0 / 127, estimate.fraction, 1e-9)
        assertEquals(77 * second, estimate.remainingMicros)
    }

    @Test fun laterHeadsCountByTheOperationTheCallerNamed() {
        val reading = progress(30, row(0, 20, 1, "load", "q", 0, 3, "checkpoint"))
        val schedules = mapOf("load" to 3, "send" to 5, "receive" to 12)
        // Head 2 is a Send the app made; head 3 is unknown and counts at the largest schedule.
        val estimate = assertNotNull(reading.estimate(3, false, head(1), schedules, operations = mapOf(head(2) to "send")))
        assertEquals(2L + 5 + 12, estimate.remaining)
        // Heads at or below the current one, or past the backlog, are not later heads.
        val ignored = assertNotNull(reading.estimate(2, false, head(1), schedules,
            operations = mapOf(head(1) to "receive", head(3) to "send")))
        assertEquals(2L + 12, ignored.remaining)
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
        assertEquals(KagemushaWalletFoldProgressV1.RUNNING_SHARE_MAX * 10 * second, estimate.runningWeight, 1e-3)
        assertEquals(KagemushaWalletFoldProgressV1.FRACTION_MAX, estimate.fraction)
    }

    @Test fun adoptedTurnsAreDurableButOnlyAnOmegaFoldsAHead() {
        val reading = progress(50,
            row(0, 1, 4, "load", null, 0, 3, "adopted"),
            row(1, 2, 4, "load", "omega", 2, 3, "adopted"),
            row(2, 3, 5, "unload", null, 1, 3, "adopted"))
        // Adopted bytes were never proved here: they cost nothing known.
        assertEquals(emptyMap(), reading.costs)
        assertEquals(mapOf("load" to 3, "unload" to 3), reading.schedules)
        val estimate = assertNotNull(reading.estimate(1, true, head(4), emptyMap()))
        assertFalse(estimate.timed)
        assertEquals(0.0, estimate.runningWeight)
        assertNull(estimate.remainingMicros)
        assertEquals(5L, estimate.proved) // load's three sub-proofs, from its Ω, and unload's first two
        assertEquals(1L, estimate.remaining)
    }

    @Test fun nothingKnownIsNoEstimate() {
        assertNull(progress(10).estimate(1, true, head(1), emptyMap()))
        assertNull(progress(10).estimate(0, true, head(1), mapOf("load" to 3)))
        // Cancelled after its step was read but before its schedule: no size for the head.
        val unsized = progress(30, row(0, 20, 1, "load", null, 0, null, "cancelled"))
        assertNull(unsized.estimate(1, true, head(1), emptyMap()))
        assertNull(unsized.estimate(1, true, head(1), mapOf("load" to 0)))
        // No turn yet: the wait's first head counts from its first sub-proof at the schedule the caller knows.
        val unstarted = assertNotNull(progress(10).estimate(1, true, head(1), mapOf("load" to 3)))
        assertEquals(0L, unstarted.proved)
        assertEquals(3L, unstarted.remaining)
        assertEquals(0.0, unstarted.fraction)
        // A later head counts at the largest schedule this reading or the caller knows.
        val sized = progress(30, row(0, 20, 1, "load", "q", 0, 3, "checkpoint"))
        assertEquals(2L + 3, assertNotNull(sized.estimate(2, true, head(1), emptyMap())).remaining)
        assertNull(sized.estimate(0, true, head(1), mapOf("load" to 3)))
    }

    @Test fun manyLaterHeadsStayExact() {
        val reading = progress(30, row(0, 20, 1, "receive", "q", 0, 12, "checkpoint"))
        val estimate = assertNotNull(reading.estimate(Long.MAX_VALUE, false, head(1), mapOf("receive" to 12)))
        assertTrue(estimate.remaining > 0)
        assertTrue(estimate.fraction in 0.0..KagemushaWalletFoldProgressV1.FRACTION_MAX)
        assertEquals(Long.MAX_VALUE, estimate.remainingMicros)
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
