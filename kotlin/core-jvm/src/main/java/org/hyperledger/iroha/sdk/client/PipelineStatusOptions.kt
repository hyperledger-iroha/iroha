package org.hyperledger.iroha.sdk.client

private val DEFAULT_FAILURE = setOf("Rejected", "Expired")

/**
 * Configuration for polling Torii pipeline status endpoints.
 *
 * Success is deliberately not configurable: exact canonical `Applied` is the
 * only status that proves execution. `Approved` and `Committed` are progress.
 */
class PipelineStatusOptions(
    @JvmField val intervalMillis: Long = 1_000L,
    @JvmField val timeoutMillis: Long? = 30_000L,
    maxAttempts: Int? = null,
    failureStatuses: Set<String> = DEFAULT_FAILURE,
    @JvmField val observer: StatusObserver? = null,
) {
    @JvmField val maxAttempts: Int? = maxAttempts?.also {
        require(it > 0) { "maxAttempts must be positive" }
    }

    @JvmField val failureStatuses: Set<String> = failureStatuses.toSet().also {
        check(it.isNotEmpty()) { "failureStatuses must contain at least one entry" }
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
