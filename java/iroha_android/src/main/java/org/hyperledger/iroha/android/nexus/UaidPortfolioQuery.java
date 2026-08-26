package org.hyperledger.iroha.android.nexus;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

/** Query parameters for `/v1/accounts/{uaid}/portfolio`. */
public final class UaidPortfolioQuery {
  private final String assetId;

  private UaidPortfolioQuery(final Builder builder) {
    this.assetId = builder.assetId;
  }

  public static Builder builder() {
    return new Builder();
  }

  public String assetId() {
    return assetId;
  }

  public Map<String, String> toQueryParameters() {
    final Map<String, String> params = new LinkedHashMap<>();
    if (assetId != null) {
      params.put("asset_id", assetId);
    }
    return Collections.unmodifiableMap(params);
  }

  /** Builder for {@link UaidPortfolioQuery}. */
  public static final class Builder {
    private String assetId;

    private Builder() {}

    public Builder setAssetId(final String assetId) {
      this.assetId = requireExactNonEmpty(assetId, "assetId");
      return this;
    }

    public UaidPortfolioQuery build() {
      return new UaidPortfolioQuery(this);
    }

    private static String requireExactNonEmpty(final String raw, final String field) {
      if (raw == null) {
        throw new NullPointerException(field);
      }
      final String trimmed = raw.trim();
      if (trimmed.isEmpty()) {
        throw new IllegalArgumentException(field + " must not be blank");
      }
      if (!trimmed.equals(raw)) {
        throw new IllegalArgumentException(field + " must not contain surrounding whitespace");
      }
      return raw;
    }
  }
}
