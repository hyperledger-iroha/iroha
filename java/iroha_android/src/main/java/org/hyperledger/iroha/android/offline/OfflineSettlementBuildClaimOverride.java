package org.hyperledger.iroha.android.offline;

import java.util.LinkedHashMap;
import java.util.Map;

/** Optional per-receipt build-claim override for offline settlement submission. */
public final class OfflineSettlementBuildClaimOverride {

  private final String txIdHex;
  private final String appId;
  private final Long buildNumber;
  private final Long issuedAtMs;
  private final Long expiresAtMs;

  private OfflineSettlementBuildClaimOverride(final Builder builder) {
    this.txIdHex =
        OfflineHashLiteral.parseHex(requireNonBlank(builder.txIdHex, "txIdHex"), "txIdHex");
    this.appId = normalizeOptional(builder.appId);
    this.buildNumber = requireNonNegative(builder.buildNumber, "buildNumber");
    this.issuedAtMs = requireNonNegative(builder.issuedAtMs, "issuedAtMs");
    this.expiresAtMs = requireNonNegative(builder.expiresAtMs, "expiresAtMs");
  }

  public static Builder builder() {
    return new Builder();
  }

  public Map<String, Object> toJsonMap() {
    final Map<String, Object> json = new LinkedHashMap<>();
    json.put("tx_id_hex", txIdHex);
    if (appId != null) {
      json.put("app_id", appId);
    }
    if (buildNumber != null) {
      json.put("build_number", buildNumber);
    }
    if (issuedAtMs != null) {
      json.put("issued_at_ms", issuedAtMs);
    }
    if (expiresAtMs != null) {
      json.put("expires_at_ms", expiresAtMs);
    }
    return json;
  }

  private static String requireNonBlank(final String value, final String name) {
    final String normalized = normalizeOptional(value);
    if (normalized == null) {
      throw new IllegalArgumentException(name + " must be provided");
    }
    return normalized;
  }

  private static String normalizeOptional(final String value) {
    if (value == null) {
      return null;
    }
    final String trimmed = value.trim();
    return trimmed.isEmpty() ? null : trimmed;
  }

  private static Long requireNonNegative(final Long value, final String name) {
    if (value != null && value < 0) {
      throw new IllegalArgumentException(name + " must not be negative");
    }
    return value;
  }

  public static final class Builder {
    private String txIdHex;
    private String appId;
    private Long buildNumber;
    private Long issuedAtMs;
    private Long expiresAtMs;

    private Builder() {}

    public Builder txIdHex(final String txIdHex) {
      this.txIdHex = txIdHex;
      return this;
    }

    public Builder appId(final String appId) {
      this.appId = appId;
      return this;
    }

    public Builder buildNumber(final Long buildNumber) {
      this.buildNumber = buildNumber;
      return this;
    }

    public Builder issuedAtMs(final Long issuedAtMs) {
      this.issuedAtMs = issuedAtMs;
      return this;
    }

    public Builder expiresAtMs(final Long expiresAtMs) {
      this.expiresAtMs = expiresAtMs;
      return this;
    }

    public OfflineSettlementBuildClaimOverride build() {
      return new OfflineSettlementBuildClaimOverride(this);
    }
  }
}
