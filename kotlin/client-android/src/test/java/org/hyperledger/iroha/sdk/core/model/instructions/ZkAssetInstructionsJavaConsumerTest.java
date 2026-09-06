package org.hyperledger.iroha.sdk.core.model.instructions;

import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.Arrays;
import java.util.LinkedHashMap;
import org.hyperledger.iroha.sdk.crypto.IrohaHash;
import org.junit.jupiter.api.Test;

/** Java Android callers validate the shared first-release ZK models against exact Rust wire bytes. */
class ZkAssetInstructionsJavaConsumerTest {
  @Test
  void confidentialEncryptedPayloadIsStrictAndDefensive() {
    final byte[] ephemeral = fill(0x11, 32);
    final byte[] nonce = fill(0x22, 24);
    final byte[] ciphertext = new byte[] {0x33, 0x34};
    final ConfidentialEncryptedPayload payload =
        new ConfidentialEncryptedPayload(ephemeral, nonce, ciphertext);
    ephemeral[0] = 0;
    nonce[0] = 0;
    ciphertext[0] = 0;
    assertTrue(payload.version == ConfidentialEncryptedPayload.VERSION_V1);
    assertTrue(payload.getEphemeralPublicKey()[0] == 0x11);
    assertTrue(payload.getNonce()[0] == 0x22);
    assertTrue(payload.getCiphertext()[0] == 0x33);
    final byte[] exposed = payload.getEphemeralPublicKey();
    exposed[0] = 0;
    assertTrue(payload.getEphemeralPublicKey()[0] == 0x11);

    expectThrows(() -> new ConfidentialEncryptedPayload(2, fill(1, 32), fill(2, 24), new byte[] {3}));
    expectThrows(() -> new ConfidentialEncryptedPayload(new byte[32], fill(2, 24), new byte[] {3}));
    final byte[] nonZeroLowOrder = new byte[32];
    nonZeroLowOrder[0] = 1;
    expectThrows(() -> new ConfidentialEncryptedPayload(nonZeroLowOrder, fill(2, 24), new byte[] {3}));
    expectThrows(() -> new ConfidentialEncryptedPayload(fill(1, 31), fill(2, 24), new byte[] {3}));
    expectThrows(() -> new ConfidentialEncryptedPayload(fill(1, 32), fill(2, 23), new byte[] {3}));
    expectThrows(() -> new ConfidentialEncryptedPayload(fill(1, 32), fill(2, 24), new byte[0]));
    expectThrows(
        () ->
            new ConfidentialEncryptedPayload(
                fill(1, 32),
                fill(2, 24),
                new byte[ConfidentialEncryptedPayload.MAX_CIPHERTEXT_BYTES + 1]));
  }

  @Test
  void confidentialEncryptedPayloadMatchesRustWireFixture() {
    final byte[] ephemeral =
        hex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f");
    final byte[] nonce =
        hex("a0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7");
    final byte[] ciphertext = hex("436f6e666964656e7469616c5061796c6f61645631");
    final byte[] serialized =
        hex(
            "01000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
                + "a0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b715"
                + "436f6e666964656e7469616c5061796c6f61645631");
    final ConfidentialEncryptedPayload payload =
        new ConfidentialEncryptedPayload(ephemeral, nonce, ciphertext);

    assertTrue(Arrays.equals(serialized, payload.toWireBytes()), "wire bytes mismatch");
    assertTrue(payload.equals(ConfidentialEncryptedPayload.fromWireBytes(serialized)), "wire decode mismatch");
    final byte[] exposedWire = payload.toWireBytes();
    exposedWire[0] = 0;
    assertTrue(Arrays.equals(serialized, payload.toWireBytes()), "wire bytes must be defensive");

    expectThrows(
        () -> ConfidentialEncryptedPayload.fromWireBytes(
            Arrays.copyOf(serialized, serialized.length - 1)));
    expectThrows(
        () -> ConfidentialEncryptedPayload.fromWireBytes(concat(serialized, new byte[] {0})));
    expectThrows(
        () -> ConfidentialEncryptedPayload.fromWireBytes(
            concat(new byte[] {0}, ephemeral, nonce, new byte[] {(byte) ciphertext.length}, ciphertext)));
    expectThrows(
        () -> ConfidentialEncryptedPayload.fromWireBytes(
            concat(new byte[] {1}, ephemeral, nonce, new byte[] {(byte) 0x95, 0}, ciphertext)));
    expectThrows(
        () ->
            ConfidentialEncryptedPayload.fromWireBytes(
                concat(
                    new byte[] {1},
                    ephemeral,
                    nonce,
                    new byte[] {(byte) 0x81, (byte) 0x80, 0x04})));
  }

  @Test
  void proofAttachmentValidatesBackendAndJsonShape() {
    final byte[] proofBytes = new byte[] {0x40, 0x41};
    final ProofAttachment attachment =
        new ProofAttachment(
            "halo2/ipa",
            proofBytes,
            new ProofVerifierKeyRef("halo2/ipa", "unshield-v3"),
            fill(0x55, 32),
            IrohaHash.prehash(proofBytes), null);
    final String json = attachment.toNativeJson();
    assertTrue(json.contains("\"backend\":\"halo2/ipa\""));
    assertTrue(json.contains("\"proof_b64\":\"QEE=\""));
    assertTrue(json.contains("\"vk_ref\":{\"backend\":\"halo2/ipa\",\"name\":\"unshield-v3\"}"));
    assertTrue(json.contains(
        "\"envelope_hash_hex\":\"99108c58a4d312fe46d8e0d5d36340d62413cd2ffb4b1c4ec8d78ea40b8679a1\""));
    assertTrue(!json.contains("vk_inline"));
    assertTrue(new ProofAttachment(
            "halo2/ipa",
            proofBytes,
            new ProofVerifierKeyRef("halo2/ipa", "unshield-v3"), null, null, null)
        .toNativeJson()
        .contains(
            "\"envelope_hash_hex\":\"99108c58a4d312fe46d8e0d5d36340d62413cd2ffb4b1c4ec8d78ea40b8679a1\""));

    expectThrows(
        () -> new ProofAttachment(
            "halo2/ipa", new byte[0], new ProofVerifierKeyRef("halo2/ipa", "vk"), null, null, null));
    expectThrows(
        () -> new ProofAttachment(
            "halo2/ipa", new byte[] {1}, new ProofVerifierKeyRef("stark/fri", "vk"), null, null, null));
    expectThrows(
        () -> new ProofAttachment(
            "halo2/ipa",
            new byte[] {1},
            new ProofVerifierKeyRef("halo2/ipa", "vk"),
            new byte[32],
            null, null));
    expectThrows(
        () -> new ProofAttachment(
            "halo2/ipa",
            proofBytes,
            new ProofVerifierKeyRef("halo2/ipa", "vk"),
            null,
            fill(0x66, 32), null));
    expectThrows(() -> ProofVerifierKeyRef.fromWireId("missing-separator"));
  }

  @Test
  void registerZkAssetInstructionBuildsVerifierControls() {
    final RegisterZkAssetInstruction instruction =
        RegisterZkAssetInstruction.builder()
            .setAsset("rose#wonderland")
            .setUnshieldVerifyingKey("halo2/ipa:unshield-v3")
            .setShieldVerifyingKey("halo2/ipa:shield-v3")
            .build();
    assertTrue(instruction.getKind() == InstructionKind.REGISTER);
    assertTrue("halo2/ipa:unshield-v3".equals(instruction.getArguments().get("vk_unshield")));
    assertTrue("halo2/ipa:shield-v3".equals(instruction.getArguments().get("vk_shield")));
    expectThrows(
        () ->
            RegisterZkAssetInstruction.builder()
                .setAsset("rose#wonderland")
                .setShieldVerifyingKey("halo2/ipa:shield-v3")
                .build());

    final LinkedHashMap<String, String> retiredArguments =
        new LinkedHashMap<>(instruction.getArguments());
    retiredArguments.put("mode", "Hybrid");
    expectThrows(() -> RegisterZkAssetInstruction.fromArguments(retiredArguments));
    retiredArguments.remove("mode");
    retiredArguments.put("vk_transfer", "halo2/ipa:transfer-v2");
    expectThrows(() -> RegisterZkAssetInstruction.fromArguments(retiredArguments));
    retiredArguments.remove("vk_transfer");
    retiredArguments.put("allow_shield", "true");
    expectThrows(() -> RegisterZkAssetInstruction.fromArguments(retiredArguments));
  }

  private static byte[] fill(final int value, final int size) {
    final byte[] out = new byte[size];
    Arrays.fill(out, (byte) value);
    return out;
  }

  private static byte[] hex(final String value) {
    if ((value.length() & 1) != 0) {
      throw new IllegalArgumentException("hex length must be even");
    }
    final byte[] out = new byte[value.length() / 2];
    for (int i = 0; i < out.length; i++) {
      out[i] = (byte) Integer.parseInt(value.substring(i * 2, i * 2 + 2), 16);
    }
    return out;
  }

  private static byte[] concat(final byte[]... parts) {
    int len = 0;
    for (final byte[] part : parts) {
      len += part.length;
    }
    final byte[] out = new byte[len];
    int offset = 0;
    for (final byte[] part : parts) {
      System.arraycopy(part, 0, out, offset, part.length);
      offset += part.length;
    }
    return out;
  }

  private static void expectThrows(final Runnable runnable) {
    try {
      runnable.run();
    } catch (final IllegalArgumentException | IllegalStateException expected) {
      return;
    }
    throw new AssertionError("expected exception");
  }
}
