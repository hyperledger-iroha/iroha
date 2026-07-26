package org.hyperledger.iroha.android.norito;

/** Raised when Norito encoding or decoding fails within the Android bindings. */
public final class NoritoException extends Exception {

  private static final long serialVersionUID = 1L;

  public NoritoException(final String message) {
    super(message);
  }

  public NoritoException(final String message, final Throwable cause) {
    super(message, cause);
  }
}
