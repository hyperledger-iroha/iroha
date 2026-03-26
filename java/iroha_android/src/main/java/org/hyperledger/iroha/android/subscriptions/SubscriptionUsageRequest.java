package org.hyperledger.iroha.android.subscriptions;

import java.nio.charset.StandardCharsets;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonEncoder;

/** Request payload for subscription usage recording. */
public final class SubscriptionUsageRequest {

  private final String authority;
  private final String unitKey;
  private final String delta;
  private final String usageTriggerId;

  private SubscriptionUsageRequest(final Builder builder) {
    this.authority = requireNonBlank(builder.authority, "authority");
    this.unitKey = requireNonBlank(builder.unitKey, "unit_key");
    this.delta = requireNumericLiteral(builder.delta, "delta");
    this.usageTriggerId = normalizeOptional(builder.usageTriggerId);
  }

  public String authority() {
    return authority;
  }

  public String unitKey() {
    return unitKey;
  }

  public String delta() {
    return delta;
  }

  public String usageTriggerId() {
    return usageTriggerId;
  }

  public Map<String, Object> toJsonMap() {
    final Map<String, Object> json = new LinkedHashMap<>();
    json.put("authority", authority);
    json.put("unit_key", unitKey);
    json.put("delta", delta);
    if (usageTriggerId != null) {
      json.put("usage_trigger_id", usageTriggerId);
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
    private String unitKey;
    private String delta;
    private String usageTriggerId;

    private Builder() {}

    public Builder authority(final String authority) {
      this.authority = authority;
      return this;
    }

    public Builder unitKey(final String unitKey) {
      this.unitKey = unitKey;
      return this;
    }

    public Builder delta(final String delta) {
      this.delta = delta;
      return this;
    }

    public Builder usageTriggerId(final String usageTriggerId) {
      this.usageTriggerId = usageTriggerId;
      return this;
    }

    public SubscriptionUsageRequest build() {
      return new SubscriptionUsageRequest(this);
    }
  }

  private static String requireNonBlank(final String value, final String field) {
    if (value == null || value.trim().isEmpty()) {
      throw new IllegalStateException(field + " is required");
    }
    return value.trim();
  }

  private static String requireNumericLiteral(final String value, final String field) {
    final String trimmed = requireNonBlank(value, field);
    int index = 0;
    final char first = trimmed.charAt(0);
    if (first == '+' || first == '-') {
      if (first == '-') {
        throw new IllegalArgumentException(field + " must be non-negative");
      }
      index = 1;
    }
    if (index >= trimmed.length()) {
      throw new IllegalArgumentException(field + " must be numeric");
    }
    boolean seenDot = false;
    boolean seenDigit = false;
    for (int i = index; i < trimmed.length(); i++) {
      final char ch = trimmed.charAt(i);
      if (ch == '.') {
        if (seenDot) {
          throw new IllegalArgumentException(field + " must be numeric");
        }
        seenDot = true;
        continue;
      }
      if (ch < '0' || ch > '9') {
        throw new IllegalArgumentException(field + " must be numeric");
      }
      seenDigit = true;
    }
    if (!seenDigit) {
      throw new IllegalArgumentException(field + " must be numeric");
    }
    return trimmed;
  }

  private static String normalizeOptional(final String value) {
    if (value == null) {
      return null;
    }
    final String trimmed = value.trim();
    return trimmed.isEmpty() ? null : trimmed;
  }
}
