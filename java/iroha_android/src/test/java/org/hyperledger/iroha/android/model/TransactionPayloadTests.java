package org.hyperledger.iroha.android.model;

import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;

import java.util.Arrays;
import java.util.Collections;
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
}
