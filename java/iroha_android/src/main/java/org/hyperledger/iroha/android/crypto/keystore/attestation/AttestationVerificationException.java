package org.hyperledger.iroha.android.crypto.keystore.attestation;

/**
 * Raised when attestation material cannot be validated or does not match the caller's policy.
 */
public final class AttestationVerificationException extends Exception {

  private static final long serialVersionUID = 1L;

  public AttestationVerificationException(final String message) {
    super(message);
  }

  public AttestationVerificationException(final String message, final Throwable cause) {
    super(message, cause);
  }
}
