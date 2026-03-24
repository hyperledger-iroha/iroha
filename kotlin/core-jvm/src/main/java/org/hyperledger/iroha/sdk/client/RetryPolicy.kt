package org.hyperledger.iroha.sdk.client

import java.time.Duration

/**
 * Configurable retry policy for `IrohaClient` submissions.
 *
 * The policy is deterministic: delays are derived from the attempt number and the configured
 * base delay, making it suitable for deterministic playback in tests and mobile environments.
 */
class RetryPolicy(
    @JvmField val maxAttempts: Int = 1,
    @JvmField val baseDelay: Duration = Duration.ofMillis(100),
    @JvmField val retryOnServerError: Boolean = true,
    @JvmField val retryOnTooManyRequests: Boolean = true,
    @JvmField val retryOnNetworkError: Boolean = true,
    additionalStatusCodes: Set<Int> = emptySet(),
    @JvmField val maxDelay: Duration = Duration.ofMillis(Long.MAX_VALUE),
) {
    init {
        require(maxAttempts >= 1) { "maxAttempts must be >= 1" }
        require(!maxDelay.isNegative) { "maxDelay must be >= 0" }
    }

    @JvmField
    val additionalStatusCodes: Set<Int> = additionalStatusCodes.toSet()

    /** Returns true when another attempt is allowed after `attempt` attempts have been made. */
    fun allowsRetry(attempt: Int): Boolean = attempt < maxAttempts

    /** Returns true if the policy should retry given the response. */
    fun shouldRetryResponse(attempt: Int, response: ClientResponse): Boolean {
        if (!allowsRetry(attempt)) return false
        val status = response.statusCode
        if (retryOnServerError && status >= 500) return true
        if (retryOnTooManyRequests && status == 429) return true
        return status in additionalStatusCodes
    }

    /**
     * Returns true if `status` is eligible for retrying regardless of remaining attempts.
     *
     * This mirrors the status-code selection logic used by `shouldRetryResponse`
     * without the attempt check so callers can distinguish transient server failures from permanent
     * rejections when deciding whether to queue a submission.
     */
    fun isRetryableStatus(status: Int): Boolean {
        if (retryOnServerError && status >= 500) return true
        if (retryOnTooManyRequests && status == 429) return true
        return status in additionalStatusCodes
    }

    /** Returns true if the policy should retry the provided failure. */
    fun shouldRetryError(attempt: Int): Boolean =
        allowsRetry(attempt) && retryOnNetworkError

    /**
     * Computes the delay before the next attempt. The delay scales linearly with the attempt number:
     * `baseDelay * attempt`.
     */
    fun delayForAttempt(attempt: Int): Duration {
        if (attempt <= 0) return Duration.ZERO
        if (baseDelay.isZero || baseDelay.isNegative) return Duration.ZERO
        val multiplier = attempt.toLong().coerceAtMost(Int.MAX_VALUE.toLong())
        return try {
            val scaled = baseDelay.multipliedBy(multiplier)
            if (scaled > maxDelay) maxDelay else scaled
        } catch (_: ArithmeticException) {
            Duration.ofMillis(Long.MAX_VALUE)
        }
    }

    class Builder {
        private var maxAttempts: Int = 1
        private var baseDelay: Duration = Duration.ofMillis(100)
        private var maxDelay: Duration = Duration.ofMillis(Long.MAX_VALUE)
        private var retryOnServerError: Boolean = true
        private var retryOnTooManyRequests: Boolean = true
        private var retryOnNetworkError: Boolean = true
        private val statusCodes = LinkedHashSet<Int>()

        fun setMaxAttempts(maxAttempts: Int): Builder { this.maxAttempts = maxAttempts; return this }
        fun setBaseDelay(baseDelay: Duration): Builder { this.baseDelay = baseDelay; return this }
        fun setMaxDelay(maxDelay: Duration): Builder { this.maxDelay = maxDelay; return this }
        fun setRetryOnServerError(value: Boolean): Builder { this.retryOnServerError = value; return this }
        fun setRetryOnTooManyRequests(value: Boolean): Builder { this.retryOnTooManyRequests = value; return this }
        fun setRetryOnNetworkError(value: Boolean): Builder { this.retryOnNetworkError = value; return this }
        fun addRetryStatusCode(statusCode: Int): Builder { statusCodes.add(statusCode); return this }
        fun build(): RetryPolicy = RetryPolicy(maxAttempts, baseDelay, retryOnServerError, retryOnTooManyRequests, retryOnNetworkError, statusCodes, maxDelay)
    }

    companion object {
        /** No retries (single attempt). */
        @JvmStatic
        fun none(): RetryPolicy = RetryPolicy()

        @JvmStatic
        fun builder(): Builder = Builder()
    }
}
