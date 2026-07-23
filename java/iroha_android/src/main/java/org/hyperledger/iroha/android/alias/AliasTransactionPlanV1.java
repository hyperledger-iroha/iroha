package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;

/** Alias transaction plan and its canonical body commitment. */
public final class AliasTransactionPlanV1 extends AliasJsonValue {
  private final AliasSetupModels.AliasTransactionPlanBodyV1 body;
  private final String planHash;

  /** Constructs an immutable alias transaction plan. */
  public AliasTransactionPlanV1(
      final AliasSetupModels.AliasTransactionPlanBodyV1 body, final String planHash) {
    if (body == null) throw new IllegalArgumentException("body must not be null");
    this.body = body;
    this.planHash = AliasNameSupport.requireHash(planHash, "planHash");
  }

  /** Returns the canonical plan body. */
  public AliasSetupModels.AliasTransactionPlanBodyV1 body() { return body; }

  /** Returns the domain-separated Iroha hash of the Norito-encoded body. */
  public String planHash() { return planHash; }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("body", body.toJsonMap());
    map.put("plan_hash", planHash);
    return map;
  }
}

