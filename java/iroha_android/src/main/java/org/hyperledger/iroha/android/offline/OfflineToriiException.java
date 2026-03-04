package org.hyperledger.iroha.android.offline;

/** Runtime error raised when offline Torii endpoints reject a request or parsing fails. */
public final class OfflineToriiException extends RuntimeException {

  private static final long serialVersionUID = 1L;
  private final Integer statusCode;
  private final String rejectCode;
  private final String responseBody;

  public OfflineToriiException(final String message) {
    this(message, null, null, null, null);
  }

  public OfflineToriiException(final String message, final Throwable cause) {
    this(message, cause, null, null, null);
  }

  public OfflineToriiException(
      final String message,
      final Integer statusCode,
      final String rejectCode,
      final String responseBody) {
    this(message, null, statusCode, rejectCode, responseBody);
  }

  public OfflineToriiException(
      final String message,
      final Throwable cause,
      final Integer statusCode,
      final String rejectCode,
      final String responseBody) {
    super(message, cause);
    this.statusCode = statusCode;
    this.rejectCode = rejectCode == null || rejectCode.isBlank() ? null : rejectCode;
    this.responseBody = responseBody == null || responseBody.isBlank() ? null : responseBody;
  }

  public java.util.Optional<Integer> statusCode() {
    return java.util.Optional.ofNullable(statusCode);
  }

  public java.util.Optional<String> rejectCode() {
    return java.util.Optional.ofNullable(rejectCode);
  }

  public java.util.Optional<String> responseBody() {
    return java.util.Optional.ofNullable(responseBody);
  }
}
