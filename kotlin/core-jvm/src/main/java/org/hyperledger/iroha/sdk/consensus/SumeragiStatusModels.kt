// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.consensus

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import java.nio.charset.StandardCharsets
import java.util.Collections
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.util.HashLiteral

/** The only authoritative Sumeragi status protocol revision accepted by this SDK. */
const val SUMERAGI_STATUS_PROTOCOL_VERSION: Int = 4

/** Maximum encoded JSON body accepted from `/v1/sumeragi/status`. */
const val SUMERAGI_STATUS_JSON_MAX_BYTES: Long = 1L * 1024L * 1024L

/** Maximum encoded JSON body accepted from `/v1/sumeragi/diagnostics`. */
const val SUMERAGI_DIAGNOSTICS_JSON_MAX_BYTES: Long = 16L * 1024L * 1024L

/** Stable value semantics for public immutable status models without data-class ABI. */
abstract class SumeragiStatusValue internal constructor() {
    protected abstract fun equalityFields(): List<Any?>

    final override fun equals(other: Any?): Boolean =
        other != null && javaClass == other.javaClass &&
            equalityFields() == (other as SumeragiStatusValue).equalityFields()

    final override fun hashCode(): Int = 31 * javaClass.hashCode() + equalityFields().hashCode()
}

/** One-element hash tuple identifying a frozen height context. */
class SumeragiStatusContextId internal constructor(
    @JvmField val hash: String,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> = listOf(hash)
}

/** Consensus round in one frozen height context. */
class SumeragiStatusRound internal constructor(
    @JvmField val contextId: SumeragiStatusContextId,
    @JvmField val height: BigInteger,
    @JvmField val view: BigInteger,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> = listOf(contextId, height, view)
}

/** Exact block and payload identity certified by Sumeragi. */
class SumeragiStatusBlockSubject internal constructor(
    @JvmField val parentBlockHash: String?,
    @JvmField val blockHash: String,
    @JvmField val payloadHash: String,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> =
        listOf(parentBlockHash, blockHash, payloadHash)
}

/** Exact Merkle root and non-zero leaf count of canonical lane-finality statements. */
class SumeragiStatusLaneFinalityManifestCommitment internal constructor(
    @JvmField val root: String,
    @JvmField val leafCount: BigInteger,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> = listOf(root, leafCount)
}

/** Exact merge-ledger entry identity authenticated by a global certificate. */
class SumeragiStatusMergeCarrierCommitment internal constructor(
    @JvmField val version: Int,
    @JvmField val entryHash: String,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> = listOf(version, entryHash)
}

/** Deterministic execution commitment authenticated by a global certificate. */
class SumeragiStatusExecutionCommitment internal constructor(
    @JvmField val parentStateRoot: String,
    @JvmField val postStateRoot: String,
    @JvmField val ordinaryWritesRoot: String,
    @JvmField val topupAnchorRoot: String?,
    @JvmField val topupAnchorCount: BigInteger,
    @JvmField val nativeAmxApplicationManifestVersion: Int,
    @JvmField val nativeAmxApplicationManifestRoot: String,
    @JvmField val nativeAmxApplicationManifestCount: BigInteger,
    @JvmField val laneFinalityManifest: SumeragiStatusLaneFinalityManifestCommitment?,
    @JvmField val mergeCarrier: SumeragiStatusMergeCarrierCommitment?,
    @JvmField val executedBlockWireLen: BigInteger,
    @JvmField val executedBlockWireHash: String,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> = listOf(
        parentStateRoot,
        postStateRoot,
        ordinaryWritesRoot,
        topupAnchorRoot,
        topupAnchorCount,
        nativeAmxApplicationManifestVersion,
        nativeAmxApplicationManifestRoot,
        nativeAmxApplicationManifestCount,
        laneFinalityManifest,
        mergeCarrier,
        executedBlockWireLen,
        executedBlockWireHash,
    )
}

/** Global voting phase carried by a quorum-certificate reference. */
enum class SumeragiStatusGlobalPhase(@JvmField val wireName: String) {
    PREPARE("prepare"),
    COMMIT("commit"),
}

/** Stable semantic reference to one Sumeragi quorum certificate. */
class SumeragiStatusQcReference internal constructor(
    @JvmField val round: SumeragiStatusRound,
    @JvmField val proposalRound: SumeragiStatusRound,
    @JvmField val phase: SumeragiStatusGlobalPhase,
    @JvmField val subject: SumeragiStatusBlockSubject,
    @JvmField val executionCommitment: SumeragiStatusExecutionCommitment,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> =
        listOf(round, proposalRound, phase, subject, executionCommitment)
}

/** Stable semantic reference to the latest installed timeout certificate. */
class SumeragiStatusTimeoutCertificate internal constructor(
    @JvmField val round: SumeragiStatusRound,
    @JvmField val highestPrepareQc: SumeragiStatusQcReference?,
    @JvmField val certificateHash: String,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> =
        listOf(round, highestPrepareQc, certificateHash)
}

/** Consensus mode frozen into the active height context. */
enum class SumeragiStatusConsensusMode(@JvmField val wireName: String) {
    PERMISSIONED("permissioned"),
    NPOS("npos"),
}

/** Equal-vote quorum inputs frozen for one height. */
class SumeragiStatusQuorum internal constructor(
    @JvmField val minSigners: BigInteger,
    @JvmField val totalPower: BigInteger,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> = listOf(minSigners, totalPower)
}

/** Frozen election and quorum inputs governing the active height. */
class SumeragiStatusHeightContext internal constructor(
    @JvmField val epoch: BigInteger,
    @JvmField val epochEndHeight: BigInteger,
    @JvmField val mode: SumeragiStatusConsensusMode,
    @JvmField val epochSeed: String,
    @JvmField val validatorCount: BigInteger,
    @JvmField val quorum: SumeragiStatusQuorum,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> =
        listOf(epoch, epochEndHeight, mode, epochSeed, validatorCount, quorum)
}

/** Latest durable CommitQC with exact count and voting-power totals. */
class SumeragiStatusCommitQc internal constructor(
    @JvmField val certificate: SumeragiStatusQcReference,
    @JvmField val validatorCount: BigInteger,
    @JvmField val signerCount: BigInteger,
    @JvmField val minSigners: BigInteger,
    @JvmField val signedPower: BigInteger,
    @JvmField val totalPower: BigInteger,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> = listOf(
        certificate,
        validatorCount,
        signerCount,
        minSigners,
        signedPower,
        totalPower,
    )
}

/** Partial dual quorum for one exact proposal round. */
class SumeragiStatusVoteQuorum internal constructor(
    @JvmField val round: SumeragiStatusRound,
    @JvmField val proposalRound: SumeragiStatusRound,
    @JvmField val subject: SumeragiStatusBlockSubject,
    @JvmField val executionCommitment: SumeragiStatusExecutionCommitment,
    @JvmField val signerCount: BigInteger,
    @JvmField val signedPower: BigInteger,
    @JvmField val minSigners: BigInteger,
    @JvmField val totalPower: BigInteger,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> = listOf(
        round,
        proposalRound,
        subject,
        executionCommitment,
        signerCount,
        signedPower,
        minSigners,
        totalPower,
    )
}

/** Partial timeout quorum for one exact round. */
class SumeragiStatusTimeoutQuorum internal constructor(
    @JvmField val round: SumeragiStatusRound,
    @JvmField val signerCount: BigInteger,
    @JvmField val signedPower: BigInteger,
    @JvmField val minSigners: BigInteger,
    @JvmField val totalPower: BigInteger,
    @JvmField val certificateFormed: Boolean,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> =
        listOf(round, signerCount, signedPower, minSigners, totalPower, certificateFormed)
}

/** Durable outbound protocol intent kind. */
enum class SumeragiStatusOutboundIntentKind(@JvmField val wireName: String) {
    PROPOSAL("proposal"),
    PREPARE_VOTE("prepare_vote"),
    COMMIT_VOTE("commit_vote"),
    TIMEOUT_VOTE("timeout_vote"),
    PREPARE_QC("prepare_qc"),
    COMMIT_QC("commit_qc"),
    TIMEOUT_CERTIFICATE("timeout_certificate"),
}

/** Durable delivery stage of an outbound protocol intent. */
enum class SumeragiStatusOutboundIntentStage(@JvmField val wireName: String) {
    PENDING_PERSISTENCE("pending_persistence"),
    PENDING_SIGNATURE("pending_signature"),
    QUEUED("queued"),
    SENT("sent"),
}

/** Durable outbound protocol intent and its exact optional proposal identity. */
class SumeragiStatusOutboundIntent internal constructor(
    @JvmField val kind: SumeragiStatusOutboundIntentKind,
    @JvmField val round: SumeragiStatusRound,
    @JvmField val proposalRound: SumeragiStatusRound?,
    @JvmField val subject: SumeragiStatusBlockSubject?,
    @JvmField val executionCommitment: SumeragiStatusExecutionCommitment?,
    @JvmField val stage: SumeragiStatusOutboundIntentStage,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> =
        listOf(kind, round, proposalRound, subject, executionCommitment, stage)
}

/** Local terminating-work stage. */
enum class SumeragiStatusWorkStage(@JvmField val wireName: String) {
    IDLE("idle"),
    QUEUED("queued"),
    RUNNING("running"),
    COMPLETE("complete"),
}

/** Local terminating-work stages for the active height. */
class SumeragiStatusWork internal constructor(
    @JvmField val candidate: SumeragiStatusWorkStage,
    @JvmField val bodyRecovery: SumeragiStatusWorkStage,
    @JvmField val bodyStore: SumeragiStatusWorkStage,
    @JvmField val validation: SumeragiStatusWorkStage,
    @JvmField val application: SumeragiStatusWorkStage,
    @JvmField val successorHeight: SumeragiStatusWorkStage,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> =
        listOf(candidate, bodyRecovery, bodyStore, validation, application, successorHeight)
}

/** Identity of a bounded reducer or runtime progress queue. */
enum class SumeragiStatusQueueKind(@JvmField val wireName: String) {
    INGRESS("ingress"),
    DEFERRED_NORMAL("deferred_normal"),
    DEFERRED_PROGRESS("deferred_progress"),
    DEFERRED_COMPLETION("deferred_completion"),
    RUNTIME_NORMAL("runtime_normal"),
    RUNTIME_PROGRESS("runtime_progress"),
    RUNTIME_COMPLETION("runtime_completion"),
    EFFECT_COMPLETION("effect_completion"),
    NETWORK_INGRESS("network_ingress"),
    EFFECT_DISPATCH("effect_dispatch"),
}

/** Occupancy and accumulated service debt for one bounded queue. */
class SumeragiStatusQueue internal constructor(
    @JvmField val queue: SumeragiStatusQueueKind,
    @JvmField val depth: BigInteger,
    @JvmField val capacity: BigInteger,
    @JvmField val oldestAgeMs: BigInteger?,
    @JvmField val serviceDebt: BigInteger,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> =
        listOf(queue, depth, capacity, oldestAgeMs, serviceDebt)
}

/** Reducer transition tracked as authoritative liveness progress. */
enum class SumeragiStatusProgressTransition(@JvmField val wireName: String) {
    PROPOSAL_ADMITTED("proposal_admitted"),
    BODY_AVAILABLE("body_available"),
    BODY_STORED("body_stored"),
    BODY_VALIDATED("body_validated"),
    PREPARE_VOTE_ADMITTED("prepare_vote_admitted"),
    COMMIT_VOTE_ADMITTED("commit_vote_admitted"),
    TIMEOUT_VOTE_ADMITTED("timeout_vote_admitted"),
    PREPARE_QUORUM("prepare_quorum"),
    LOCK_INSTALLED("lock_installed"),
    COMMIT_QUORUM("commit_quorum"),
    TIMEOUT_CERTIFICATE_INSTALLED("timeout_certificate_installed"),
    DECISION_PERSISTED("decision_persisted"),
    APPLIED("applied"),
    SUCCESSOR_HEIGHT_ACTIVATED("successor_height_activated"),
    RECOVERY_REPLAYED("recovery_replayed"),
}

/** Last tracked reducer transition and its local age. */
class SumeragiStatusProgress internal constructor(
    @JvmField val generation: BigInteger,
    @JvmField val round: SumeragiStatusRound,
    @JvmField val transition: SumeragiStatusProgressTransition,
    @JvmField val ageMs: BigInteger,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> = listOf(generation, round, transition, ageMs)
}

/** Classified cause of an active no-progress interval. */
enum class SumeragiStatusLivenessBlocker(@JvmField val wireName: String) {
    MISSING_PROPOSAL("missing_proposal"),
    BODY_UNAVAILABLE("body_unavailable"),
    PREPARE_QUORUM_MISSING("prepare_quorum_missing"),
    COMMIT_QUORUM_MISSING("commit_quorum_missing"),
    TIMEOUT_CERTIFICATE_MISSING("timeout_certificate_missing"),
    SCHEDULER_STARVATION("scheduler_starvation"),
    APPLICATION_PENDING("application_pending"),
    SUCCESSOR_ACTIVATION_PENDING("successor_activation_pending"),
    LOCAL_CONTROL_PENDING("local_control_pending"),
}

/** Closed reducer reason for safely ignoring an input. */
enum class SumeragiStatusIgnoreReason(@JvmField val wireName: String) {
    WRONG_HEIGHT("wrong_height"),
    WRONG_VIEW("wrong_view"),
    STALE_GENERATION("stale_generation"),
    BUSY("busy"),
    DUPLICATE("duplicate"),
    NO_MATCHING_WORK("no_matching_work"),
    OBSERVER("observer"),
    VIEW_CLOSED("view_closed"),
    ALREADY_DECIDED("already_decided"),
    RECOVERY_PENDING("recovery_pending"),
    IRRELEVANT_VIEW("irrelevant_view"),
    UNSAFE_PROPOSAL("unsafe_proposal"),
}

/** Per-height count for one closed reducer ignore reason. */
class SumeragiStatusIgnoreCount internal constructor(
    @JvmField val reason: SumeragiStatusIgnoreReason,
    @JvmField val count: BigInteger,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> = listOf(reason, count)
}

/** Authoritative progress diagnostics for the active height. */
class SumeragiStatusLiveness internal constructor(
    @JvmField val generation: BigInteger,
    prepareQuorums: List<SumeragiStatusVoteQuorum>,
    commitQuorums: List<SumeragiStatusVoteQuorum>,
    timeoutQuorums: List<SumeragiStatusTimeoutQuorum>,
    outboundIntents: List<SumeragiStatusOutboundIntent>,
    @JvmField val work: SumeragiStatusWork,
    queues: List<SumeragiStatusQueue>,
    @JvmField val lastProgress: SumeragiStatusProgress?,
    @JvmField val noProgressAgeMs: BigInteger,
    @JvmField val blocker: SumeragiStatusLivenessBlocker?,
    ignoreCounts: List<SumeragiStatusIgnoreCount>,
) : SumeragiStatusValue() {
    @JvmField val prepareQuorums: List<SumeragiStatusVoteQuorum> = immutableCopy(prepareQuorums)
    @JvmField val commitQuorums: List<SumeragiStatusVoteQuorum> = immutableCopy(commitQuorums)
    @JvmField val timeoutQuorums: List<SumeragiStatusTimeoutQuorum> = immutableCopy(timeoutQuorums)
    @JvmField val outboundIntents: List<SumeragiStatusOutboundIntent> = immutableCopy(outboundIntents)
    @JvmField val queues: List<SumeragiStatusQueue> = immutableCopy(queues)
    @JvmField val ignoreCounts: List<SumeragiStatusIgnoreCount> = immutableCopy(ignoreCounts)

    override fun equalityFields(): List<Any?> = listOf(
        generation,
        prepareQuorums,
        commitQuorums,
        timeoutQuorums,
        outboundIntents,
        work,
        queues,
        lastProgress,
        noProgressAgeMs,
        blocker,
        ignoreCounts,
    )
}

/** Active authoritative reducer phase. */
enum class SumeragiStatusPhase(@JvmField val wireName: String) {
    AWAITING_PROPOSAL("awaiting_proposal"),
    RECONSTRUCTING_PAYLOAD("reconstructing_payload"),
    VALIDATING_PAYLOAD("validating_payload"),
    PREPARE("prepare"),
    COMMIT("commit"),
    PENDING_APPLY("pending_apply"),
}

/** Local body state paired with the authoritative reducer phase. */
enum class SumeragiStatusBodyState(@JvmField val wireName: String) {
    MISSING("missing"),
    RECONSTRUCTING("reconstructing"),
    STORED("stored"),
    VALIDATED("validated"),
    PENDING_APPLY("pending_apply"),
    APPLIED("applied"),
}

/**
 * Strict authoritative response returned by `GET /v1/sumeragi/status`.
 *
 * This JSON projection deliberately does not reuse [SumeragiV2Wire.SumeragiV2Status]: every
 * unsigned 64-bit JSON value is represented by [BigInteger], so values above `Long.MAX_VALUE`
 * remain exact on the JVM.
 */
class SumeragiV2Status internal constructor(
    @JvmField val protocolVersion: Int,
    @JvmField val nodeFingerprint: String,
    @JvmField val buildFingerprint: String,
    @JvmField val configFingerprint: String,
    @JvmField val restartRequired: Boolean,
    @JvmField val heightContextId: SumeragiStatusContextId,
    @JvmField val height: BigInteger,
    @JvmField val view: BigInteger,
    @JvmField val phase: SumeragiStatusPhase,
    @JvmField val leader: BigInteger,
    @JvmField val lockedPrepareQc: SumeragiStatusQcReference?,
    @JvmField val highestPrepareQc: SumeragiStatusQcReference?,
    @JvmField val lastTimeoutCertificate: SumeragiStatusTimeoutCertificate?,
    @JvmField val bodyState: SumeragiStatusBodyState,
    @JvmField val pendingPersistenceId: BigInteger?,
    @JvmField val lastCommittedHeight: BigInteger,
    @JvmField val lastCommittedSubject: SumeragiStatusBlockSubject?,
    @JvmField val heightContext: SumeragiStatusHeightContext,
    @JvmField val lastCommitQc: SumeragiStatusCommitQc?,
    @JvmField val liveness: SumeragiStatusLiveness,
) : SumeragiStatusValue() {
    override fun equalityFields(): List<Any?> = listOf(
        protocolVersion,
        nodeFingerprint,
        buildFingerprint,
        configFingerprint,
        restartRequired,
        heightContextId,
        height,
        view,
        phase,
        leader,
        lockedPrepareQc,
        highestPrepareQc,
        lastTimeoutCertificate,
        bodyState,
        pendingPersistenceId,
        lastCommittedHeight,
        lastCommittedSubject,
        heightContext,
        lastCommitQc,
        liveness,
    )

    companion object {
        /** Parse a fatal-UTF-8, duplicate-key rejecting authoritative status response. */
        @JvmStatic
        fun parseJson(payload: ByteArray): SumeragiV2Status {
            require(payload.isNotEmpty()) { "Sumeragi status response must not be empty" }
            require(payload.size.toLong() <= SUMERAGI_STATUS_JSON_MAX_BYTES) {
                "Sumeragi status response exceeds $SUMERAGI_STATUS_JSON_MAX_BYTES bytes"
            }
            return SumeragiStatusParser.parse(
                SumeragiJsonPrimitives.decodeUtf8(payload, "Sumeragi status"),
            )
        }

        /** Parse a duplicate-key rejecting authoritative status JSON response. */
        @JvmStatic
        fun parseJson(payload: String): SumeragiV2Status = SumeragiStatusParser.parse(payload)
    }
}

private object SumeragiStatusParser {
    private val topLevelRequired = setOf(
        "protocol_version", "node_fingerprint", "build_fingerprint", "config_fingerprint",
        "restart_required", "height_context_id", "height", "view", "phase", "leader",
        "body_state", "last_committed_height", "height_context", "liveness",
    )
    private val topLevelOptional = setOf(
        "locked_prepare_qc", "highest_prepare_qc", "last_timeout_certificate",
        "pending_persistence_id", "last_committed_subject", "last_commit_qc",
    )

    fun parse(payload: String): SumeragiV2Status {
        val root = SumeragiJsonPrimitives.parseObject(payload, "Sumeragi status")
        SumeragiJsonPrimitives.requireFields(
            root,
            topLevelRequired,
            topLevelOptional,
            "Sumeragi status",
        )
        val protocolVersion = SumeragiJsonPrimitives.u16(
            root["protocol_version"],
            "Sumeragi status.protocol_version",
        )
        require(protocolVersion == SUMERAGI_STATUS_PROTOCOL_VERSION) {
            "Sumeragi status.protocol_version must equal $SUMERAGI_STATUS_PROTOCOL_VERSION"
        }
        val heightContextId = contextId(root["height_context_id"], "Sumeragi status.height_context_id")
        val height = SumeragiJsonPrimitives.positiveU64(root["height"], "Sumeragi status.height")
        val view = SumeragiJsonPrimitives.u64(root["view"], "Sumeragi status.view")
        val phase = taggedEnum(
            root["phase"], "phase", "Sumeragi status.phase", SumeragiStatusPhase.values(),
        )
        val leader = SumeragiJsonPrimitives.u32(root["leader"], "Sumeragi status.leader")
        val bodyState = taggedEnum(
            root["body_state"], "state", "Sumeragi status.body_state",
            SumeragiStatusBodyState.values(),
        )
        val pendingPersistenceId = root["pending_persistence_id"]?.let {
            SumeragiJsonPrimitives.positiveU64(it, "Sumeragi status.pending_persistence_id")
        }
        val heightContext = heightContext(root["height_context"], "Sumeragi status.height_context")
        require(heightContext.epochEndHeight >= height) {
            "Sumeragi status height context must cover the active height"
        }
        require(leader < heightContext.validatorCount) {
            "Sumeragi status leader must index the frozen validator roster"
        }

        val lockedPrepareQc = root["locked_prepare_qc"]?.let {
            qcReference(it, "Sumeragi status.locked_prepare_qc")
        }
        val highestPrepareQc = root["highest_prepare_qc"]?.let {
            qcReference(it, "Sumeragi status.highest_prepare_qc")
        }
        val lastTimeoutCertificate = root["last_timeout_certificate"]?.let {
            timeoutCertificate(it, "Sumeragi status.last_timeout_certificate")
        }
        val lastCommittedHeight = SumeragiJsonPrimitives.u64(
            root["last_committed_height"],
            "Sumeragi status.last_committed_height",
        )
        val lastCommittedSubject = root["last_committed_subject"]?.let {
            blockSubject(it, "Sumeragi status.last_committed_subject")
        }
        val lastCommitQc = root["last_commit_qc"]?.let {
            commitQc(it, "Sumeragi status.last_commit_qc")
        }
        val liveness = liveness(
            root["liveness"],
            height,
            view,
            heightContextId,
            heightContext,
            "Sumeragi status.liveness",
        )

        validatePhaseAndFrontier(
            phase,
            bodyState,
            height,
            lockedPrepareQc,
            highestPrepareQc,
            lastTimeoutCertificate,
            lastCommittedHeight,
            lastCommittedSubject,
            lastCommitQc,
            view,
            heightContextId,
            heightContext,
        )

        return SumeragiV2Status(
            protocolVersion = protocolVersion,
            nodeFingerprint = SumeragiJsonPrimitives.hash(
                root["node_fingerprint"], "Sumeragi status.node_fingerprint",
            ),
            buildFingerprint = SumeragiJsonPrimitives.hash(
                root["build_fingerprint"], "Sumeragi status.build_fingerprint",
            ),
            configFingerprint = SumeragiJsonPrimitives.hash(
                root["config_fingerprint"], "Sumeragi status.config_fingerprint",
            ),
            restartRequired = SumeragiJsonPrimitives.boolean(
                root["restart_required"], "Sumeragi status.restart_required",
            ),
            heightContextId = heightContextId,
            height = height,
            view = view,
            phase = phase,
            leader = leader,
            lockedPrepareQc = lockedPrepareQc,
            highestPrepareQc = highestPrepareQc,
            lastTimeoutCertificate = lastTimeoutCertificate,
            bodyState = bodyState,
            pendingPersistenceId = pendingPersistenceId,
            lastCommittedHeight = lastCommittedHeight,
            lastCommittedSubject = lastCommittedSubject,
            heightContext = heightContext,
            lastCommitQc = lastCommitQc,
            liveness = liveness,
        )
    }

    private fun validatePhaseAndFrontier(
        phase: SumeragiStatusPhase,
        bodyState: SumeragiStatusBodyState,
        height: BigInteger,
        lockedPrepareQc: SumeragiStatusQcReference?,
        highestPrepareQc: SumeragiStatusQcReference?,
        lastTimeoutCertificate: SumeragiStatusTimeoutCertificate?,
        lastCommittedHeight: BigInteger,
        lastCommittedSubject: SumeragiStatusBlockSubject?,
        lastCommitQc: SumeragiStatusCommitQc?,
        view: BigInteger,
        activeContextId: SumeragiStatusContextId,
        activeHeightContext: SumeragiStatusHeightContext,
    ) {
        val phaseBodyValid = when (phase) {
            SumeragiStatusPhase.AWAITING_PROPOSAL -> bodyState == SumeragiStatusBodyState.MISSING
            SumeragiStatusPhase.RECONSTRUCTING_PAYLOAD ->
                bodyState == SumeragiStatusBodyState.RECONSTRUCTING
            SumeragiStatusPhase.VALIDATING_PAYLOAD -> bodyState == SumeragiStatusBodyState.STORED
            SumeragiStatusPhase.PREPARE, SumeragiStatusPhase.COMMIT ->
                bodyState == SumeragiStatusBodyState.VALIDATED
            SumeragiStatusPhase.PENDING_APPLY ->
                bodyState == SumeragiStatusBodyState.PENDING_APPLY ||
                    bodyState == SumeragiStatusBodyState.APPLIED
        }
        require(phaseBodyValid) { "Sumeragi status phase and body state are inconsistent" }
        require(phase != SumeragiStatusPhase.COMMIT || lockedPrepareQc != null) {
            "Sumeragi status commit phase requires a PrepareQC lock"
        }
        require(phase != SumeragiStatusPhase.PREPARE || lockedPrepareQc == null) {
            "Sumeragi status prepare phase cannot carry a PrepareQC lock"
        }
        if (phase == SumeragiStatusPhase.PENDING_APPLY) {
            require(
                lastCommittedHeight == height &&
                    lastCommittedSubject != null &&
                    lastCommitQc != null,
            ) { "pending-apply status must carry the current decided height and subject" }
        } else {
            require(lastCommittedHeight < height) {
                "non-decided Sumeragi status must have a committed height below the active height"
            }
        }
        require(lastCommittedHeight != BigInteger.ZERO ||
            (lastCommittedSubject == null && lastCommitQc == null)) {
            "pre-genesis commit frontier cannot carry a subject or CommitQC"
        }
        require((lastCommittedSubject == null) == (lastCommitQc == null)) {
            "Sumeragi status committed subject and CommitQC must be paired"
        }
        if (lastCommitQc != null && lastCommittedSubject != null) {
            val certificate = lastCommitQc.certificate
            require(
                certificate.phase == SumeragiStatusGlobalPhase.COMMIT &&
                    certificate.round.height == lastCommittedHeight &&
                    sameSubject(certificate.subject, lastCommittedSubject),
            ) { "Sumeragi status CommitQC does not certify the committed frontier" }
            if (lastCommittedHeight == height) {
                require(sameContext(certificate.round.contextId, activeContextId)) {
                    "Sumeragi status CommitQC context does not match the active context"
                }
            }
            if (sameContext(certificate.round.contextId, activeContextId)) {
                require(
                    lastCommitQc.validatorCount == activeHeightContext.validatorCount &&
                        lastCommitQc.minSigners == activeHeightContext.quorum.minSigners &&
                        lastCommitQc.totalPower == activeHeightContext.quorum.totalPower,
                ) { "Sumeragi status CommitQC quorum differs from the active height context" }
            }
        }

        fun validatePrepare(reference: SumeragiStatusQcReference) {
            require(sameContext(reference.round.contextId, activeContextId)) {
                "Sumeragi status certificate context does not match the active context"
            }
            require(reference.round.height == height) {
                "Sumeragi status certificate height does not match the active height"
            }
            require(reference.phase == SumeragiStatusGlobalPhase.PREPARE) {
                "Sumeragi status QC reference must be a PrepareQC"
            }
            require(reference.round.view <= view) {
                "Sumeragi status QC reference is from a future view"
            }
        }
        lockedPrepareQc?.let(::validatePrepare)
        highestPrepareQc?.let(::validatePrepare)
        require(lockedPrepareQc == null || highestPrepareQc != null) {
            "Sumeragi status lock requires a highest PrepareQC"
        }
        if (lockedPrepareQc != null && highestPrepareQc != null) {
            require(lockedPrepareQc.round.view <= highestPrepareQc.round.view) {
                "Sumeragi status lock is above its highest PrepareQC"
            }
            require(
                lockedPrepareQc.round.view != highestPrepareQc.round.view ||
                    sameQc(lockedPrepareQc, highestPrepareQc),
            ) { "Sumeragi status lock and highest PrepareQC conflict at the same view" }
        }
        lastTimeoutCertificate?.let { timeout ->
            require(sameContext(timeout.round.contextId, activeContextId)) {
                "Sumeragi status timeout context does not match the active context"
            }
            require(timeout.round.height == height) {
                "Sumeragi status timeout height does not match the active height"
            }
            require(timeout.round.view < view) {
                "Sumeragi status timeout certificate must precede the current view"
            }
            timeout.highestPrepareQc?.let { highest ->
                validatePrepare(highest)
                require(highest.round.view <= timeout.round.view) {
                    "Sumeragi status timeout certificate carries a future PrepareQC"
                }
            }
        }
    }

    private fun contextId(value: Any?, context: String): SumeragiStatusContextId {
        val tuple = SumeragiJsonPrimitives.array(value, context, 1)
        return SumeragiStatusContextId(SumeragiJsonPrimitives.hash(tuple[0], "$context[0]"))
    }

    private fun round(value: Any?, context: String): SumeragiStatusRound {
        val record = SumeragiJsonPrimitives.exactObject(
            value, setOf("context_id", "height", "view"), context,
        )
        return SumeragiStatusRound(
            contextId(record["context_id"], "$context.context_id"),
            SumeragiJsonPrimitives.u64(record["height"], "$context.height"),
            SumeragiJsonPrimitives.u64(record["view"], "$context.view"),
        )
    }

    private fun blockSubject(value: Any?, context: String): SumeragiStatusBlockSubject {
        val record = SumeragiJsonPrimitives.objectValue(value, context)
        SumeragiJsonPrimitives.requireFields(
            record,
            setOf("block_hash", "payload_hash"),
            setOf("parent_block_hash"),
            context,
        )
        return SumeragiStatusBlockSubject(
            record["parent_block_hash"]?.let {
                SumeragiJsonPrimitives.hash(it, "$context.parent_block_hash")
            },
            SumeragiJsonPrimitives.hash(record["block_hash"], "$context.block_hash"),
            SumeragiJsonPrimitives.hash(record["payload_hash"], "$context.payload_hash"),
        )
    }

    private fun executionCommitment(
        value: Any?,
        context: String,
    ): SumeragiStatusExecutionCommitment {
        val record = SumeragiJsonPrimitives.objectValue(value, context)
        SumeragiJsonPrimitives.requireFields(
            record,
            setOf(
                "parent_state_root", "post_state_root", "ordinary_writes_root",
                "topup_anchor_count", "native_amx_application_manifest_version",
                "native_amx_application_manifest_root", "native_amx_application_manifest_count",
                "lane_finality_manifest", "merge_carrier", "executed_block_wire_len",
                "executed_block_wire_hash",
            ),
            setOf("topup_anchor_root"),
            context,
        )
        val topupCount = SumeragiJsonPrimitives.unsigned(
            record["topup_anchor_count"], BigInteger.valueOf(16), "$context.topup_anchor_count",
        )
        val topupRoot = record["topup_anchor_root"]?.let {
            SumeragiJsonPrimitives.hash(it, "$context.topup_anchor_root")
        }
        require((topupCount == BigInteger.ZERO) == (topupRoot == null)) {
            "$context.topup_anchor_root must be present exactly when topup_anchor_count is positive"
        }
        val manifestVersion = SumeragiJsonPrimitives.u16(
            record["native_amx_application_manifest_version"],
            "$context.native_amx_application_manifest_version",
        )
        require(manifestVersion == NATIVE_AMX_APPLICATION_MANIFEST_VERSION) {
            "$context.native_amx_application_manifest_version must equal " +
                NATIVE_AMX_APPLICATION_MANIFEST_VERSION
        }
        val manifestRoot = SumeragiJsonPrimitives.hash(
            record["native_amx_application_manifest_root"],
            "$context.native_amx_application_manifest_root",
        )
        val manifestCount = SumeragiJsonPrimitives.unsigned(
            record["native_amx_application_manifest_count"],
            BigInteger.valueOf(NATIVE_AMX_APPLICATION_MANIFEST_MAX_LEAVES.toLong()),
            "$context.native_amx_application_manifest_count",
        )
        require(
            (manifestCount == BigInteger.ZERO) ==
                (manifestRoot == NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT),
        ) {
            "$context.native_amx_application_manifest_count must be zero exactly for the " +
                "canonical empty root"
        }
        val laneFinalityManifest = if (record["lane_finality_manifest"] == null) {
            null
        } else {
            val laneContext = "$context.lane_finality_manifest"
            val lane = SumeragiJsonPrimitives.exactObject(
                record["lane_finality_manifest"], setOf("root", "leaf_count"), laneContext,
            )
            SumeragiStatusLaneFinalityManifestCommitment(
                SumeragiJsonPrimitives.hash(lane["root"], "$laneContext.root"),
                SumeragiJsonPrimitives.unsigned(
                    lane["leaf_count"],
                    BigInteger.valueOf(LANE_FINALITY_MANIFEST_MAX_LEAVES.toLong()),
                    "$laneContext.leaf_count",
                    positive = true,
                ),
            )
        }
        val mergeCarrier = if (record["merge_carrier"] == null) {
            null
        } else {
            val mergeContext = "$context.merge_carrier"
            val merge = SumeragiJsonPrimitives.exactObject(
                record["merge_carrier"], setOf("version", "entry_hash"), mergeContext,
            )
            val version = SumeragiJsonPrimitives.u16(merge["version"], "$mergeContext.version")
            require(version == MERGE_CARRIER_COMMITMENT_VERSION) {
                "$mergeContext.version must equal $MERGE_CARRIER_COMMITMENT_VERSION"
            }
            SumeragiStatusMergeCarrierCommitment(
                version,
                SumeragiJsonPrimitives.hash(merge["entry_hash"], "$mergeContext.entry_hash"),
            )
        }
        return SumeragiStatusExecutionCommitment(
            parentStateRoot = SumeragiJsonPrimitives.hash(
                record["parent_state_root"], "$context.parent_state_root",
            ),
            postStateRoot = SumeragiJsonPrimitives.hash(
                record["post_state_root"], "$context.post_state_root",
            ),
            ordinaryWritesRoot = SumeragiJsonPrimitives.hash(
                record["ordinary_writes_root"], "$context.ordinary_writes_root",
            ),
            topupAnchorRoot = topupRoot,
            topupAnchorCount = topupCount,
            nativeAmxApplicationManifestVersion = manifestVersion,
            nativeAmxApplicationManifestRoot = manifestRoot,
            nativeAmxApplicationManifestCount = manifestCount,
            laneFinalityManifest = laneFinalityManifest,
            mergeCarrier = mergeCarrier,
            executedBlockWireLen = SumeragiJsonPrimitives.positiveU64(
                record["executed_block_wire_len"], "$context.executed_block_wire_len",
            ),
            executedBlockWireHash = SumeragiJsonPrimitives.hash(
                record["executed_block_wire_hash"], "$context.executed_block_wire_hash",
            ),
        )
    }

    private fun qcReference(value: Any?, context: String): SumeragiStatusQcReference {
        val record = SumeragiJsonPrimitives.exactObject(
            value,
            setOf("round", "proposal_round", "phase", "subject", "execution_commitment"),
            context,
        )
        val round = round(record["round"], "$context.round")
        val proposalRound = round(record["proposal_round"], "$context.proposal_round")
        require(sameRound(round, proposalRound)) {
            "$context.proposal_round must equal round"
        }
        return SumeragiStatusQcReference(
            round,
            proposalRound,
            taggedEnum(
                record["phase"], "phase", "$context.phase", SumeragiStatusGlobalPhase.values(),
            ),
            blockSubject(record["subject"], "$context.subject"),
            executionCommitment(record["execution_commitment"], "$context.execution_commitment"),
        )
    }

    private fun timeoutCertificate(
        value: Any?,
        context: String,
    ): SumeragiStatusTimeoutCertificate {
        val record = SumeragiJsonPrimitives.objectValue(value, context)
        SumeragiJsonPrimitives.requireFields(
            record,
            setOf("round", "certificate_hash"),
            setOf("highest_prepare_qc"),
            context,
        )
        return SumeragiStatusTimeoutCertificate(
            round(record["round"], "$context.round"),
            record["highest_prepare_qc"]?.let {
                qcReference(it, "$context.highest_prepare_qc")
            },
            SumeragiJsonPrimitives.hash(record["certificate_hash"], "$context.certificate_hash"),
        )
    }

    private fun heightContext(value: Any?, context: String): SumeragiStatusHeightContext {
        val record = SumeragiJsonPrimitives.exactObject(
            value,
            setOf("epoch", "epoch_end_height", "mode", "epoch_seed", "validator_count", "quorum"),
            context,
        )
        val validatorCount = SumeragiJsonPrimitives.unsigned(
            record["validator_count"], BigInteger.valueOf(31), "$context.validator_count", true,
        )
        require(
            validatorCount >= BigInteger.valueOf(4) &&
                validatorCount.subtract(BigInteger.ONE).mod(BigInteger.valueOf(3)) == BigInteger.ZERO,
        ) { "$context.validator_count must have bounded 3f + 1 geometry" }
        val quorumRecord = SumeragiJsonPrimitives.exactObject(
            record["quorum"], setOf("min_signers", "total_power"), "$context.quorum",
        )
        val minSigners = SumeragiJsonPrimitives.unsigned(
            quorumRecord["min_signers"], BigInteger.valueOf(31),
            "$context.quorum.min_signers", true,
        )
        val totalPower = SumeragiJsonPrimitives.positiveU64(
            quorumRecord["total_power"], "$context.quorum.total_power",
        )
        val canonicalMinSigners = validatorCount.multiply(BigInteger.valueOf(2))
            .divide(BigInteger.valueOf(3)).add(BigInteger.ONE)
        require(minSigners == canonicalMinSigners && totalPower == validatorCount) {
            "$context.quorum is not canonical for validator_count"
        }
        return SumeragiStatusHeightContext(
            epoch = SumeragiJsonPrimitives.u64(record["epoch"], "$context.epoch"),
            epochEndHeight = SumeragiJsonPrimitives.u64(
                record["epoch_end_height"], "$context.epoch_end_height",
            ),
            mode = taggedEnum(
                record["mode"], "mode", "$context.mode", SumeragiStatusConsensusMode.values(),
            ),
            epochSeed = SumeragiJsonPrimitives.byte32(record["epoch_seed"], "$context.epoch_seed"),
            validatorCount = validatorCount,
            quorum = SumeragiStatusQuorum(minSigners, totalPower),
        )
    }

    private fun commitQc(value: Any?, context: String): SumeragiStatusCommitQc {
        val record = SumeragiJsonPrimitives.exactObject(
            value,
            setOf(
                "certificate", "validator_count", "signer_count", "min_signers",
                "signed_power", "total_power",
            ),
            context,
        )
        val validatorCount = SumeragiJsonPrimitives.unsigned(
            record["validator_count"], BigInteger.valueOf(31), "$context.validator_count", true,
        )
        require(
            validatorCount >= BigInteger.valueOf(4) &&
                validatorCount.subtract(BigInteger.ONE).mod(BigInteger.valueOf(3)) == BigInteger.ZERO,
        ) { "$context.validator_count must have bounded 3f + 1 geometry" }
        val signerCount = SumeragiJsonPrimitives.unsigned(
            record["signer_count"], validatorCount, "$context.signer_count",
        )
        val minSigners = SumeragiJsonPrimitives.unsigned(
            record["min_signers"], BigInteger.valueOf(31), "$context.min_signers", true,
        )
        val signedPower = SumeragiJsonPrimitives.u64(record["signed_power"], "$context.signed_power")
        val totalPower = SumeragiJsonPrimitives.positiveU64(
            record["total_power"], "$context.total_power",
        )
        val canonicalMinSigners = validatorCount.multiply(BigInteger.valueOf(2))
            .divide(BigInteger.valueOf(3)).add(BigInteger.ONE)
        require(
            signerCount == minSigners && minSigners == canonicalMinSigners &&
                signedPower == signerCount && totalPower == validatorCount &&
                signedPower.multiply(BigInteger.valueOf(3)) >
                    totalPower.multiply(BigInteger.valueOf(2)),
        ) { "$context does not satisfy its exact frozen certificate quorum" }
        return SumeragiStatusCommitQc(
            qcReference(record["certificate"], "$context.certificate"),
            validatorCount,
            signerCount,
            minSigners,
            signedPower,
            totalPower,
        )
    }

    private fun liveness(
        value: Any?,
        activeHeight: BigInteger,
        activeView: BigInteger,
        activeContextId: SumeragiStatusContextId,
        heightContext: SumeragiStatusHeightContext,
        context: String,
    ): SumeragiStatusLiveness {
        val record = SumeragiJsonPrimitives.objectValue(value, context)
        SumeragiJsonPrimitives.requireFields(
            record,
            setOf(
                "generation", "prepare_quorums", "commit_quorums", "timeout_quorums",
                "outbound_intents", "work", "queues", "no_progress_age_ms", "ignore_counts",
            ),
            setOf("last_progress", "blocker"),
            context,
        )
        val generation = SumeragiJsonPrimitives.u64(record["generation"], "$context.generation")

        fun boundRound(raw: Any?, itemContext: String): SumeragiStatusRound {
            val parsed = round(raw, itemContext)
            require(
                sameContext(parsed.contextId, activeContextId) && parsed.height == activeHeight,
            ) { "$itemContext must match the active height context" }
            return parsed
        }

        fun nonFutureRound(raw: Any?, itemContext: String): SumeragiStatusRound {
            val parsed = boundRound(raw, itemContext)
            require(parsed.view <= activeView) { "$itemContext.view must not exceed the active view" }
            return parsed
        }

        fun partialQuorumFields(
            item: Map<String, Any?>,
            itemContext: String,
        ): Array<BigInteger> {
            val signerCount = SumeragiJsonPrimitives.unsigned(
                item["signer_count"], heightContext.validatorCount, "$itemContext.signer_count",
            )
            val signedPower = SumeragiJsonPrimitives.u64(
                item["signed_power"], "$itemContext.signed_power",
            )
            val minSigners = SumeragiJsonPrimitives.unsigned(
                item["min_signers"], heightContext.validatorCount, "$itemContext.min_signers",
            )
            val totalPower = SumeragiJsonPrimitives.positiveU64(
                item["total_power"], "$itemContext.total_power",
            )
            require(
                minSigners == heightContext.quorum.minSigners &&
                    totalPower == heightContext.quorum.totalPower &&
                    signedPower == signerCount,
            ) { "$itemContext disagrees with the frozen dual quorum" }
            return arrayOf(signerCount, signedPower, minSigners, totalPower)
        }

        fun voteQuorums(field: String, maximum: Int): List<SumeragiStatusVoteQuorum> =
            SumeragiJsonPrimitives.array(record[field], "$context.$field", maximum).mapIndexed {
                    index, raw ->
                val itemContext = "$context.$field[$index]"
                val item = SumeragiJsonPrimitives.exactObject(
                    raw,
                    setOf(
                        "round", "proposal_round", "subject", "execution_commitment",
                        "signer_count", "signed_power", "min_signers", "total_power",
                    ),
                    itemContext,
                )
                val quorumRound = nonFutureRound(item["round"], "$itemContext.round")
                val proposalRound = nonFutureRound(
                    item["proposal_round"], "$itemContext.proposal_round",
                )
                require(sameRound(quorumRound, proposalRound)) {
                    "$itemContext.proposal_round must equal round"
                }
                val fields = partialQuorumFields(item, itemContext)
                SumeragiStatusVoteQuorum(
                    quorumRound,
                    proposalRound,
                    blockSubject(item["subject"], "$itemContext.subject"),
                    executionCommitment(
                        item["execution_commitment"], "$itemContext.execution_commitment",
                    ),
                    fields[0],
                    fields[1],
                    fields[2],
                    fields[3],
                )
            }

        val prepareQuorums = voteQuorums("prepare_quorums", 31)
        val commitQuorums = voteQuorums("commit_quorums", 32)
        val timeoutQuorums = SumeragiJsonPrimitives.array(
            record["timeout_quorums"], "$context.timeout_quorums", 31,
        ).mapIndexed { index, raw ->
            val itemContext = "$context.timeout_quorums[$index]"
            val item = SumeragiJsonPrimitives.exactObject(
                raw,
                setOf(
                    "round", "signer_count", "signed_power", "min_signers", "total_power",
                    "certificate_formed",
                ),
                itemContext,
            )
            val fields = partialQuorumFields(item, itemContext)
            val certificateFormed = SumeragiJsonPrimitives.boolean(
                item["certificate_formed"], "$itemContext.certificate_formed",
            )
            if (certificateFormed) {
                require(
                    fields[0] >= fields[2] &&
                        fields[1].multiply(BigInteger.valueOf(3)) >
                            fields[3].multiply(BigInteger.valueOf(2)),
                ) { "$itemContext does not form its advertised dual quorum" }
            }
            SumeragiStatusTimeoutQuorum(
                nonFutureRound(item["round"], "$itemContext.round"),
                fields[0], fields[1], fields[2], fields[3], certificateFormed,
            )
        }

        val proposalKinds = setOf(
            SumeragiStatusOutboundIntentKind.PROPOSAL,
            SumeragiStatusOutboundIntentKind.PREPARE_VOTE,
            SumeragiStatusOutboundIntentKind.COMMIT_VOTE,
            SumeragiStatusOutboundIntentKind.PREPARE_QC,
            SumeragiStatusOutboundIntentKind.COMMIT_QC,
        )
        val outboundIntents = SumeragiJsonPrimitives.array(
            record["outbound_intents"], "$context.outbound_intents", 7,
        ).mapIndexed { index, raw ->
            val itemContext = "$context.outbound_intents[$index]"
            val item = SumeragiJsonPrimitives.objectValue(raw, itemContext)
            SumeragiJsonPrimitives.requireFields(
                item,
                setOf("kind", "round", "stage"),
                setOf("proposal_round", "subject", "execution_commitment"),
                itemContext,
            )
            val kind = taggedEnum(
                item["kind"], "kind", "$itemContext.kind",
                SumeragiStatusOutboundIntentKind.values(),
            )
            val stage = taggedEnum(
                item["stage"], "stage", "$itemContext.stage",
                SumeragiStatusOutboundIntentStage.values(),
            )
            val intentRound = boundRound(item["round"], "$itemContext.round")
            require(kind == SumeragiStatusOutboundIntentKind.COMMIT_QC || intentRound.view <= activeView) {
                "$itemContext.round.view must not exceed the active view"
            }
            val proposalRound = item["proposal_round"]?.let {
                boundRound(it, "$itemContext.proposal_round")
            }
            require((kind in proposalKinds) == (proposalRound != null)) {
                "$itemContext has inconsistent proposal_round for ${kind.wireName}"
            }
            proposalRound?.let {
                require(sameRound(intentRound, it)) {
                    "$itemContext.proposal_round must equal round"
                }
            }
            val subject = item["subject"]?.let { blockSubject(it, "$itemContext.subject") }
            val commitment = item["execution_commitment"]?.let {
                executionCommitment(it, "$itemContext.execution_commitment")
            }
            val validShape = when (kind) {
                SumeragiStatusOutboundIntentKind.PROPOSAL ->
                    subject != null && commitment == null
                SumeragiStatusOutboundIntentKind.TIMEOUT_VOTE,
                SumeragiStatusOutboundIntentKind.TIMEOUT_CERTIFICATE,
                -> subject == null && commitment == null
                else -> subject != null && commitment != null
            }
            require(validShape) { "$itemContext has inconsistent proposal fields" }
            SumeragiStatusOutboundIntent(
                kind, intentRound, proposalRound, subject, commitment, stage,
            )
        }

        val workRecord = SumeragiJsonPrimitives.exactObject(
            record["work"],
            setOf(
                "candidate", "body_recovery", "body_store", "validation", "application",
                "successor_height",
            ),
            "$context.work",
        )
        fun workStage(field: String): SumeragiStatusWorkStage = taggedEnum(
            workRecord[field], "stage", "$context.work.$field", SumeragiStatusWorkStage.values(),
        )
        val work = SumeragiStatusWork(
            workStage("candidate"),
            workStage("body_recovery"),
            workStage("body_store"),
            workStage("validation"),
            workStage("application"),
            workStage("successor_height"),
        )

        val seenQueues = HashSet<SumeragiStatusQueueKind>()
        val queues = SumeragiJsonPrimitives.array(
            record["queues"], "$context.queues", 10,
        ).mapIndexed { index, raw ->
            val itemContext = "$context.queues[$index]"
            val item = SumeragiJsonPrimitives.objectValue(raw, itemContext)
            SumeragiJsonPrimitives.requireFields(
                item,
                setOf("queue", "depth", "capacity", "service_debt"),
                setOf("oldest_age_ms"),
                itemContext,
            )
            val kind = taggedEnum(
                item["queue"], "queue", "$itemContext.queue", SumeragiStatusQueueKind.values(),
            )
            require(seenQueues.add(kind)) { "$itemContext.queue is duplicated" }
            val depth = SumeragiJsonPrimitives.u32(item["depth"], "$itemContext.depth")
            val capacity = SumeragiJsonPrimitives.positiveU32(
                item["capacity"], "$itemContext.capacity",
            )
            val oldestAge = item["oldest_age_ms"]?.let {
                SumeragiJsonPrimitives.u64(it, "$itemContext.oldest_age_ms")
            }
            require(depth <= capacity && ((depth == BigInteger.ZERO) == (oldestAge == null))) {
                "$itemContext has inconsistent occupancy and age"
            }
            SumeragiStatusQueue(
                kind,
                depth,
                capacity,
                oldestAge,
                SumeragiJsonPrimitives.u64(item["service_debt"], "$itemContext.service_debt"),
            )
        }

        val lastProgress = record["last_progress"]?.let { raw ->
            val itemContext = "$context.last_progress"
            val item = SumeragiJsonPrimitives.exactObject(
                raw, setOf("generation", "round", "transition", "age_ms"), itemContext,
            )
            val progressGeneration = SumeragiJsonPrimitives.u64(
                item["generation"], "$itemContext.generation",
            )
            require(progressGeneration <= generation) {
                "$itemContext.generation is from the future"
            }
            val progressRound = boundRound(item["round"], "$itemContext.round")
            val transition = taggedEnum(
                item["transition"], "transition", "$itemContext.transition",
                SumeragiStatusProgressTransition.values(),
            )
            val permitsFutureView = transition == SumeragiStatusProgressTransition.COMMIT_QUORUM ||
                transition == SumeragiStatusProgressTransition.DECISION_PERSISTED
            require(progressRound.view <= activeView || permitsFutureView) {
                "$itemContext.round.view must not exceed the active view"
            }
            SumeragiStatusProgress(
                progressGeneration,
                progressRound,
                transition,
                SumeragiJsonPrimitives.u64(item["age_ms"], "$itemContext.age_ms"),
            )
        }
        val blocker = record["blocker"]?.let {
            taggedEnum(
                it, "blocker", "$context.blocker", SumeragiStatusLivenessBlocker.values(),
            )
        }
        val seenReasons = HashSet<SumeragiStatusIgnoreReason>()
        val ignoreCounts = SumeragiJsonPrimitives.array(
            record["ignore_counts"], "$context.ignore_counts", 12,
        ).mapIndexed { index, raw ->
            val itemContext = "$context.ignore_counts[$index]"
            val item = SumeragiJsonPrimitives.exactObject(
                raw, setOf("reason", "count"), itemContext,
            )
            val reason = taggedEnum(
                item["reason"], "reason", "$itemContext.reason",
                SumeragiStatusIgnoreReason.values(),
            )
            require(seenReasons.add(reason)) { "$itemContext.reason is duplicated" }
            SumeragiStatusIgnoreCount(
                reason,
                SumeragiJsonPrimitives.u64(item["count"], "$itemContext.count"),
            )
        }
        return SumeragiStatusLiveness(
            generation,
            prepareQuorums,
            commitQuorums,
            timeoutQuorums,
            outboundIntents,
            work,
            queues,
            lastProgress,
            SumeragiJsonPrimitives.u64(
                record["no_progress_age_ms"], "$context.no_progress_age_ms",
            ),
            blocker,
            ignoreCounts,
        )
    }

    private fun <T> taggedEnum(
        value: Any?,
        tag: String,
        context: String,
        values: Array<T>,
    ): T where T : Enum<T> {
        val wireName = SumeragiJsonPrimitives.taggedUnit(value, tag, context)
        return values.firstOrNull {
            when (it) {
                is SumeragiStatusGlobalPhase -> it.wireName == wireName
                is SumeragiStatusConsensusMode -> it.wireName == wireName
                is SumeragiStatusOutboundIntentKind -> it.wireName == wireName
                is SumeragiStatusOutboundIntentStage -> it.wireName == wireName
                is SumeragiStatusWorkStage -> it.wireName == wireName
                is SumeragiStatusQueueKind -> it.wireName == wireName
                is SumeragiStatusProgressTransition -> it.wireName == wireName
                is SumeragiStatusLivenessBlocker -> it.wireName == wireName
                is SumeragiStatusIgnoreReason -> it.wireName == wireName
                is SumeragiStatusPhase -> it.wireName == wireName
                is SumeragiStatusBodyState -> it.wireName == wireName
                else -> false
            }
        } ?: throw IllegalArgumentException("$context.$tag is not a supported v4 variant")
    }

    private fun sameContext(
        left: SumeragiStatusContextId,
        right: SumeragiStatusContextId,
    ): Boolean = left.hash == right.hash

    private fun sameRound(left: SumeragiStatusRound, right: SumeragiStatusRound): Boolean =
        sameContext(left.contextId, right.contextId) &&
            left.height == right.height && left.view == right.view

    private fun sameSubject(
        left: SumeragiStatusBlockSubject,
        right: SumeragiStatusBlockSubject,
    ): Boolean = left.parentBlockHash == right.parentBlockHash &&
        left.blockHash == right.blockHash && left.payloadHash == right.payloadHash

    private fun sameCommitment(
        left: SumeragiStatusExecutionCommitment,
        right: SumeragiStatusExecutionCommitment,
    ): Boolean = left.parentStateRoot == right.parentStateRoot &&
        left.postStateRoot == right.postStateRoot &&
        left.ordinaryWritesRoot == right.ordinaryWritesRoot &&
        left.topupAnchorRoot == right.topupAnchorRoot &&
        left.topupAnchorCount == right.topupAnchorCount &&
        left.nativeAmxApplicationManifestVersion == right.nativeAmxApplicationManifestVersion &&
        left.nativeAmxApplicationManifestRoot == right.nativeAmxApplicationManifestRoot &&
        left.nativeAmxApplicationManifestCount == right.nativeAmxApplicationManifestCount &&
        left.laneFinalityManifest?.root == right.laneFinalityManifest?.root &&
        left.laneFinalityManifest?.leafCount == right.laneFinalityManifest?.leafCount &&
        left.mergeCarrier?.version == right.mergeCarrier?.version &&
        left.mergeCarrier?.entryHash == right.mergeCarrier?.entryHash &&
        left.executedBlockWireLen == right.executedBlockWireLen &&
        left.executedBlockWireHash == right.executedBlockWireHash

    private fun sameQc(left: SumeragiStatusQcReference, right: SumeragiStatusQcReference): Boolean =
        sameRound(left.round, right.round) && sameRound(left.proposalRound, right.proposalRound) &&
            left.phase == right.phase && sameSubject(left.subject, right.subject) &&
            sameCommitment(left.executionCommitment, right.executionCommitment)

    private const val NATIVE_AMX_APPLICATION_MANIFEST_VERSION = 1
    private const val NATIVE_AMX_APPLICATION_MANIFEST_MAX_LEAVES = 1_024
    private const val LANE_FINALITY_MANIFEST_MAX_LEAVES = 1_024
    private const val MERGE_CARRIER_COMMITMENT_VERSION = 1
    private const val NATIVE_AMX_APPLICATION_MANIFEST_EMPTY_ROOT =
        "hash:45A5D35A09D284480FBA74A402D7F303B82DA0C153FC1E1083AEFC822ED07C2D#7C0F"
}

/** Shared strict scalar rules used by authoritative status and operational diagnostics. */
internal object SumeragiJsonPrimitives {
    private val u64Max = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
    private val u32Max = BigInteger.ONE.shiftLeft(32).subtract(BigInteger.ONE)
    private val canonicalHash = Regex("^hash:[0-9A-F]{64}#[0-9A-F]{4}$")
    private val canonicalByte32 = Regex("^[0-9A-F]{64}$")

    fun decodeUtf8(payload: ByteArray, context: String): String {
        val decoder = StandardCharsets.UTF_8.newDecoder()
            .onMalformedInput(CodingErrorAction.REPORT)
            .onUnmappableCharacter(CodingErrorAction.REPORT)
        return try {
            decoder.decode(ByteBuffer.wrap(payload)).toString()
        } catch (error: Exception) {
            throw IllegalArgumentException("$context must be valid UTF-8", error)
        }
    }

    fun parseObject(payload: String, context: String): Map<String, Any?> {
        rejectNegativeZeroTokens(payload, context)
        return objectValue(JsonParser.parse(payload), context)
    }

    @Suppress("UNCHECKED_CAST")
    fun objectValue(value: Any?, context: String): Map<String, Any?> {
        require(value is Map<*, *>) { "$context must be a JSON object" }
        require(value.keys.all { it is String }) { "$context contains a non-string field name" }
        return value as Map<String, Any?>
    }

    fun exactObject(
        value: Any?,
        fields: Set<String>,
        context: String,
    ): Map<String, Any?> {
        val record = objectValue(value, context)
        requireFields(record, fields, emptySet(), context)
        return record
    }

    fun requireFields(
        record: Map<String, Any?>,
        required: Set<String>,
        optional: Set<String>,
        context: String,
    ) {
        val allowed = required + optional
        val unknown = record.keys.firstOrNull { it !in allowed }
        require(unknown == null) { "$context contains unknown field $unknown" }
        val missing = required.firstOrNull { !record.containsKey(it) }
        require(missing == null) { "$context is missing required field $missing" }
    }

    fun array(value: Any?, context: String, maximum: Int): List<Any?> {
        require(value is List<*>) { "$context must be a JSON array" }
        require(value.size <= maximum) { "$context exceeds its protocol item bound" }
        return value
    }

    fun unsigned(
        value: Any?,
        maximum: BigInteger,
        context: String,
        positive: Boolean = false,
    ): BigInteger {
        val parsed = when (value) {
            is Long -> BigInteger.valueOf(value)
            is BigInteger -> value
            else -> throw IllegalArgumentException("$context must be an unquoted integer")
        }
        require(parsed.signum() >= 0) { "$context must be non-negative" }
        require(!positive || parsed.signum() > 0) { "$context must be positive" }
        require(parsed <= maximum) { "$context exceeds its protocol bound" }
        return parsed
    }

    fun u64(value: Any?, context: String): BigInteger = unsigned(value, u64Max, context)

    fun positiveU64(value: Any?, context: String): BigInteger =
        unsigned(value, u64Max, context, true)

    fun u32(value: Any?, context: String): BigInteger = unsigned(value, u32Max, context)

    fun positiveU32(value: Any?, context: String): BigInteger =
        unsigned(value, u32Max, context, true)

    fun u16(value: Any?, context: String): Int =
        unsigned(value, BigInteger.valueOf(0xffff), context).toInt()

    fun boolean(value: Any?, context: String): Boolean {
        require(value is Boolean) { "$context must be a boolean" }
        return value
    }

    fun hash(value: Any?, context: String, nonzero: Boolean = false): String {
        require(value is String && canonicalHash.matches(value)) {
            "$context must be a canonical Iroha hash literal"
        }
        val bytes = try {
            HashLiteral.decode(value)
        } catch (error: IllegalArgumentException) {
            throw IllegalArgumentException("$context must have a valid canonical checksum", error)
        }
        require((bytes[bytes.lastIndex].toInt() and 1) == 1) {
            "$context has an invalid Iroha hash marker bit"
        }
        require(!nonzero || bytes.any { it.toInt() != 0 }) { "$context must not be the zero hash" }
        return value
    }

    fun byte32(value: Any?, context: String): String {
        require(value is String && canonicalByte32.matches(value)) {
            "$context must be canonical uppercase 32-byte hex"
        }
        return value
    }

    fun taggedUnit(value: Any?, tag: String, context: String): String {
        val record = exactObject(value, setOf(tag, "details"), context)
        val variant = record[tag]
        require(variant is String && variant.isNotEmpty()) { "$context.$tag must be a string" }
        require(record["details"] == null) { "$context.details must be explicitly null" }
        return variant
    }

    fun requireU64(value: BigInteger, context: String): BigInteger {
        require(value.signum() >= 0 && value <= u64Max) {
            "$context must fit in an unsigned 64-bit integer"
        }
        return value
    }

    fun requireCanonicalNonzeroHash(value: String, context: String) {
        hash(value, context, nonzero = true)
    }

    private fun rejectNegativeZeroTokens(payload: String, context: String) {
        var inString = false
        var escaped = false
        var index = 0
        while (index < payload.length) {
            val current = payload[index]
            if (inString) {
                if (escaped) {
                    escaped = false
                } else if (current == '\\') {
                    escaped = true
                } else if (current == '"') {
                    inString = false
                }
                index += 1
                continue
            }
            if (current == '"') {
                inString = true
                index += 1
                continue
            }
            if (current == '-' && index + 1 < payload.length && payload[index + 1] == '0') {
                val after = payload.getOrNull(index + 2)
                if (after == null || after in " \t\r\n,]}") {
                    throw IllegalArgumentException("$context contains noncanonical negative zero")
                }
            }
            index += 1
        }
    }
}

private fun <T> immutableCopy(values: List<T>): List<T> =
    Collections.unmodifiableList(ArrayList(values))
