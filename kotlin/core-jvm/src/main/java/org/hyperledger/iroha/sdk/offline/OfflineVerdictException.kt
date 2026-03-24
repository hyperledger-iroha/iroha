package org.hyperledger.iroha.sdk.offline

/** Raised when cached verdict metadata is missing or no longer valid. */
class OfflineVerdictException(
    val reason: Reason,
    val certificateIdHex: String?,
    val deadlineKind: OfflineVerdictWarning.DeadlineKind?,
    val deadlineMs: Long?,
    val expectedNonceHex: String?,
    val providedNonceHex: String?,
    message: String,
) : RuntimeException(message) {

    enum class Reason {
        METADATA_MISSING,
        NONCE_MISMATCH,
        DEADLINE_EXPIRED,
    }

    companion object {
        private const val serialVersionUID: Long = 1L
    }
}
