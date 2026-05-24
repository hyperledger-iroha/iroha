package org.hyperledger.iroha.android.sccp;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotEquals;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import org.junit.Test;

/** Tests for Solana SCCP local proof request helpers. */
public final class SolanaSccpProverTests {
  @Test
  public void normalizesWitnessAndBuildsDeterministicRequest() {
    final SolanaSccpProver.WitnessInput input = sampleWitnessInput();
    final SolanaSccpProver.Witness witness = SolanaSccpProver.normalizeWitness(input);

    assertEquals(SolanaSccpProver.DOMAIN_SOLANA, witness.sourceDomain());
    assertEquals(SolanaSccpProver.DOMAIN_SORA, witness.targetDomain());
    assertEquals("0x" + repeat("aa", 32), witness.bankHash());
    assertEquals("123456789", witness.finalizedSlot());

    final SolanaSccpProver.ProofRequest first = SolanaSccpProver.buildProofRequest(input);
    final SolanaSccpProver.ProofRequest second = SolanaSccpProver.buildProofRequest(input);
    assertEquals(first.witnessHash(), second.witnessHash());
    assertTrue(first.witnessHash().matches("0x[0-9a-f]{64}"));

    final SolanaSccpProver.WitnessInput changedDigest =
        new SolanaSccpProver.WitnessInput(
            "123456789",
            "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosg1kA",
            repeat("aa", 32),
            repeat("bb", 32),
            repeat("cc", 32),
            "3mJr7AoUXx2Wqd",
            "Sccp111111111111111111111111111111111111111",
            repeat("dd", 32),
            repeat("ee", 32),
            repeat("12", 32),
            repeat("35", 32));
    assertNotEquals(
        first.witnessHash(), SolanaSccpProver.buildProofRequest(changedDigest).witnessHash());
  }

  @Test
  public void requiresCallerSuppliedSourceEventDigest() {
    assertThrows(
        NullPointerException.class,
        () ->
            SolanaSccpProver.normalizeWitness(
                new SolanaSccpProver.WitnessInput(
                    "123456789",
                    "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosg1kA",
                    repeat("aa", 32),
                    repeat("bb", 32),
                    repeat("cc", 32),
                    "3mJr7AoUXx2Wqd",
                    "Sccp111111111111111111111111111111111111111",
                    repeat("dd", 32),
                    repeat("ee", 32),
                    repeat("12", 32),
                    null)));
  }

  @Test
  public void buildsMessageProofHashFromInclusionWitness() {
    final byte[][] branch = new byte[][] {repeatByte((byte) 0x56, 32)};
    final String hash =
        SolanaSccpProver.messageProofHash(repeat("34", 32), repeat("bb", 32), branch);

    assertTrue(hash.matches("0x[0-9a-f]{64}"));
    assertTrue(
        SolanaSccpProver.canonicalMessageProofBytes(repeat("34", 32), repeat("bb", 32), branch)
                .length
            > 0);
    final IllegalArgumentException ex =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SolanaSccpProver.messageProofHash(
                    repeat("34", 32), repeat("bb", 32), new byte[][] {repeatByte((byte) 0xab, 31)}));
    assertTrue(ex.getMessage().contains("inclusionBranch[0]"));
  }

  @Test
  public void proverRequiresLinkedProofEngine() {
    final IllegalStateException ex =
        assertThrows(
            IllegalStateException.class,
            () -> new SolanaSccpProver().prove(sampleWitnessInput()));
    assertTrue(ex.getMessage().contains("not linked"));
  }

  @Test
  public void proverWrapsExternalProofBytes() {
    final SolanaSccpProver prover =
        new SolanaSccpProver(
            null,
            request -> {
              assertEquals(SolanaSccpProver.RECURSIVE_PROOF_BACKEND_V1, request.backend());
              return new byte[] {1, 2, 3, 4};
            });

    final SolanaSccpProver.ProofResult result = prover.prove(sampleWitnessInput());
    assertArrayEquals(new byte[] {1, 2, 3, 4}, result.proofBytes());
    assertEquals("AQIDBA==", result.proofBase64());
    assertTrue(result.envelopeHash().matches("0x[0-9a-f]{64}"));
  }

  private static SolanaSccpProver.WitnessInput sampleWitnessInput() {
    return new SolanaSccpProver.WitnessInput(
        "123456789",
        "7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosg1kA",
        repeat("aa", 32),
        repeat("bb", 32),
        repeat("cc", 32),
        "3mJr7AoUXx2Wqd",
        "Sccp111111111111111111111111111111111111111",
        repeat("dd", 32),
        repeat("ee", 32),
        repeat("12", 32),
        repeat("34", 32));
  }

  private static String repeat(final String value, final int count) {
    final StringBuilder out = new StringBuilder(value.length() * count);
    for (int i = 0; i < count; i++) {
      out.append(value);
    }
    return out.toString();
  }

  private static byte[] repeatByte(final byte value, final int count) {
    final byte[] out = new byte[count];
    for (int i = 0; i < count; i++) {
      out[i] = value;
    }
    return out;
  }
}
