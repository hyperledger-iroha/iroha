// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.consensus

import java.math.BigInteger
import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.JsonObject

/** Private JSON transport records feed the immutable, validating diagnostics API. */
@Serializable
@SerialName("org.hyperledger.iroha.sdk.consensus.SumeragiNposDiagnostics")
private class SumeragiNposDiagnosticsWire(
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("epoch_length_blocks")
    val epochLengthBlocks: BigInteger,
    @SerialName("epoch_seed") val epochSeed: List<Int>,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("prf_height")
    val prfHeight: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("prf_view")
    val prfView: BigInteger,
) {
    fun value(): SumeragiNposDiagnostics = SumeragiNposDiagnostics(
        epochLengthBlocks = epochLengthBlocks,
        epochSeed = epochSeed,
        prfHeight = prfHeight,
        prfView = prfView,
    )

    companion object {
        fun from(value: SumeragiNposDiagnostics): SumeragiNposDiagnosticsWire = SumeragiNposDiagnosticsWire(
                epochLengthBlocks = value.epochLengthBlocks,
                epochSeed = value.epochSeed,
                prfHeight = value.prfHeight,
                prfView = value.prfView,
        )
    }
}

internal object SumeragiNposDiagnosticsSerializer : KSerializer<SumeragiNposDiagnostics> {
    private val wireSerializer = SumeragiNposDiagnosticsWire.serializer()
    override val descriptor: SerialDescriptor = wireSerializer.descriptor

    override fun serialize(encoder: Encoder, value: SumeragiNposDiagnostics) {
        encoder.encodeSerializableValue(wireSerializer, SumeragiNposDiagnosticsWire.from(value))
    }

    override fun deserialize(decoder: Decoder): SumeragiNposDiagnostics =
        decoder.decodeSerializableValue(wireSerializer).value()
}

@Serializable
@SerialName("org.hyperledger.iroha.sdk.consensus.SumeragiDiagnosticsStatus")
private class SumeragiDiagnosticsStatusWire(
    @SerialName("pipeline_execution")
    val pipelineExecution: SumeragiPipelineExecutionStatus,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("tx_queue_depth")
    val txQueueDepth: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("tx_queue_capacity")
    val txQueueCapacity: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("tx_queue_retained_bytes")
    val txQueueRetainedBytes: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("tx_queue_max_retained_bytes")
    val txQueueMaxRetainedBytes: BigInteger,
    @SerialName("tx_queue_saturated") val txQueueSaturated: Boolean,
    @SerialName("tx_queue_saturated_by_count") val txQueueSaturatedByCount: Boolean,
    @SerialName("tx_queue_saturated_by_bytes") val txQueueSaturatedByBytes: Boolean,
    @SerialName("tx_queue_saturated_by_age") val txQueueSaturatedByAge: Boolean,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("tx_queue_oldest_queued_age_ms")
    val txQueueOldestQueuedAgeMs: BigInteger,
    val npos: SumeragiNposDiagnostics? = null,
    @SerialName("lane_commitments") val laneCommitments: List<JsonObject>,
    @SerialName("dataspace_commitments") val dataspaceCommitments: List<JsonObject>,
    @SerialName("lane_settlement_commitments")
    val laneSettlementCommitments: List<JsonObject>,
    @SerialName("lane_relay_envelopes") val laneRelayEnvelopes: List<JsonObject>,
    @SerialName("lane_payload_ownerships") val lanePayloadOwnerships: List<JsonObject>,
    @SerialName("committed_lane_blocks") val committedLaneBlocks: List<JsonObject>,
    @SerialName("lane_block_sessions") val laneBlockSessions: List<JsonObject>,
    @SerialName("lane_governance_sealed_total") val laneGovernanceSealedTotal: Long,
    @SerialName("lane_governance_sealed_aliases")
    val laneGovernanceSealedAliases: List<String>,
    @SerialName("lane_governance") val laneGovernance: List<JsonObject>,
    @SerialName("native_amx_participant_applications")
    val nativeAmxParticipantApplications: List<SumeragiNativeAmxParticipantApplication>,
    @SerialName("autonomous_lane_executions")
    val autonomousLaneExecutions: List<SumeragiAutonomousLaneExecution>,
) {
    fun value(): SumeragiDiagnosticsStatus = SumeragiDiagnosticsStatus(
        pipelineExecution = pipelineExecution,
        txQueueDepth = txQueueDepth,
        txQueueCapacity = txQueueCapacity,
        txQueueRetainedBytes = txQueueRetainedBytes,
        txQueueMaxRetainedBytes = txQueueMaxRetainedBytes,
        txQueueSaturated = txQueueSaturated,
        txQueueSaturatedByCount = txQueueSaturatedByCount,
        txQueueSaturatedByBytes = txQueueSaturatedByBytes,
        txQueueSaturatedByAge = txQueueSaturatedByAge,
        txQueueOldestQueuedAgeMs = txQueueOldestQueuedAgeMs,
        npos = npos,
        laneCommitments = laneCommitments,
        dataspaceCommitments = dataspaceCommitments,
        laneSettlementCommitments = laneSettlementCommitments,
        laneRelayEnvelopes = laneRelayEnvelopes,
        lanePayloadOwnerships = lanePayloadOwnerships,
        committedLaneBlocks = committedLaneBlocks,
        laneBlockSessions = laneBlockSessions,
        laneGovernanceSealedTotal = laneGovernanceSealedTotal,
        laneGovernanceSealedAliases = laneGovernanceSealedAliases,
        laneGovernance = laneGovernance,
        nativeAmxParticipantApplications = nativeAmxParticipantApplications,
        autonomousLaneExecutions = autonomousLaneExecutions,
    )

    companion object {
        fun from(value: SumeragiDiagnosticsStatus): SumeragiDiagnosticsStatusWire = SumeragiDiagnosticsStatusWire(
                pipelineExecution = value.pipelineExecution,
                txQueueDepth = value.txQueueDepth,
                txQueueCapacity = value.txQueueCapacity,
                txQueueRetainedBytes = value.txQueueRetainedBytes,
                txQueueMaxRetainedBytes = value.txQueueMaxRetainedBytes,
                txQueueSaturated = value.txQueueSaturated,
                txQueueSaturatedByCount = value.txQueueSaturatedByCount,
                txQueueSaturatedByBytes = value.txQueueSaturatedByBytes,
                txQueueSaturatedByAge = value.txQueueSaturatedByAge,
                txQueueOldestQueuedAgeMs = value.txQueueOldestQueuedAgeMs,
                npos = value.npos,
                laneCommitments = value.laneCommitments,
                dataspaceCommitments = value.dataspaceCommitments,
                laneSettlementCommitments = value.laneSettlementCommitments,
                laneRelayEnvelopes = value.laneRelayEnvelopes,
                lanePayloadOwnerships = value.lanePayloadOwnerships,
                committedLaneBlocks = value.committedLaneBlocks,
                laneBlockSessions = value.laneBlockSessions,
                laneGovernanceSealedTotal = value.laneGovernanceSealedTotal,
                laneGovernanceSealedAliases = value.laneGovernanceSealedAliases,
                laneGovernance = value.laneGovernance,
                nativeAmxParticipantApplications = value.nativeAmxParticipantApplications,
                autonomousLaneExecutions = value.autonomousLaneExecutions,
        )
    }
}

internal object SumeragiDiagnosticsStatusSerializer : KSerializer<SumeragiDiagnosticsStatus> {
    private val wireSerializer = SumeragiDiagnosticsStatusWire.serializer()
    override val descriptor: SerialDescriptor = wireSerializer.descriptor

    override fun serialize(encoder: Encoder, value: SumeragiDiagnosticsStatus) {
        encoder.encodeSerializableValue(wireSerializer, SumeragiDiagnosticsStatusWire.from(value))
    }

    override fun deserialize(decoder: Decoder): SumeragiDiagnosticsStatus =
        decoder.decodeSerializableValue(wireSerializer).value()
}
