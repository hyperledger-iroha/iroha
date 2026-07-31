package org.hyperledger.iroha.android.model.instructions;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.junit.Test;

/** First-release ProofAttachment model and native-JSON validation tests. */
public final class ProofAttachmentModelTests {
  @Test
  public void portableVerifierIdentifiersMatchRustGrammar() {
    final ProofVerifierKeyRef namespaced =
        new ProofVerifierKeyRef("halo2/ipa", "halo2/ipa::transfer_v1");
    assert namespaced.equals(ProofVerifierKeyRef.fromWireId(namespaced.wireId()));

    for (final String invalid :
        new String[] {
          " leading", "trailing ", "Uppercase", ".hidden", "trailing_", "a..b", "a//b",
          "a:::b", "a/:b", "a:/b", "a/.b", "a./b", "a:.b", "a.:b", "a\\b",
          "a\u200bb", "a\nb", "a+b"
        }) {
      expectThrows(() -> new ProofVerifierKeyRef("halo2/ipa", invalid));
    }
    expectThrows(() -> new ProofVerifierKeyRef("halo2/ipa", repeated('a', 257)));
  }

  @Test
  public void completeProofBoxBoundIsCheckedWithoutLargeAllocations() {
    final long maximum = ProofAttachment.MAXIMUM_ENCODED_PROOF_BOX_BYTES;
    final long backendBytes = "halo2/ipa".getBytes(StandardCharsets.UTF_8).length;
    assert ProofAttachment.canonicalProofBoxEncodedLength(
            backendBytes, maximum - 32L - backendBytes)
        == maximum;
    assert ProofAttachment.canonicalProofBoxEncodedLength(
            backendBytes, maximum - 31L - backendBytes)
        == maximum + 1L;
    expectThrows(() -> ProofAttachment.canonicalProofBoxEncodedLength(Long.MAX_VALUE, 1L));
  }

  @Test
  public void lanePrivacyIsTypedCanonicalDefensiveAndExactJson() {
    final byte[] leaf = fill(0x11, 32);
    final byte[] firstSibling = fill(0x22, 32);
    final LanePrivacyMerkleWitness witness =
        new LanePrivacyMerkleWitness(
            leaf, 1L, Arrays.asList(firstSibling, fill(0x44, 32)));
    leaf[0] = 0;
    firstSibling[0] = 0;
    assert witness.leaf()[0] == 0x11;
    assert witness.auditPath().get(0)[0] == 0x22;
    assert (witness.auditPath().get(0)[31] & 0xff) == 0x23;
    final byte[] exposed = witness.auditPath().get(0);
    exposed[0] = 0;
    assert witness.auditPath().get(0)[0] == 0x22;

    final LanePrivacyProof lane =
        new LanePrivacyProof(7, LanePrivacyWitness.merkle(witness));
    final byte[] proof = new byte[] {1, 2};
    final String json =
        new ProofAttachment(
                "halo2/ipa",
                proof,
                new ProofVerifierKeyRef("halo2/ipa", "vk_transfer"),
                fill(0x55, 32),
                IrohaHash.prehash(proof),
                lane)
            .toNativeJson();
    assert json.contains("\"lane_privacy\":{\"commitment_id\":7");
    assert json.contains("\"kind\":\"merkle\"");
    assert json.contains("\"leaf_index\":1");
    assert !json.contains("proof_backend");
    assert !json.contains("vk_inline");
    assert !json.contains("vk_reference");
    assert !json.contains("null");

    expectThrows(
        () ->
            new ProofAttachment(
                "halo2/ipa",
                proof,
                new ProofVerifierKeyRef("halo2/ipa", "vk_transfer"),
                new byte[32],
                null));
    expectThrows(
        () ->
            new ProofAttachment(
                "halo2/ipa",
                proof,
                new ProofVerifierKeyRef("halo2/ipa", "vk_transfer"),
                null,
                fill(0x66, 32)));
    expectThrows(
        () ->
            new ProofAttachment(
                "halo2/ipa",
                proof,
                new ProofVerifierKeyRef("stark/fri", "vk_transfer")));
  }

  @Test
  public void malformedLanePrivacyResourcesFailClosed() {
    expectThrows(
        () -> new LanePrivacyMerkleWitness(fill(1, 32), 0L, Collections.emptyList()));
    expectThrows(
        () ->
            new LanePrivacyMerkleWitness(
                fill(1, 32), 2L, Collections.singletonList(fill(2, 32))));
    final List<byte[]> tooDeep = new ArrayList<>();
    for (int index = 0; index <= LanePrivacyMerkleWitness.MAX_DEPTH; index++) {
      tooDeep.add(fill(2, 32));
    }
    expectThrows(() -> new LanePrivacyMerkleWitness(fill(1, 32), 0L, tooDeep));
    expectThrows(
        () ->
            new LanePrivacyMerkleWitness(
                fill(1, 31), 0L, Collections.singletonList(fill(2, 32))));
    expectThrows(
        () ->
            new LanePrivacyMerkleWitness(
                fill(1, 32), 0x1_0000_0000L, Collections.singletonList(fill(2, 32))));
    final LanePrivacyMerkleWitness valid =
        new LanePrivacyMerkleWitness(
            fill(1, 32), 0L, Collections.singletonList(fill(2, 32)));
    expectThrows(() -> new LanePrivacyProof(0x1_0000, LanePrivacyWitness.merkle(valid)));
  }

  private static byte[] fill(final int value, final int count) {
    final byte[] bytes = new byte[count];
    Arrays.fill(bytes, (byte) value);
    return bytes;
  }

  private static String repeated(final char value, final int count) {
    final char[] chars = new char[count];
    Arrays.fill(chars, value);
    return new String(chars);
  }

  private static void expectThrows(final Runnable action) {
    try {
      action.run();
    } catch (final RuntimeException expected) {
      return;
    }
    throw new AssertionError("expected invalid ProofAttachment input to fail");
  }
}
