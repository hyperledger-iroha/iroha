package org.hyperledger.iroha.android.subscriptions;

import java.nio.charset.StandardCharsets;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

import org.hyperledger.iroha.android.client.JsonEncoder;

/** Request payload for `POST /v1/subscriptions`. */
public final class SubscriptionCreateRequest {

  private final String authority;
  private final String privateKey;
  private final String subscriptionId;
  private final String planId;
  private final String billingTriggerId;
  private final String usageTriggerId;
  private final Long firstChargeMs;
  private final Boolean grantUsageToProvider;

  private SubscriptionCreateRequest(final Builder builder) {
    this.authority = requireNonBlank(builder.authority, "authority");
    this.privateKey = requireNonBlank(builder.privateKey, "private_key");
    this.subscriptionId = requireNonBlank(builder.subscriptionId, "subscription_id");
    this.planId = requireNonBlank(builder.planId, "plan_id");
    this.billingTriggerId = normalizeOptional(builder.billingTriggerId);
    this.usageTriggerId = normalizeOptional(builder.usageTriggerId);
    this.firstChargeMs = builder.firstChargeMs;
    this.grantUsageToProvider = builder.grantUsageToProvider;
  }

  public String authority() {
    return authority;
  }

  public String privateKey() {
    return privateKey;
  }

  public String subscriptionId() {
    return subscriptionId;
  }

  public String planId() {
    return planId;
  }

  public String billingTriggerId() {
    return billingTriggerId;
  }

  public String usageTriggerId() {
    return usageTriggerId;
  }

  public Long firstChargeMs() {
    return firstChargeMs;
  }

  public Boolean grantUsageToProvider() {
    return grantUsageToProvider;
  }

  public Map<String, Object> toJsonMap() {
    final Map<String, Object> json = new LinkedHashMap<>();
    json.put("authority", authority);
    json.put("private_key", privateKey);
    json.put("subscription_id", subscriptionId);
    json.put("plan_id", planId);
    if (billingTriggerId != null) {
      json.put("billing_trigger_id", billingTriggerId);
    }
    if (usageTriggerId != null) {
      json.put("usage_trigger_id", usageTriggerId);
    }
    if (firstChargeMs != null) {
      json.put("first_charge_ms", firstChargeMs);
    }
    if (grantUsageToProvider != null) {
      json.put("grant_usage_to_provider", grantUsageToProvider);
    }
    return Collections.unmodifiableMap(json);
  }

  public byte[] toJsonBytes() {
    return JsonEncoder.encode(toJsonMap()).getBytes(StandardCharsets.UTF_8);
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {
    private String authority;
    private String privateKey;
    private String subscriptionId;
    private String planId;
    private String billingTriggerId;
    private String usageTriggerId;
    private Long firstChargeMs;
    private Boolean grantUsageToProvider;

    private Builder() {}

    public Builder authority(final String authority) {
      this.authority = authority;
      return this;
    }

    public Builder privateKey(final String privateKey) {
      this.privateKey = privateKey;
      return this;
    }

    public Builder subscriptionId(final String subscriptionId) {
      this.subscriptionId = subscriptionId;
      return this;
    }

    public Builder planId(final String planId) {
      this.planId = planId;
      return this;
    }

    public Builder billingTriggerId(final String billingTriggerId) {
      this.billingTriggerId = billingTriggerId;
      return this;
    }

    public Builder usageTriggerId(final String usageTriggerId) {
      this.usageTriggerId = usageTriggerId;
      return this;
    }

    public Builder firstChargeMs(final Long firstChargeMs) {
      if (firstChargeMs != null && firstChargeMs < 0) {
        throw new IllegalArgumentException("firstChargeMs must be non-negative");
      }
      this.firstChargeMs = firstChargeMs;
      return this;
    }

    public Builder grantUsageToProvider(final Boolean grantUsageToProvider) {
      this.grantUsageToProvider = grantUsageToProvider;
      return this;
    }

    public SubscriptionCreateRequest build() {
      return new SubscriptionCreateRequest(this);
    }
  }

  private static String requireNonBlank(final String value, final String field) {
    if (value == null || value.trim().isEmpty()) {
      throw new IllegalStateException(field + " is required");
    }
    return value.trim();
  }

  private static String normalizeOptional(final String value) {
    if (value == null) {
      return null;
    }
    final String trimmed = value.trim();
    return trimmed.isEmpty() ? null : trimmed;
  }
}
