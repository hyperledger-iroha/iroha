package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger

/** Pollable state of a Torii Offline operation. */
sealed class OfflineOperationStatus(
    @JvmField val operationId: String,
) {
    init {
        requireOperationId(operationId)
    }

    /** The transaction is queued or awaiting finality. */
    class Pending(
        operationId: String,
        @JvmField val kind: OfflineOperationKind,
        transactionHash: String,
        submittedAtMs: BigInteger,
    ) : OfflineOperationStatus(operationId) {
        @JvmField
        val transactionHash: String = requireExactNonEmptyText(transactionHash, "transactionHash")

        @JvmField
        val submittedAtMs: BigInteger = requireU64(submittedAtMs, "submittedAtMs")
    }

    /** The transaction was applied and finalized. */
    class Applied(
        operationId: String,
        @JvmField val result: Result,
    ) : OfflineOperationStatus(operationId)

    /** The transaction reached a terminal rejection. */
    class Rejected(
        operationId: String,
        @JvmField val kind: OfflineOperationKind,
        transactionHash: String,
        @JvmField val error: Error,
    ) : OfflineOperationStatus(operationId) {
        @JvmField
        val transactionHash: String = requireExactNonEmptyText(transactionHash, "transactionHash")
    }

    /** Operation-specific result of an applied command. */
    sealed class Result {
        /** Applied top-up result. */
        class TopUp(@JvmField val value: TopUpResult) : Result()

        /** Applied redemption result. */
        class Redeem(@JvmField val value: RedeemResult) : Result()
    }

    /** Finalized top-up result. */
    class TopUpResult(
        transactionHash: String,
        finalizedBlockHeight: BigInteger,
        serverTimeMs: BigInteger,
        @JvmField val anchor: TopUpAnchor,
    ) {
        @JvmField
        val transactionHash: String = requireExactNonEmptyText(transactionHash, "transactionHash")

        @JvmField
        val finalizedBlockHeight: BigInteger = requireU64(finalizedBlockHeight, "finalizedBlockHeight")

        @JvmField
        val serverTimeMs: BigInteger = requireU64(serverTimeMs, "serverTimeMs")
    }

    /** Finalized redemption result. */
    class RedeemResult(
        transactionHash: String,
        finalizedBlockHeight: BigInteger,
        serverTimeMs: BigInteger,
    ) {
        @JvmField
        val transactionHash: String = requireExactNonEmptyText(transactionHash, "transactionHash")

        @JvmField
        val finalizedBlockHeight: BigInteger = requireU64(finalizedBlockHeight, "finalizedBlockHeight")

        @JvmField
        val serverTimeMs: BigInteger = requireU64(serverTimeMs, "serverTimeMs")
    }

    /**
     * Schema-bound top-up anchor archive.
     *
     * The Kotlin SDK does not yet expose a field-level
     * `KagemushaRecursiveSpendTopUpAnchorV2` decoder; callers can retain this
     * canonical archive for the wallet prover without treating the whole
     * operation status as an untyped payload.
     */
    class TopUpAnchor internal constructor(noritoArchive: ByteArray) {
        private val archive = noritoArchive.copyOf()

        /** Return a defensive copy of the canonical anchor archive. */
        fun noritoArchive(): ByteArray = archive.copyOf()
    }

    /** Stable typed Torii rejection. */
    class Error(
        code: String,
        message: String,
        @JvmField val details: ErrorDetails?,
    ) {
        @JvmField
        val code: String = requireExactNonEmptyText(code, "error.code")

        @JvmField
        val message: String = requireExactNonEmptyText(message, "error.message")
    }

    /** Structured optional metadata attached to a rejection. */
    class ErrorDetails(
        @JvmField val layer: String?,
        @JvmField val rejectCode: String?,
        @JvmField val queue: QueueErrorSnapshot?,
        retryAfterSeconds: BigInteger?,
        @JvmField val endpoint: String?,
        @JvmField val field: String?,
        @JvmField val expected: String?,
        @JvmField val actual: String?,
        @JvmField val profile: String?,
        @JvmField val chainDiscriminant: Int?,
        @JvmField val transactionHash: String?,
        @JvmField val lastStatus: String?,
        @JvmField val hint: String?,
        @JvmField val axt: AxtErrorDetails?,
    ) {
        @JvmField
        val retryAfterSeconds: BigInteger? = retryAfterSeconds?.let {
            requireU64(it, "error.details.retryAfterSeconds")
        }

        init {
            require(chainDiscriminant == null || chainDiscriminant in 0..0xffff) {
                "error.details.chainDiscriminant must fit in an unsigned 16-bit integer"
            }
        }
    }

    /** Queue pressure snapshot attached to a rejection. */
    class QueueErrorSnapshot(
        state: String,
        queued: BigInteger,
        capacity: BigInteger,
        @JvmField val saturated: Boolean,
    ) {
        @JvmField
        val state: String = requireExactNonEmptyText(state, "error.details.queue.state")

        @JvmField
        val queued: BigInteger = requireU64(queued, "error.details.queue.queued")

        @JvmField
        val capacity: BigInteger = requireU64(capacity, "error.details.queue.capacity")
    }

    /** AXT rejection metadata attached to a validation failure. */
    class AxtErrorDetails(
        @JvmField val code: String?,
        @JvmField val reason: String?,
        snapshotVersion: BigInteger?,
        dataspace: BigInteger?,
        @JvmField val lane: Long?,
        nextMinHandleEra: BigInteger?,
        nextMinSubNonce: BigInteger?,
    ) {
        @JvmField
        val snapshotVersion: BigInteger? = snapshotVersion?.let {
            requireU64(it, "error.details.axt.snapshotVersion")
        }

        @JvmField
        val dataspace: BigInteger? = dataspace?.let {
            requireU64(it, "error.details.axt.dataspace")
        }

        @JvmField
        val nextMinHandleEra: BigInteger? = nextMinHandleEra?.let {
            requireU64(it, "error.details.axt.nextMinHandleEra")
        }

        @JvmField
        val nextMinSubNonce: BigInteger? = nextMinSubNonce?.let {
            requireU64(it, "error.details.axt.nextMinSubNonce")
        }

        init {
            require(lane == null || lane in 0..0xffff_ffffL) {
                "error.details.axt.lane must fit in an unsigned 32-bit integer"
            }
        }
    }
}
