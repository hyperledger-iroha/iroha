package org.hyperledger.iroha.sdk.tx.norito;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;

import java.util.Collections;
import org.hyperledger.iroha.sdk.core.model.InstructionBox;
import org.junit.jupiter.api.Test;

/** Java consumers use the Kotlin-owned encoder and shared Rust multisig hash fixture. */
class InstructionBatchHashJavaConsumerTest {
  @Test
  void canonicalInstructionHashMatchesRust() throws Exception {
    InstructionBox instruction = InstructionBox.fromWirePayload(
        "iroha.custom", InstructionBatchHashFixture.bytes("custom_instruction_frame_hex"));
    byte[] encoded = NoritoJavaCodecAdapter.encodeInstructionBox(instruction);
    assertArrayEquals(encoded,
        NoritoJavaCodecAdapter.encodeInstructionBox(NoritoJavaCodecAdapter.decodeInstructionBox(encoded)));
    assertArrayEquals(InstructionBatchHashFixture.bytes("instruction_batch_hash_hex"),
        NoritoJavaCodecAdapter.hashCanonicalInstructionBoxes(Collections.singletonList(encoded)));
  }
}
