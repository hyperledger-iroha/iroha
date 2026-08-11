package org.hyperledger.iroha.android.client;

import java.util.Map;
import java.util.Objects;
import java.util.OptionalInt;
import java.util.concurrent.CompletableFuture;

/**
 * Raised after exactly one signed-transaction dispatch was attempted but no authoritative
 * admission outcome was obtained.
 *
 * <p>The exact signed bytes were not retried or queued by the SDK. Call {@link #reconcileWith} (or
 * query the same {@link #hashHex()} through another trusted client) before deciding whether to
 * construct and sign a replacement transaction.
 */
public final class AmbiguousTransactionSubmissionException extends RuntimeException {

  private static final long serialVersionUID = 1L;

  private final String hashHex;
  private final Integer statusCode;

  AmbiguousTransactionSubmissionException(
      final String hashHex, final Integer statusCode, final Throwable cause) {
    super(buildMessage(hashHex, statusCode), cause);
    this.hashHex = Objects.requireNonNull(hashHex, "hashHex");
    this.statusCode = statusCode;
  }

  /** Returns the canonical transaction hash that must be reconciled. */
  public String hashHex() {
    return hashHex;
  }

  /** Returns the ambiguous HTTP status when a response was received. */
  public OptionalInt statusCode() {
    return statusCode == null ? OptionalInt.empty() : OptionalInt.of(statusCode.intValue());
  }

  /** Polls the authoritative pipeline status for the exact transaction hash. */
  public CompletableFuture<Map<String, Object>> reconcileWith(final IrohaClient client) {
    return reconcileWith(client, null);
  }

  /** Polls the authoritative pipeline status with explicit polling bounds. */
  public CompletableFuture<Map<String, Object>> reconcileWith(
      final IrohaClient client, final PipelineStatusOptions options) {
    return Objects.requireNonNull(client, "client").waitForTransactionStatus(hashHex, options);
  }

  private static String buildMessage(final String hashHex, final Integer statusCode) {
    final StringBuilder message =
        new StringBuilder("Transaction ")
            .append(hashHex)
            .append(" had exactly one dispatch attempt, but its admission outcome is unknown");
    if (statusCode != null) {
      message.append(" after HTTP status ").append(statusCode.intValue());
    }
    return message
        .append(". Do not resend the signed bytes; reconcile by transaction hash.")
        .toString();
  }
}
