package org.hyperledger.iroha.android.subscriptions;

import java.util.LinkedHashMap;
import java.util.Map;

/** Query parameters for subscription listings. */
public final class SubscriptionListParams {

  private final String ownedBy;
  private final String provider;
  private final String status;
  private final Long limit;
  private final Long offset;

  private SubscriptionListParams(final Builder builder) {
    this.ownedBy = normalizeOptional(builder.ownedBy);
    this.provider = normalizeOptional(builder.provider);
    this.status = builder.status;
    this.limit = builder.limit;
    this.offset = builder.offset;
  }

  public String ownedBy() {
    return ownedBy;
  }

  public String provider() {
    return provider;
  }

  public String status() {
    return status;
  }

  public Long limit() {
    return limit;
  }

  public Long offset() {
    return offset;
  }

  public Map<String, String> toQueryParameters() {
    final Map<String, String> params = new LinkedHashMap<>();
    if (ownedBy != null) {
      params.put("owned_by", ownedBy);
    }
    if (provider != null) {
      params.put("provider", provider);
    }
    if (status != null) {
      params.put("status", status);
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
    private String ownedBy;
    private String provider;
    private String status;
    private Long limit;
    private Long offset;

    private Builder() {}

    public Builder ownedBy(final String ownedBy) {
      this.ownedBy = ownedBy;
      return this;
    }

    public Builder provider(final String provider) {
      this.provider = provider;
      return this;
    }

    public Builder status(final SubscriptionStatus status) {
      this.status = status == null ? null : status.slug();
      return this;
    }

    public Builder status(final String status) {
      if (status == null) {
        this.status = null;
        return this;
      }
      final String trimmed = status.trim();
      if (trimmed.isEmpty()) {
        this.status = null;
        return this;
      }
      this.status = SubscriptionStatus.fromString(trimmed).slug();
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

    public SubscriptionListParams build() {
      return new SubscriptionListParams(this);
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
