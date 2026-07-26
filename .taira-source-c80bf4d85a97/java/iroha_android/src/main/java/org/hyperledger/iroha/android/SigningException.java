package org.hyperledger.iroha.android;

/** Raised when signing cannot complete (missing algorithm support, invalid key material, etc.). */
public final class SigningException extends Exception {

  private static final long serialVersionUID = 1L;

  public SigningException(final String message) {
    super(message);
  }

  public SigningException(final String message, final Throwable cause) {
    super(message, cause);
  }
}
