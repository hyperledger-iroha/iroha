package org.hyperledger.iroha.android.offline.attestation;

/** Runtime exception thrown when Safety Detect attestation fails. */
public final class SafetyDetectException extends RuntimeException {

  private static final long serialVersionUID = 1L;

  public SafetyDetectException(final String message) {
    super(message);
  }

  public SafetyDetectException(final String message, final Throwable cause) {
    super(message, cause);
  }
}
