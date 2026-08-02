package org.hyperledger.iroha.android.client;

/** Fail-closed transport or response validation failure from the issuance client. */
public final class BootleLanternIssuanceClientExceptionV1 extends RuntimeException {
  private final Integer statusCode;
  private final String code;
  private final Long retryAfterSeconds;

  /** Creates a validation failure. */
  public BootleLanternIssuanceClientExceptionV1(final String message) {
    super(message);
    this.statusCode = null;
    this.code = null;
    this.retryAfterSeconds = null;
  }

  /** Creates a canonical structured Torii issuance error. */
  public BootleLanternIssuanceClientExceptionV1(
      final String message,
      final int statusCode,
      final String code,
      final Long retryAfterSeconds) {
    super(message);
    this.statusCode = statusCode;
    this.code = code;
    this.retryAfterSeconds = retryAfterSeconds;
  }

  /** HTTP response status for a canonical structured error, otherwise {@code null}. */
  public Integer statusCode() {
    return statusCode;
  }

  /** Stable Torii issuance error code, otherwise {@code null}. */
  public String code() {
    return code;
  }

  /** Retry hint in seconds; present only for canonical HTTP 429. */
  public Long retryAfterSeconds() {
    return retryAfterSeconds;
  }
}
