package org.hyperledger.iroha.android.model;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Objects;

/** Represents the executable payload embedded in a transaction. */
public final class Executable {

  private enum Variant {
    INSTRUCTIONS,
    CONTRACT_CALL,
    BATCH,
    IVM
  }

  private final Variant variant;
  private final List<InstructionBox> instructions;
  private final ContractInvocation contractInvocation;
  private final List<ExecutableBatchItem> batchItems;
  private final byte[] ivmBytes;

  private Executable(
      final Variant variant,
      final List<InstructionBox> instructions,
      final ContractInvocation contractInvocation,
      final List<ExecutableBatchItem> batchItems,
      final byte[] ivmBytes) {
    this.variant = variant;
    this.instructions = immutableCopy(instructions, "instruction");
    this.contractInvocation = contractInvocation;
    this.batchItems = immutableCopy(batchItems, "batchItem");
    this.ivmBytes = ivmBytes == null ? new byte[0] : Arrays.copyOf(ivmBytes, ivmBytes.length);
  }

  public static Executable instructions(final List<? extends InstructionBox> instructions) {
    Objects.requireNonNull(instructions, "instructions");
    return new Executable(
        Variant.INSTRUCTIONS, new ArrayList<>(instructions), null, null, null);
  }

  public static Executable contractCall(final ContractInvocation invocation) {
    return new Executable(
        Variant.CONTRACT_CALL,
        null,
        Objects.requireNonNull(invocation, "contractInvocation"),
        null,
        null);
  }

  /** Creates an ordered, flat mix of instructions and deployed-contract calls. */
  public static Executable batch(final List<? extends ExecutableBatchItem> items) {
    Objects.requireNonNull(items, "items");
    if (items.isEmpty()) {
      throw new IllegalArgumentException("executable batch must contain at least one item");
    }
    return new Executable(Variant.BATCH, null, null, new ArrayList<>(items), null);
  }

  public static Executable ivm(final byte[] ivmBytes) {
    Objects.requireNonNull(ivmBytes, "ivmBytes");
    return new Executable(Variant.IVM, null, null, null, ivmBytes);
  }

  public boolean isInstructions() {
    return variant == Variant.INSTRUCTIONS;
  }

  public boolean isIvm() {
    return variant == Variant.IVM;
  }

  public boolean isContractCall() {
    return variant == Variant.CONTRACT_CALL;
  }

  public boolean isBatch() {
    return variant == Variant.BATCH;
  }

  /** Returns the instruction list. Typed payloads are hydrated when bindings are available. */
  public List<InstructionBox> instructions() {
    if (!isInstructions()) {
      return Collections.emptyList();
    }
    return instructions;
  }

  public ContractInvocation contractInvocation() {
    if (!isContractCall()) {
      throw new IllegalStateException("Executable does not contain a contract call");
    }
    return contractInvocation;
  }

  /** Returns the ordered mixed-batch items, or an empty list for another variant. */
  public List<ExecutableBatchItem> batchItems() {
    if (!isBatch()) {
      return Collections.emptyList();
    }
    return batchItems;
  }

  public byte[] ivmBytes() {
    if (!isIvm()) {
      throw new IllegalStateException("Executable does not contain IVM bytecode");
    }
    return Arrays.copyOf(ivmBytes, ivmBytes.length);
  }

  /** Whether this executable requires a signature-bound transaction gas limit. */
  public boolean requiresTransactionGasLimit() {
    if (isContractCall() || isIvm()) {
      return true;
    }
    if (isBatch()) {
      for (final ExecutableBatchItem item : batchItems) {
        if (item.isContractCall()) {
          return true;
        }
      }
    }
    return false;
  }

  @Override
  public boolean equals(final Object other) {
    if (this == other) {
      return true;
    }
    if (!(other instanceof Executable)) {
      return false;
    }
    final Executable that = (Executable) other;
    return variant == that.variant
        && instructions.equals(that.instructions)
        && Objects.equals(contractInvocation, that.contractInvocation)
        && batchItems.equals(that.batchItems)
        && Arrays.equals(ivmBytes, that.ivmBytes);
  }

  @Override
  public int hashCode() {
    int result = Objects.hash(variant, instructions, contractInvocation, batchItems);
    result = 31 * result + Arrays.hashCode(ivmBytes);
    return result;
  }

  private static <T> List<T> immutableCopy(final List<? extends T> values, final String field) {
    if (values == null) {
      return Collections.emptyList();
    }
    final ArrayList<T> copy = new ArrayList<>(values.size());
    for (final T value : values) {
      copy.add(Objects.requireNonNull(value, field));
    }
    return Collections.unmodifiableList(copy);
  }
}
