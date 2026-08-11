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
    cause: Throwable?,
) : RuntimeException(buildMessage(hashHex, statusCode), cause) {

    /** Poll the authoritative pipeline status for the exact transaction hash. */
    @JvmOverloads
    fun reconcileWith(
        client: IrohaClient,
        options: PipelineStatusOptions? = null,
    ): CompletableFuture<Map<String, Any>> = client.waitForTransactionStatus(hashHex, options)

    private companion object {
        fun buildMessage(hashHex: String, statusCode: Int?): String = buildString {
            append("Transaction ").append(hashHex)
            append(" had exactly one dispatch attempt, but its admission outcome is unknown")
            if (statusCode != null) append(" after HTTP status ").append(statusCode)
            append(". Do not resend the signed bytes; reconcile by transaction hash.")
        }
    }
}
