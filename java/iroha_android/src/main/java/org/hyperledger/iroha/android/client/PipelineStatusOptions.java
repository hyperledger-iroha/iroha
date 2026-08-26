package org.hyperledger.iroha.android.client;

import java.util.Map;

/**
 * Configuration for polling Torii pipeline status endpoints.
 *
 * <p>Success is deliberately not configurable: exact canonical {@code Applied} is the only status
 * that proves execution. Exact canonical {@code Rejected} and {@code Expired} are the only
 * terminal failures. Every other status remains progress.
 */
public final class PipelineStatusOptions {

  public interface StatusObserver {
    void onStatus(String statusKind, Map<String, Object> payload, int attempt);
  }

  private final long intervalMillis;
  private final Long timeoutMillis;
  private final Integer maxAttempts;
  private final StatusObserver observer;

  private PipelineStatusOptions(final Builder builder) {
    this.intervalMillis = builder.intervalMillis;
    this.timeoutMillis = builder.timeoutMillis;
    this.maxAttempts = builder.maxAttempts;
    this.observer = builder.observer;
  }

  public static Builder builder() {
    return new Builder();
  }

  static PipelineStatusOptions resolve(final PipelineStatusOptions options) {
    return options != null ? options : builder().build();
  }

  public long intervalMillis() {
    return intervalMillis;
  }

  public Long timeoutMillis() {
    return timeoutMillis;
  }

  public Integer maxAttempts() {
    return maxAttempts;
  }

  public StatusObserver observer() {
    return observer;
  }

  public static final class Builder {
    private long intervalMillis = 1_000L;
    private Long timeoutMillis = 30_000L;
    private Integer maxAttempts = null;
    private StatusObserver observer = null;

    private Builder() {}

    public Builder intervalMillis(final long intervalMillis) {
      if (intervalMillis < 0L) {
        throw new IllegalArgumentException("intervalMillis must be non-negative");
      }
      this.intervalMillis = intervalMillis;
      return this;
    }

    public Builder timeoutMillis(final Long timeoutMillis) {
      if (timeoutMillis == null) {
        this.timeoutMillis = null;
      } else if (timeoutMillis < 0L) {
        throw new IllegalArgumentException("timeoutMillis must be non-negative");
      } else {
        this.timeoutMillis = timeoutMillis;
      }
      return this;
    }

    public Builder maxAttempts(final Integer maxAttempts) {
      if (maxAttempts == null) {
        this.maxAttempts = null;
      } else if (maxAttempts <= 0) {
        throw new IllegalArgumentException("maxAttempts must be positive");
      } else {
        this.maxAttempts = maxAttempts;
      }
      return this;
    }

    public Builder observer(final StatusObserver observer) {
      this.observer = observer;
      return this;
    }

    public PipelineStatusOptions build() {
      return new PipelineStatusOptions(this);
    }
  }
}
