package org.hyperledger.iroha.sdk.client

import org.hyperledger.iroha.sdk.json.JsonObject

/**
 * Raised when transaction status polling receives an unexpected HTTP status code.
 *
 * [status], [code], [details] and [rejectCode] come from the response (see [ToriiApiException]);
 * [responseBody] is a bounded excerpt of the server's error text.
 */
class TransactionStatusHttpException internal constructor(
    @JvmField val hashHex: String,
    status: Int,
    code: String?,
    details: JsonObject?,
    rejectCode: String?,
    responseBody: String?,
) : ToriiApiException(
    status,
    code,
    buildMessage(hashHex, status, rejectCode, responseBody),
    details,
    rejectCode?.trim()?.ifBlank { null },
) {
    /** Bounded excerpt of the server's error text, when present. */
    @JvmField
    val responseBody: String? = responseBody?.ifBlank { null }

    internal companion object {
        fun from(hashHex: String, status: Int, rejectCode: String?, body: ByteArray?): TransactionStatusHttpException {
            val envelope = ToriiErrorEnvelope.decode(body)
            return TransactionStatusHttpException(
                hashHex,
                status,
                envelope?.code,
                envelope?.details,
                rejectCode ?: envelope?.rejectCode,
                HttpErrorMessageExtractor.extractMessage(body),
            )
        }

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
