package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Raised when transaction status polling receives an unexpected HTTP status code. */
public final class TransactionStatusHttpException extends RuntimeException {

  private static final long serialVersionUID = 1L;

  private final String hashHex;
  private final int statusCode;
  private final String rejectCode;
  private final String responseBody;

  public TransactionStatusHttpException(
      final String hashHex,
      final int statusCode,
      final String rejectCode,
      final String responseBody) {
    super(buildMessage(hashHex, statusCode, rejectCode, responseBody));
    this.hashHex = Objects.requireNonNull(hashHex, "hashHex");
    this.statusCode = statusCode;
    this.rejectCode = rejectCode == null || rejectCode.isBlank() ? null : rejectCode.trim();
    this.responseBody = responseBody == null || responseBody.isBlank() ? null : responseBody;
  }

  public String hashHex() {
    return hashHex;
  }

  public int statusCode() {
    return statusCode;
  }

  public java.util.Optional<String> rejectCode() {
    return java.util.Optional.ofNullable(rejectCode);
  }

  public java.util.Optional<String> responseBody() {
    return java.util.Optional.ofNullable(responseBody);
  }

  private static String buildMessage(
      final String hashHex, final int statusCode, final String rejectCode, final String responseBody) {
    final StringBuilder message =
        new StringBuilder("Unexpected pipeline status code ")
            .append(statusCode)
            .append(" for transaction ")
            .append(hashHex);
    if (rejectCode != null && !rejectCode.isBlank()) {
      message.append(" (reject_code=").append(rejectCode.trim()).append(")");
    }
    if (responseBody != null && !responseBody.isBlank()) {
      message.append(". body=").append(responseBody);
    }
    return message.toString();
  }
}
