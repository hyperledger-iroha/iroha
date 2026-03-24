package org.hyperledger.iroha.sdk.connect

/**
 * Deterministic exponential back-off with full jitter for Connect transports.
 * Mirrors `iroha_torii_shared::connect_retry::policy` so Android clients
 * match the reconnection cadence of the Rust, Swift, and JavaScript SDKs.
 */
class ConnectRetryPolicy @JvmOverloads constructor(
    @JvmField val baseDelayMs: Long = DEFAULT_BASE_DELAY_MS,
    @JvmField val maxDelayMs: Long = DEFAULT_MAX_DELAY_MS,
) {
    init {
        // Fields are clamped to non-negative in the getter-style to match Java behavior,
        // but we just clamp at construction time.
    }

    private val clampedBase = maxOf(0L, baseDelayMs)
    private val clampedMax = maxOf(0L, maxDelayMs)

    /** Maximum (capped) delay for the provided attempt. */
    fun capMillis(attempt: Int): Long {
        if (clampedBase == 0L) return 0L
        var cap = clampedBase
        var remaining = maxOf(0, attempt)
        while (remaining > 0 && cap < clampedMax) {
            val doubled = cap shl 1
            if (doubled <= cap || doubled > clampedMax) {
                cap = clampedMax
                break
            }
            cap = doubled
            remaining -= 1
        }
        return minOf(cap, clampedMax)
    }

    /** Deterministic delay in milliseconds for [attempt] and the given seed. */
    fun delayMillis(attempt: Int, seed: ByteArray): Long =
        delayMillis(attempt, seed, 0, seed.size)

    /** Deterministic delay using a slice of [seed]. */
    fun delayMillis(attempt: Int, seed: ByteArray, offset: Int, length: Int): Long {
        require(offset >= 0 && length >= 0 && offset + length <= seed.size) {
            "invalid seed slice"
        }
        val cap = capMillis(attempt)
        if (cap == 0L) return 0L
        val span = cap + 1L
        val sample = deterministicSample(seed, offset, length, maxOf(0, attempt))
        return Math.floorMod(sample, span)
    }

    override fun toString(): String =
        "ConnectRetryPolicy{baseDelayMs=$clampedBase, maxDelayMs=$clampedMax}"

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is ConnectRetryPolicy) return false
        return clampedBase == other.clampedBase && clampedMax == other.clampedMax
    }

    override fun hashCode(): Int {
        return longArrayOf(clampedBase, clampedMax).contentHashCode()
    }

    companion object {
        const val DEFAULT_BASE_DELAY_MS = 5_000L
        const val DEFAULT_MAX_DELAY_MS = 60_000L

        private const val GAMMA = -0x61C8864680B583EBL // 0x9E3779B97F4A7C15L
        private const val ATTEMPT_MIX = -0x2E4AB5CD2E6D12FDL // 0xD1B54A32D192ED03L
        private const val SEED_INIT = -0x5F89E29B87429BD1L // 0xA0761D6478BD642FL

        private fun deterministicSample(seed: ByteArray, offset: Int, length: Int, attempt: Int): Long {
            var state = SEED_INIT
            var consumed = 0
            while (consumed < length) {
                val chunk = minOf(8, length - consumed)
                val loaded = loadLittleEndian(seed, offset + consumed, chunk)
                state += GAMMA
                state = state xor (loaded * GAMMA)
                state = splitmix64(state)
                consumed += chunk
            }
            state += GAMMA
            state = state xor ((attempt.toLong() and 0xFFFF_FFFFL) * ATTEMPT_MIX)
            return splitmix64(state)
        }

        private fun loadLittleEndian(seed: ByteArray, offset: Int, length: Int): Long {
            var value = 0L
            for (i in 0 until length) {
                value = value or ((seed[offset + i].toLong() and 0xFFL) shl (i * 8))
            }
            return value
        }

        private fun splitmix64(x: Long): Long {
            var z = x
            z = z xor (z ushr 30)
            z *= -0x40A7B892E31B1A47L // 0xBF58476D1CE4E5B9L
            z = z xor (z ushr 27)
            z *= -0x6B2FB644ECCEEE15L // 0x94D049BB133111EBL
            z = z xor (z ushr 31)
            return z
        }
    }
}
