package org.hyperledger.iroha.android.subscriptions;

/** Supported subscription status literals accepted by Torii list filters. */
public enum SubscriptionStatus {
  ACTIVE("active"),
  PAUSED("paused"),
  PAST_DUE("past_due"),
  CANCELED("canceled"),
  SUSPENDED("suspended");

  private final String slug;

  SubscriptionStatus(final String slug) {
    this.slug = slug;
  }

  public String slug() {
    return slug;
  }
}
