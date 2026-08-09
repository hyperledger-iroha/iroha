package org.hyperledger.iroha.android.model;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertNotEquals;
import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import org.junit.Test;

public final class ExecutableTests {

  private static final String CONTRACT_ADDRESS =
      "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh";

  @Test
  public void contractInvocationDefensivelyCopiesHashAndArguments() {
    final byte[] hash = repeatedByte(0x31, 32);
    final byte[] arguments = new byte[] {0x01, 0x02, 0x03};
    final ContractInvocation invocation =
        new ContractInvocation(CONTRACT_ADDRESS, hash, "run", arguments);

    hash[0] = 0x55;
    arguments[0] = 0x66;
    assertEquals(0x31, invocation.expectedCodeHash()[0] & 0xFF);
    assertEquals(0x01, invocation.arguments()[0] & 0xFF);

    final byte[] returnedHash = invocation.expectedCodeHash();
    final byte[] returnedArguments = invocation.arguments();
    returnedHash[1] = 0x77;
    returnedArguments[1] = 0x77;
    assertEquals(0x31, invocation.expectedCodeHash()[1] & 0xFF);
    assertEquals(0x02, invocation.arguments()[1] & 0xFF);

    assertEquals(
        invocation,
        new ContractInvocation(
            CONTRACT_ADDRESS, repeatedByte(0x31, 32), "run", new byte[] {1, 2, 3}));
    assertNotEquals(
        invocation,
        new ContractInvocation(CONTRACT_ADDRESS, repeatedByte(0x31, 32), "other", null));
  }

  @Test
  public void contractInvocationValidatesBoundedWireFields() {
    assertIllegalArgument(
        () -> new ContractInvocation(CONTRACT_ADDRESS, new byte[31], "run", null),
        "expectedCodeHash must contain exactly 32 bytes");
    assertIllegalArgument(
        () -> new ContractInvocation(CONTRACT_ADDRESS, new byte[32], "run", null),
        "expectedCodeHash must use Iroha's marked hash encoding");
    assertIllegalArgument(
        () ->
            new ContractInvocation(
                CONTRACT_ADDRESS,
                repeatedByte(0x01, 32),
                "run",
                new byte[ContractInvocation.MAX_ARGUMENT_BYTES + 1]),
        "arguments must not exceed");
    assertIllegalArgument(
        () ->
            new ContractInvocation(
                " " + CONTRACT_ADDRESS, repeatedByte(0x01, 32), "run", null),
        "contractAddress must not contain surrounding whitespace");
    assertIllegalArgument(
        () ->
            new ContractInvocation(
                "tairac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqddcyq8",
                repeatedByte(0x01, 32),
                "run",
                null),
        "contractAddress must use the canonical irohac prefix");
    assertIllegalArgument(
        () ->
            new ContractInvocation(CONTRACT_ADDRESS, repeatedByte(0x01, 32), " ", null),
        "entrypoint must not be blank");
  }

  @Test
  public void batchPreservesOrderAndCopiesItsInput() {
    final InstructionBox first = wireInstruction("iroha.batch.first", "first");
    final InstructionBox last = wireInstruction("iroha.batch.last", "last");
    final ContractInvocation invocation = invocation();
    final List<ExecutableBatchItem> source = new ArrayList<>();
    source.add(ExecutableBatchItem.instruction(first));
    source.add(ExecutableBatchItem.contractCall(invocation));
    source.add(ExecutableBatchItem.instruction(last));

    final Executable executable = Executable.batch(source);
    source.clear();

    assertTrue(executable.isBatch());
    assertEquals(3, executable.batchItems().size());
    assertEquals(first, executable.batchItems().get(0).instruction());
    assertEquals(invocation, executable.batchItems().get(1).contractInvocation());
    assertEquals(last, executable.batchItems().get(2).instruction());
    assertTrue(executable.requiresTransactionGasLimit());
    try {
      executable.batchItems().add(ExecutableBatchItem.instruction(first));
      fail("Batch items must be immutable");
    } catch (final UnsupportedOperationException expected) {
      // Expected immutable list behavior.
    }
  }

  @Test
  public void executableGasRequirementMatchesItsVariant() {
    final InstructionBox instruction = wireInstruction("iroha.batch.only", "only");
    assertFalse(Executable.instructions(Collections.singletonList(instruction))
        .requiresTransactionGasLimit());
    assertFalse(Executable.batch(
            Collections.singletonList(ExecutableBatchItem.instruction(instruction)))
        .requiresTransactionGasLimit());
    assertTrue(Executable.contractCall(invocation()).requiresTransactionGasLimit());
    assertTrue(Executable.ivm(new byte[] {1}).requiresTransactionGasLimit());
  }

  @Test
  public void emptyBatchesAreRejectedBeforeSigning() {
    assertIllegalArgument(
        () -> Executable.batch(Collections.emptyList()),
        "executable batch must contain at least one item");
  }

  @Test
  public void contractInvocationAcceptsOnlyCanonicalV1Bech32mAddresses() {
    invocation();
    final String[] invalidAddresses = {
      "abc",
      " " + CONTRACT_ADDRESS,
      CONTRACT_ADDRESS.toUpperCase(java.util.Locale.ROOT),
      CONTRACT_ADDRESS.substring(0, CONTRACT_ADDRESS.length() - 1) + "q",
      "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjqc3gg99",
      "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjpvkdn59",
      "irohac1qyqqqqqqqqqqqqpupm8207",
      "irohac1qgqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3lquf7"
    };
    for (final String invalidAddress : invalidAddresses) {
      assertIllegalArgument(
          () ->
              new ContractInvocation(
                  invalidAddress, repeatedByte(0x01, 32), "run", null),
          "contractAddress");
    }
  }

  @Test
  public void batchItemAccessorsRejectTheWrongVariant() {
    final ExecutableBatchItem instruction =
        ExecutableBatchItem.instruction(wireInstruction("iroha.batch.item", "item"));
    final ExecutableBatchItem contractCall = ExecutableBatchItem.contractCall(invocation());

    assertTrue(instruction.isInstruction());
    assertFalse(instruction.isContractCall());
    assertTrue(contractCall.isContractCall());
    assertFalse(contractCall.isInstruction());
    assertIllegalState(instruction::contractInvocation);
    assertIllegalState(contractCall::instruction);
  }

  private static ContractInvocation invocation() {
    return new ContractInvocation(
        CONTRACT_ADDRESS, repeatedByte(0x21, 32), "run", new byte[] {0x4B, 0x4F});
  }

  private static InstructionBox wireInstruction(final String name, final String payload) {
    return InstructionBox.fromWirePayload(name, payload.getBytes(StandardCharsets.UTF_8));
  }

  private static byte[] repeatedByte(final int value, final int length) {
    final byte[] bytes = new byte[length];
    Arrays.fill(bytes, (byte) value);
    return bytes;
  }

  private static void assertIllegalArgument(final Runnable action, final String messageFragment) {
    try {
      action.run();
      fail("Expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      assertTrue(expected.getMessage().contains(messageFragment));
    }
  }

  private static void assertIllegalState(final Runnable action) {
    try {
      action.run();
      fail("Expected IllegalStateException");
    } catch (final IllegalStateException expected) {
      // Expected wrong-variant access failure.
    }
  }
}
