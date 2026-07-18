package org.hyperledger.iroha.android.model;

import java.util.Objects;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.numeric.NumericV1;

/** Exact asset and maximum amount authorized for one fee component. */
public final class FeeChargeLimit {
  private final FeeChargeKind kind;
  private final String assetDefinitionId;
  private final String maxAmount;

  public FeeChargeLimit(
      final FeeChargeKind kind, final String assetDefinitionId, final String maxAmount) {
    this.kind = Objects.requireNonNull(kind, "kind");
    if (!AssetDefinitionIdEncoder.isCanonicalAddress(assetDefinitionId)) {
      throw new IllegalArgumentException(
          "assetDefinitionId must be a canonical unprefixed Base58 asset definition id");
    }
    this.assetDefinitionId = assetDefinitionId;
    final NumericV1.QuantityValue amount = NumericV1.QuantityValue.parseCanonical(maxAmount);
    if (amount.mantissa().signum() <= 0) {
      throw new IllegalArgumentException("maxAmount must be positive");
    }
    this.maxAmount = amount.toString();
  }

  public FeeChargeKind kind() { return kind; }
  public String assetDefinitionId() { return assetDefinitionId; }
  public String maxAmount() { return maxAmount; }

  @Override
  public boolean equals(final Object other) {
    if (this == other) return true;
    if (!(other instanceof FeeChargeLimit)) return false;
    final FeeChargeLimit that = (FeeChargeLimit) other;
    return kind == that.kind
        && assetDefinitionId.equals(that.assetDefinitionId)
        && maxAmount.equals(that.maxAmount);
  }

  @Override
  public int hashCode() { return Objects.hash(kind, assetDefinitionId, maxAmount); }
}
