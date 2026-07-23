package org.hyperledger.iroha.android.model;

import java.util.Objects;

/** One ordered native-instruction or deployed-contract-call item in an executable batch. */
public final class ExecutableBatchItem {

  private enum Variant {
    INSTRUCTION,
    CONTRACT_CALL
  }

  private final Variant variant;
  private final InstructionBox instruction;
  private final ContractInvocation contractInvocation;

  private ExecutableBatchItem(
      final Variant variant,
      final InstructionBox instruction,
      final ContractInvocation contractInvocation) {
    this.variant = Objects.requireNonNull(variant, "variant");
    this.instruction = instruction;
    this.contractInvocation = contractInvocation;
  }

  /** Wraps one native Iroha Special Instruction. */
  public static ExecutableBatchItem instruction(final InstructionBox instruction) {
    return new ExecutableBatchItem(
        Variant.INSTRUCTION, Objects.requireNonNull(instruction, "instruction"), null);
  }

  /** Wraps one deployed-contract invocation. */
  public static ExecutableBatchItem contractCall(final ContractInvocation invocation) {
    return new ExecutableBatchItem(
        Variant.CONTRACT_CALL,
        null,
        Objects.requireNonNull(invocation, "contractInvocation"));
  }

  public boolean isInstruction() {
    return variant == Variant.INSTRUCTION;
  }

  public boolean isContractCall() {
    return variant == Variant.CONTRACT_CALL;
  }

  public InstructionBox instruction() {
    if (!isInstruction()) {
      throw new IllegalStateException("Batch item does not contain an instruction");
    }
    return instruction;
  }

  public ContractInvocation contractInvocation() {
    if (!isContractCall()) {
      throw new IllegalStateException("Batch item does not contain a contract call");
    }
    return contractInvocation;
  }

  @Override
  public boolean equals(final Object other) {
    if (this == other) {
      return true;
    }
    if (!(other instanceof ExecutableBatchItem)) {
      return false;
    }
    final ExecutableBatchItem that = (ExecutableBatchItem) other;
    return variant == that.variant
        && Objects.equals(instruction, that.instruction)
        && Objects.equals(contractInvocation, that.contractInvocation);
  }

  @Override
  public int hashCode() {
    return Objects.hash(variant, instruction, contractInvocation);
  }
}
