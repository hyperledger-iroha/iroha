package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;

/** Owner-configured deterministic native alias auto-renew policy. */
public final class AliasAutoRenewConfigV1 extends AliasJsonValue {
  private final int termYears;
  private final int policyVersion;
  private final String paymentAsset;
  private final String maxAmount;
  private final long renewBeforeExpiryMs;
  private final long retryBackoffMs;
  private final long maxFailures;

  /** Constructs a bounded owner auto-renew policy. */
  public AliasAutoRenewConfigV1(
      final int termYears,
      final int policyVersion,
      final String paymentAsset,
      final String maxAmount,
      final long renewBeforeExpiryMs,
      final long retryBackoffMs,
      final long maxFailures) {
    if (termYears < 1 || termYears > 0xff) {
      throw new IllegalArgumentException("termYears must fit in a positive unsigned byte");
    }
    if (policyVersion < 0 || policyVersion > 0xffff) {
      throw new IllegalArgumentException("policyVersion must fit in an unsigned 16-bit integer");
    }
    if (!AssetDefinitionIdEncoder.isCanonicalAddress(paymentAsset)) {
      throw new IllegalArgumentException(
          "paymentAsset must use a canonical unprefixed Base58 asset-definition address");
    }
    this.termYears = termYears;
    this.policyVersion = policyVersion;
    this.paymentAsset = paymentAsset;
    this.maxAmount = AliasQuoteGuardV1.canonicalQuantity(maxAmount, "maxAmount");
    this.renewBeforeExpiryMs =
        requirePositive(renewBeforeExpiryMs, "renewBeforeExpiryMs");
    this.retryBackoffMs = requirePositive(retryBackoffMs, "retryBackoffMs");
    if (maxFailures < 1 || maxFailures > 0xffff_ffffL) {
      throw new IllegalArgumentException("maxFailures must fit in a positive unsigned 32-bit integer");
    }
    this.maxFailures = maxFailures;
  }

  /** Returns the renewal term in whole years. */
  public int termYears() { return termYears; }

  /** Returns the accepted SNS policy version. */
  public int policyVersion() { return policyVersion; }

  /** Returns the accepted payment asset. */
  public String paymentAsset() { return paymentAsset; }

  /** Returns the maximum exact renewal charge. */
  public String maxAmount() { return maxAmount; }

  /** Returns how early renewal attempts begin. */
  public long renewBeforeExpiryMs() { return renewBeforeExpiryMs; }

  /** Returns the deterministic retry delay. */
  public long retryBackoffMs() { return retryBackoffMs; }

  /** Returns the failure limit. */
  public long maxFailures() { return maxFailures; }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("term_years", termYears);
    map.put("policy_version", policyVersion);
    map.put("payment_asset", paymentAsset);
    map.put("max_amount", maxAmount);
    map.put("renew_before_expiry_ms", renewBeforeExpiryMs);
    map.put("retry_backoff_ms", retryBackoffMs);
    map.put("max_failures", maxFailures);
    return map;
  }

  private static long requirePositive(final long value, final String field) {
    if (value <= 0) throw new IllegalArgumentException(field + " must be positive");
    return value;
  }
}
