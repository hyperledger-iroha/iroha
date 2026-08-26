package org.hyperledger.iroha.sdk.client

/**
 * Configuration for polling Torii pipeline status endpoints.
 *
 * Success is deliberately not configurable: exact canonical `Applied` is the
 * only status that proves execution. Exact canonical `Rejected` and `Expired`
 * are the only terminal failures. Every other status remains progress.
 */
class PipelineStatusOptions(
    intervalMillis: Long = 1_000L,
    timeoutMillis: Long? = 30_000L,
    maxAttempts: Int? = null,
    @JvmField val observer: StatusObserver? = null,
) {
    @JvmField val intervalMillis: Long = intervalMillis.also {
        require(it >= 0L) { "intervalMillis must be non-negative" }
    }
    @JvmField val timeoutMillis: Long? = timeoutMillis?.also {
        require(it >= 0L) { "timeoutMillis must be non-negative" }
    }
    @JvmField val maxAttempts: Int? = maxAttempts?.also {
        require(it > 0) { "maxAttempts must be positive" }
    }

    fun interface StatusObserver {
        fun onStatus(statusKind: String, payload: Map<String, Any>, attempt: Int)
    }

    companion object {
        @JvmStatic
        fun resolve(options: PipelineStatusOptions?): PipelineStatusOptions =
            options ?: PipelineStatusOptions()
    }
}
