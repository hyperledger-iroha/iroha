package org.hyperledger.iroha.android.subscriptions;

import java.util.LinkedHashMap;
import java.util.Map;

/** Query parameters for subscription plan listings. */
public final class SubscriptionPlanListParams {

  private final String provider;
  private final Long limit;
  private final Long offset;

  private SubscriptionPlanListParams(final Builder builder) {
    this.provider = normalizeOptional(builder.provider);
    this.limit = builder.limit;
    this.offset = builder.offset;
  }

  public String provider() {
    return provider;
  }

  public Long limit() {
    return limit;
  }

  public Long offset() {
    return offset;
  }

  public Map<String, String> toQueryParameters() {
    final Map<String, String> params = new LinkedHashMap<>();
    if (provider != null) {
      params.put("provider", provider);
    }
    if (limit != null) {
      params.put("limit", String.valueOf(limit));
    }
    if (offset != null) {
      params.put("offset", String.valueOf(offset));
    }
    return params;
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {
    private String provider;
    private Long limit;
    private Long offset;

    private Builder() {}

    public Builder provider(final String provider) {
      this.provider = provider;
      return this;
    }

    public Builder limit(final Long limit) {
      if (limit != null && limit < 0) {
        throw new IllegalArgumentException("limit must be non-negative");
      }
      this.limit = limit;
      return this;
    }

    public Builder offset(final Long offset) {
      if (offset != null && offset < 0) {
        throw new IllegalArgumentException("offset must be non-negative");
      }
      this.offset = offset;
      return this;
    }

    public SubscriptionPlanListParams build() {
      return new SubscriptionPlanListParams(this);
    }
  }

  private static String normalizeOptional(final String value) {
    if (value == null) {
      return null;
    }
    final String trimmed = value.trim();
    return trimmed.isEmpty() ? null : trimmed;
  }
}
