package org.hyperledger.iroha.android.model.instructions;

import org.hyperledger.iroha.android.model.InstructionBox;

/**
 * Produces an {@link InstructionBox}. Implementations capture the typed fields for each instruction
 * variant before converting to the argument-map representation used by local helpers. Transaction
 * encoding still requires wire-framed instruction payloads.
 */
public interface InstructionTemplate extends InstructionBox.InstructionPayload {

  default InstructionBox toInstructionBox() {
    return InstructionBox.of(this);
  }
}
