package org.hyperledger.iroha.android.alias;

import java.util.Map;
import org.hyperledger.iroha.android.client.JsonEncoder;

/** Base for immutable alias planner values with structural equality. */
public abstract class AliasJsonValue {
  /** Returns the Norito-JSON-compatible object shape for this value. */
  public abstract Map<String, Object> toJsonMap();

  @Override
  public final boolean equals(final Object other) {
    return other != null
        && getClass().equals(other.getClass())
        && toJsonMap().equals(((AliasJsonValue) other).toJsonMap());
  }

  @Override
  public final int hashCode() {
    return toJsonMap().hashCode();
  }

  @Override
  public String toString() {
    return JsonEncoder.encode(toJsonMap());
  }
}

