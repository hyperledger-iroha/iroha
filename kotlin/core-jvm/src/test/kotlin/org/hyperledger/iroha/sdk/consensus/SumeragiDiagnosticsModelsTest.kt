// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.consensus

import java.math.BigInteger
import kotlinx.serialization.SerializationException
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import org.hyperledger.iroha.sdk.core.util.HashLiteral
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class SumeragiDiagnosticsModelsTest {
    @Test
    fun `native participant application uses exact diagnostics JSON names`() {
        val row = application(3)
        val encoded = Json.encodeToString(row)

        assertTrue(encoded.contains("\"lane_id\":3"))
        assertTrue(encoded.contains("\"predecessor_descriptor_hash\""))
        assertTrue(encoded.contains("\"application_block_height\":15"))
        assertTrue(encoded.contains("\"state\":\"durably_applied\""))
        assertEquals(row, Json.decodeFromString<SumeragiNativeAmxParticipantApplication>(encoded))
    }

    @Test
    fun `native participant vector enforces bounds and canonical order`() {
        val ordered = SumeragiNativeAmxParticipantApplications(
            listOf(application(3), application(4)),
        )
        assertEquals(2, ordered.rows.size)

        assertFailsWith<IllegalArgumentException> {
            SumeragiNativeAmxParticipantApplications(
                List(SUMERAGI_NATIVE_AMX_PARTICIPANT_APPLICATIONS_MAX + 1) {
                    application(3)
                },
            )
        }
        assertFailsWith<IllegalArgumentException> {
            SumeragiNativeAmxParticipantApplications(
                listOf(application(4), application(3)),
            )
        }
    }

    @Test
    fun `native participant row rejects incomplete carrier and oversized group`() {
        assertFailsWith<IllegalArgumentException> {
            application(3).copy(applicationBlockHash = null)
        }
        assertFailsWith<IllegalArgumentException> {
            application(3).copy(sourceCount = 4_097)
        }
    }

    @Test
    fun `native participant diagnostics preserve complete u64 numeric tokens`() {
        val max = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
        val row = application(3).copy(
            dataspaceId = max,
            participantHeight = max,
            participantView = max,
            predecessorHeight = max.subtract(BigInteger.ONE),
            applicationBlockHeight = max,
        )

        val encoded = Json.encodeToString(row)
        assertTrue(encoded.contains("\"dataspace_id\":$max"))
        assertTrue(encoded.contains("\"participant_height\":$max"))
        assertTrue(encoded.contains("\"participant_view\":$max"))
        assertTrue(encoded.contains("\"application_block_height\":$max"))
        assertEquals(row, Json.decodeFromString<SumeragiNativeAmxParticipantApplication>(encoded))
    }

    @Test
    fun `native participant diagnostics reject non-integer strings and out-of-range tokens`() {
        val encoded = Json.encodeToString(application(3))
        val rejected = listOf(
            "18446744073709551616",
            "-1",
            "1.0",
            "1e0",
            "01",
            "\"18446744073709551615\"",
        )

        rejected.forEach { token ->
            val wire = encoded.replace("\"dataspace_id\":8", "\"dataspace_id\":$token")
            assertFailsWith<SerializationException>(token) {
                Json.decodeFromString<SumeragiNativeAmxParticipantApplication>(wire)
            }
        }
    }

    @Test
    fun `native participant diagnostics order full-width dataspaces exactly`() {
        val high = BigInteger.ONE.shiftLeft(63)
        val max = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
        val lower = application(3).copy(dataspaceId = high)
        val upper = application(3).copy(dataspaceId = max)

        assertEquals(
            listOf(lower, upper),
            SumeragiNativeAmxParticipantApplications(listOf(lower, upper)).rows,
        )
        assertFailsWith<IllegalArgumentException> {
            SumeragiNativeAmxParticipantApplications(listOf(upper, lower))
        }
    }

    @Test
    fun `native participant diagnostics reject u64 overflow and predecessor wraparound`() {
        val overflow = BigInteger.ONE.shiftLeft(64)
        assertFailsWith<IllegalArgumentException> {
            application(3).copy(dataspaceId = overflow)
        }
        assertFailsWith<IllegalArgumentException> {
            application(3).copy(
                participantHeight = BigInteger.ONE,
                predecessorHeight = overflow.subtract(BigInteger.ONE),
            )
        }
    }

    @Test
    fun `autonomous execution preserves stages optional carrier and explicit conflict`() {
        val applied = autonomousExecution(3)
        val encoded = Json.encodeToString(applied)
        assertTrue(encoded.contains("\"highest_durable_stage\":\"kura_wsv_application_receipt_durable\""))
        assertEquals(applied, Json.decodeFromString<SumeragiAutonomousLaneExecution>(encoded))

        val conflict = autonomousExecution(
            3,
            SumeragiAutonomousLaneExecutionStage.CONFLICT,
            SumeragiAutonomousLaneExecutionStuckReason.EVIDENCE_CONFLICT,
            reservationCount = 1,
        )
        assertEquals(
            listOf(conflict),
            SumeragiAutonomousLaneExecutions(listOf(conflict)).rows,
        )
        assertFailsWith<IllegalArgumentException> {
            SumeragiAutonomousLaneExecutions(listOf(applied, applied))
        }
        assertFailsWith<IllegalArgumentException> {
            SumeragiAutonomousLaneExecutions(List(129) { conflict })
        }
        assertFailsWith<IllegalArgumentException> {
            autonomousExecution(3, reservationCount = 1)
        }
        assertFailsWith<IllegalArgumentException> {
            autonomousExecution(
                3,
                SumeragiAutonomousLaneExecutionStage.CONFLICT,
                SumeragiAutonomousLaneExecutionStuckReason.AWAITING_MERGE_SELECTION,
            )
        }
    }

    private fun application(laneId: Long): SumeragiNativeAmxParticipantApplication =
        SumeragiNativeAmxParticipantApplication(
            laneId = laneId,
            dataspaceId = BigInteger.valueOf(8),
            laneIncarnation = hash(0x51 + laneId.toInt()),
            participantHeight = BigInteger.valueOf(8),
            participantView = BigInteger.ONE,
            predecessorHeight = BigInteger.valueOf(7),
            predecessorDescriptorHash = hash(0x61),
            descriptorHash = hash(0x71),
            proposalHash = hash(0x73),
            settlementHash = hash(0x75),
            sourceCount = 2,
            applicationBlockHeight = BigInteger.valueOf(15),
            applicationBlockHash = hash(0x77),
            state = SumeragiNativeAmxParticipantApplicationState.DURABLY_APPLIED,
        )

    private fun autonomousExecution(
        laneId: Long,
        stage: SumeragiAutonomousLaneExecutionStage =
            SumeragiAutonomousLaneExecutionStage.KURA_WSV_APPLICATION_RECEIPT_DURABLE,
        reason: SumeragiAutonomousLaneExecutionStuckReason =
            SumeragiAutonomousLaneExecutionStuckReason.QUEUE_FINALIZATION_UNVERIFIABLE,
        reservationCount: Long = 2,
    ) = SumeragiAutonomousLaneExecution(
        laneId = laneId,
        dataspaceId = BigInteger.valueOf(8),
        laneIncarnation = hash(0x51 + laneId.toInt()),
        laneBlockHeight = BigInteger.valueOf(8),
        laneBlockView = BigInteger.ONE,
        proposalHeight = BigInteger.TEN,
        proposalView = BigInteger.valueOf(2),
        proposalHash = hash(0x73),
        descriptorHash = hash(0x75),
        executablePayloadHash = hash(0x77),
        sourceBundleHash = hash(0x79),
        mergeEntryHash = hash(0x7b),
        applicationBlockHeight = BigInteger.valueOf(12),
        applicationBlockHash = hash(0x7d),
        reservationCount = reservationCount,
        transactionCount = 2,
        highestDurableStage = stage,
        stuckReason = reason,
    )

    private fun hash(seed: Int): String =
        HashLiteral.canonicalize(ByteArray(32) { seed.toByte() })
}
