// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.consensus

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import kotlinx.serialization.SerializationException
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import org.hyperledger.iroha.sdk.core.util.HashLiteral
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFails
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
    fun `native participant row enforces carrier state geometry and group bound`() {
        assertFailsWith<IllegalArgumentException> {
            application(3).copy(applicationBlockHash = null)
        }
        assertFailsWith<IllegalArgumentException> {
            application(3).copy(sourceCount = 4_097)
        }

        val geometryError =
            "Native AMX participant state and application block identity disagree"
        for (state in listOf(
            SumeragiNativeAmxParticipantApplicationState.CERTIFIED_PENDING_CARRIER,
            SumeragiNativeAmxParticipantApplicationState.CONFLICT,
        )) {
            assertEquals(
                state,
                application(3).copy(
                    applicationBlockHeight = null,
                    applicationBlockHash = null,
                    state = state,
                ).state,
            )
            val error = assertFailsWith<IllegalArgumentException> {
                application(3).copy(state = state)
            }
            assertEquals(geometryError, error.message)
        }
        for (state in listOf(
            SumeragiNativeAmxParticipantApplicationState.COMMITTED_EVIDENCE_PENDING,
            SumeragiNativeAmxParticipantApplicationState.DURABLY_APPLIED,
        )) {
            assertEquals(state, application(3).copy(state = state).state)
            val error = assertFailsWith<IllegalArgumentException> {
                application(3).copy(
                    applicationBlockHeight = null,
                    applicationBlockHash = null,
                    state = state,
                )
            }
            assertEquals(geometryError, error.message)
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
        assertTrue(encoded.contains("\"proposal_identity_hash\""))
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

    @Test
    fun `autonomous reservations require provisional identity and exact geometry`() {
        val applied = autonomousExecution(3)
        val appliedObject = Json.parseToJsonElement(Json.encodeToString(applied)).jsonObject
        for (field in listOf(
            "reservation_owner_hash", "proposal_identity_hash", "reservation_group_hash",
        )) {
            assertFails {
                Json.decodeFromString<SumeragiAutonomousLaneExecution>(
                    JsonObject(appliedObject - field).toString(),
                )
            }
            for (invalid in listOf(
                JsonPrimitive("hash:${"00".repeat(32)}#6A0A"),
                JsonPrimitive("ab".repeat(32)),
                JsonArray(List(32) { JsonPrimitive(1) }),
            )) {
                assertFails {
                    Json.decodeFromString<SumeragiAutonomousLaneExecution>(
                        JsonObject(appliedObject + (field to invalid)).toString(),
                    )
                }
            }
        }
        for (missing in listOf("proposal_hash", "descriptor_hash")) {
            assertFailsWith<IllegalArgumentException> {
                Json.decodeFromString<SumeragiAutonomousLaneExecution>(
                    JsonObject(appliedObject - missing).toString(),
                )
            }
        }
        assertFailsWith<IllegalArgumentException> {
            autonomousExecution(3, proposalHash = null, descriptorHash = null)
        }
        assertEquals(null, autonomousExecution(3, proposalView = null).proposalView)

        val diagnosticsRoot = Json.parseToJsonElement(
            Json.encodeToString(diagnostics()),
        ).jsonObject
        val diagnosticRows = diagnosticsRoot["autonomous_lane_executions"] as JsonArray
        val nullViewRow = JsonObject(
            diagnosticRows[0].jsonObject + ("proposal_view" to JsonNull),
        )
        val explicitNullView = JsonObject(
            diagnosticsRoot +
                ("autonomous_lane_executions" to JsonArray(listOf(nullViewRow))),
        )
        assertEquals(
            null,
            SumeragiDiagnosticsStatus.parseJson(explicitNullView.toString())
                .autonomousLaneExecutions[0].proposalView,
        )

        val reservations = autonomousExecution(
            3,
            SumeragiAutonomousLaneExecutionStage.RESERVATIONS_DURABLE,
            SumeragiAutonomousLaneExecutionStuckReason.AWAITING_EXECUTABLE_PAYLOAD,
            proposalView = null,
            proposalHash = null,
            descriptorHash = null,
            executablePayloadHash = null,
            sourceBundleHash = null,
            mergeEntryHash = null,
            applicationBlockHeight = null,
            applicationBlockHash = null,
        )
        assertEquals(null, reservations.proposalHash)
        assertEquals(
            SumeragiAutonomousLaneExecutionStuckReason.AWAITING_EXECUTABLE_PAYLOAD,
            reservations.stuckReason,
        )
        assertFailsWith<IllegalArgumentException> {
            autonomousExecution(
                3,
                SumeragiAutonomousLaneExecutionStage.RESERVATIONS_DURABLE,
                SumeragiAutonomousLaneExecutionStuckReason.AWAITING_EXECUTABLE_PAYLOAD,
                proposalView = BigInteger.ZERO,
                proposalHash = null,
                descriptorHash = null,
                executablePayloadHash = null,
                sourceBundleHash = null,
                mergeEntryHash = null,
                applicationBlockHeight = null,
                applicationBlockHash = null,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            autonomousExecution(
                3,
                SumeragiAutonomousLaneExecutionStage.RESERVATIONS_DURABLE,
                SumeragiAutonomousLaneExecutionStuckReason.AWAITING_PAYLOAD_AVAILABILITY,
                proposalView = null,
                proposalHash = null,
                descriptorHash = null,
                executablePayloadHash = null,
                sourceBundleHash = null,
                mergeEntryHash = null,
                applicationBlockHeight = null,
                applicationBlockHash = null,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            autonomousExecution(
                3,
                SumeragiAutonomousLaneExecutionStage.RESERVATIONS_DURABLE,
                SumeragiAutonomousLaneExecutionStuckReason.AWAITING_EXECUTABLE_PAYLOAD,
                proposalView = null,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            autonomousExecution(
                3,
                SumeragiAutonomousLaneExecutionStage.RESERVATIONS_DURABLE,
                SumeragiAutonomousLaneExecutionStuckReason.AWAITING_EXECUTABLE_PAYLOAD,
                proposalView = null,
                proposalHash = null,
                descriptorHash = null,
                sourceBundleHash = null,
                mergeEntryHash = null,
                applicationBlockHeight = null,
                applicationBlockHash = null,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            autonomousExecution(
                3,
                SumeragiAutonomousLaneExecutionStage.RESERVATIONS_DURABLE,
                SumeragiAutonomousLaneExecutionStuckReason.AWAITING_EXECUTABLE_PAYLOAD,
                reservationCount = 1,
                proposalView = null,
                proposalHash = null,
                descriptorHash = null,
                executablePayloadHash = null,
                sourceBundleHash = null,
                mergeEntryHash = null,
                applicationBlockHeight = null,
                applicationBlockHash = null,
            )
        }

        val sameProvisionalIdentity = autonomousExecution(
            3,
            proposalHash = hash(0x7e),
            descriptorHash = hash(0x7f),
        )
        assertFailsWith<IllegalArgumentException> {
            SumeragiAutonomousLaneExecutions(listOf(applied, sameProvisionalIdentity))
        }
        assertFailsWith<IllegalArgumentException> {
            SumeragiAutonomousLaneExecutions(
                listOf(
                    autonomousExecution(3, proposalIdentityHash = hash(0x90)),
                    autonomousExecution(3, proposalIdentityHash = hash(0x80)),
                ),
            )
        }
    }

    @Test
    fun `complete diagnostics parser preserves every required first release vector`() {
        val expected = diagnostics()
        val parsed = SumeragiDiagnosticsStatus.parseJson(Json.encodeToString(expected))

        assertEquals(expected.pipelineExecution, parsed.pipelineExecution)
        assertEquals(expected.nativeAmxParticipantApplications, parsed.nativeAmxParticipantApplications)
        assertEquals(expected.autonomousLaneExecutions, parsed.autonomousLaneExecutions)
        assertEquals(BigInteger.ONE, parsed.txQueueCapacity)
    }

    @Test
    fun `complete diagnostics parser validates Native AMX settlement and relay evidence`() {
        val root = Json.parseToJsonElement(Json.encodeToString(diagnostics())).jsonObject
        val settlement = nativeAmxReceiptGroupFixture()
        val relay = JsonObject(mapOf("settlement_commitment" to settlement))
        val wire = JsonObject(
            root + mapOf(
                "lane_settlement_commitments" to JsonArray(listOf(settlement)),
                "lane_relay_envelopes" to JsonArray(listOf(relay)),
            ),
        )

        val parsed = SumeragiDiagnosticsStatus.parseJson(wire.toString())

        assertEquals(listOf(settlement), parsed.laneSettlementCommitments)
        assertEquals(listOf(relay), parsed.laneRelayEnvelopes)
    }

    @Test
    fun `complete diagnostics parser rejects malformed Native AMX settlement and relay evidence`() {
        val root = Json.parseToJsonElement(Json.encodeToString(diagnostics())).jsonObject
        val malformed = malformedNativeAmxReceiptGroup(nativeAmxReceiptGroupFixture())

        val directError = assertFails {
            SumeragiDiagnosticsStatus.parseJson(
                JsonObject(
                    root +
                        ("lane_settlement_commitments" to JsonArray(listOf(malformed))),
                ).toString(),
            )
        }
        assertStrictNativeAmxFailure(directError)

        val relay = JsonObject(mapOf("settlement_commitment" to malformed))
        val relayError = assertFails {
            SumeragiDiagnosticsStatus.parseJson(
                JsonObject(
                    root + ("lane_relay_envelopes" to JsonArray(listOf(relay))),
                ).toString(),
            )
        }
        assertStrictNativeAmxFailure(relayError)
    }

    @Test
    fun `complete diagnostics parser rejects unknown and missing fields`() {
        val root = Json.parseToJsonElement(Json.encodeToString(diagnostics())).jsonObject

        assertFails {
            SumeragiDiagnosticsStatus.parseJson(
                JsonObject(root + ("legacy_round" to JsonPrimitive(1))).toString(),
            )
        }
        assertFails {
            SumeragiDiagnosticsStatus.parseJson(
                JsonObject(root - "autonomous_lane_executions").toString(),
            )
        }
        val pipeline = root.getValue("pipeline_execution").jsonObject
        assertFails {
            SumeragiDiagnosticsStatus.parseJson(
                JsonObject(
                    root +
                        (
                            "pipeline_execution" to
                                JsonObject(pipeline + ("legacy_total" to JsonPrimitive(0)))
                            ),
                ).toString(),
            )
        }
    }

    @Test
    fun `complete diagnostics parser enforces queue bounds and canonical vector order`() {
        val root = Json.parseToJsonElement(Json.encodeToString(diagnostics())).jsonObject
        assertFails {
            SumeragiDiagnosticsStatus.parseJson(
                JsonObject(root + ("tx_queue_depth" to JsonPrimitive(2))).toString(),
            )
        }

        val unorderedApplications = JsonArray(
            listOf(
                Json.parseToJsonElement(Json.encodeToString(application(4))),
                Json.parseToJsonElement(Json.encodeToString(application(3))),
            ),
        )
        assertFails {
            SumeragiDiagnosticsStatus.parseJson(
                JsonObject(
                    root + ("native_amx_participant_applications" to unorderedApplications),
                ).toString(),
            )
        }

        val autonomous = Json.parseToJsonElement(
            Json.encodeToString(autonomousExecution(3)),
        )
        assertFails {
            SumeragiDiagnosticsStatus.parseJson(
                JsonObject(
                    root +
                        ("autonomous_lane_executions" to JsonArray(listOf(autonomous, autonomous))),
                ).toString(),
            )
        }
    }

    private fun malformedNativeAmxReceiptGroup(group: JsonObject): JsonObject {
        val receipts = group.getValue("native_amx_receipts") as JsonArray
        val first = receipts.first().jsonObject
        val malformedFirst = JsonObject(first + ("version" to JsonPrimitive(1)))
        val malformedReceipts = JsonArray(
            listOf(malformedFirst) + receipts.drop(1),
        )
        return JsonObject(group + ("native_amx_receipts" to malformedReceipts))
    }

    private fun assertStrictNativeAmxFailure(error: Throwable) {
        assertTrue(
            generateSequence(error) { it.cause }.any {
                it.message?.contains("version must equal 2") == true
            },
            "diagnostics rejection must originate from strict Native AMX V2 validation: $error",
        )
    }

    private fun nativeAmxReceiptGroupFixture(): JsonObject {
        val fixture = Json.parseToJsonElement(
            String(Files.readAllBytes(nativeAmxFixturePath()), StandardCharsets.UTF_8),
        ).jsonObject
        return fixture.getValue("golden").jsonObject.getValue("receipt_group").jsonObject
    }

    private fun nativeAmxFixturePath(): Path {
        var current = Paths.get("").toAbsolutePath()
        while (true) {
            val candidate =
                current.resolve("fixtures/sumeragi_v2/native_amx_v2_grouped.json")
            if (Files.isRegularFile(candidate)) return candidate
            current = current.parent
                ?: error("fixtures/sumeragi_v2/native_amx_v2_grouped.json was not found")
        }
    }

    private fun diagnostics(): SumeragiDiagnosticsStatus =
        SumeragiDiagnosticsStatus(
            pipelineExecution = pipeline(),
            txQueueDepth = BigInteger.ZERO,
            txQueueCapacity = BigInteger.ONE,
            txQueueRetainedBytes = BigInteger.ZERO,
            txQueueMaxRetainedBytes = BigInteger.ONE,
            txQueueSaturated = false,
            txQueueSaturatedByCount = false,
            txQueueSaturatedByBytes = false,
            txQueueSaturatedByAge = false,
            txQueueOldestQueuedAgeMs = BigInteger.ZERO,
            laneCommitments = emptyList(),
            dataspaceCommitments = emptyList(),
            laneSettlementCommitments = emptyList(),
            laneRelayEnvelopes = emptyList(),
            lanePayloadOwnerships = emptyList(),
            committedLaneBlocks = emptyList(),
            laneBlockSessions = emptyList(),
            laneGovernanceSealedTotal = 0,
            laneGovernanceSealedAliases = emptyList(),
            laneGovernance = emptyList(),
            nativeAmxParticipantApplications = listOf(application(3)),
            autonomousLaneExecutions = listOf(autonomousExecution(3)),
        )

    private fun pipeline(): SumeragiPipelineExecutionStatus {
        val zero = BigInteger.ZERO
        return SumeragiPipelineExecutionStatus(
            txVerticesTotal = zero,
            txEdgesTotal = zero,
            overlayCountTotal = zero,
            overlayInstrTotal = zero,
            overlayBytesTotal = zero,
            rbcChunksTotal = zero,
            rbcBytesTotal = zero,
            detachedPreparedTotal = zero,
            detachedMergedTotal = zero,
            detachedFallbackTotal = zero,
            detachedFallbackFeePostprocessingTotal = zero,
            detachedFallbackUserExecutorTotal = zero,
            detachedFallbackDurableStateTotal = zero,
            detachedFallbackUnsupportedInstructionTotal = zero,
            detachedFallbackRejectedEvalTotal = zero,
            detachedFallbackOverlayErrorTotal = zero,
            quarantineExecutedTotal = zero,
        )
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
        proposalView: BigInteger? = BigInteger.valueOf(2),
        reservationOwnerHash: String = hash(0x6f),
        proposalIdentityHash: String = hash(0x70),
        reservationGroupHash: String = hash(0x71),
        proposalHash: String? = hash(0x73),
        descriptorHash: String? = hash(0x75),
        executablePayloadHash: String? = hash(0x77),
        sourceBundleHash: String? = hash(0x79),
        mergeEntryHash: String? = hash(0x7b),
        applicationBlockHeight: BigInteger? = BigInteger.valueOf(12),
        applicationBlockHash: String? = hash(0x7d),
    ) = SumeragiAutonomousLaneExecution(
        laneId = laneId,
        dataspaceId = BigInteger.valueOf(8),
        laneIncarnation = hash(0x51 + laneId.toInt()),
        laneBlockHeight = BigInteger.valueOf(8),
        laneBlockView = BigInteger.ONE,
        proposalHeight = BigInteger.TEN,
        proposalView = proposalView,
        reservationOwnerHash = reservationOwnerHash,
        proposalIdentityHash = proposalIdentityHash,
        reservationGroupHash = reservationGroupHash,
        proposalHash = proposalHash,
        descriptorHash = descriptorHash,
        executablePayloadHash = executablePayloadHash,
        sourceBundleHash = sourceBundleHash,
        mergeEntryHash = mergeEntryHash,
        applicationBlockHeight = applicationBlockHeight,
        applicationBlockHash = applicationBlockHash,
        reservationCount = reservationCount,
        transactionCount = 2,
        highestDurableStage = stage,
        stuckReason = reason,
    )

    private fun hash(seed: Int): String =
        HashLiteral.canonicalize(ByteArray(32) { seed.toByte() })
}
