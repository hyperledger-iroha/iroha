package org.hyperledger.iroha.sdk.offline

/** Runtime exception thrown when offline counter validation fails. */
class OfflineCounterException(
    val reason: Reason,
    message: String,
) : RuntimeException(message) {

    enum class Reason {
        SUMMARY_HASH_MISMATCH,
        COUNTER_VIOLATION,
        INVALID_SCOPE,
    }
}
