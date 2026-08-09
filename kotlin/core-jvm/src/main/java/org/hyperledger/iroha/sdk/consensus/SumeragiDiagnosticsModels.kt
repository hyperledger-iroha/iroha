// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.consensus

import java.math.BigInteger
import java.util.Collections
import kotlinx.serialization.KSerializer
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.SerializationException
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.PrimitiveSerialDescriptor
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonDecoder
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonEncoder
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.decodeFromString
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.util.HashLiteral

/** Maximum number of Native AMX participant-application rows in diagnostics. */
const val SUMERAGI_NATIVE_AMX_PARTICIPANT_APPLICATIONS_MAX: Int = 1_024

/** Maximum grouped source count represented by one Native AMX diagnostics row. */
const val SUMERAGI_NATIVE_AMX_PARTICIPANT_APPLICATION_SOURCES_MAX: Long = 4_096
const val SUMERAGI_AUTONOMOUS_LANE_EXECUTIONS_MAX: Int = 128
const val SUMERAGI_DIAGNOSTIC_LANES_MAX: Int = 128

/** Aggregate execution diagnostics for the latest block-pipeline run. */
@Serializable
data class SumeragiPipelineExecutionStatus(
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("tx_vertices_total")
    val txVerticesTotal: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("tx_edges_total")
    val txEdgesTotal: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("overlay_count_total")
    val overlayCountTotal: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("overlay_instr_total")
    val overlayInstrTotal: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("overlay_bytes_total")
    val overlayBytesTotal: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("rbc_chunks_total")
    val rbcChunksTotal: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("rbc_bytes_total")
    val rbcBytesTotal: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("detached_prepared_total")
    val detachedPreparedTotal: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("detached_merged_total")
    val detachedMergedTotal: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("detached_fallback_total")
    val detachedFallbackTotal: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("detached_fallback_fee_postprocessing_total")
    val detachedFallbackFeePostprocessingTotal: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("detached_fallback_user_executor_total")
    val detachedFallbackUserExecutorTotal: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("detached_fallback_durable_state_total")
    val detachedFallbackDurableStateTotal: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("detached_fallback_unsupported_instruction_total")
    val detachedFallbackUnsupportedInstructionTotal: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("detached_fallback_rejected_eval_total")
    val detachedFallbackRejectedEvalTotal: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("detached_fallback_overlay_error_total")
    val detachedFallbackOverlayErrorTotal: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("quarantine_executed_total")
    val quarantineExecutedTotal: BigInteger,
)

/** Permissionless-election diagnostics present only while NPoS mode is active. */
@Serializable
data class SumeragiNposDiagnostics(
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("epoch_length_blocks")
    val epochLengthBlocks: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("vrf_commit_deadline_offset")
    val vrfCommitDeadlineOffset: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("vrf_reveal_deadline_offset")
    val vrfRevealDeadlineOffset: BigInteger,
    @SerialName("epoch_seed") val epochSeed: List<Int>,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("prf_height")
    val prfHeight: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("prf_view")
    val prfView: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("vrf_penalty_epoch")
    val vrfPenaltyEpoch: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("vrf_committed_no_reveal_total")
    val vrfCommittedNoRevealTotal: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("vrf_no_participation_total")
    val vrfNoParticipationTotal: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("vrf_late_reveals_total")
    val vrfLateRevealsTotal: BigInteger,
) {
    init {
        requireU64(epochLengthBlocks, "epochLengthBlocks")
        requireU64(vrfCommitDeadlineOffset, "vrfCommitDeadlineOffset")
        requireU64(vrfRevealDeadlineOffset, "vrfRevealDeadlineOffset")
        require(
            epochLengthBlocks.signum() > 0 &&
                vrfCommitDeadlineOffset.signum() > 0 &&
                vrfRevealDeadlineOffset.signum() > 0 &&
                vrfCommitDeadlineOffset < vrfRevealDeadlineOffset &&
                vrfRevealDeadlineOffset <= epochLengthBlocks
        ) { "NPoS diagnostics windows must be strictly ordered within the epoch" }
        require(epochSeed.size == 32 && epochSeed.all { it in 0..255 } && epochSeed.any { it != 0 }) {
            "NPoS diagnostics epoch seed must be an exact non-zero 32-byte vector"
        }
        listOf(
            prfHeight,
            prfView,
            vrfPenaltyEpoch,
            vrfCommittedNoRevealTotal,
            vrfNoParticipationTotal,
            vrfLateRevealsTotal,
        ).forEach { requireU64(it, "NPoS diagnostics counter") }
    }
}

/** Evidence-derived Native AMX participant application state. */
@Serializable
enum class SumeragiNativeAmxParticipantApplicationState {
    /** Participant QCs exist, but no canonical global carrier is committed. */
    @SerialName("certified_pending_carrier")
    CERTIFIED_PENDING_CARRIER,

    /** A carrier is committed, but its exact durable evidence is incomplete. */
    @SerialName("committed_evidence_pending")
    COMMITTED_EVIDENCE_PENDING,

    /** The application sidecar and replicated frontier both revalidate. */
    @SerialName("durably_applied")
    DURABLY_APPLIED,

    /** Same-height authenticated evidence contains conflicting identities. */
    @SerialName("conflict")
    CONFLICT,
}

/** One Native AMX participant-application row from `/v1/sumeragi/diagnostics`. */
@Serializable
data class SumeragiNativeAmxParticipantApplication(
    @SerialName("lane_id") val laneId: Long,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("dataspace_id")
    val dataspaceId: BigInteger,
    @SerialName("lane_incarnation") val laneIncarnation: String,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("participant_height")
    val participantHeight: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("participant_view")
    val participantView: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("predecessor_height")
    val predecessorHeight: BigInteger,
    @SerialName("predecessor_descriptor_hash") val predecessorDescriptorHash: String? = null,
    @SerialName("descriptor_hash") val descriptorHash: String,
    @SerialName("proposal_hash") val proposalHash: String,
    @SerialName("settlement_hash") val settlementHash: String,
    @SerialName("source_count") val sourceCount: Long,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("application_block_height")
    val applicationBlockHeight: BigInteger? = null,
    @SerialName("application_block_hash") val applicationBlockHash: String? = null,
    val state: SumeragiNativeAmxParticipantApplicationState,
) {
    init {
        require(laneId in 0..0xffff_ffffL) { "laneId must be an unsigned 32-bit value" }
        requireU64(dataspaceId, "dataspaceId")
        requireU64(participantHeight, "participantHeight")
        requireU64(participantView, "participantView")
        requireU64(predecessorHeight, "predecessorHeight")
        require(participantHeight.signum() > 0) {
            "participant height must be positive and view must be non-negative"
        }
        require(
            predecessorHeight.add(BigInteger.ONE) == participantHeight &&
                (predecessorHeight == BigInteger.ZERO) == (predecessorDescriptorHash == null)
        ) { "Native AMX participant predecessor geometry is inconsistent" }
        require(sourceCount in 1..SUMERAGI_NATIVE_AMX_PARTICIPANT_APPLICATION_SOURCES_MAX) {
            "Native AMX participant source count is out of bounds"
        }
        require((applicationBlockHeight == null) == (applicationBlockHash == null)) {
            "application block height and hash must appear together"
        }
        applicationBlockHeight?.let {
            requireU64(it, "applicationBlockHeight")
            require(it.signum() > 0) { "application block height must be positive" }
        }
        val requiresApplicationBlock =
            state == SumeragiNativeAmxParticipantApplicationState.COMMITTED_EVIDENCE_PENDING ||
                state == SumeragiNativeAmxParticipantApplicationState.DURABLY_APPLIED
        require((applicationBlockHeight != null) == requiresApplicationBlock) {
            "Native AMX participant state and application block identity disagree"
        }

        requireCanonicalNonzeroHash(laneIncarnation, "laneIncarnation")
        predecessorDescriptorHash?.let {
            requireCanonicalNonzeroHash(it, "predecessorDescriptorHash")
        }
        requireCanonicalNonzeroHash(descriptorHash, "descriptorHash")
        requireCanonicalNonzeroHash(proposalHash, "proposalHash")
        requireCanonicalNonzeroHash(settlementHash, "settlementHash")
        applicationBlockHash?.let {
            requireCanonicalNonzeroHash(it, "applicationBlockHash")
        }
    }
}

/**
 * Bounded, canonically ordered Native AMX participant diagnostics vector.
 *
 * The key order matches Rust and the other SDKs:
 * `(lane_id, dataspace_id, lane_incarnation)`.
 */
class SumeragiNativeAmxParticipantApplications(rows: List<SumeragiNativeAmxParticipantApplication>) {
    /** Immutable ordered rows. */
    @JvmField
    val rows: List<SumeragiNativeAmxParticipantApplication>

    init {
        require(rows.size <= SUMERAGI_NATIVE_AMX_PARTICIPANT_APPLICATIONS_MAX) {
            "Native AMX participant diagnostics exceed the 1024-row limit"
        }
        rows.zipWithNext().forEach { (previous, current) ->
            require(compareRoute(previous, current) < 0) {
                "Native AMX participant diagnostics must be strictly ordered by route and incarnation"
            }
        }
        this.rows = Collections.unmodifiableList(rows.toList())
    }

    private fun compareRoute(
        left: SumeragiNativeAmxParticipantApplication,
        right: SumeragiNativeAmxParticipantApplication,
    ): Int {
        val lane = left.laneId.compareTo(right.laneId)
        if (lane != 0) return lane
        val dataspace = left.dataspaceId.compareTo(right.dataspaceId)
        if (dataspace != 0) return dataspace
        return left.laneIncarnation.compareTo(right.laneIncarnation)
    }
}

@Serializable
enum class SumeragiAutonomousLaneExecutionStage {
    @SerialName("reservations_durable") RESERVATIONS_DURABLE,
    @SerialName("executable_payload_durable") EXECUTABLE_PAYLOAD_DURABLE,
    @SerialName("payload_availability_certified") PAYLOAD_AVAILABILITY_CERTIFIED,
    @SerialName("lane_certified") LANE_CERTIFIED,
    @SerialName("certified_bundle_durable") CERTIFIED_BUNDLE_DURABLE,
    @SerialName("merge_candidate_durable") MERGE_CANDIDATE_DURABLE,
    @SerialName("global_carrier_committed") GLOBAL_CARRIER_COMMITTED,
    @SerialName("kura_wsv_application_receipt_durable") KURA_WSV_APPLICATION_RECEIPT_DURABLE,
    @SerialName("queue_finalized") QUEUE_FINALIZED,
    @SerialName("conflict") CONFLICT,
}

@Serializable
enum class SumeragiAutonomousLaneExecutionStuckReason {
    @SerialName("awaiting_executable_payload") AWAITING_EXECUTABLE_PAYLOAD,
    @SerialName("awaiting_payload_availability") AWAITING_PAYLOAD_AVAILABILITY,
    @SerialName("awaiting_lane_certification") AWAITING_LANE_CERTIFICATION,
    @SerialName("certified_bundle_unavailable") CERTIFIED_BUNDLE_UNAVAILABLE,
    @SerialName("awaiting_merge_selection") AWAITING_MERGE_SELECTION,
    @SerialName("awaiting_global_carrier") AWAITING_GLOBAL_CARRIER,
    @SerialName("awaiting_application_receipt") AWAITING_APPLICATION_RECEIPT,
    @SerialName("queue_finalization_unverifiable") QUEUE_FINALIZATION_UNVERIFIABLE,
    @SerialName("evidence_conflict") EVIDENCE_CONFLICT,
}

@Serializable
class SumeragiAutonomousLaneExecution(
    @SerialName("lane_id") val laneId: Long,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("dataspace_id") val dataspaceId: BigInteger,
    @SerialName("lane_incarnation") val laneIncarnation: String,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("lane_block_height") val laneBlockHeight: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("lane_block_view") val laneBlockView: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("proposal_height") val proposalHeight: BigInteger,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("proposal_view") val proposalView: BigInteger? = null,
    @SerialName("reservation_owner_hash") val reservationOwnerHash: String,
    @SerialName("proposal_identity_hash") val proposalIdentityHash: String,
    @SerialName("reservation_group_hash") val reservationGroupHash: String,
    @SerialName("proposal_hash") val proposalHash: String? = null,
    @SerialName("descriptor_hash") val descriptorHash: String? = null,
    @SerialName("executable_payload_hash") val executablePayloadHash: String? = null,
    @SerialName("source_bundle_hash") val sourceBundleHash: String? = null,
    @SerialName("merge_entry_hash") val mergeEntryHash: String? = null,
    @Serializable(with = SumeragiU64Serializer::class)
    @SerialName("application_block_height") val applicationBlockHeight: BigInteger? = null,
    @SerialName("application_block_hash") val applicationBlockHash: String? = null,
    @SerialName("reservation_count") val reservationCount: Long,
    @SerialName("transaction_count") val transactionCount: Long,
    @SerialName("highest_durable_stage")
    val highestDurableStage: SumeragiAutonomousLaneExecutionStage,
    @SerialName("stuck_reason") val stuckReason: SumeragiAutonomousLaneExecutionStuckReason? = null,
) {
    init {
        require(laneId in 0..0xffff_ffffL)
        listOf(dataspaceId, laneBlockHeight, laneBlockView, proposalHeight)
            .forEach { requireU64(it, "autonomous execution coordinate") }
        proposalView?.let { requireU64(it, "autonomous proposal view") }
        require(laneBlockHeight.signum() > 0 && proposalHeight.signum() > 0)
        require(transactionCount in 1..4_096 && reservationCount in 0..4_096)
        require((applicationBlockHeight == null) == (applicationBlockHash == null))
        applicationBlockHeight?.let {
            requireU64(it, "applicationBlockHeight")
            require(it.signum() > 0)
        }
        listOf(
            laneIncarnation, reservationOwnerHash, proposalIdentityHash, reservationGroupHash,
        ).forEach {
            requireCanonicalNonzeroHash(it, "autonomous execution hash")
        }
        listOfNotNull(
            proposalHash, descriptorHash, executablePayloadHash, sourceBundleHash,
            mergeEntryHash, applicationBlockHash,
        ).forEach { requireCanonicalNonzeroHash(it, "autonomous execution optional hash") }
        val expectedReason = when (highestDurableStage) {
            SumeragiAutonomousLaneExecutionStage.RESERVATIONS_DURABLE ->
                SumeragiAutonomousLaneExecutionStuckReason.AWAITING_EXECUTABLE_PAYLOAD
            SumeragiAutonomousLaneExecutionStage.EXECUTABLE_PAYLOAD_DURABLE ->
                SumeragiAutonomousLaneExecutionStuckReason.AWAITING_PAYLOAD_AVAILABILITY
            SumeragiAutonomousLaneExecutionStage.PAYLOAD_AVAILABILITY_CERTIFIED ->
                SumeragiAutonomousLaneExecutionStuckReason.AWAITING_LANE_CERTIFICATION
            SumeragiAutonomousLaneExecutionStage.LANE_CERTIFIED ->
                SumeragiAutonomousLaneExecutionStuckReason.CERTIFIED_BUNDLE_UNAVAILABLE
            SumeragiAutonomousLaneExecutionStage.CERTIFIED_BUNDLE_DURABLE ->
                SumeragiAutonomousLaneExecutionStuckReason.AWAITING_MERGE_SELECTION
            SumeragiAutonomousLaneExecutionStage.MERGE_CANDIDATE_DURABLE ->
                SumeragiAutonomousLaneExecutionStuckReason.AWAITING_GLOBAL_CARRIER
            SumeragiAutonomousLaneExecutionStage.GLOBAL_CARRIER_COMMITTED ->
                SumeragiAutonomousLaneExecutionStuckReason.AWAITING_APPLICATION_RECEIPT
            SumeragiAutonomousLaneExecutionStage.KURA_WSV_APPLICATION_RECEIPT_DURABLE ->
                SumeragiAutonomousLaneExecutionStuckReason.QUEUE_FINALIZATION_UNVERIFIABLE
            SumeragiAutonomousLaneExecutionStage.QUEUE_FINALIZED -> null
            SumeragiAutonomousLaneExecutionStage.CONFLICT ->
                SumeragiAutonomousLaneExecutionStuckReason.EVIDENCE_CONFLICT
        }
        require(stuckReason == expectedReason)
        require((proposalHash == null) == (descriptorHash == null)) {
            "Autonomous proposal and descriptor hashes must appear together"
        }
        if (highestDurableStage != SumeragiAutonomousLaneExecutionStage.CONFLICT) {
            require(reservationCount == transactionCount)
            require(
                (highestDurableStage == SumeragiAutonomousLaneExecutionStage.RESERVATIONS_DURABLE) ==
                    (proposalHash == null)
            ) { "Autonomous finalized identity disagrees with durable stage" }
            require(
                highestDurableStage != SumeragiAutonomousLaneExecutionStage.RESERVATIONS_DURABLE ||
                    proposalView == null
            ) { "Autonomous proposal view disagrees with durable stage" }
            val hasPayload = executablePayloadHash != null
            val hasBundle = sourceBundleHash != null
            val hasMerge = mergeEntryHash != null
            val hasCarrier = applicationBlockHeight != null
            val geometryMatches = when (highestDurableStage) {
                SumeragiAutonomousLaneExecutionStage.RESERVATIONS_DURABLE ->
                    !hasPayload && !hasBundle && !hasMerge && !hasCarrier
                SumeragiAutonomousLaneExecutionStage.EXECUTABLE_PAYLOAD_DURABLE,
                SumeragiAutonomousLaneExecutionStage.PAYLOAD_AVAILABILITY_CERTIFIED,
                SumeragiAutonomousLaneExecutionStage.LANE_CERTIFIED,
                -> hasPayload && !hasBundle && !hasMerge && !hasCarrier
                SumeragiAutonomousLaneExecutionStage.CERTIFIED_BUNDLE_DURABLE ->
                    hasPayload && hasBundle && !hasMerge && !hasCarrier
                SumeragiAutonomousLaneExecutionStage.MERGE_CANDIDATE_DURABLE,
                SumeragiAutonomousLaneExecutionStage.GLOBAL_CARRIER_COMMITTED,
                -> hasPayload && hasBundle && hasMerge && !hasCarrier
                SumeragiAutonomousLaneExecutionStage.KURA_WSV_APPLICATION_RECEIPT_DURABLE,
                SumeragiAutonomousLaneExecutionStage.QUEUE_FINALIZED,
                -> hasPayload && hasBundle && hasMerge && hasCarrier
                SumeragiAutonomousLaneExecutionStage.CONFLICT -> true
            }
            require(geometryMatches)
        }
    }

    override fun equals(other: Any?): Boolean =
        other is SumeragiAutonomousLaneExecution &&
            laneId == other.laneId && dataspaceId == other.dataspaceId &&
            laneIncarnation == other.laneIncarnation &&
            laneBlockHeight == other.laneBlockHeight && laneBlockView == other.laneBlockView &&
            proposalHeight == other.proposalHeight && proposalView == other.proposalView &&
            reservationOwnerHash == other.reservationOwnerHash &&
            proposalIdentityHash == other.proposalIdentityHash &&
            reservationGroupHash == other.reservationGroupHash &&
            proposalHash == other.proposalHash && descriptorHash == other.descriptorHash &&
            executablePayloadHash == other.executablePayloadHash &&
            sourceBundleHash == other.sourceBundleHash && mergeEntryHash == other.mergeEntryHash &&
            applicationBlockHeight == other.applicationBlockHeight &&
            applicationBlockHash == other.applicationBlockHash &&
            reservationCount == other.reservationCount && transactionCount == other.transactionCount &&
            highestDurableStage == other.highestDurableStage && stuckReason == other.stuckReason

    override fun hashCode(): Int = listOf(
        laneId, dataspaceId, laneIncarnation, laneBlockHeight, laneBlockView,
        proposalHeight, proposalView, reservationOwnerHash, proposalIdentityHash,
        reservationGroupHash, proposalHash, descriptorHash, executablePayloadHash,
        sourceBundleHash, mergeEntryHash, applicationBlockHeight, applicationBlockHash,
        reservationCount, transactionCount, highestDurableStage, stuckReason,
    ).hashCode()
}

class SumeragiAutonomousLaneExecutions(rows: List<SumeragiAutonomousLaneExecution>) {
    @JvmField val rows: List<SumeragiAutonomousLaneExecution>

    init {
        require(rows.size <= SUMERAGI_AUTONOMOUS_LANE_EXECUTIONS_MAX)
        rows.zipWithNext().forEach { (left, right) ->
            val leftKey = listOf(
                BigInteger.valueOf(left.laneId), left.dataspaceId,
                BigInteger(1, HashLiteral.decode(left.laneIncarnation)),
                left.laneBlockHeight, left.laneBlockView, left.proposalHeight,
                BigInteger(1, HashLiteral.decode(left.proposalIdentityHash)),
            )
            val rightKey = listOf(
                BigInteger.valueOf(right.laneId), right.dataspaceId,
                BigInteger(1, HashLiteral.decode(right.laneIncarnation)),
                right.laneBlockHeight, right.laneBlockView, right.proposalHeight,
                BigInteger(1, HashLiteral.decode(right.proposalIdentityHash)),
            )
            require(leftKey.zip(rightKey).firstOrNull { it.first != it.second }
                ?.let { it.first < it.second } == true)
        }
        this.rows = Collections.unmodifiableList(rows.toList())
    }
}

/**
 * Complete operational response returned by `/v1/sumeragi/diagnostics`.
 *
 * Lane evidence types that are not yet interpreted by core-jvm are retained as exact JSON
 * objects. Native-bearing settlement commitments, including relay-contained commitments, are
 * additionally routed through the strict Native AMX V2 parser. Native participant applications
 * and autonomous execution rows are parsed into their closed, bounded models and checked in the
 * canonical Rust ordering.
 */
@Serializable
data class SumeragiDiagnosticsStatus(
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
    init {
        requireU64(txQueueDepth, "txQueueDepth")
        requireU64(txQueueCapacity, "txQueueCapacity")
        requireU64(txQueueRetainedBytes, "txQueueRetainedBytes")
        requireU64(txQueueMaxRetainedBytes, "txQueueMaxRetainedBytes")
        requireU64(txQueueOldestQueuedAgeMs, "txQueueOldestQueuedAgeMs")
        require(txQueueDepth <= txQueueCapacity) {
            "Sumeragi diagnostics transaction queue depth exceeds capacity"
        }
        require(txQueueRetainedBytes <= txQueueMaxRetainedBytes) {
            "Sumeragi diagnostics retained queue bytes exceed the byte budget"
        }
        require(
            txQueueSaturated ==
                (txQueueSaturatedByCount || txQueueSaturatedByBytes || txQueueSaturatedByAge)
        ) { "Sumeragi diagnostics queue saturation disagrees with its causes" }
        val laneVectors = listOf(
            laneCommitments,
            dataspaceCommitments,
            laneSettlementCommitments,
            laneRelayEnvelopes,
            lanePayloadOwnerships,
            committedLaneBlocks,
            laneBlockSessions,
            laneGovernance,
        )
        require(laneVectors.all { it.size <= SUMERAGI_DIAGNOSTIC_LANES_MAX }) {
            "Sumeragi diagnostics lane vector exceeds the 128-row limit"
        }
        validateNativeAmxDiagnosticsEvidence(
            laneSettlementCommitments,
            laneRelayEnvelopes,
        )
        require(laneGovernanceSealedTotal in 0..0xffff_ffffL) {
            "laneGovernanceSealedTotal must be an unsigned 32-bit value"
        }
        require(laneGovernanceSealedAliases.size <= SUMERAGI_DIAGNOSTIC_LANES_MAX) {
            "sealed lane aliases exceed the 128-row limit"
        }
        require(
            laneGovernanceSealedAliases.all {
                it.isNotEmpty() && it.trim() == it
            } &&
                laneGovernanceSealedAliases.distinct().size ==
                laneGovernanceSealedAliases.size &&
                laneGovernanceSealedTotal == laneGovernanceSealedAliases.size.toLong()
        ) { "sealed lane aliases must be exact, unique, and match the sealed total" }
        SumeragiNativeAmxParticipantApplications(nativeAmxParticipantApplications)
        SumeragiAutonomousLaneExecutions(autonomousLaneExecutions)
    }

    companion object {
        /** Parse one strict UTF-8 JSON diagnostics response. */
        @JvmStatic
        fun parseJson(payload: ByteArray): SumeragiDiagnosticsStatus {
            require(payload.isNotEmpty()) { "Sumeragi diagnostics response must not be empty" }
            require(payload.size.toLong() <= SUMERAGI_DIAGNOSTICS_JSON_MAX_BYTES) {
                "Sumeragi diagnostics response exceeds $SUMERAGI_DIAGNOSTICS_JSON_MAX_BYTES bytes"
            }
            return parseJson(SumeragiJsonPrimitives.decodeUtf8(payload, "Sumeragi diagnostics"))
        }

        /** Parse one strict JSON diagnostics response. */
        @JvmStatic
        fun parseJson(payload: String): SumeragiDiagnosticsStatus {
            JsonParser.parse(payload)
            return STRICT_SUMERAGI_DIAGNOSTICS_JSON.decodeFromString(payload)
        }
    }
}

private val STRICT_SUMERAGI_DIAGNOSTICS_JSON = Json {
    ignoreUnknownKeys = false
    isLenient = false
    coerceInputValues = false
}

internal object SumeragiU64Serializer : KSerializer<BigInteger> {
    override val descriptor: SerialDescriptor =
        PrimitiveSerialDescriptor("org.hyperledger.iroha.sdk.consensus.UInt64", PrimitiveKind.LONG)

    override fun serialize(encoder: Encoder, value: BigInteger) {
        val jsonEncoder = encoder as? JsonEncoder
            ?: throw SerializationException("Sumeragi UInt64 values require JSON encoding")
        requireU64(value, "Sumeragi UInt64")
        jsonEncoder.encodeJsonElement(JsonPrimitive(value))
    }

    override fun deserialize(decoder: Decoder): BigInteger {
        val jsonDecoder = decoder as? JsonDecoder
            ?: throw SerializationException("Sumeragi UInt64 values require JSON decoding")
        val primitive = jsonDecoder.decodeJsonElement() as? JsonPrimitive
            ?: throw SerializationException("Sumeragi UInt64 value must be a JSON number")
        if (primitive.isString || !CANONICAL_U64_TOKEN.matches(primitive.content)) {
            throw SerializationException(
                "Sumeragi UInt64 value must be an unquoted canonical integer token",
            )
        }
        val value = try {
            BigInteger(primitive.content)
        } catch (error: NumberFormatException) {
            throw SerializationException("Sumeragi UInt64 value is malformed", error)
        }
        if (value.signum() < 0 || value > U64_MAX) {
            throw SerializationException("Sumeragi UInt64 value is out of range")
        }
        return value
    }
}

private val CANONICAL_U64_TOKEN = Regex("0|[1-9][0-9]*")
private val U64_MAX: BigInteger = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)

private fun validateNativeAmxDiagnosticsEvidence(
    settlements: List<JsonObject>,
    relays: List<JsonObject>,
) {
    settlements.forEachIndexed { index, settlement ->
        validateNativeAmxSettlementEvidence(
            settlement,
            "lane_settlement_commitments[$index]",
        )
    }
    relays.forEachIndexed { index, relay ->
        val settlement = relay["settlement_commitment"]
        require(settlement is JsonObject) {
            "lane_relay_envelopes[$index].settlement_commitment must be a JSON object"
        }
        validateNativeAmxSettlementEvidence(
            settlement,
            "lane_relay_envelopes[$index].settlement_commitment",
        )
    }
}

private fun validateNativeAmxSettlementEvidence(
    settlement: JsonObject,
    field: String,
) {
    val nativeReceipts = settlement["native_amx_receipts"]
    require(nativeReceipts is JsonArray) {
        "$field.native_amx_receipts must be a JSON array"
    }
    if (nativeReceipts.isEmpty()) return

    try {
        NativeAmxV2.parseReceiptGroup(settlement.toString())
    } catch (error: IllegalArgumentException) {
        throw IllegalArgumentException(
            "$field contains invalid Native AMX V2 evidence",
            error,
        )
    }
}

private fun requireU64(value: BigInteger, field: String): BigInteger {
    return SumeragiJsonPrimitives.requireU64(value, field)
}

private fun requireCanonicalNonzeroHash(value: String, field: String) {
    SumeragiJsonPrimitives.requireCanonicalNonzeroHash(value, field)
}
