package org.hyperledger.iroha.android.norito;

import java.io.ByteArrayOutputStream;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import org.hyperledger.iroha.android.model.instructions.LanePrivacyMerkleWitness;
import org.hyperledger.iroha.android.model.instructions.LanePrivacyProof;
import org.hyperledger.iroha.android.model.instructions.LanePrivacyWitness;
import org.hyperledger.iroha.android.model.instructions.ProofAttachment;
import org.hyperledger.iroha.android.model.instructions.ProofVerifierKeyRef;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.junit.Test;

/** Exact first-release ProofAttachment Norito and adversarial decoding checks. */
public final class ProofAttachmentNoritoTests {
  public ProofAttachmentNoritoTests() {}

  public static void main(final String[] args) {
    final ProofAttachmentNoritoTests tests = new ProofAttachmentNoritoTests();
    tests.thirdTailMatchesCanonicalNoritoLayout();
    tests.roundtripsEverySupportedSequenceLayout();
    tests.decoderRejectsMalformedAndResourceHostilePaths();
    System.out.println("[IrohaAndroid] ProofAttachmentNoritoTests passed.");
  }

  @Test
  public void roundtripsEverySupportedSequenceLayout() {
    final ProofAttachment attachment = sampleAttachment();
    for (final int flags :
        new int[] {
          NoritoHeader.COMPACT_LEN,
          NoritoHeader.PACKED_SEQ,
          NoritoHeader.COMPACT_LEN | NoritoHeader.PACKED_SEQ
        }) {
      final byte[] encoded =
          TransactionPayloadAdapter.encodeProofAttachmentPayload(attachment, flags);
      assert attachment.equals(
              TransactionPayloadAdapter.decodeProofAttachmentPayload(encoded, flags))
          : "roundtrip mismatch for flags=" + flags;
    }
  }

  @Test
  public void thirdTailMatchesCanonicalNoritoLayout() {
    final ProofAttachment attachment = sampleAttachment();
    final byte[] encoded = TransactionPayloadAdapter.encodeProofAttachmentPayload(attachment);
    final byte[] expected =
        manualAttachmentPayload(1L, Arrays.asList(marked(0x22), marked(0x44)), 0L);
    assert Arrays.equals(expected, encoded) : "canonical lane privacy tail mismatch";
    assert attachment.equals(TransactionPayloadAdapter.decodeProofAttachmentPayload(encoded))
        : "ProofAttachment roundtrip mismatch";
  }

  @Test
  public void decoderRejectsMalformedAndResourceHostilePaths() {
    final List<byte[]> tooDeep = new ArrayList<>();
    for (int index = 0; index <= LanePrivacyMerkleWitness.MAX_DEPTH; index++) {
      tooDeep.add(marked(0x22));
    }
    final List<byte[]> missingSibling = new ArrayList<>();
    missingSibling.add(null);
    final List<byte[]> malformed =
        Arrays.asList(
            manualAttachmentPayload(0L, Collections.emptyList(), 0L),
            manualAttachmentPayload(0L, missingSibling, 0L),
            manualAttachmentPayload(0L, Collections.singletonList(fill(0x22, 32)), 0L),
            manualAttachmentPayload(2L, Collections.singletonList(marked(0x22)), 0L),
            manualAttachmentPayload(0L, tooDeep, 0L),
            manualAttachmentPayload(0L, Collections.singletonList(fill(0x23, 31)), 0L),
            manualAttachmentPayload(0L, Collections.singletonList(marked(0x22)), 7L));
    for (final byte[] payload : malformed) {
      expectThrows(() -> TransactionPayloadAdapter.decodeProofAttachmentPayload(payload));
    }

    final byte[] trailingTail =
        concat(
            manualAttachmentPayload(
                0L, Collections.singletonList(marked(0x22)), 0L),
            field(new byte[] {0}));
    expectThrows(() -> TransactionPayloadAdapter.decodeProofAttachmentPayload(trailingTail));

    final ByteArrayOutputStream oversized = new ByteArrayOutputStream();
    write(oversized, field(encodeString("halo2/ipa")));
    write(oversized, u64(ProofAttachment.MAXIMUM_ENCODED_PROOF_BOX_BYTES + 1L));
    final ByteArrayOutputStream oversizedProofBackend = new ByteArrayOutputStream();
    write(oversizedProofBackend, field(encodeString("halo2/ipa")));
    write(oversizedProofBackend, field(u64(8L + 256L + 1L)));
    final ByteArrayOutputStream oversizedVerifierReference = new ByteArrayOutputStream();
    write(oversizedVerifierReference, requiredAttachmentPrefix());
    write(oversizedVerifierReference, u64(2L * (8L + 8L + 256L) + 1L));
    final ByteArrayOutputStream oversizedCommitment = new ByteArrayOutputStream();
    write(oversizedCommitment, requiredAttachmentPrefix());
    write(oversizedCommitment, u64(1L + 8L + 32L * 9L + 1L));
    for (final byte[] payload :
        Arrays.asList(
            oversized.toByteArray(),
            u64(8L + 256L + 1L),
            oversizedProofBackend.toByteArray(),
            oversizedVerifierReference.toByteArray(),
            oversizedCommitment.toByteArray())) {
      expectThrows(() -> TransactionPayloadAdapter.decodeProofAttachmentPayload(payload));
    }
  }

  private static ProofAttachment sampleAttachment() {
    final LanePrivacyMerkleWitness witness =
        new LanePrivacyMerkleWitness(
            fill(0xaa, 32),
            1L,
            Arrays.asList(fill(0x22, 32), fill(0x44, 32)));
    return new ProofAttachment(
        "halo2/ipa",
        new byte[] {1, 2},
        new ProofVerifierKeyRef("halo2/ipa", "vk_transfer"),
        null,
        null,
        new LanePrivacyProof(7, LanePrivacyWitness.merkle(witness)));
  }

  private static byte[] manualAttachmentPayload(
      final long leafIndex, final List<byte[]> auditPath, final long witnessTag) {
    final ByteArrayOutputStream output = new ByteArrayOutputStream();
    write(output, requiredAttachmentPrefix());
    write(output, field(new byte[] {0}));
    write(output, field(new byte[] {0}));
    write(output, field(option(manualLanePrivacy(leafIndex, auditPath, witnessTag))));
    return output.toByteArray();
  }

  private static byte[] requiredAttachmentPrefix() {
    final String backend = "halo2/ipa";
    final ByteArrayOutputStream output = new ByteArrayOutputStream();
    write(output, field(encodeString(backend)));
    write(output, field(manualProofBox(backend, new byte[] {1, 2})));
    write(output, field(manualVerifyingKeyRef(backend, "vk_transfer")));
    return output.toByteArray();
  }

  private static byte[] manualProofBox(final String backend, final byte[] proof) {
    final ByteArrayOutputStream output = new ByteArrayOutputStream();
    write(output, field(encodeString(backend)));
    write(output, field(concat(u64(proof.length), proof)));
    return output.toByteArray();
  }

  private static byte[] manualVerifyingKeyRef(final String backend, final String name) {
    final ByteArrayOutputStream output = new ByteArrayOutputStream();
    write(output, field(encodeString(backend)));
    write(output, field(encodeString(name)));
    return output.toByteArray();
  }

  private static byte[] manualLanePrivacy(
      final long leafIndex, final List<byte[]> auditPath, final long witnessTag) {
    final ByteArrayOutputStream path = new ByteArrayOutputStream();
    write(path, u64(auditPath.size()));
    for (final byte[] sibling : auditPath) {
      write(path, field(option(sibling)));
    }
    final byte[] merkleProof =
        concat(field(u32(leafIndex)), field(path.toByteArray()));
    final byte[] merkleWitness =
        concat(field(fixedBytes(fill(0xaa, 32))), field(merkleProof));
    final byte[] witness = concat(u32(witnessTag), field(merkleWitness));
    return concat(field(u16(7)), field(witness));
  }

  private static byte[] fixedBytes(final byte[] bytes) {
    final ByteArrayOutputStream output = new ByteArrayOutputStream();
    for (final byte value : bytes) {
      write(output, u64(1L));
      output.write(value & 0xff);
    }
    return output.toByteArray();
  }

  private static byte[] option(final byte[] payload) {
    return payload == null
        ? new byte[] {0}
        : concat(new byte[] {1}, u64(payload.length), payload);
  }

  private static byte[] field(final byte[] payload) {
    return concat(u64(payload.length), payload);
  }

  private static byte[] encodeString(final String value) {
    final byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
    return concat(u64(bytes.length), bytes);
  }

  private static byte[] u16(final int value) {
    return ByteBuffer.allocate(2)
        .order(ByteOrder.LITTLE_ENDIAN)
        .putShort((short) value)
        .array();
  }

  private static byte[] u32(final long value) {
    return ByteBuffer.allocate(4)
        .order(ByteOrder.LITTLE_ENDIAN)
        .putInt((int) value)
        .array();
  }

  private static byte[] u64(final long value) {
    return ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(value).array();
  }

  private static byte[] marked(final int value) {
    final byte[] bytes = fill(value, 32);
    bytes[bytes.length - 1] = (byte) (bytes[bytes.length - 1] | 1);
    return bytes;
  }

  private static byte[] fill(final int value, final int count) {
    final byte[] bytes = new byte[count];
    Arrays.fill(bytes, (byte) value);
    return bytes;
  }

  private static byte[] concat(final byte[]... parts) {
    int length = 0;
    for (final byte[] part : parts) {
      length = Math.addExact(length, part.length);
    }
    final byte[] output = new byte[length];
    int offset = 0;
    for (final byte[] part : parts) {
      System.arraycopy(part, 0, output, offset, part.length);
      offset += part.length;
    }
    return output;
  }

  private static void write(final ByteArrayOutputStream output, final byte[] bytes) {
    output.write(bytes, 0, bytes.length);
  }

  private static void expectThrows(final Runnable action) {
    try {
      action.run();
    } catch (final RuntimeException expected) {
      return;
    }
    throw new AssertionError("expected malformed ProofAttachment to fail");
  }
}
