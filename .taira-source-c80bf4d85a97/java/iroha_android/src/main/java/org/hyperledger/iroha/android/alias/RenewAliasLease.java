package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;

/** Expiry-CAS alias lease renewal; the transaction authority is the payer. */
public final class RenewAliasLease extends AliasJsonValue {
  /** Stable instruction registry identifier. */
  public static final String WIRE_ID = "iroha.alias.lease.renew";

  private final AliasSetupModels.AliasTargetV1 target;
  private final long expectedCurrentExpiryMs;
  private final long targetExpiryMs;
  private final AliasQuoteGuardV1 quoteGuard;

  /** Constructs a guarded absolute-expiry renewal. */
  public RenewAliasLease(
      final AliasSetupModels.AliasTargetV1 target,
      final long expectedCurrentExpiryMs,
      final long targetExpiryMs,
      final AliasQuoteGuardV1 quoteGuard) {
    if (target == null || quoteGuard == null) {
      throw new IllegalArgumentException("target and quoteGuard must not be null");
    }
    this.target = target;
    this.expectedCurrentExpiryMs =
        AliasNameSupport.requireNonNegative(expectedCurrentExpiryMs, "expectedCurrentExpiryMs");
    this.targetExpiryMs =
        AliasNameSupport.requireNonNegative(targetExpiryMs, "targetExpiryMs");
    if (this.targetExpiryMs <= this.expectedCurrentExpiryMs) {
      throw new IllegalArgumentException(
          "targetExpiryMs must be later than expectedCurrentExpiryMs");
    }
    this.quoteGuard = quoteGuard;
  }

  /** Returns the exact resolved lease target. */
  public AliasSetupModels.AliasTargetV1 target() { return target; }

  /** Returns the expiry that must still be current. */
  public long expectedCurrentExpiryMs() { return expectedCurrentExpiryMs; }

  /** Returns the absolute target expiry. */
  public long targetExpiryMs() { return targetExpiryMs; }

  /** Returns the policy/asset/cap/deadline guard. */
  public AliasQuoteGuardV1 quoteGuard() { return quoteGuard; }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("target", target.toJsonMap());
    map.put("expected_current_expiry_ms", expectedCurrentExpiryMs);
    map.put("target_expiry_ms", targetExpiryMs);
    map.put("quote_guard", quoteGuard.toJsonMap());
    return map;
  }
}
