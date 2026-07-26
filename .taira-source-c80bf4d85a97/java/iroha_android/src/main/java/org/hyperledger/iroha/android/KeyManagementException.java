package org.hyperledger.iroha.android;

/**
 * Raised when a key provider cannot complete a requested operation.
 *
 * <p>This exception is surfaced to callers so they can decide whether to retry with a different
 * security preference or surface a user prompt. Providers should include actionable messages.
 */
public final class KeyManagementException extends Exception {

  private static final long serialVersionUID = 1L;

  public KeyManagementException(final String message) {
    super(message);
  }

  public KeyManagementException(final String message, final Throwable cause) {
    super(message, cause);
  }
}
