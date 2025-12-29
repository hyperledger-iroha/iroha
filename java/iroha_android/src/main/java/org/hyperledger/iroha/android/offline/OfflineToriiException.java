package org.hyperledger.iroha.android.offline;

/** Runtime error raised when offline Torii endpoints reject a request or parsing fails. */
public final class OfflineToriiException extends RuntimeException {

  private static final long serialVersionUID = 1L;

  public OfflineToriiException(final String message) {
    super(message);
  }

  public OfflineToriiException(final String message, final Throwable cause) {
    super(message, cause);
  }
}
