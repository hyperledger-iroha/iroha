package org.hyperledger.iroha.android.nexus;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import org.hyperledger.iroha.android.address.AssetIdLiteral;

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
    if (assetId != null && !assetId.isBlank()) {
      params.put("asset_id", assetId.trim());
    }
    return Collections.unmodifiableMap(params);
  }

  /** Builder for {@link UaidPortfolioQuery}. */
  public static final class Builder {
    private String assetId;

    private Builder() {}

    public Builder setAssetId(final String assetId) {
      this.assetId = AssetIdLiteral.normalizeEncoded(assetId, "assetId");
      return this;
    }

    public UaidPortfolioQuery build() {
      return new UaidPortfolioQuery(this);
    }
  }
}
