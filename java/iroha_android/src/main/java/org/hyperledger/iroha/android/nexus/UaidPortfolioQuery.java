package org.hyperledger.iroha.android.nexus;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

/** Query parameters for `/v1/accounts/{uaid}/portfolio`. */
public final class UaidPortfolioQuery {
  private final String asset;
  private final String scope;

  private UaidPortfolioQuery(final Builder builder) {
    this.asset = builder.asset;
    this.scope = builder.scope;
  }

  public static Builder builder() {
    return new Builder();
  }

  public String asset() {
    return asset;
  }

  public String scope() {
    return scope;
  }

  public Map<String, String> toQueryParameters() {
    final Map<String, String> params = new LinkedHashMap<>();
    if (asset != null && !asset.isBlank()) {
      params.put("asset", asset.trim());
    }
    if (scope != null && !scope.isBlank()) {
      params.put("scope", scope.trim());
    }
    return Collections.unmodifiableMap(params);
  }

  /** Builder for {@link UaidPortfolioQuery}. */
  public static final class Builder {
    private String asset;
    private String scope;

    private Builder() {}

    public Builder setAsset(final String asset) {
      this.asset = normalizeNonEmpty(asset, "asset");
      return this;
    }

    public Builder setScope(final String scope) {
      this.scope = normalizeNonEmpty(scope, "scope");
      return this;
    }

    public UaidPortfolioQuery build() {
      return new UaidPortfolioQuery(this);
    }

    private static String normalizeNonEmpty(final String raw, final String field) {
      if (raw == null) {
        throw new NullPointerException(field);
      }
      final String trimmed = raw.trim();
      if (trimmed.isEmpty()) {
        throw new IllegalArgumentException(field + " must not be blank");
      }
      if (trimmed.regionMatches(true, 0, "norito:", 0, "norito:".length())
          || trimmed.regionMatches(true, 0, "aid:", 0, "aid:".length())) {
        throw new IllegalArgumentException(field + " must use canonical asset selectors, not legacy prefixes");
      }
      return trimmed;
    }
  }
}
