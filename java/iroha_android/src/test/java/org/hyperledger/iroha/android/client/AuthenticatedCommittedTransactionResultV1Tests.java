// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.math.BigInteger;
import org.junit.Test;

public final class AuthenticatedCommittedTransactionResultV1Tests {
  @Test
  public void successAndRejectionRemainMutuallyExclusive() {
    final AuthenticatedCommittedTransactionResultV1 success = fixture(true, null, BigInteger.TEN);
    assertTrue(success.resultOk());
    assertNull(success.rejectionMessage());

    final AuthenticatedCommittedTransactionResultV1 rejection =
        fixture(false, "policy epoch is stale", BigInteger.valueOf(11));
    assertFalse(rejection.resultOk());
    assertEquals("policy epoch is stale", rejection.rejectionMessage());

    assertThrows(
        IllegalArgumentException.class,
        () -> fixture(true, "contradiction", BigInteger.ONE));
    assertThrows(
        IllegalArgumentException.class,
        () -> fixture(false, null, BigInteger.ONE));
  }

  @Test
  public void hashesAndUnsignedCommittedHeightFailClosed() {
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new AuthenticatedCommittedTransactionResultV1(
                "AB".repeat(32),
                "canonical-authority",
                "cd".repeat(32),
                "ef".repeat(32),
                true,
                null,
                BigInteger.ONE));
    assertThrows(
        IllegalArgumentException.class,
        () -> fixture(true, null, BigInteger.ZERO));
    assertThrows(
        IllegalArgumentException.class,
        () -> fixture(true, null, BigInteger.ONE.shiftLeft(64)));
    assertThrows(
        IllegalArgumentException.class,
        () -> fixture(false, " padded ", BigInteger.ONE));
    assertThrows(
        IllegalArgumentException.class,
        () -> fixture(false, "policy\u0001rejected", BigInteger.ONE));
    assertThrows(
        IllegalArgumentException.class,
        () -> fixture(false, "é".repeat(513), BigInteger.ONE));
  }

  private static AuthenticatedCommittedTransactionResultV1 fixture(
      final boolean resultOk,
      final String rejectionMessage,
      final BigInteger committedBlockHeight) {
    return new AuthenticatedCommittedTransactionResultV1(
        "ab".repeat(32),
        "canonical-authority",
        "cd".repeat(32),
        "ef".repeat(32),
        resultOk,
        rejectionMessage,
        committedBlockHeight);
  }
}
