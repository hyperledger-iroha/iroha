package org.hyperledger.iroha.sdk.offline

/** Legacy proof request types retained for local fixture parsing. */
enum class OfflineProofRequestKind {
    SUM,
    COUNTER,
    REPLAY;

    /** Lowercase slug used by the Torii `kind` parameter. */
    fun asParameter(): String = name.lowercase()
}
