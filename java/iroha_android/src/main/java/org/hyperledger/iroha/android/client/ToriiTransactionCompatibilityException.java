package org.hyperledger.iroha.android.client;

/** Base class for transaction-submission capability guard failures. */
public class ToriiTransactionCompatibilityException extends IllegalStateException {
  /** Creates a compatibility failure with an explanatory message. */
  public ToriiTransactionCompatibilityException(final String message) {
    super(message);
  }

  /** Creates a compatibility failure with its underlying cause. */
  public ToriiTransactionCompatibilityException(
      final String message, final Throwable cause) {
    super(message, cause);
  }
}
