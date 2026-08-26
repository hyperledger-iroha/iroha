package org.hyperledger.iroha.sdk.client

import java.util.concurrent.CompletableFuture

/**
 * Raised after exactly one signed-transaction dispatch was attempted but no authoritative
 * admission outcome was obtained.
 *
 * The exact signed bytes were not retried or queued by the SDK. Call [reconcileWith] (or query the
 * same [hashHex] through another trusted client) before deciding whether to construct and sign a
 * replacement transaction.
 */
class AmbiguousTransactionSubmissionException internal constructor(
    @JvmField val hashHex: String,
    @JvmField val statusCode: Int?,
    rejectCode: String?,
    responseBody: String?,
    cause: Throwable?,
) : RuntimeException(buildMessage(hashHex, statusCode, rejectCode, responseBody), cause) {

    @JvmField
    val rejectCode: String? = rejectCode?.trim()?.ifBlank { null }

    @JvmField
    val responseBody: String? = responseBody?.ifBlank { null }

    /** Poll the authoritative pipeline status for the exact transaction hash. */
    @JvmOverloads
    fun reconcileWith(
        client: IrohaClient,
        options: PipelineStatusOptions? = null,
    ): CompletableFuture<Map<String, Any>> = client.waitForTransactionStatus(hashHex, options)

    private companion object {
        fun buildMessage(
            hashHex: String,
            statusCode: Int?,
            rejectCode: String?,
            responseBody: String?,
        ): String = buildString {
            append("Transaction ").append(hashHex)
            append(" had exactly one dispatch attempt, but its admission outcome is unknown")
            if (statusCode != null) append(" after HTTP status ").append(statusCode)
            val trimmedCode = rejectCode?.trim()
            if (!trimmedCode.isNullOrBlank()) {
                append(" (reject_code=").append(trimmedCode).append(")")
            }
            if (!responseBody.isNullOrBlank()) {
                append(". body=").append(responseBody)
            }
            append(". Do not resend the signed bytes; reconcile by transaction hash.")
        }
    }
}
