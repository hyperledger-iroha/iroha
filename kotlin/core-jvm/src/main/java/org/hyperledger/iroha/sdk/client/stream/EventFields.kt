package org.hyperledger.iroha.sdk.client.stream

import org.hyperledger.iroha.sdk.query.Field
import org.hyperledger.iroha.sdk.query.Filter

/**
 * Fields accepted by `GET /v1/events/sse?filter=<text>`.
 *
 * ```kotlin
 * val filter = (EventFields.TX_HASH eq hash) and EventFields.TX_STATUS.isIn("Approved", "Rejected")
 * client.newEventStreamClient().subscribe(filter, listener)
 * ```
 */
object EventFields {
    /** Transaction hash (lowercase hex). */
    @JvmField val TX_HASH: Field = Filter.field("tx_hash")

    /** Transaction status: `Queued`, `Expired`, `Approved` or `Rejected` ([TransactionEventStatus]). */
    @JvmField val TX_STATUS: Field = Filter.field("tx_status")

    /** Height of the block containing the transaction. */
    @JvmField val TX_BLOCK_HEIGHT: Field = Filter.field("tx_block_height")

    /** Lane of the transaction. */
    @JvmField val TX_LANE_ID: Field = Filter.field("tx_lane_id")

    /** Dataspace of the transaction. */
    @JvmField val TX_DATASPACE_ID: Field = Filter.field("tx_dataspace_id")

    /** Block status: `Created`, `Approved`, `Rejected`, `Committed` or `Applied` ([BlockEventStatus]). */
    @JvmField val BLOCK_STATUS: Field = Filter.field("block_status")

    /** Block height. */
    @JvmField val BLOCK_HEIGHT: Field = Filter.field("block_height")

    /** Proof verifier backend. */
    @JvmField val PROOF_BACKEND: Field = Filter.field("proof_backend")

    /** Proof call hash (64 hex characters). */
    @JvmField val PROOF_CALL_HASH: Field = Filter.field("proof_call_hash")

    /** Proof envelope hash (64 hex characters). */
    @JvmField val PROOF_ENVELOPE_HASH: Field = Filter.field("proof_envelope_hash")
}
