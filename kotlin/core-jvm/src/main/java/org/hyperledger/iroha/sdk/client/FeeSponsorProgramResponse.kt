package org.hyperledger.iroha.sdk.client

import org.hyperledger.iroha.sdk.core.model.FeeSponsorProgramId

/** Consensus-visible lifecycle of one exact fee sponsor program. */
enum class FeeSponsorProgramLifecycle {
    STAGED,
    PAUSED,
    ACTIVE,
    CLOSING,
    CLOSED,
}

/** Delayed activation scheduled for an immutable sponsor-program revision. */
class FeeSponsorProgramActivation(
    @JvmField val revision: Long,
    @JvmField val activateAtHeight: Long,
) {
    init {
        require(revision > 0) { "revision must be positive" }
        require(activateAtHeight >= 0) { "activateAtHeight must be non-negative" }
    }

    override fun equals(other: Any?): Boolean =
        other is FeeSponsorProgramActivation &&
            revision == other.revision &&
            activateAtHeight == other.activateAtHeight

    override fun hashCode(): Int = 31 * revision.hashCode() + activateAtHeight.hashCode()
}

/** Exact on-chain fee sponsor program returned by Torii. */
class FeeSponsorProgramResponse(
    @JvmField val id: FeeSponsorProgramId,
    @JvmField val lifecycle: FeeSponsorProgramLifecycle,
    @JvmField val activeRevision: Long?,
    @JvmField val stagedRevision: Long?,
    @JvmField val scheduledActivation: FeeSponsorProgramActivation?,
) {
    init {
        if (activeRevision != null) require(activeRevision > 0) { "activeRevision must be positive" }
        if (stagedRevision != null) require(stagedRevision > 0) { "stagedRevision must be positive" }
    }

    override fun equals(other: Any?): Boolean =
        other is FeeSponsorProgramResponse &&
            id == other.id &&
            lifecycle == other.lifecycle &&
            activeRevision == other.activeRevision &&
            stagedRevision == other.stagedRevision &&
            scheduledActivation == other.scheduledActivation

    override fun hashCode(): Int {
        var result = id.hashCode()
        result = 31 * result + lifecycle.hashCode()
        result = 31 * result + (activeRevision?.hashCode() ?: 0)
        result = 31 * result + (stagedRevision?.hashCode() ?: 0)
        result = 31 * result + (scheduledActivation?.hashCode() ?: 0)
        return result
    }
}
