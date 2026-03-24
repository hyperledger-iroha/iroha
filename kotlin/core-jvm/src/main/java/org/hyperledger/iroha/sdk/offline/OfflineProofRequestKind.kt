package org.hyperledger.iroha.sdk.offline

/** Proof request types supported by Torii (`/v1/offline/transfers/proof`). */
enum class OfflineProofRequestKind {
    SUM,
    COUNTER,
    REPLAY;

    /** Lowercase slug used by the Torii `kind` parameter. */
    fun asParameter(): String = name.lowercase()
}
