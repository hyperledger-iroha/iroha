package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;

/** Whether a lifecycle plan requires a transaction or is an exact no-op. */
public enum AliasLifecyclePlanDispositionV1 {
  NO_OP("no_op"),
  APPLY("apply");

  private final String wireValue;

  AliasLifecyclePlanDispositionV1(final String wireValue) {
    this.wireValue = wireValue;
  }

  /** Returns the stable wire value. */
  public String wireValue() {
    return wireValue;
  }

  /** Returns the tagged Norito JSON representation. */
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("kind", wireValue);
    map.put("value", null);
    return map;
  }
}
