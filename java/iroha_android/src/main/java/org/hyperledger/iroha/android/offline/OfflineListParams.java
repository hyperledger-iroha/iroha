package org.hyperledger.iroha.android.offline;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;

/** Query parameters for Torii offline listing endpoints. */
public final class OfflineListParams {

  private final String filter;
  private final Long limit;
  private final Long offset;
  private final String sort;
  private final String addressFormat;
  private final String assetId;
  private final Long certificateExpiresBeforeMs;
  private final Long certificateExpiresAfterMs;
  private final Long policyExpiresBeforeMs;
  private final Long policyExpiresAfterMs;
  private final PlatformPolicy platformPolicy;
  private final String verdictIdHex;
  private final boolean requireVerdict;
  private final boolean onlyMissingVerdict;

  private OfflineListParams(final Builder builder) {
    this.filter = builder.filter;
    this.limit = builder.limit;
    this.offset = builder.offset;
    this.sort = builder.sort;
    this.addressFormat = builder.addressFormat;
    this.assetId = builder.assetId;
    this.certificateExpiresBeforeMs = builder.certificateExpiresBeforeMs;
    this.certificateExpiresAfterMs = builder.certificateExpiresAfterMs;
    this.policyExpiresBeforeMs = builder.policyExpiresBeforeMs;
    this.policyExpiresAfterMs = builder.policyExpiresAfterMs;
    this.platformPolicy = builder.platformPolicy;
    this.verdictIdHex = builder.verdictIdHex;
    this.requireVerdict = builder.requireVerdict;
    this.onlyMissingVerdict = builder.onlyMissingVerdict;
  }

  public Optional<String> filter() {
    return Optional.ofNullable(filter);
  }

  public Optional<Long> limit() {
    return Optional.ofNullable(limit);
  }

  public Optional<Long> offset() {
    return Optional.ofNullable(offset);
  }

  public Optional<String> sort() {
    return Optional.ofNullable(sort);
  }

  public Optional<String> addressFormat() {
    return Optional.ofNullable(addressFormat);
  }

  /** Encodes the parameters into a string map suitable for query strings. */
  public Map<String, String> toQueryParameters() {
    final Map<String, String> params = new LinkedHashMap<>();
    filter().filter(value -> !value.isBlank()).ifPresent(value -> params.put("filter", value));
    limit().ifPresent(value -> params.put("limit", String.valueOf(value)));
    offset().ifPresent(value -> params.put("offset", String.valueOf(value)));
    sort().filter(value -> !value.isBlank()).ifPresent(value -> params.put("sort", value));
    addressFormat()
        .filter(value -> !value.isBlank())
        .ifPresent(value -> params.put("address_format", value));
    if (assetId != null && !assetId.isBlank()) {
      params.put("asset_id", assetId.trim());
    }
    if (certificateExpiresBeforeMs != null) {
      params.put("certificate_expires_before_ms", String.valueOf(certificateExpiresBeforeMs));
    }
    if (certificateExpiresAfterMs != null) {
      params.put("certificate_expires_after_ms", String.valueOf(certificateExpiresAfterMs));
    }
    if (policyExpiresBeforeMs != null) {
      params.put("policy_expires_before_ms", String.valueOf(policyExpiresBeforeMs));
    }
    if (policyExpiresAfterMs != null) {
      params.put("policy_expires_after_ms", String.valueOf(policyExpiresAfterMs));
    }
    if (platformPolicy != null) {
      params.put("platform_policy", platformPolicy.slug());
    }
    if (verdictIdHex != null && !verdictIdHex.isBlank()) {
      params.put("verdict_id_hex", verdictIdHex.toLowerCase());
    }
    if (requireVerdict) {
      params.put("require_verdict", "true");
    }
    if (onlyMissingVerdict) {
      params.put("only_missing_verdict", "true");
    }
    return params;
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {
    private String filter;
    private Long limit;
    private Long offset;
    private String sort;
    private String addressFormat;
    private String assetId;
    private Long certificateExpiresBeforeMs;
    private Long certificateExpiresAfterMs;
    private Long policyExpiresBeforeMs;
    private Long policyExpiresAfterMs;
    private PlatformPolicy platformPolicy;
    private String verdictIdHex;
    private boolean requireVerdict;
    private boolean onlyMissingVerdict;

    private Builder() {}

    public Builder filter(final String filter) {
      this.filter = filter;
      return this;
    }

    public Builder limit(final Long limit) {
      if (limit != null && limit < 0) {
        throw new IllegalArgumentException("limit must be positive");
      }
      this.limit = limit;
      return this;
    }

    public Builder offset(final Long offset) {
      if (offset != null && offset < 0) {
        throw new IllegalArgumentException("offset must be positive");
      }
      this.offset = offset;
      return this;
    }

    public Builder sort(final String sort) {
      this.sort = sort;
      return this;
    }

    public Builder addressFormat(final String addressFormat) {
      this.addressFormat = addressFormat;
      return this;
    }

    public Builder assetId(final String assetId) {
      this.assetId = assetId;
      return this;
    }

    public Builder certificateExpiresBeforeMs(final Long value) {
      if (value != null && value < 0) {
        throw new IllegalArgumentException("certificateExpiresBeforeMs must be positive");
      }
      this.certificateExpiresBeforeMs = value;
      return this;
    }

    public Builder certificateExpiresAfterMs(final Long value) {
      if (value != null && value < 0) {
        throw new IllegalArgumentException("certificateExpiresAfterMs must be positive");
      }
      this.certificateExpiresAfterMs = value;
      return this;
    }

    public Builder policyExpiresBeforeMs(final Long value) {
      if (value != null && value < 0) {
        throw new IllegalArgumentException("policyExpiresBeforeMs must be positive");
      }
      this.policyExpiresBeforeMs = value;
      return this;
    }

    public Builder policyExpiresAfterMs(final Long value) {
      if (value != null && value < 0) {
        throw new IllegalArgumentException("policyExpiresAfterMs must be positive");
      }
      this.policyExpiresAfterMs = value;
      return this;
    }

    public Builder verdictIdHex(final String verdictIdHex) {
      this.verdictIdHex = verdictIdHex;
      return this;
    }

    public Builder platformPolicy(final PlatformPolicy platformPolicy) {
      this.platformPolicy = platformPolicy;
      return this;
    }

    public Builder requireVerdict(final boolean requireVerdict) {
      this.requireVerdict = requireVerdict;
      return this;
    }

    public Builder onlyMissingVerdict(final boolean onlyMissingVerdict) {
      this.onlyMissingVerdict = onlyMissingVerdict;
      return this;
    }

    public OfflineListParams build() {
      if (requireVerdict && onlyMissingVerdict) {
        throw new IllegalArgumentException(
            "`requireVerdict` cannot be combined with `onlyMissingVerdict`");
      }
      if (onlyMissingVerdict && verdictIdHex != null && !verdictIdHex.isBlank()) {
        throw new IllegalArgumentException(
            "`verdictIdHex` cannot be combined with `onlyMissingVerdict`");
      }
      return new OfflineListParams(this);
    }
  }

  public enum PlatformPolicy {
    PLAY_INTEGRITY("play_integrity"),
    HMS_SAFETY_DETECT("hms_safety_detect");

    private final String slug;

    PlatformPolicy(final String slug) {
      this.slug = slug;
    }

    String slug() {
      return slug;
    }
  }
}
