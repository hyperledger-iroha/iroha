// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.consensus

import java.util.Collections
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import org.hyperledger.iroha.sdk.core.util.HashLiteral

/** Maximum number of Native AMX participant-application rows in diagnostics. */
const val SUMERAGI_NATIVE_AMX_PARTICIPANT_APPLICATIONS_MAX: Int = 1_024

/** Maximum grouped source count represented by one Native AMX diagnostics row. */
const val SUMERAGI_NATIVE_AMX_PARTICIPANT_APPLICATION_SOURCES_MAX: Long = 4_096

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
    @SerialName("dataspace_id") val dataspaceId: Long,
    @SerialName("lane_incarnation") val laneIncarnation: String,
    @SerialName("participant_height") val participantHeight: Long,
    @SerialName("participant_view") val participantView: Long,
    @SerialName("predecessor_height") val predecessorHeight: Long,
    @SerialName("predecessor_descriptor_hash") val predecessorDescriptorHash: String? = null,
    @SerialName("descriptor_hash") val descriptorHash: String,
    @SerialName("proposal_hash") val proposalHash: String,
    @SerialName("settlement_hash") val settlementHash: String,
    @SerialName("source_count") val sourceCount: Long,
    @SerialName("application_block_height") val applicationBlockHeight: Long? = null,
    @SerialName("application_block_hash") val applicationBlockHash: String? = null,
    val state: SumeragiNativeAmxParticipantApplicationState,
) {
    init {
        require(laneId in 0..0xffff_ffffL) { "laneId must be an unsigned 32-bit value" }
        require(dataspaceId >= 0) { "dataspaceId must be non-negative" }
        require(participantHeight > 0 && participantView >= 0) {
            "participant height must be positive and view must be non-negative"
        }
        require(
            predecessorHeight >= 0 &&
                predecessorHeight < Long.MAX_VALUE &&
                predecessorHeight + 1 == participantHeight &&
                (predecessorHeight == 0L) == (predecessorDescriptorHash == null)
        ) { "Native AMX participant predecessor geometry is inconsistent" }
        require(sourceCount in 1..SUMERAGI_NATIVE_AMX_PARTICIPANT_APPLICATION_SOURCES_MAX) {
            "Native AMX participant source count is out of bounds"
        }
        require((applicationBlockHeight == null) == (applicationBlockHash == null)) {
            "application block height and hash must appear together"
        }
        require(applicationBlockHeight == null || applicationBlockHeight > 0) {
            "application block height must be positive"
        }
        if (state == SumeragiNativeAmxParticipantApplicationState.DURABLY_APPLIED) {
            require(applicationBlockHeight != null) {
                "durably applied Native AMX evidence requires an application block"
            }
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

private val CANONICAL_HASH = Regex("^hash:[0-9A-F]{64}#[0-9A-F]{4}$")

private fun requireCanonicalNonzeroHash(value: String, field: String) {
    require(CANONICAL_HASH.matches(value)) { "$field must be a canonical Iroha hash literal" }
    val bytes = HashLiteral.decode(value)
    require(bytes.any { it.toInt() != 0 }) { "$field must not be the zero hash" }
    require((bytes[bytes.lastIndex].toInt() and 1) == 1) {
        "$field has an invalid Iroha hash marker bit"
    }
}
