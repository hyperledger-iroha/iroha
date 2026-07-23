package org.hyperledger.iroha.android.model;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;

import java.util.Arrays;
import java.util.Collections;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.junit.Test;

/** Admission checks performed while authoring transaction payloads. */
public final class TransactionPayloadTests {
  private static final String CONTRACT_ADDRESS =
      "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8";

  @Test
  public void vmAndContractExecutablesRequireGasBeforeSigning() {
    final ContractInvocation invocation =
        new ContractInvocation(CONTRACT_ADDRESS, repeatedByte(0x01, 32), "run", null);
    final Executable[] executables = {
      Executable.ivm(new byte[] {1}),
      Executable.contractCall(invocation),
      Executable.batch(
          Collections.singletonList(ExecutableBatchItem.contractCall(invocation)))
    };

    for (final Executable executable : executables) {
      assertIllegalState(
          () ->
              TransactionPayload.builder()
                  .setExecutable(executable)
                  .setFeePayment(FeePaymentIntent.authority(Collections.emptyList()))
                  .build(),
          "feePayment.gasLimit is required");
      assertNotNull(
          TransactionPayload.builder()
              .setExecutable(executable)
              .setFeePayment(FeePaymentIntent.authority(Collections.emptyList(), 1L))
              .build());
    }
  }

  @Test
  public void nativeInstructionsDoNotRequireGas() {
    assertNotNull(
        TransactionPayload.builder()
            .setInstructions(
                Collections.singletonList(
                    InstructionBox.fromWirePayload("iroha.test", new byte[] {1})))
            .setFeePayment(FeePaymentIntent.authority(Collections.emptyList()))
            .build());
  }

  @Test
  public void nonceSupportsTheFullNonzeroU32Range() throws NoritoException {
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setInstructions(Collections.emptyList())
            .setFeePayment(FeePaymentIntent.authority(Collections.emptyList()))
            .setNonce(0xffff_ffffL)
            .build();
    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final TransactionPayload decoded =
        adapter.decodeTransaction(adapter.encodeTransaction(payload));

    assertEquals(Long.valueOf(0xffff_ffffL), decoded.nonce().orElse(null));
    assertIllegalArgument(() -> TransactionPayload.builder().setNonce(0L));
    assertIllegalArgument(() -> TransactionPayload.builder().setNonce(0x1_0000_0000L));
  }

  private static byte[] repeatedByte(final int value, final int length) {
    final byte[] bytes = new byte[length];
    Arrays.fill(bytes, (byte) value);
    return bytes;
  }

  private static void assertIllegalState(final Runnable action, final String messageFragment) {
    try {
      action.run();
      fail("Expected IllegalStateException");
    } catch (final IllegalStateException expected) {
      assertTrue(expected.getMessage().contains(messageFragment));
    }
  }

  private static void assertIllegalArgument(final Runnable action) {
    try {
      action.run();
      fail("Expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      assertTrue(expected.getMessage().contains("nonzero u32"));
    }
  }
}
