package org.hyperledger.iroha.android.offline;

/** Raised when cached verdict metadata is missing or no longer valid. */
public final class OfflineVerdictException extends RuntimeException {

  private static final long serialVersionUID = 1L;

  private final Reason reason;
  private final String certificateIdHex;
  private final OfflineVerdictWarning.DeadlineKind deadlineKind;
  private final Long deadlineMs;
  private final String expectedNonceHex;
  private final String providedNonceHex;

  public OfflineVerdictException(
      final Reason reason,
      final String certificateIdHex,
      final OfflineVerdictWarning.DeadlineKind deadlineKind,
      final Long deadlineMs,
      final String expectedNonceHex,
      final String providedNonceHex,
      final String message) {
    super(message);
    this.reason = reason;
    this.certificateIdHex = certificateIdHex;
    this.deadlineKind = deadlineKind;
    this.deadlineMs = deadlineMs;
    this.expectedNonceHex = expectedNonceHex;
    this.providedNonceHex = providedNonceHex;
  }

  public Reason reason() {
    return reason;
  }

  public String certificateIdHex() {
    return certificateIdHex;
  }

  public OfflineVerdictWarning.DeadlineKind deadlineKind() {
    return deadlineKind;
  }

  public Long deadlineMs() {
    return deadlineMs;
  }

  public String expectedNonceHex() {
    return expectedNonceHex;
  }

  public String providedNonceHex() {
    return providedNonceHex;
  }

  public enum Reason {
    METADATA_MISSING,
    NONCE_MISMATCH,
    DEADLINE_EXPIRED
  }
}
