package org.hyperledger.iroha.android.client;

import java.util.Map;

/** Canonical transaction-execution finality checks shared by high-level SDK facades. */
public final class TransactionFinality {

  /** Closed authoritative execution-terminal states. */
  public enum TerminalState {
    APPLIED,
    REJECTED
  }

  private TransactionFinality() {}

  /**
   * Require the authoritative global, state-resolved {@code Applied} envelope for {@code hashHex}.
   *
   * @throws IllegalStateException if the hash, scope, resolution source, height, or status is not
   *     authoritative execution finality
   */
  public static void requireApplied(
      final Map<String, Object> payload, final String hashHex) {
    final String kind = PipelineStatusExtractor.requireAuthoritativeStatus(payload, hashHex);
    if (!"Applied".equals(kind)) {
      throw new IllegalStateException(
          "Transaction did not reach exact Applied execution finality");
    }
  }

  /**
   * Resolve an exact global, state-derived terminal without treating dispatch acceptance as final.
   *
   * @throws IllegalStateException for a pending, cache-only, hash-mismatched, or unknown state
   */
  public static TerminalState requireTerminal(
      final Map<String, Object> payload, final String hashHex) {
    final String kind = PipelineStatusExtractor.requireAuthoritativeStatus(payload, hashHex);
    if ("Applied".equals(kind)) {
      return TerminalState.APPLIED;
    }
    if ("Rejected".equals(kind)) {
      return TerminalState.REJECTED;
    }
    throw new IllegalStateException(
        "Transaction did not reach an exact authoritative execution terminal");
  }
}
