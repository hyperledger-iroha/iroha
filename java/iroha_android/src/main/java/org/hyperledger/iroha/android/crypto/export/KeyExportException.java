package org.hyperledger.iroha.android.crypto.export;

/** Raised when key export or import fails. */
public final class KeyExportException extends Exception {

  private static final long serialVersionUID = 1L;

  public KeyExportException(final String message) {
    super(message);
  }

  public KeyExportException(final String message, final Throwable cause) {
    super(message, cause);
  }
}
