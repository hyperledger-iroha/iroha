package org.hyperledger.iroha.android.model;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Objects;

/**
 * Represents the executable payload embedded in a transaction. Mirrors the Rust enum
 * `Executable::{Instructions, Ivm}`.
 */
public final class Executable {

  private enum Variant {
    INSTRUCTIONS,
    IVM
  }

  private final Variant variant;
  private final List<InstructionBox> instructions;
  private final byte[] ivmBytes;

  private Executable(final Variant variant, final List<InstructionBox> instructions, final byte[] ivmBytes) {
    this.variant = variant;
    this.instructions = instructions == null
        ? Collections.emptyList()
        : Collections.unmodifiableList(new ArrayList<>(instructions));
    this.ivmBytes = ivmBytes == null ? new byte[0] : Arrays.copyOf(ivmBytes, ivmBytes.length);
  }

  public static Executable instructions(final List<? extends InstructionBox> instructions) {
    Objects.requireNonNull(instructions, "instructions");
    return new Executable(Variant.INSTRUCTIONS, new ArrayList<>(instructions), null);
  }

  public static Executable ivm(final byte[] ivmBytes) {
    Objects.requireNonNull(ivmBytes, "ivmBytes");
    return new Executable(Variant.IVM, null, ivmBytes);
  }

  public boolean isInstructions() {
    return variant == Variant.INSTRUCTIONS;
  }

  public boolean isIvm() {
    return variant == Variant.IVM;
  }

  /** Returns the instruction list. Typed payloads are hydrated when bindings are available. */
  public List<InstructionBox> instructions() {
    if (!isInstructions()) {
      return List.of();
    }
    return instructions;
  }

  public byte[] ivmBytes() {
    if (!isIvm()) {
      throw new IllegalStateException("Executable does not contain IVM bytecode");
    }
    return Arrays.copyOf(ivmBytes, ivmBytes.length);
  }
}
