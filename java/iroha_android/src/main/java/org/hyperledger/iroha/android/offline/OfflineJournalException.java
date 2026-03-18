package org.hyperledger.iroha.android.offline;

/** Checked error raised by {@link OfflineJournal} operations. */
public final class OfflineJournalException extends Exception {

  private static final long serialVersionUID = 1L;

  /** Enumerates the failure categories surfaced by the journal. */
  public enum Reason {
    INVALID_KEY_LENGTH,
    INVALID_TX_ID_LENGTH,
    DUPLICATE_PENDING,
    ALREADY_COMMITTED,
    NOT_PENDING,
    IO,
    INTEGRITY_VIOLATION,
    PAYLOAD_TOO_LARGE
  }

  private final Reason reason;

  OfflineJournalException(final Reason reason, final String message) {
    super(message);
    this.reason = reason;
  }

  OfflineJournalException(final Reason reason, final String message, final Throwable cause) {
    super(message, cause);
    this.reason = reason;
  }

  public Reason reason() {
    return reason;
  }
}
