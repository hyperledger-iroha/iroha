package org.hyperledger.iroha.android.subscriptions;

import java.util.Objects;

/** Response payload returned after subscription actions (pause/resume/cancel/charge-now/usage). */
public final class SubscriptionActionResponse {

  private final boolean ok;
  private final String subscriptionId;
  private final String txHashHex;

  public SubscriptionActionResponse(
      final boolean ok, final String subscriptionId, final String txHashHex) {
    this.ok = ok;
    this.subscriptionId = Objects.requireNonNull(subscriptionId, "subscriptionId");
    this.txHashHex = Objects.requireNonNull(txHashHex, "txHashHex");
  }

  public boolean ok() {
    return ok;
  }

  public String subscriptionId() {
    return subscriptionId;
  }

  public String txHashHex() {
    return txHashHex;
  }
}
