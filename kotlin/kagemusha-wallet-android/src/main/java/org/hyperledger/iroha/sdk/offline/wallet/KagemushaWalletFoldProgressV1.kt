// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

import java.math.BigInteger
import kotlin.math.roundToLong
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

/**
 * How far Native's background fold has come toward the current wallet head, read from its public
 * fold telemetry ([KagemushaWalletV1.foldMetricsJSON]). Observation only: it confers no Send, Unload
 * or Request readiness, and the snapshot's folded head remains the only completion.
 *
 * Native folds each released head through an ordered schedule of sub-proofs that ends in Ω. It
 * records every turn that selected one once the turn ends: the head's sequence, its operation, the
 * ordinal it worked on, which is the head's durable sub-proof count when the turn began, and, once the
 * turn read the head's schedule, the head's whole schedule with Ω (`stages`). A turn cancelled by a
 * payment therefore still says where its head stands and how far it has to go. A head Native has not
 * started, or whose first sub-proof is still running, has no record, so callers pass what earlier
 * readings of the same owner taught them for those heads: sub-proofs per head and the time each
 * sub-proof took by operation name ([schedules], [costs]), and the operations of heads they released.
 */
class KagemushaWalletFoldProgressV1 private constructor(
    /** Native's monotonic time of this reading, in microseconds from its process-local origin. */
    val sampledNativeMicros: Long,
    private val turns: List<Turn>,
) {
    private class Turn(
        val startedNativeMicros: Long,
        val endedNativeMicros: Long,
        val sequence: BigInteger,
        val operation: String?,
        val stage: String?,
        val ordinal: Long,
        val stages: Long?,
        val outcome: String,
    ) {
        /** The turn left a durable sub-proof, proved now or adopted from an interrupted publication. */
        val durable: Boolean get() = outcome in DURABLE
        /** The durable sub-proof was the head's Ω. */
        val folded: Boolean get() = outcome == "folded" || durable && stage == "omega"
        /** The turn proved its sub-proof on this phone; adopted bytes were proved before. */
        val proved: Boolean get() = outcome == "checkpoint" || outcome == "folded"
    }

    /** The head Native is folding as of this reading: the last turn's, or the next one after its Ω; null before a turn. */
    val head: BigInteger? = turns.lastOrNull()?.let { if (it.folded) it.sequence + BigInteger.ONE else it.sequence }

    /** Sub-proofs per head by operation name: the largest schedule a retained turn of that operation reported. */
    val schedules: Map<String, Int> = buildMap {
        for (turn in turns) {
            val operation = turn.operation ?: continue
            val stages = turn.stages ?: continue
            if (stages > Int.MAX_VALUE) continue
            if (stages > (get(operation) ?: 0)) put(operation, stages.toInt())
        }
    }

    /**
     * The wall time of each sub-proof on this phone, in microseconds by operation name and ordinal: the mean of the
     * retained turns that proved it. Sub-proofs of one schedule differ in cost, so a caller that keeps these between
     * readings can weight a wait by time before its turns are recorded.
     */
    val costs: Map<String, Map<Long, Long>> = turns.filter { it.proved && it.operation != null }
        .groupBy { checkNotNull(it.operation) }
        .mapValues { (_, proved) ->
            proved.groupBy { it.ordinal }.mapValues { (_, same) ->
                same.sumOf { it.endedNativeMicros - it.startedNativeMicros } / same.size
            }.filterValues { it > 0 }
        }.filterValues { it.isNotEmpty() }

    /**
     * One estimate of a fold wait, weighted by what each sub-proof costs: microseconds at this phone's pace once it has
     * proved one ([timed]), sub-proofs before that. [fraction] stays below one until the snapshot reports the folded head.
     */
    class Estimate internal constructor(
        /** Sub-proofs done toward the wait: the current head's durable ones and every sub-proof of a head folded since it began. */
        val proved: Long,
        /** Sub-proofs still to prove, the running one included; a head of unknown size counts as the largest schedule. */
        val remaining: Long,
        /** What the [proved] sub-proofs cost. */
        val provedWeight: Double,
        /** What the [remaining] sub-proofs cost, the running one in full. */
        val remainingWeight: Double,
        /** The running sub-proof's part already spent, at most [RUNNING_SHARE_MAX] of its cost. */
        val runningWeight: Double,
        /** Whether the weights are microseconds at this phone's pace; before it proved a sub-proof they count sub-proofs. */
        val timed: Boolean,
    ) {
        val fraction: Double get() =
            ((provedWeight + runningWeight) / (provedWeight + remainingWeight)).coerceIn(0.0, FRACTION_MAX)
        /** Expected monotonic time until the last released head is folded at this phone's pace, or null unless [timed]. */
        val remainingMicros: Long? get() =
            if (timed) (remainingWeight - runningWeight).coerceAtLeast(0.0).roundToLong() else null
    }

    /**
     * Estimate the wait until every released head is folded.
     *
     * [backlog] counts the released heads still needing folds, as the latest snapshot reports it. [running] says
     * Native's worker is proving now. [firstHead] is where the caller's wait began: the [head] of its first reading, or
     * before any turn the snapshot's oldest unfolded head (its sequence minus its backlog, plus one). Every head folded
     * since then counts in full, from its own Ω. A reading with no turn of the current head knows none of its sub-proofs
     * durable, so the head after the last Ω, or [firstHead] before any turn, counts from its first.
     *
     * A head counts at the schedule its own turns reported. Heads without one count by their operation, named by a turn
     * or by the caller in [operations] (such as the heads its own Load, Send, Receive or Unload released), in [schedules]
     * merged with this reading's, and at the largest schedule when no operation is known. Each sub-proof weighs what
     * this reading's or the caller's [costs] say it took, else its operation's mean, else the mean of every known one.
     * The running sub-proof is timed from the last turn's end or [since], whichever is later: the caller's earliest
     * reading at which it may have started, such as its first reading of the wait or its last that found the worker
     * parked. No head is counted finished before its Ω. Null when nothing is known yet: no backlog, or no size for a
     * head still to fold.
     */
    fun estimate(
        backlog: Long,
        running: Boolean,
        firstHead: BigInteger,
        schedules: Map<String, Int> = emptyMap(),
        costs: Map<String, Map<Long, Long>> = emptyMap(),
        operations: Map<BigInteger, String> = emptyMap(),
        since: Long? = null,
    ): Estimate? {
        if (backlog <= 0) return null
        val sizes = HashMap(schedules)
        for ((operation, count) in this.schedules) if (count > (sizes[operation] ?: 0)) sizes[operation] = count
        val learned = HashMap<String, Map<Long, Long>>()
        for ((operation, byOrdinal) in costs) learned[operation] = byOrdinal.filterValues { it > 0 }
        for ((operation, byOrdinal) in this.costs) learned[operation] = learned[operation].orEmpty() + byOrdinal
        val pace = learned.values.flatMap { it.values }.takeIf { it.isNotEmpty() }?.average()
        val means = learned.mapValues { (_, byOrdinal) -> byOrdinal.values.takeIf { it.isNotEmpty() }?.average() }
        // Sub-proofs [from] until [until] of a head of [operation]: known ordinals at their cost, the rest at a mean.
        fun weight(operation: String?, from: Long, until: Long): Double {
            if (until <= from) return 0.0
            if (pace == null) return (until - from).toDouble()
            val byOrdinal = operation?.let { learned[it] }.orEmpty().filterKeys { it in from until until }
            val fallback = operation?.let { means[it] } ?: pace
            return byOrdinal.values.sum() + (until - from - byOrdinal.size) * fallback
        }
        val largest = sizes.values.filter { it > 0 }.maxOrNull()?.toLong()
        val last = turns.lastOrNull()
        val head = head?.takeIf { it >= firstHead } ?: firstHead
        val done = when {
            last == null || last.sequence != head -> 0L
            last.durable -> last.ordinal + 1
            else -> last.ordinal
        }
        val ofHead = turns.filter { it.sequence == head }
        // Native's own schedule for a head it has started; otherwise the head's operation, or the largest.
        val operation = ofHead.lastOrNull { it.operation != null }?.operation ?: operations[head]
        val known = ofHead.lastOrNull { it.stages != null }?.stages
            ?: operation?.let(sizes::get)?.takeIf { it > 0 }?.toLong() ?: largest ?: return null
        val size = maxOf(known, done + 1)
        var proved = done
        var provedWeight = weight(operation, 0, done)
        for (omega in turns.filter { it.folded && it.sequence >= firstHead && it.sequence < head }.groupBy { it.sequence }.values) {
            val count = omega.maxOf { it.ordinal } + 1
            proved += count
            provedWeight += weight(omega.firstNotNullOfOrNull { it.operation }, 0, count)
        }
        var remaining = size - done
        var remainingWeight = weight(operation, done, size)
        if (backlog > 1) {
            val end = head + BigInteger.valueOf(backlog)
            val named = operations.filterKeys { it > head && it < end }.values
            for (later in named) {
                val each = sizes[later]?.takeIf { it > 0 }?.toLong() ?: largest ?: return null
                remaining += each
                remainingWeight += weight(later, 0, each)
            }
            // The other later heads count at the largest schedule; the bound keeps the sum exact in a Long.
            val unnamed = backlog - 1 - named.size
            if (unnamed > 0) {
                val each = largest ?: return null
                remaining += unnamed.coerceAtMost(Long.MAX_VALUE / 4 / each) * each
                remainingWeight += unnamed.toDouble() * weight(null, 0, each)
            }
        }
        val start = listOfNotNull(last?.endedNativeMicros, since).maxOrNull()
        val runningWeight = if (running && pace != null && start != null) {
            (sampledNativeMicros - start).coerceAtLeast(0).toDouble()
                .coerceAtMost(RUNNING_SHARE_MAX * weight(operation, done, done + 1))
        } else 0.0
        return Estimate(proved, remaining, provedWeight, remainingWeight, runningWeight, timed = pace != null)
    }

    companion object {
        /** The telemetry schema this reading accepts. */
        const val SCHEMA = "iroha.kagemusha.fold-metrics.v3"
        /** The running sub-proof never counts as more than this share of its cost before Native records it. */
        const val RUNNING_SHARE_MAX = 0.9
        /** Only the snapshot's folded head completes a wait. */
        const val FRACTION_MAX = 0.99
        private val DURABLE = setOf("checkpoint", "folded", "adopted")
        private val OUTCOMES = DURABLE + setOf("cancelled", "discarded", "failed")
        private val DECIMAL = Regex("0|[1-9][0-9]*")

        /** Read [KagemushaWalletV1.foldMetricsJSON]; malformed or foreign telemetry is INVALID_NATIVE_OUTPUT. */
        @JvmStatic
        fun parse(json: String): KagemushaWalletFoldProgressV1 = try {
            val root = Json.parseToJsonElement(json) as JsonObject
            require(text(root, "schema") == SCHEMA)
            val sampled = number(root, "sampledNativeMicros").toLong()
            val turns = (root.getValue("records") as JsonArray).map { element ->
                val record = element as JsonObject
                val started = number(record, "startedNativeMicros").toLong()
                val ended = number(record, "endedNativeMicros").toLong()
                require(started <= ended && ended <= sampled)
                val ordinal = number(record, "ordinal").toLong()
                require(ordinal <= UInt.MAX_VALUE.toLong())
                // A schedule always ends in Ω, so a reported one has at least one sub-proof.
                val stages = nullableNumber(record, "stages")?.toLong()
                require(stages == null || stages in 1..UInt.MAX_VALUE.toLong())
                val outcome = text(record, "outcome")
                require(outcome in OUTCOMES)
                Turn(started, ended, BigInteger(number(record, "sequence")), name(record, "operation"),
                    name(record, "stage"), ordinal, stages, outcome)
            }
            KagemushaWalletFoldProgressV1(sampled, turns)
        } catch (_: Exception) {
            throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
        }

        private fun number(record: JsonObject, key: String): String {
            val value = record.getValue(key) as JsonPrimitive
            require(!value.isString && DECIMAL.matches(value.content))
            return value.content
        }

        private fun nullableNumber(record: JsonObject, key: String): String? =
            if (record.getValue(key) == JsonNull) null else number(record, key)

        private fun text(record: JsonObject, key: String): String {
            val value = record.getValue(key) as JsonPrimitive
            require(value.isString)
            return value.content
        }

        private fun name(record: JsonObject, key: String): String? =
            if (record.getValue(key) == JsonNull) null else text(record, key)
    }
}
