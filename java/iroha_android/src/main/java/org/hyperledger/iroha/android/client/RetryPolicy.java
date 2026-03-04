package org.hyperledger.iroha.android.client;

import java.time.Duration;
import java.util.Collections;
import java.util.HashSet;
import java.util.Objects;
import java.util.Set;

/**
 * Configurable retry policy for {@link IrohaClient} submissions.
 *
 * <p>The policy is deterministic: delays are derived from the attempt number and the configured
 * base delay, making it suitable for deterministic playback in tests and mobile environments.
 */
public final class RetryPolicy {

  private final int maxAttempts;
  private final Duration baseDelay;
  private final boolean retryOnServerError;
  private final boolean retryOnTooManyRequests;
  private final boolean retryOnNetworkError;
  private final Set<Integer> additionalStatusCodes;
  private final Duration maxDelay;

  private RetryPolicy(final Builder builder) {
    this.maxAttempts = builder.maxAttempts;
    this.baseDelay = builder.baseDelay;
    this.retryOnServerError = builder.retryOnServerError;
    this.retryOnTooManyRequests = builder.retryOnTooManyRequests;
    this.retryOnNetworkError = builder.retryOnNetworkError;
    this.additionalStatusCodes =
        Collections.unmodifiableSet(new HashSet<>(builder.additionalStatusCodes));
    this.maxDelay = builder.maxDelay;
  }

  /** No retries (single attempt). */
  public static RetryPolicy none() {
    return builder().build();
  }

  public static Builder builder() {
    return new Builder();
  }

  /** Returns true when another attempt is allowed after {@code attempt} attempts have been made. */
  public boolean allowsRetry(final int attempt) {
    return attempt < maxAttempts;
  }

  /** Returns true if the policy should retry given the response. */
  public boolean shouldRetryResponse(final int attempt, final ClientResponse response) {
    if (!allowsRetry(attempt)) {
      return false;
    }
    final int status = response.statusCode();
    if (retryOnServerError && status >= 500) {
      return true;
    }
    if (retryOnTooManyRequests && status == 429) {
      return true;
    }
    return additionalStatusCodes.contains(status);
  }

  /**
   * Returns true if {@code status} is eligible for retrying regardless of remaining attempts.
   *
   * <p>This mirrors the status-code selection logic used by {@link #shouldRetryResponse(int, ClientResponse)}
   * without the attempt check so callers can distinguish transient server failures from permanent
   * rejections when deciding whether to queue a submission.
   */
  public boolean isRetryableStatus(final int status) {
    if (retryOnServerError && status >= 500) {
      return true;
    }
    if (retryOnTooManyRequests && status == 429) {
      return true;
    }
    return additionalStatusCodes.contains(status);
  }

  /** Returns true if the policy should retry the provided failure. */
  public boolean shouldRetryError(final int attempt) {
    return allowsRetry(attempt) && retryOnNetworkError;
  }

  /**
   * Computes the delay before the next attempt. The delay scales linearly with the attempt number:
   * {@code baseDelay * attempt}.
   */
  public Duration delayForAttempt(final int attempt) {
    if (attempt <= 0) {
      return Duration.ZERO;
    }
    if (baseDelay.isZero() || baseDelay.isNegative()) {
      return Duration.ZERO;
    }
    final long multiplier = Math.min(Integer.MAX_VALUE, attempt);
    try {
      final Duration scaled = baseDelay.multipliedBy(multiplier);
      if (scaled.compareTo(maxDelay) > 0) {
        return maxDelay;
      }
      return scaled;
    } catch (final ArithmeticException overflow) {
      // Clamp to maximum supported duration if multiplication overflows.
      return Duration.ofMillis(Long.MAX_VALUE);
    }
  }

  public static final class Builder {
    private int maxAttempts = 1;
    private Duration baseDelay = Duration.ofMillis(100);
    private boolean retryOnServerError = true;
    private boolean retryOnTooManyRequests = true;
    private boolean retryOnNetworkError = true;
    private final Set<Integer> additionalStatusCodes = new HashSet<>();
    private Duration maxDelay = Duration.ofMillis(Long.MAX_VALUE);

    /** Sets the maximum number of attempts (including the initial attempt). Defaults to 1. */
    public Builder setMaxAttempts(final int maxAttempts) {
      if (maxAttempts < 1) {
        throw new IllegalArgumentException("maxAttempts must be >= 1");
      }
      this.maxAttempts = maxAttempts;
      return this;
    }

    /**
     * Sets the base delay applied between attempts. The effective delay grows linearly with the
     * attempt number (attempt 1 -> base delay, attempt 2 -> base delay * 2, ...).
     */
    public Builder setBaseDelay(final Duration baseDelay) {
      this.baseDelay = Objects.requireNonNull(baseDelay, "baseDelay");
      return this;
    }

    public Builder setRetryOnServerError(final boolean retryOnServerError) {
      this.retryOnServerError = retryOnServerError;
      return this;
    }

    public Builder setRetryOnTooManyRequests(final boolean retryOnTooManyRequests) {
      this.retryOnTooManyRequests = retryOnTooManyRequests;
      return this;
    }

    public Builder setRetryOnNetworkError(final boolean retryOnNetworkError) {
      this.retryOnNetworkError = retryOnNetworkError;
      return this;
    }

    /**
     * Sets the upper bound applied to {@link #delayForAttempt(int)}. When unset the delay grows
     * linearly without a cap (except for {@link Long#MAX_VALUE} overflow protection).
     */
    public Builder setMaxDelay(final Duration maxDelay) {
      this.maxDelay = Objects.requireNonNull(maxDelay, "maxDelay");
      if (this.maxDelay.isNegative()) {
        throw new IllegalArgumentException("maxDelay must be >= 0");
      }
      return this;
    }

    public Builder addRetryStatusCode(final int statusCode) {
      this.additionalStatusCodes.add(statusCode);
      return this;
    }

    public RetryPolicy build() {
      return new RetryPolicy(this);
    }
  }
}
