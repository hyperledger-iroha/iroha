package org.hyperledger.iroha.android.subscriptions;

import java.util.Objects;

/** Response payload returned after registering a subscription plan. */
public final class SubscriptionPlanCreateResponse {

  private final boolean ok;
  private final String planId;
  private final String txHashHex;

  public SubscriptionPlanCreateResponse(
      final boolean ok, final String planId, final String txHashHex) {
    this.ok = ok;
    this.planId = Objects.requireNonNull(planId, "planId");
    this.txHashHex = Objects.requireNonNull(txHashHex, "txHashHex");
  }

  public boolean ok() {
    return ok;
  }

  public String planId() {
    return planId;
  }

  public String txHashHex() {
    return txHashHex;
  }
}
