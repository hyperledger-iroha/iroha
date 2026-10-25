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
 * records every turn that selected one with the head's sequence, its operation and the ordinal it
 * worked on, which is the head's durable sub-proof count when the turn began; a turn cancelled by a
 * payment therefore still says where the head stands. Native does not report a schedule's length,
 * so callers pass sub-proofs per head by operation name, such as the [schedules] that earlier
 * readings of the same installed producer catalog showed.
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
        val outcome: String,
    ) {
        /** The turn left a durable sub-proof, proved now or adopted from an interrupted publication. */
        val durable: Boolean get() = outcome in DURABLE
        /** The durable sub-proof was the head's Ω. */
        val folded: Boolean get() = outcome == "folded" || durable && stage == "omega"
    }

    /** The head Native is folding as of this reading: the last turn's, or the next one after its Ω; null before a turn. */
    val head: BigInteger? = turns.lastOrNull()?.let { if (it.folded) it.sequence + BigInteger.ONE else it.sequence }

    /** Sub-proofs per head by operation name: each retained Ω's ordinal plus one, the largest per operation. */
    val schedules: Map<String, Int> = buildMap {
        for (turn in turns) {
            val operation = turn.operation ?: continue
            if (!turn.folded || turn.ordinal >= Int.MAX_VALUE) continue
            val count = (turn.ordinal + 1).toInt()
            if (count > (get(operation) ?: 0)) put(operation, count)
        }
    }

    /** Mean wall time of the last [PACE_TURNS] proved sub-proofs on this phone, or null before one. */
    val subProofMicros: Long? = turns.filter { it.outcome == "checkpoint" || it.outcome == "folded" }
        .takeLast(PACE_TURNS).takeIf { it.isNotEmpty() }
        ?.let { proved -> proved.sumOf { it.endedNativeMicros - it.startedNativeMicros } / proved.size }

    /** One estimate of a fold wait. [fraction] stays below one until the snapshot reports the folded head. */
    class Estimate internal constructor(
        /** Sub-proofs done toward the wait: the current head's durable ones and every sub-proof of a head folded since it began. */
        val proved: Long,
        /** Sub-proofs still to prove, the running one included; a head of unknown size counts as the largest schedule. */
        val remaining: Long,
        /** Part of the running sub-proof already spent at this phone's pace, from zero up to [RUNNING_SHARE_MAX]. */
        val runningShare: Double,
        /** Expected monotonic time until the last released head is folded at this phone's pace, or null before a proved sub-proof. */
        val remainingMicros: Long?,
    ) {
        val fraction: Double get() = ((proved + runningShare) / (proved + remaining)).coerceIn(0.0, FRACTION_MAX)
    }

    /**
     * Estimate the wait until every released head is folded.
     *
     * [backlog] counts the released heads still needing folds, as the latest snapshot reports it.
     * [running] says Native's worker is proving now. [firstHead] is the [head] of the reading that began
     * the caller's wait; every head folded since then counts in full, from its own Ω. [schedules] gives
     * sub-proofs per head by operation name. A head whose size is unknown counts as the largest known
     * schedule, so an estimate can only shrink once the head is identified, and no head is counted
     * finished before its Ω. Null when nothing is known yet: no turn, no schedule or no backlog.
     */
    fun estimate(backlog: Long, running: Boolean, firstHead: BigInteger, schedules: Map<String, Int>): Estimate? {
        val last = turns.lastOrNull() ?: return null
        val head = head ?: return null
        val largest = schedules.values.filter { it > 0 }.maxOrNull()?.toLong() ?: return null
        if (backlog <= 0) return null
        val done = when {
            last.folded -> 0L
            last.durable -> last.ordinal + 1
            else -> last.ordinal
        }
        val operation = turns.lastOrNull { it.sequence == head && it.operation != null }?.operation
        val size = maxOf(operation?.let(schedules::get)?.takeIf { it > 0 }?.toLong() ?: largest, done + 1)
        val earlier = turns.filter { it.folded && it.sequence >= firstHead && it.sequence < head }
            .groupBy { it.sequence }.values.sumOf { omega -> omega.maxOf { it.ordinal } + 1 }
        // Later heads are counted at the largest schedule; the bound keeps the sum exact in a Long.
        val later = (backlog - 1).coerceAtMost(Long.MAX_VALUE / 4 / largest) * largest
        val remaining = size - done + later
        val pace = subProofMicros?.takeIf { it > 0 }
        val share = if (running && pace != null) {
            ((sampledNativeMicros - last.endedNativeMicros).toDouble() / pace).coerceIn(0.0, RUNNING_SHARE_MAX)
        } else 0.0
        return Estimate(earlier + done, remaining, share, pace?.let { ((remaining - share) * it).roundToLong() })
    }

    companion object {
        /** The telemetry schema this reading accepts. */
        const val SCHEMA = "iroha.kagemusha.fold-metrics.v2"
        /** The running sub-proof never counts as more than this share of one before Native records it. */
        const val RUNNING_SHARE_MAX = 0.9
        /** Only the snapshot's folded head completes a wait. */
        const val FRACTION_MAX = 0.99
        private const val PACE_TURNS = 8
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
                val outcome = text(record, "outcome")
                require(outcome in OUTCOMES)
                Turn(started, ended, BigInteger(number(record, "sequence")), name(record, "operation"),
                    name(record, "stage"), ordinal, outcome)
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

        private fun text(record: JsonObject, key: String): String {
            val value = record.getValue(key) as JsonPrimitive
            require(value.isString)
            return value.content
        }

        private fun name(record: JsonObject, key: String): String? =
            if (record.getValue(key) == JsonNull) null else text(record, key)
    }
}
