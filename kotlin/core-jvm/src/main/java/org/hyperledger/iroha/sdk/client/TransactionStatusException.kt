package org.hyperledger.iroha.sdk.client

/** Raised when a transaction reports a terminal failure status. */
class TransactionStatusException(
    @JvmField val hashHex: String,
    @JvmField val status: String,
    rejectionReason: String? = null,
    @JvmField val payload: Map<String, Any>?,
) : RuntimeException(buildMessage(hashHex, status, rejectionReason)) {

    @JvmField
    val rejectionReason: String? =
        rejectionReason?.trim()?.ifBlank { null }

    companion object {
        private fun buildMessage(
            hashHex: String,
            status: String,
            rejectionReason: String?,
        ): String = buildString {
            append("Transaction ")
            append(hashHex)
            append(" reported failure status ")
            append(status)
            val trimmed = rejectionReason?.trim()
            if (!trimmed.isNullOrBlank()) {
                append(" (reason=").append(trimmed).append(")")
            }
        }
    }
}
