package org.hyperledger.iroha.android.connect;

/** Exception raised when Connect queue journals encounter IO or corruption errors. */
public final class ConnectJournalException extends Exception {
  private static final long serialVersionUID = 1L;

  public ConnectJournalException(final String message) {
    super(message);
  }

  public ConnectJournalException(final String message, final Throwable cause) {
    super(message, cause);
  }
}
