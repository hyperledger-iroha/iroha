// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;

import java.util.Map;
import org.junit.Test;

public final class TransactionFinalityTests {
  private static final String HASH = "ab".repeat(32);

  @Test
  public void exactStateResolvedAppliedAndRejectedAreTheOnlyTerminals() {
    assertEquals(
        TransactionFinality.TerminalState.APPLIED,
        TransactionFinality.requireTerminal(status("Applied", "global", "state", 17L), HASH));
    assertEquals(
        TransactionFinality.TerminalState.REJECTED,
        TransactionFinality.requireTerminal(status("Rejected", "global", "state", null), HASH));

    assertThrows(
        IllegalStateException.class,
        () -> TransactionFinality.requireTerminal(status("Committed", "global", "state", 17L), HASH));
    assertThrows(
        IllegalStateException.class,
        () -> TransactionFinality.requireTerminal(status("Rejected", "global", "cache", null), HASH));
    assertThrows(
        IllegalStateException.class,
        () -> TransactionFinality.requireTerminal(status("Rejected", "local", "state", null), HASH));
    assertThrows(
        IllegalStateException.class,
        () -> TransactionFinality.requireTerminal(status("Rejected", "global", "state", null), "cd".repeat(32)));
  }

  private static Map<String, Object> status(
      final String kind,
      final String scope,
      final String resolvedFrom,
      final Long blockHeight) {
    final Map<String, Object> status =
        blockHeight == null
            ? Map.of("kind", kind)
            : Map.of("kind", kind, "block_height", blockHeight);
    return Map.of(
        "hash", HASH,
        "status", status,
        "scope", scope,
        "resolved_from", resolvedFrom);
  }
}
