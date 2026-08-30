package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.fail;

import java.nio.ByteBuffer;
import java.util.Arrays;
import org.junit.Test;

/** Closed-model tests for routing hints, checkpoint persistence, and top-up agreement. */
public final class AuthenticatedFinalizedKagemushaOutcomeV1Tests {
  @Test
  public void checkpointProjectionIsExactlyFortyBytesAndDefensive() {
    final byte[] context = repeated((byte) 0x11);
    final AuthenticatedFinalityCheckpointV1 checkpoint =
        new AuthenticatedFinalityCheckpointV1(9L, context);
    context[0] ^= 0x7f;

    final byte[] projection = checkpoint.projectionBytes();
    assertEquals(40, projection.length);
    assertEquals(9L, ByteBuffer.wrap(projection).getLong());
    assertEquals((byte) 0x11, checkpoint.heightContextId()[0]);
    projection[8] ^= 0x7f;
    assertEquals((byte) 0x11, checkpoint.projectionBytes()[8]);
  }

  @Test
  public void finalizedContentAddressesRequireTheIrohaHashMarker() {
    expectIllegalArgument(
        () -> new AuthenticatedFinalityProofPageV1(new byte[] {0x01}, "22".repeat(32)));
    new AuthenticatedFinalityProofPageV1(new byte[] {0x01}, "23".repeat(32));
  }

  @Test
  public void routingHintsMustAgreeWithAuthenticatedTerminalResult() {
    final AuthenticatedFinalizedKagemushaOutcomeV1 applied =
        outcome("top_up", AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState.APPLIED, repeated((byte) 0x21));
    AuthenticatedTransactionDetailsNativeBridge.requireCarrierRoutingHintsAgreeV1(
        9L, true, applied);

    expectIllegalArgument(
        () -> AuthenticatedTransactionDetailsNativeBridge.requireCarrierRoutingHintsAgreeV1(
            8L, true, applied));
    expectIllegalArgument(
        () -> AuthenticatedTransactionDetailsNativeBridge.requireCarrierRoutingHintsAgreeV1(
            9L, false, applied));

    final AuthenticatedFinalizedKagemushaOutcomeV1 rejected =
        outcome("top_up", AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState.REJECTED, repeated((byte) 0x21));
    AuthenticatedTransactionDetailsNativeBridge.requireCarrierRoutingHintsAgreeV1(
        9L, false, rejected);
    expectIllegalArgument(
        () -> AuthenticatedTransactionDetailsNativeBridge.requireCarrierRoutingHintsAgreeV1(
            9L, true, rejected));
    expectIllegalArgument(
        () -> outcome(
            "top_up",
            AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState.REJECTED,
            repeated((byte) 0x21),
            "server_error"));
  }

  @Test
  public void specializedTopUpAgreementRejectsKindAndOperationSubstitution() {
    final byte[] operationId = repeated((byte) 0x21);
    final byte[] contextId = repeated((byte) 0x11);
    final AuthenticatedFinalizedKagemushaOutcomeV1 applied =
        outcome("top_up", AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState.APPLIED, operationId);
    AuthenticatedTransactionDetailsNativeBridge.requireKagemushaTopUpFinalityAgreementFieldsV1(
        applied, operationId, hash(0x11), 9L, hash(0x22), contextId);

    final byte[] anotherOperation = operationId.clone();
    anotherOperation[0] ^= 0x01;
    expectIllegalArgument(
        () -> AuthenticatedTransactionDetailsNativeBridge
            .requireKagemushaTopUpFinalityAgreementFieldsV1(
                applied, anotherOperation, hash(0x11), 9L, hash(0x22), contextId));
    expectIllegalArgument(
        () -> AuthenticatedTransactionDetailsNativeBridge
            .requireKagemushaTopUpFinalityAgreementFieldsV1(
                applied, operationId, hash(0x12), 9L, hash(0x22), contextId));
    expectIllegalArgument(
        () -> AuthenticatedTransactionDetailsNativeBridge
            .requireKagemushaTopUpFinalityAgreementFieldsV1(
                applied, operationId, hash(0x11), 8L, hash(0x22), contextId));
    expectIllegalArgument(
        () -> AuthenticatedTransactionDetailsNativeBridge
            .requireKagemushaTopUpFinalityAgreementFieldsV1(
                applied, operationId, hash(0x11), 9L, hash(0x23), contextId));
    final byte[] anotherContext = contextId.clone();
    anotherContext[0] ^= 0x01;
    expectIllegalArgument(
        () -> AuthenticatedTransactionDetailsNativeBridge
            .requireKagemushaTopUpFinalityAgreementFieldsV1(
                applied, operationId, hash(0x11), 9L, hash(0x22), anotherContext));

    final AuthenticatedFinalizedKagemushaOutcomeV1 redeem =
        outcome("redeem", AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState.APPLIED, operationId);
    expectIllegalArgument(
        () -> AuthenticatedTransactionDetailsNativeBridge
            .requireKagemushaTopUpFinalityAgreementFieldsV1(
                redeem, operationId, hash(0x11), 9L, hash(0x22), contextId));
  }

  private static AuthenticatedFinalizedKagemushaOutcomeV1 outcome(
      final String kind,
      final AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState state,
      final byte[] operationId) {
    return outcome(kind, state, operationId, "validation");
  }

  private static AuthenticatedFinalizedKagemushaOutcomeV1 outcome(
      final String kind,
      final AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState state,
      final byte[] operationId,
      final String rejectionCode) {
    final boolean rejected =
        state == AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState.REJECTED;
    return new AuthenticatedFinalizedKagemushaOutcomeV1(
        state,
        operationId,
        kind,
        hash(0x11),
        "wallet-query-authority",
        "issuer-transaction-authority",
        hash(0x22),
        hash(0x33),
        9L,
        new AuthenticatedFinalityCheckpointV1(9L, repeated((byte) 0x11)),
        hash(0x44),
        rejected ? rejectionCode : null,
        rejected ? "request rejected" : null,
        hash(0x55),
        hash(0x66),
        hash(0x77));
  }

  private static String hash(final int value) {
    final char[] output = new char[64];
    Arrays.fill(output, Character.forDigit(value & 0x0f, 16));
    output[output.length - 1] = Character.forDigit((value | 1) & 0x0f, 16);
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
      // Expected fail-closed model rejection.
    }
  }
}
