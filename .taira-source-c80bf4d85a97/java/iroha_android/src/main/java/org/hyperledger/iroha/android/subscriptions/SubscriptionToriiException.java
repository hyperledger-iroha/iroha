package org.hyperledger.iroha.android.subscriptions;

/** Runtime error raised when subscription Torii endpoints reject a request or parsing fails. */
public final class SubscriptionToriiException extends RuntimeException {

  private static final long serialVersionUID = 1L;

  public SubscriptionToriiException(final String message) {
    super(message);
  }

  public SubscriptionToriiException(final String message, final Throwable cause) {
    super(message, cause);
  }
}
