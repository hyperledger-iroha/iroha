package org.hyperledger.iroha.sdk.client

private val DEFAULT_SUCCESS = setOf("Approved", "Committed", "Applied")
private val DEFAULT_FAILURE = setOf("Rejected", "Expired")

/** Configuration for polling Torii pipeline status endpoints. */
class PipelineStatusOptions(
    @JvmField val intervalMillis: Long = 1_000L,
    @JvmField val timeoutMillis: Long? = 30_000L,
    maxAttempts: Int? = null,
    successStatuses: Set<String> = DEFAULT_SUCCESS,
    failureStatuses: Set<String> = DEFAULT_FAILURE,
    @JvmField val observer: StatusObserver? = null,
) {
    @JvmField val maxAttempts: Int? = maxAttempts?.also {
        require(it > 0) { "maxAttempts must be positive" }
    }

    @JvmField val successStatuses: Set<String> = successStatuses.toSet().also {
        check(it.isNotEmpty()) { "successStatuses must contain at least one entry" }
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
