package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.numeric.NumericV1;

/** Policy, payment asset, cap, and deadline guard for one alias lease operation. */
public final class AliasQuoteGuardV1 extends AliasJsonValue {
  private final int expectedPolicyVersion;
  private final String expectedPaymentAsset;
  private final String maxAmount;
  private final long validUntilMs;

  /** Constructs an exact bounded quote guard. */
  public AliasQuoteGuardV1(
      final int expectedPolicyVersion,
      final String expectedPaymentAsset,
      final String maxAmount,
      final long validUntilMs) {
    if (expectedPolicyVersion < 0 || expectedPolicyVersion > 0xffff) {
      throw new IllegalArgumentException(
          "expectedPolicyVersion must fit in an unsigned 16-bit integer");
    }
    this.expectedPolicyVersion = expectedPolicyVersion;
    if (!AssetDefinitionIdEncoder.isCanonicalAddress(expectedPaymentAsset)) {
      throw new IllegalArgumentException(
          "expectedPaymentAsset must use a canonical unprefixed Base58 asset-definition address");
    }
    this.expectedPaymentAsset = expectedPaymentAsset;
    this.maxAmount = canonicalQuantity(maxAmount, "maxAmount");
    this.validUntilMs = AliasNameSupport.requireNonNegative(validUntilMs, "validUntilMs");
  }

  /** Returns the expected policy version. */
  public int expectedPolicyVersion() {
    return expectedPolicyVersion;
  }

  /** Returns the expected payment asset. */
  public String expectedPaymentAsset() {
    return expectedPaymentAsset;
  }

  /** Returns the maximum authorized quantity. */
  public String maxAmount() {
    return maxAmount;
  }

  /** Returns the last valid block timestamp. */
  public long validUntilMs() {
    return validUntilMs;
  }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("expected_policy_version", expectedPolicyVersion);
    map.put("expected_payment_asset", expectedPaymentAsset);
    map.put("max_amount", maxAmount);
    map.put("valid_until_ms", validUntilMs);
    return map;
  }

  static String canonicalQuantity(final String value, final String field) {
    try {
      NumericV1.QuantityValue.parseCanonical(value);
      return value;
    } catch (final IllegalArgumentException exception) {
      throw new IllegalArgumentException(
          field + " must use a canonical non-negative quantity", exception);
    }
  }
}
