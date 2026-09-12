package org.hyperledger.iroha.sdk.privacy;

import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.fail;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import org.hyperledger.iroha.sdk.privacy.fixtures.RetiredPrivacyConfidentialNoteWitnessV1;
import org.hyperledger.iroha.sdk.privacy.fixtures.RetiredPrivacyConfidentialTransferOutputWitnessV1;
import org.hyperledger.iroha.sdk.privacy.fixtures.RetiredPrivacyConfidentialWitnessV1;
import org.hyperledger.iroha.sdk.privacy.fixtures.RetiredPrivacyConfidentialWitnessCodecs;
import org.hyperledger.iroha.sdk.testing.TestNetworkIds;
import org.junit.jupiter.api.Test;

/** Retained Java assertions and current-carrier rejection of historical witness fixtures. */
public final class PrivacyRetiredWitnessBoundaryJavaConsumerTest {
  @Test
  public void historicalWitnessJava8SurfaceCopiesLists() {
    final RetiredPrivacyConfidentialNoteWitnessV1 input =
        new RetiredPrivacyConfidentialNoteWitnessV1(
            "7", repeatedByte(0x22), repeatedByte(0x33), 0L);
    final RetiredPrivacyConfidentialTransferOutputWitnessV1 output =
        new RetiredPrivacyConfidentialTransferOutputWitnessV1(
            "7", repeatedByte(0x44), repeatedByte(0x55));
    final List<RetiredPrivacyConfidentialNoteWitnessV1> inputs = new ArrayList<>();
    final List<RetiredPrivacyConfidentialTransferOutputWitnessV1> outputs = new ArrayList<>();
    inputs.add(input);
    outputs.add(output);

    final RetiredPrivacyConfidentialWitnessV1 witness =
        new RetiredPrivacyConfidentialWitnessV1(
            TestNetworkIds.INSTANCE.canonical(),
            "xor#universal",
            repeatedByte(0x11),
            Collections.singletonList(repeatedByte(0x10)),
            inputs,
            outputs,
            Collections.emptyList(),
            "0",
            repeatedByte(0x66));
    inputs.clear();
    outputs.clear();

    assertTrue(witness.getInputs().size() == 1, "privacy witness inputs must be copied");
    assertTrue(
        witness.getTransferOutputs().size() == 1, "privacy witness transfer outputs must be copied");
    try {
      witness.getInputs().add(input);
      fail("privacy witness input lists must be immutable");
    } catch (final UnsupportedOperationException expected) {
      // Expected immutable list behavior.
    }
  }

  @Test
  public void currentTypedCarrierRejectsRetiredTransferAndUnshieldArchives() {
    final RetiredPrivacyConfidentialNoteWitnessV1 input =
        new RetiredPrivacyConfidentialNoteWitnessV1(
            "7", repeatedByte(0x22), repeatedByte(0x33), 0L);
    final RetiredPrivacyConfidentialTransferOutputWitnessV1 output =
        new RetiredPrivacyConfidentialTransferOutputWitnessV1(
            "7", repeatedByte(0x44), repeatedByte(0x55));
    final RetiredPrivacyConfidentialWitnessV1 transfer =
        new RetiredPrivacyConfidentialWitnessV1(
            TestNetworkIds.INSTANCE.canonical(), "xor#universal", repeatedByte(0x11),
            Collections.singletonList(repeatedByte(0x10)), Collections.singletonList(input),
            Collections.singletonList(output), Collections.emptyList(), "0", repeatedByte(0x66));
    final RetiredPrivacyConfidentialWitnessV1 unshield =
        new RetiredPrivacyConfidentialWitnessV1(
            TestNetworkIds.INSTANCE.canonical(), "xor#universal", repeatedByte(0x11),
            Collections.singletonList(repeatedByte(0x10)), Collections.singletonList(input),
            Collections.emptyList(), Collections.emptyList(), "7", repeatedByte(0x66));
    final byte[] transferArchive = RetiredPrivacyConfidentialWitnessCodecs.encodeTransferWitness(transfer);
    final byte[] unshieldArchive = RetiredPrivacyConfidentialWitnessCodecs.encodeUnshieldWitness(unshield);
    assertThrows(IllegalArgumentException.class,
        () -> PrivacyExact12FixtureCodecV1.decodeCanonical(transferArchive));
    assertThrows(IllegalArgumentException.class,
        () -> PrivacyExact12FixtureCodecV1.decodeCanonical(unshieldArchive));
  }

  private static byte[] repeatedByte(final int value) {
    final byte[] out = new byte[32];
    java.util.Arrays.fill(out, (byte) value);
    return out;
  }
}
