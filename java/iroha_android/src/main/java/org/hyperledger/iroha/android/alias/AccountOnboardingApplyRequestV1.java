package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;

/** Apply body containing only a previously issued stateless receipt. */
public final class AccountOnboardingApplyRequestV1 extends AliasJsonValue {
  private final AccountOnboardingPlanReceiptV1 receipt;

  /** Constructs an exact apply request. */
  public AccountOnboardingApplyRequestV1(final AccountOnboardingPlanReceiptV1 receipt) {
    if (receipt == null) throw new IllegalArgumentException("receipt must not be null");
    this.receipt = receipt;
  }

  public AccountOnboardingPlanReceiptV1 receipt() { return receipt; }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("receipt", receipt.toJsonMap());
    return map;
  }
}
