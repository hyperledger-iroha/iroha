package org.hyperledger.iroha.android.client;

import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;

/**
 * Configuration for polling Torii pipeline status endpoints.
 */
public final class PipelineStatusOptions {

  private static final List<String> DEFAULT_SUCCESS = List.of("Approved", "Committed", "Applied");
  private static final List<String> DEFAULT_FAILURE = List.of("Rejected", "Expired");

  public interface StatusObserver {
    void onStatus(String statusKind, Map<String, Object> payload, int attempt);
  }

  private final long intervalMillis;
  private final Long timeoutMillis;
  private final Integer maxAttempts;
  private final Set<String> successStatuses;
  private final Set<String> failureStatuses;
  private final StatusObserver observer;

  private PipelineStatusOptions(final Builder builder) {
    this.intervalMillis = builder.intervalMillis;
    this.timeoutMillis = builder.timeoutMillis;
    this.maxAttempts = builder.maxAttempts;
    this.successStatuses = Collections.unmodifiableSet(new HashSet<>(builder.successStatuses));
    this.failureStatuses = Collections.unmodifiableSet(new HashSet<>(builder.failureStatuses));
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

  public Set<String> successStatuses() {
    return successStatuses;
  }

  public Set<String> failureStatuses() {
    return failureStatuses;
  }

  public StatusObserver observer() {
    return observer;
  }

  public static final class Builder {
    private long intervalMillis = 1_000L;
    private Long timeoutMillis = 30_000L;
    private Integer maxAttempts = null;
    private final Set<String> successStatuses = new HashSet<>();
    private final Set<String> failureStatuses = new HashSet<>();
    private StatusObserver observer = null;

    private Builder() {
      this.successStatuses.addAll(DEFAULT_SUCCESS);
      this.failureStatuses.addAll(DEFAULT_FAILURE);
    }

    public Builder intervalMillis(final long intervalMillis) {
      this.intervalMillis = Math.max(0L, intervalMillis);
      return this;
    }

    public Builder timeoutMillis(final Long timeoutMillis) {
      if (timeoutMillis == null) {
        this.timeoutMillis = null;
      } else {
        this.timeoutMillis = Math.max(0L, timeoutMillis);
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

    public Builder successStatuses(final Iterable<String> statuses) {
      this.successStatuses.clear();
      if (statuses != null) {
        for (final String status : statuses) {
          if (status != null) {
            this.successStatuses.add(status);
          }
        }
      }
      return this;
    }

    public Builder failureStatuses(final Iterable<String> statuses) {
      this.failureStatuses.clear();
      if (statuses != null) {
        for (final String status : statuses) {
          if (status != null) {
            this.failureStatuses.add(status);
          }
        }
      }
      return this;
    }

    public Builder observer(final StatusObserver observer) {
      this.observer = observer;
      return this;
    }

    public PipelineStatusOptions build() {
      if (successStatuses.isEmpty()) {
        throw new IllegalStateException("successStatuses must contain at least one entry");
      }
      if (failureStatuses.isEmpty()) {
        throw new IllegalStateException("failureStatuses must contain at least one entry");
      }
      return new PipelineStatusOptions(this);
    }
  }
}
