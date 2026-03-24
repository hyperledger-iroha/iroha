package org.hyperledger.iroha.sdk.offline

/** Checked error raised by `OfflineJournal` operations. */
class OfflineJournalException : Exception {

    val reason: Reason

    internal constructor(reason: Reason, message: String) : super(message) {
        this.reason = reason
    }

    internal constructor(reason: Reason, message: String, cause: Throwable) : super(message, cause) {
        this.reason = reason
    }

    /** Enumerates the failure categories surfaced by the journal. */
    enum class Reason {
        INVALID_KEY_LENGTH,
        INVALID_TX_ID_LENGTH,
        DUPLICATE_PENDING,
        ALREADY_COMMITTED,
        NOT_PENDING,
        IO,
        INTEGRITY_VIOLATION,
        PAYLOAD_TOO_LARGE,
    }
}
