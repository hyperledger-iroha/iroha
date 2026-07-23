package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;

/** Alias lifecycle transaction plan and canonical body commitment. */
public final class AliasLifecycleTransactionPlanV1 extends AliasJsonValue {
  private final AliasLifecycleTransactionPlanBodyV1 body;
  private final String planHash;

  /** Constructs one exact lifecycle plan. */
  public AliasLifecycleTransactionPlanV1(
      final AliasLifecycleTransactionPlanBodyV1 body, final String planHash) {
    if (body == null) throw new IllegalArgumentException("body must not be null");
    this.body = body;
    this.planHash = AliasNameSupport.requireHash(planHash, "planHash");
  }

  /** Returns the canonical body. */
  public AliasLifecycleTransactionPlanBodyV1 body() { return body; }

  /** Returns the domain-separated body hash. */
  public String planHash() { return planHash; }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("body", body.toJsonMap());
    map.put("plan_hash", planHash);
    return map;
  }
}
