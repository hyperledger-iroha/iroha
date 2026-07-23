// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.consensus

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

    private fun application(laneId: Long): SumeragiNativeAmxParticipantApplication =
        SumeragiNativeAmxParticipantApplication(
            laneId = laneId,
            dataspaceId = 8,
            laneIncarnation = hash(0x51 + laneId.toInt()),
            participantHeight = 8,
            participantView = 1,
            predecessorHeight = 7,
            predecessorDescriptorHash = hash(0x61),
            descriptorHash = hash(0x71),
            proposalHash = hash(0x73),
            settlementHash = hash(0x75),
            sourceCount = 2,
            applicationBlockHeight = 15,
            applicationBlockHash = hash(0x77),
            state = SumeragiNativeAmxParticipantApplicationState.DURABLY_APPLIED,
        )

    private fun hash(seed: Int): String =
        HashLiteral.canonicalize(ByteArray(32) { seed.toByte() })
}
