package org.hyperledger.iroha.android.client;

import java.util.Map;

/** Canonical transaction-execution finality checks shared by high-level SDK facades. */
public final class TransactionFinality {

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
}
