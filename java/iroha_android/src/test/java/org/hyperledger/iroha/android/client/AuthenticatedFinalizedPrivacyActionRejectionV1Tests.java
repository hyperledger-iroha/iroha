// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.fail;

import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import org.hyperledger.iroha.sdk.privacy.PrivacyLedgerEffectKindV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyOperationSchemaV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyProtocolIdV1;
import org.junit.Test;

/** Closed-model and public-surface tests for finalized Exact12 rejection evidence. */
public final class AuthenticatedFinalizedPrivacyActionRejectionV1Tests {
  @Test
  public void closedSixCaseModelPreservesTypedActionAndDefensiveDigests() {
    assertEquals(
        Arrays.asList(
            "account_does_not_exist",
            "limit_check",
            "validation",
            "instruction_execution",
            "ivm_execution",
            "trigger_execution"),
        Arrays.stream(AuthenticatedPrivacyActionRejectionCodeV1.values())
            .map(AuthenticatedPrivacyActionRejectionCodeV1::canonicalLabel)
            .collect(java.util.stream.Collectors.toList()));
    for (final AuthenticatedPrivacyActionRejectionCodeV1 code
        : AuthenticatedPrivacyActionRejectionCodeV1.values()) {
      final byte[] source = repeated((byte) 0x22);
      final AuthenticatedFinalizedPrivacyActionRejectionV1 rejection =
          rejection(code, source, 9L);
      source[0] ^= 0x7f;
      assertEquals(code.canonicalLabel(), rejection.rejectionCode().canonicalLabel());
      assertEquals(
          PrivacyOperationSchemaV1.ZK_ACE_AUTHORIZATION_ACTION_V1,
          rejection.operationSchema());
      assertEquals(9L, rejection.committedBlockHeight());
      assertArrayEquals(repeated((byte) 0x22), rejection.transactionIntentDigest());
      final byte[] escaped = rejection.transactionIntentDigest();
      escaped[0] ^= 0x7f;
      assertArrayEquals(repeated((byte) 0x22), rejection.transactionIntentDigest());
    }
  }

  @Test
  public void unknownCodesAndContradictoryFinalityFailClosed() {
    expectIllegalArgument(
        () -> AuthenticatedPrivacyActionRejectionCodeV1.fromCanonicalLabel("server_error"));
    expectIllegalArgument(
        () -> rejection(
            AuthenticatedPrivacyActionRejectionCodeV1.VALIDATION,
            repeated((byte) 0x22),
            8L));
  }

  @Test
  public void publicBridgeExposesPageAndProofArrayOverloads() {
    final List<Method> methods = new ArrayList<>();
    for (final Method method : AuthenticatedPrivacyActionReceiptNativeBridge.class.getMethods()) {
      if ("projectFinalizedPrivacyActionRejectionV1".equals(method.getName())) {
        methods.add(method);
      }
    }
    assertEquals(2, methods.size());
    for (final Method method : methods) assertEquals(6, method.getParameterCount());
  }

  private static AuthenticatedFinalizedPrivacyActionRejectionV1 rejection(
      final AuthenticatedPrivacyActionRejectionCodeV1 code,
      final byte[] intent,
      final long checkpointHeight) {
    return new AuthenticatedFinalizedPrivacyActionRejectionV1(
        hash(0x11),
        PrivacyProtocolIdV1.ZK_ACE_PQ_AUTHORIZATION_V0,
        PrivacyOperationSchemaV1.ZK_ACE_AUTHORIZATION_ACTION_V1,
        PrivacyLedgerEffectKindV1.ZK_ACE_TRANSPARENT_TRANSFER,
        hash(0x21),
        0,
        intent,
        repeated((byte) 0x24),
        repeated((byte) 0x26),
        "wallet-query-authority",
        "exact12-transaction-authority",
        hash(0x31),
        hash(0x41),
        code,
        "Exact12 validation rejected the action",
        9L,
        new AuthenticatedFinalityCheckpointV1(
            checkpointHeight,
            repeated((byte) 0x11)),
        hash(0x51),
        hash(0x61),
        hash(0x71),
        hash(0x81));
  }

  private static String hash(final int value) {
    final char[] output = new char[64];
    Arrays.fill(output, Character.forDigit((value | 1) & 0x0f, 16));
    return new String(output);
  }

  private static byte[] repeated(final byte value) {
    final byte[] output = new byte[32];
    Arrays.fill(output, value);
    return output;
  }

  private static void expectIllegalArgument(final Runnable action) {
    try {
      action.run();
      fail("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // Expected fail-closed validation.
    }
  }
}
