package org.hyperledger.iroha.android.client;

import java.util.Map;
import java.util.Objects;

/** Raised when a transaction fails to reach a terminal status within the configured bounds. */
public final class TransactionTimeoutException extends RuntimeException {

  private static final long serialVersionUID = 1L;

  private final String hashHex;
  private final int attempts;
  private final transient Map<String, Object> lastPayload;

  public TransactionTimeoutException(
      final String message,
      final String hashHex,
      final int attempts,
      final Map<String, Object> lastPayload) {
    super(message);
    this.hashHex = Objects.requireNonNull(hashHex, "hashHex");
    this.attempts = attempts;
    this.lastPayload = lastPayload;
  }

  public String hashHex() {
    return hashHex;
  }

  public int attempts() {
    return attempts;
  }

  public Map<String, Object> lastPayload() {
    return lastPayload;
  }
}
