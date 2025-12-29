package org.hyperledger.iroha.android.connect;

import java.util.Arrays;

/**
 * Deterministic exponential back-off with full jitter for Connect transports.
 * Mirrors {@code iroha_torii_shared::connect_retry::policy} so Android clients
 * match the reconnection cadence of the Rust, Swift, and JavaScript SDKs.
 */
public final class ConnectRetryPolicy {
    public static final long DEFAULT_BASE_DELAY_MS = 5_000L;
    public static final long DEFAULT_MAX_DELAY_MS = 60_000L;

    private static final long GAMMA = 0x9E3779B97F4A7C15L;
    private static final long ATTEMPT_MIX = 0xD1B54A32D192ED03L;
    private static final long SEED_INIT = 0xA0761D6478BD642FL;

    private final long baseDelayMs;
    private final long maxDelayMs;

    public ConnectRetryPolicy() {
        this(DEFAULT_BASE_DELAY_MS, DEFAULT_MAX_DELAY_MS);
    }

    public ConnectRetryPolicy(long baseDelayMs, long maxDelayMs) {
        this.baseDelayMs = Math.max(0L, baseDelayMs);
        this.maxDelayMs = Math.max(0L, maxDelayMs);
    }

    public long getBaseDelayMs() {
        return baseDelayMs;
    }

    public long getMaxDelayMs() {
        return maxDelayMs;
    }

    /** Maximum (capped) delay for the provided attempt. */
    public long capMillis(int attempt) {
        if (baseDelayMs == 0L) {
            return 0L;
        }
        long cap = baseDelayMs;
        int remaining = Math.max(0, attempt);
        while (remaining > 0 && cap < maxDelayMs) {
            long doubled = cap << 1;
            if (doubled <= cap || doubled > maxDelayMs) {
                cap = maxDelayMs;
                break;
            }
            cap = doubled;
            remaining -= 1;
        }
        return Math.min(cap, maxDelayMs);
    }

    /** Deterministic delay in milliseconds for {@code attempt} and the given seed. */
    public long delayMillis(int attempt, byte[] seed) {
        return delayMillis(attempt, seed, 0, seed.length);
    }

    /** Deterministic delay using a slice of {@code seed}. */
    public long delayMillis(int attempt, byte[] seed, int offset, int length) {
        if (seed == null) {
            throw new IllegalArgumentException("seed must not be null");
        }
        if (offset < 0 || length < 0 || offset + length > seed.length) {
            throw new IndexOutOfBoundsException("invalid seed slice");
        }
        long cap = capMillis(attempt);
        if (cap == 0L) {
            return 0L;
        }
        long span = cap + 1L;
        long sample = deterministicSample(seed, offset, length, Math.max(0, attempt));
        return Math.floorMod(sample, span);
    }

    private static long deterministicSample(byte[] seed, int offset, int length, int attempt) {
        long state = SEED_INIT;
        int consumed = 0;
        while (consumed < length) {
            int chunk = Math.min(8, length - consumed);
            long loaded = loadLittleEndian(seed, offset + consumed, chunk);
            state = state + GAMMA;
            state ^= loaded * GAMMA;
            state = splitmix64(state);
            consumed += chunk;
        }
        state = state + GAMMA;
        state ^= (attempt & 0xFFFF_FFFFL) * ATTEMPT_MIX;
        return splitmix64(state);
    }

    private static long loadLittleEndian(byte[] seed, int offset, int length) {
        long value = 0L;
        for (int i = 0; i < length; i++) {
            value |= (seed[offset + i] & 0xFFL) << (i * 8);
        }
        return value;
    }

    private static long splitmix64(long x) {
        long z = x;
        z ^= z >>> 30;
        z *= 0xBF58476D1CE4E5B9L;
        z ^= z >>> 27;
        z *= 0x94D049BB133111EBL;
        z ^= z >>> 31;
        return z;
    }

    @Override
    public String toString() {
        return "ConnectRetryPolicy{" +
                "baseDelayMs=" + baseDelayMs +
                ", maxDelayMs=" + maxDelayMs +
                '}';
    }

    @Override
    public boolean equals(Object other) {
        if (this == other) {
            return true;
        }
        if (!(other instanceof ConnectRetryPolicy)) {
            return false;
        }
        ConnectRetryPolicy that = (ConnectRetryPolicy) other;
        return baseDelayMs == that.baseDelayMs && maxDelayMs == that.maxDelayMs;
    }

    @Override
    public int hashCode() {
        return Arrays.hashCode(new long[]{baseDelayMs, maxDelayMs});
    }
}
