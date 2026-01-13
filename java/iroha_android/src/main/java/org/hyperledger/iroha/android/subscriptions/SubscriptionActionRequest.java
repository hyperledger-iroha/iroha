package org.hyperledger.iroha.android.subscriptions;

import java.nio.charset.StandardCharsets;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonEncoder;

/** Request payload for subscription action endpoints (pause/resume/cancel/charge-now). */
public final class SubscriptionActionRequest {

  private final String authority;
  private final String privateKey;
  private final Long chargeAtMs;
  private final CancelMode cancelMode;

  private SubscriptionActionRequest(final Builder builder) {
    this.authority = requireNonBlank(builder.authority, "authority");
    this.privateKey = requireNonBlank(builder.privateKey, "private_key");
    this.chargeAtMs = builder.chargeAtMs;
    this.cancelMode = builder.cancelMode;
  }

  public String authority() {
    return authority;
  }

  public String privateKey() {
    return privateKey;
  }

  public Long chargeAtMs() {
    return chargeAtMs;
  }

  public CancelMode cancelMode() {
    return cancelMode;
  }

  public Map<String, Object> toJsonMap() {
    final Map<String, Object> json = new LinkedHashMap<>();
    json.put("authority", authority);
    json.put("private_key", privateKey);
    if (chargeAtMs != null) {
      json.put("charge_at_ms", chargeAtMs);
    }
    if (cancelMode != null) {
      json.put("cancel_mode", cancelMode.value());
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
    private Long chargeAtMs;
    private CancelMode cancelMode;

    private Builder() {}

    public Builder authority(final String authority) {
      this.authority = authority;
      return this;
    }

    public Builder privateKey(final String privateKey) {
      this.privateKey = privateKey;
      return this;
    }

    public Builder chargeAtMs(final Long chargeAtMs) {
      if (chargeAtMs != null && chargeAtMs < 0) {
        throw new IllegalArgumentException("chargeAtMs must be non-negative");
      }
      this.chargeAtMs = chargeAtMs;
      return this;
    }

    public Builder cancelMode(final CancelMode cancelMode) {
      this.cancelMode = cancelMode;
      return this;
    }

    public SubscriptionActionRequest build() {
      return new SubscriptionActionRequest(this);
    }
  }

  public enum CancelMode {
    IMMEDIATE("immediate"),
    PERIOD_END("period_end");

    private final String value;

    CancelMode(final String value) {
      this.value = value;
    }

    public String value() {
      return value;
    }
  }

  private static String requireNonBlank(final String value, final String field) {
    if (value == null || value.trim().isEmpty()) {
      throw new IllegalStateException(field + " is required");
    }
    return value.trim();
  }
}
