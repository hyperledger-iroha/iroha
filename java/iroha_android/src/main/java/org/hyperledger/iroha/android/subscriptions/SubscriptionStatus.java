package org.hyperledger.iroha.android.subscriptions;

import java.util.Locale;

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

  public static SubscriptionStatus fromString(final String value) {
    if (value == null) {
      throw new IllegalArgumentException("status must not be null");
    }
    final String normalized = value.trim().toLowerCase(Locale.ROOT);
    if (normalized.isEmpty()) {
      throw new IllegalArgumentException("status must not be blank");
    }
    for (final SubscriptionStatus status : values()) {
      if (status.slug.equals(normalized)) {
        return status;
      }
    }
    throw new IllegalArgumentException("unknown subscription status: " + value);
  }
}
