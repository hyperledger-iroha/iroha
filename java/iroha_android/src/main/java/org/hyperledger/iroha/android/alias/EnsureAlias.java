package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;

/** One declarative `iroha.alias.ensure` instruction. */
public final class EnsureAlias extends AliasJsonValue {
  /** Stable instruction registry identifier. */
  public static final String WIRE_ID = "iroha.alias.ensure";

  private final AliasSetupModels.AliasIntentV1 intent;
  private final AliasSetupModels.AliasLeaseAcquisitionV1 acquisition;
  private final AliasQuoteGuardV1 quoteGuard;

  /** Constructs one exact declarative alias instruction. */
  public EnsureAlias(
      final AliasSetupModels.AliasIntentV1 intent,
      final AliasSetupModels.AliasLeaseAcquisitionV1 acquisition,
      final AliasQuoteGuardV1 quoteGuard) {
    if (intent == null || acquisition == null || quoteGuard == null) {
      throw new IllegalArgumentException("intent, acquisition, and quoteGuard must not be null");
    }
    this.intent = intent;
    this.acquisition = acquisition;
    this.quoteGuard = quoteGuard;
  }

  /** Returns the exact desired state. */
  public AliasSetupModels.AliasIntentV1 intent() { return intent; }

  /** Returns acquisition-only lease terms. */
  public AliasSetupModels.AliasLeaseAcquisitionV1 acquisition() { return acquisition; }

  /** Returns the policy/asset/cap/deadline guard. */
  public AliasQuoteGuardV1 quoteGuard() { return quoteGuard; }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("intent", intent.toJsonMap());
    map.put("acquisition", acquisition.toJsonMap());
    map.put("quote_guard", quoteGuard.toJsonMap());
    return map;
  }
}

