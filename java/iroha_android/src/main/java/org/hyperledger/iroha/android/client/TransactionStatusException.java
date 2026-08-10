package org.hyperledger.iroha.android.client;

import java.util.Map;
import java.util.Objects;

/** Raised when a transaction reports a terminal failure status. */
public final class TransactionStatusException extends RuntimeException {

  private static final long serialVersionUID = 1L;

  private final String hashHex;
  private final String status;
  private final transient Map<String, Object> payload;

  public TransactionStatusException(
      final String hashHex, final String status, final Map<String, Object> payload) {
    super("Transaction " + hashHex + " reported failure status " + status);
    this.hashHex = Objects.requireNonNull(hashHex, "hashHex");
    this.status = Objects.requireNonNull(status, "status");
    this.payload = payload;
  }

  public String hashHex() {
    return hashHex;
  }

  public String status() {
    return status;
  }

  public Map<String, Object> payload() {
    return payload;
  }
}
