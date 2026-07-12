package org.hyperledger.iroha.android.client;

/** Runtime error raised when a confidential-asset Torii request fails. */
public final class ConfidentialAssetToriiException extends RuntimeException {
  private static final long serialVersionUID = 1L;

  private final Integer statusCode;
  private final String rejectCode;
  private final String responseBody;

  public ConfidentialAssetToriiException(final String message, final Throwable cause) {
    this(message, cause, null, null, null);
  }

  public ConfidentialAssetToriiException(
      final String message,
      final Integer statusCode,
      final String rejectCode,
      final String responseBody) {
    this(message, null, statusCode, rejectCode, responseBody);
  }

  public ConfidentialAssetToriiException(
      final String message,
      final Throwable cause,
      final Integer statusCode,
      final String rejectCode,
      final String responseBody) {
    super(message, cause);
    this.statusCode = statusCode;
    this.rejectCode = blankToNull(rejectCode);
    this.responseBody = blankToNull(responseBody);
  }

  public Integer getStatusCode() {
    return statusCode;
  }

  public String getRejectCode() {
    return rejectCode;
  }

  public String getResponseBody() {
    return responseBody;
  }

  private static String blankToNull(final String value) {
    return value == null || value.trim().isEmpty() ? null : value;
  }
}
