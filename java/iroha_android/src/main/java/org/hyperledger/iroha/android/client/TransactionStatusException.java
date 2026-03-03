package org.hyperledger.iroha.android.client;

import java.util.Map;
import java.util.Objects;

/** Raised when a transaction reports a terminal failure status. */
public final class TransactionStatusException extends RuntimeException {

  private static final long serialVersionUID = 1L;

  private final String hashHex;
  private final String status;
  private final String rejectionReason;
  private final transient Map<String, Object> payload;

  public TransactionStatusException(
      final String hashHex, final String status, final Map<String, Object> payload) {
    this(hashHex, status, null, payload);
  }

  public TransactionStatusException(
      final String hashHex,
      final String status,
      final String rejectionReason,
      final Map<String, Object> payload) {
    super(buildMessage(hashHex, status, rejectionReason));
    this.hashHex = Objects.requireNonNull(hashHex, "hashHex");
    this.status = Objects.requireNonNull(status, "status");
    this.rejectionReason =
        rejectionReason == null || rejectionReason.isBlank() ? null : rejectionReason.trim();
    this.payload = payload;
  }

  public String hashHex() {
    return hashHex;
  }

  public String status() {
    return status;
  }

  public java.util.Optional<String> rejectionReason() {
    return java.util.Optional.ofNullable(rejectionReason);
  }

  public Map<String, Object> payload() {
    return payload;
  }

  private static String buildMessage(
      final String hashHex, final String status, final String rejectionReason) {
    final StringBuilder message =
        new StringBuilder("Transaction ")
            .append(hashHex)
            .append(" reported failure status ")
            .append(status);
    if (rejectionReason != null && !rejectionReason.isBlank()) {
      message.append(" (reason=").append(rejectionReason.trim()).append(")");
    }
    return message.toString();
  }
}
