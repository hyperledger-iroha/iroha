package org.hyperledger.iroha.sdk.client

/** Raised when transaction status polling receives an unexpected HTTP status code. */
class TransactionStatusHttpException(
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
            append("Unexpected pipeline status code ")
            append(statusCode)
            append(" for transaction ")
            append(hashHex)
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
