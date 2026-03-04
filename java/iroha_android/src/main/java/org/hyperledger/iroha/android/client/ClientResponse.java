package org.hyperledger.iroha.android.client;

import java.util.Arrays;
import java.util.Objects;

/** Simple container for responses returned by {@link IrohaClient} implementations. */
public final class ClientResponse {
  private final int statusCode;
  private final byte[] body;
  private final String message;
  private final String transactionHashHex;
  private final String rejectCode;

  public ClientResponse(final int statusCode, final byte[] body) {
    this(statusCode, body, "", null, null);
  }

  public ClientResponse(final int statusCode, final byte[] body, final String message) {
    this(statusCode, body, message, null, null);
  }

  public ClientResponse(
      final int statusCode,
      final byte[] body,
      final String message,
      final String transactionHashHex) {
    this(statusCode, body, message, transactionHashHex, null);
  }

  public ClientResponse(
      final int statusCode,
      final byte[] body,
      final String message,
      final String transactionHashHex,
      final String rejectCode) {
    this.statusCode = statusCode;
    this.body = Arrays.copyOf(Objects.requireNonNull(body, "body"), body.length);
    this.message = Objects.requireNonNull(message, "message");
    this.transactionHashHex = transactionHashHex == null ? null : transactionHashHex;
    this.rejectCode = rejectCode == null || rejectCode.isBlank() ? null : rejectCode;
  }

  public int statusCode() {
    return statusCode;
  }

  public byte[] body() {
    return Arrays.copyOf(body, body.length);
  }

  public String message() {
    return message;
  }

  public java.util.Optional<String> hashHex() {
    return java.util.Optional.ofNullable(transactionHashHex);
  }

  public java.util.Optional<String> rejectCode() {
    return java.util.Optional.ofNullable(rejectCode);
  }
}
