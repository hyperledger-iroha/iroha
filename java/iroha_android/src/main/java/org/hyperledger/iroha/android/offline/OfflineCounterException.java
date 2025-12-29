package org.hyperledger.iroha.android.offline;

/** Runtime exception thrown when offline counter validation fails. */
public final class OfflineCounterException extends RuntimeException {

  private static final long serialVersionUID = 1L;

  public enum Reason {
    SUMMARY_HASH_MISMATCH,
    COUNTER_VIOLATION,
    INVALID_SCOPE
  }

  private final Reason reason;

  public OfflineCounterException(final Reason reason, final String message) {
    super(message);
    this.reason = reason;
  }

  public Reason reason() {
    return reason;
  }
}
