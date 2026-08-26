package org.hyperledger.iroha.sdk.client

/** Raised when transaction submission returns anything other than the canonical HTTP 202. */
class TransactionSubmissionHttpException(
    @JvmField val hashHex: String,
    @JvmField val statusCode: Int,
    rejectCode: String?,
    responseBody: String?,
) : RuntimeException(buildMessage(hashHex, statusCode, rejectCode, responseBody)) {

    @JvmField
    val rejectCode: String? = rejectCode?.trim()?.ifBlank { null }

    @JvmField
    val responseBody: String? = responseBody?.ifBlank { null }

    companion object {
        private fun buildMessage(
            hashHex: String,
            statusCode: Int,
            rejectCode: String?,
            responseBody: String?,
        ): String = buildString {
            append("Transaction submission for ")
            append(hashHex)
            append(" must return HTTP 202, got ")
            append(statusCode)
            val trimmedCode = rejectCode?.trim()
            if (!trimmedCode.isNullOrBlank()) {
                append(" (reject_code=").append(trimmedCode).append(")")
            }
            if (!responseBody.isNullOrBlank()) {
                append(". body=").append(responseBody)
            }
        }
    }
}
