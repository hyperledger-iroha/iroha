package org.hyperledger.iroha.android.offline;

import java.util.LinkedHashMap;
import java.util.Locale;
import java.util.Map;

/** Request payload for issuing an operator-signed offline build claim. */
public final class OfflineBuildClaimIssueRequest {

  private final String certificateIdHex;
  private final String txIdHex;
  private final String platform;
  private final String appId;
  private final Long buildNumber;
  private final Long issuedAtMs;
  private final Long expiresAtMs;

  private OfflineBuildClaimIssueRequest(final Builder builder) {
    this.certificateIdHex =
        OfflineHashLiteral.parseHex(requireNonBlank(builder.certificateIdHex, "certificateIdHex"),
            "certificateIdHex");
    this.txIdHex =
        OfflineHashLiteral.parseHex(requireNonBlank(builder.txIdHex, "txIdHex"), "txIdHex");
    this.platform = requireNonBlank(builder.platform, "platform").toLowerCase(Locale.ROOT);
    requireSupportedPlatform(this.platform);
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
    json.put("certificate_id_hex", certificateIdHex);
    json.put("tx_id_hex", txIdHex);
    json.put("platform", platform);
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

  private static void requireSupportedPlatform(final String platform) {
    if (!"apple".equals(platform) && !"android".equals(platform)) {
      throw new IllegalArgumentException("platform must be either \"apple\" or \"android\"");
    }
  }

  public static final class Builder {
    private String certificateIdHex;
    private String txIdHex;
    private String platform;
    private String appId;
    private Long buildNumber;
    private Long issuedAtMs;
    private Long expiresAtMs;

    private Builder() {}

    public Builder certificateIdHex(final String certificateIdHex) {
      this.certificateIdHex = certificateIdHex;
      return this;
    }

    public Builder txIdHex(final String txIdHex) {
      this.txIdHex = txIdHex;
      return this;
    }

    public Builder platform(final String platform) {
      this.platform = platform;
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

    public OfflineBuildClaimIssueRequest build() {
      return new OfflineBuildClaimIssueRequest(this);
    }
  }
}
