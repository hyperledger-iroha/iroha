package org.hyperledger.iroha.android.subscriptions;

import java.util.Objects;

/** Response payload returned after creating a subscription. */
public final class SubscriptionCreateResponse {

  private final boolean ok;
  private final String subscriptionId;
  private final String billingTriggerId;
  private final String usageTriggerId;
  private final long firstChargeMs;
  private final String txHashHex;

  public SubscriptionCreateResponse(
      final boolean ok,
      final String subscriptionId,
      final String billingTriggerId,
      final String usageTriggerId,
      final long firstChargeMs,
      final String txHashHex) {
    this.ok = ok;
    this.subscriptionId = Objects.requireNonNull(subscriptionId, "subscriptionId");
    this.billingTriggerId = Objects.requireNonNull(billingTriggerId, "billingTriggerId");
    this.usageTriggerId = usageTriggerId;
    this.firstChargeMs = firstChargeMs;
    this.txHashHex = Objects.requireNonNull(txHashHex, "txHashHex");
  }

  public boolean ok() {
    return ok;
  }

  public String subscriptionId() {
    return subscriptionId;
  }

  public String billingTriggerId() {
    return billingTriggerId;
  }

  public String usageTriggerId() {
    return usageTriggerId;
  }

  public long firstChargeMs() {
    return firstChargeMs;
  }

  public String txHashHex() {
    return txHashHex;
  }
}
