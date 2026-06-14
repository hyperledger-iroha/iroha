package org.hyperledger.iroha.android.model.instructions;

import java.util.Arrays;
import java.util.Collections;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.crypto.NativeSignedTransaction;
import org.hyperledger.iroha.android.crypto.NativeSignerBridge;
import org.hyperledger.iroha.android.crypto.SigningAlgorithm;
import org.hyperledger.iroha.android.model.JsonValue;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;
import org.hyperledger.iroha.android.tx.SignedTransaction;

public final class ZkAssetInstructionsTest {
  private ZkAssetInstructionsTest() {}

  public static void main(final String[] args) throws Exception {
    confidentialEncryptedPayloadIsStrictAndDefensive();
    confidentialEncryptedPayloadMatchesRustWireFixture();
    proofAttachmentValidatesBackendAndJsonShape();
    shieldInstructionValidatesCanonicalFieldsAndCopiesBytes();
    unshieldInstructionValidatesInputsOutputsAndProof();
    registerZkAssetInstructionValidatesModeAndVerifierIds();
    nativeSignerZkMethodsRejectBadInputsBeforeNativeDispatch();
    nativeSignedTransactionCopiesInputsAndOutputs();
    nativeSignerZkRegisterIncludesGasMetadataWhenBridgeAvailable();
    System.out.println("[IrohaAndroid] ZkAssetInstructionsTest passed.");
  }

  private static void confidentialEncryptedPayloadIsStrictAndDefensive() {
    final byte[] ephemeral = fill(0x11, 32);
    final byte[] nonce = fill(0x22, 24);
    final byte[] ciphertext = new byte[] {0x33, 0x34};
    final ConfidentialEncryptedPayload payload =
        new ConfidentialEncryptedPayload(ephemeral, nonce, ciphertext);
    ephemeral[0] = 0;
    nonce[0] = 0;
    ciphertext[0] = 0;
    assert payload.version() == ConfidentialEncryptedPayload.VERSION_V1;
    assert payload.ephemeralPublicKey()[0] == 0x11;
    assert payload.nonce()[0] == 0x22;
    assert payload.ciphertext()[0] == 0x33;
    final byte[] exposed = payload.ephemeralPublicKey();
    exposed[0] = 0;
    assert payload.ephemeralPublicKey()[0] == 0x11;

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

  private static void confidentialEncryptedPayloadMatchesRustWireFixture() {
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

    assert Arrays.equals(serialized, payload.toWireBytes()) : "wire bytes mismatch";
    assert payload.equals(ConfidentialEncryptedPayload.fromWireBytes(serialized))
        : "wire decode mismatch";
    final byte[] exposedWire = payload.toWireBytes();
    exposedWire[0] = 0;
    assert Arrays.equals(serialized, payload.toWireBytes()) : "wire bytes must be defensive";

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

  private static void proofAttachmentValidatesBackendAndJsonShape() {
    final ProofAttachment attachment =
        new ProofAttachment(
            "halo2/ipa",
            new byte[] {0x40, 0x41},
            new ProofVerifierKeyRef("halo2/ipa", "unshield-v3"),
            fill(0x55, 32),
            fill(0x66, 32));
    final String json = attachment.toNativeJson();
    assert json.contains("\"backend\":\"halo2/ipa\"");
    assert json.contains("\"proof_b64\":\"QEE=\"");
    assert json.contains("\"vk_ref\":{\"backend\":\"halo2/ipa\",\"name\":\"unshield-v3\"}");
    assert !json.contains("vk_inline");

    expectThrows(
        () -> new ProofAttachment(
            "halo2/ipa", new byte[0], new ProofVerifierKeyRef("halo2/ipa", "vk")));
    expectThrows(
        () -> new ProofAttachment(
            "halo2/ipa", new byte[] {1}, new ProofVerifierKeyRef("stark/fri", "vk")));
    expectThrows(
        () -> new ProofAttachment(
            "halo2/ipa",
            new byte[] {1},
            new ProofVerifierKeyRef("halo2/ipa", "vk"),
            new byte[32],
            null));
    expectThrows(() -> ProofVerifierKeyRef.fromWireId("missing-separator"));
  }

  private static void shieldInstructionValidatesCanonicalFieldsAndCopiesBytes() {
    final byte[] commitment = fill(0x7a, 32);
    final ShieldInstruction instruction =
        ShieldInstruction.builder()
            .setAsset("rose#wonderland")
            .setFrom("alice")
            .setAmount("340282366920938463463374607431768211455")
            .setNoteCommitment(commitment)
            .setEncryptedPayload(samplePayload())
            .build();
    commitment[0] = 0;
    assert "Shield".equals(instruction.toArguments().get("action"));
    assert "340282366920938463463374607431768211455".equals(instruction.amount());
    assert instruction.noteCommitment()[0] == 0x7a;
    final byte[] exposed = instruction.noteCommitment();
    exposed[0] = 0;
    assert instruction.noteCommitment()[0] == 0x7a;

    expectThrows(() -> ShieldInstruction.builder().setAmount("01"));
    expectThrows(() -> ShieldInstruction.builder().setAmount("-1"));
    expectThrows(() -> ShieldInstruction.builder().setAmount("340282366920938463463374607431768211456"));
    expectThrows(() -> ShieldInstruction.builder().setNoteCommitment(new byte[32]));
  }

  private static void unshieldInstructionValidatesInputsOutputsAndProof() {
    final byte[] input = fill(0x20, 32);
    final byte[] output = fill(0x21, 32);
    final byte[] root = fill(0x22, 32);
    final UnshieldInstruction instruction =
        UnshieldInstruction.builder()
            .setAsset("rose#wonderland")
            .setTo("bob")
            .setPublicAmount("0")
            .addInput(input)
            .addOutput(output)
            .setProof(sampleProof())
            .setRootHint(root)
            .build();
    input[0] = 0;
    output[0] = 0;
    root[0] = 0;
    assert "Unshield".equals(instruction.toArguments().get("action"));
    assert "0".equals(instruction.publicAmount());
    assert instruction.inputs().size() == 1;
    assert instruction.outputs().size() == 1;
    assert instruction.inputs().get(0)[0] == 0x20;
    assert instruction.outputs().get(0)[0] == 0x21;
    assert instruction.rootHint()[0] == 0x22;

    expectThrows(
        () -> UnshieldInstruction.builder()
            .setAsset("rose#wonderland")
            .setTo("bob")
            .setPublicAmount("1")
            .setProof(sampleProof())
            .build());
    expectThrows(() -> UnshieldInstruction.builder().addInput(new byte[32]));
    expectThrows(() -> UnshieldInstruction.builder().addOutput(fill(1, 31)));
    expectThrows(() -> UnshieldInstruction.builder().setRootHint(fill(1, 31)));
  }

  private static void registerZkAssetInstructionValidatesModeAndVerifierIds() {
    final RegisterZkAssetInstruction instruction =
        RegisterZkAssetInstruction.builder()
            .setAsset("rose#wonderland")
            .setMode(ZkAssetMode.HYBRID)
            .setAllowShield(true)
            .setAllowUnshield(false)
            .setTransferVerifyingKey("halo2/ipa:transfer-v2")
            .build();
    assert instruction.kind() == InstructionKind.REGISTER;
    assert "Hybrid".equals(instruction.toArguments().get("mode"));
    assert "false".equals(instruction.toArguments().get("allow_unshield"));
    assert ZkAssetMode.fromWireName("ZkNative") == ZkAssetMode.ZK_NATIVE;
    expectThrows(() -> ZkAssetMode.fromWireName("zk-native"));
    expectThrows(() -> RegisterZkAssetInstruction.builder().setTransferVerifyingKey("halo2/ipa"));
  }

  private static void nativeSignerZkMethodsRejectBadInputsBeforeNativeDispatch() {
    final ShieldInstruction shield =
        ShieldInstruction.builder()
            .setAsset("rose#wonderland")
            .setFrom("alice")
            .setAmount("1")
            .setNoteCommitment(fill(1, 32))
            .setEncryptedPayload(samplePayload())
            .build();
    final UnshieldInstruction unshield =
        UnshieldInstruction.builder()
            .setAsset("rose#wonderland")
            .setTo("bob")
            .setPublicAmount("1")
            .addInput(fill(2, 32))
            .setProof(sampleProof())
            .build();
    final RegisterZkAssetInstruction register =
        RegisterZkAssetInstruction.builder().setAsset("rose#wonderland").build();

    expectThrows(
        () -> NativeSignerBridge.encodeShieldSignedTransaction(
            SigningAlgorithm.ED25519, "chain", "alice", -1, null, shield, new byte[] {1}));
    expectThrows(
        () -> NativeSignerBridge.encodeUnshieldSignedTransaction(
            SigningAlgorithm.ED25519, " chain ", "alice", 0, null, unshield, new byte[] {1}));
    expectThrows(
        () -> NativeSignerBridge.encodeRegisterZkAssetSignedTransaction(
            SigningAlgorithm.ED25519, "chain", "alice", 0, 0L, register, new byte[] {1}));
    expectThrows(
        () -> NativeSignerBridge.encodeShieldSignedTransaction(
            SigningAlgorithm.ED25519, "chain", "alice", 0, null, shield, new byte[0]));
  }

  private static void nativeSignedTransactionCopiesInputsAndOutputs() {
    final byte[] versioned = new byte[] {1, 2, 3};
    final byte[] hash = fill(0x30, 32);
    final NativeSignedTransaction signed = new NativeSignedTransaction(versioned, hash);
    versioned[0] = 9;
    hash[0] = 9;
    assert Arrays.equals(new byte[] {1, 2, 3}, signed.versionedSignedTransaction());
    assert signed.transactionHash()[0] == 0x30;
    final byte[] exposed = signed.versionedSignedTransaction();
    exposed[0] = 9;
    assert Arrays.equals(new byte[] {1, 2, 3}, signed.versionedSignedTransaction());

    expectThrows(() -> new NativeSignedTransaction(new byte[0], fill(1, 32)));
    expectThrows(() -> new NativeSignedTransaction(new byte[] {1}, fill(1, 31)));
  }

  private static void nativeSignerZkRegisterIncludesGasMetadataWhenBridgeAvailable()
      throws Exception {
    assert NativeSignerBridge.REQUIRED_BRIDGE_ABI_VERSION == 8;
    if (!NativeSignerBridge.isNativeAvailable()) {
      return;
    }

    final byte[] seed = new byte[32];
    for (int i = 0; i < seed.length; i++) {
      seed[i] = (byte) (i + 1);
    }
    final NativeSignerBridge.KeypairBytes keypair =
        NativeSignerBridge.keypairFromSeed(SigningAlgorithm.ED25519, seed);
    final String authority =
        AccountAddress.fromAccount(keypair.publicKey(), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    final String gasAssetId = "7EAD8EFYUx1aVKZPUU1fyKvr8dF1";
    final long gasLimit = 1_000L;
    final RegisterZkAssetInstruction instruction =
        RegisterZkAssetInstruction.builder()
            .setAsset(gasAssetId)
            .setMode(ZkAssetMode.HYBRID)
            .setAllowShield(true)
            .setAllowUnshield(true)
            .build();

    final NativeSignedTransaction nativeTx =
        NativeSignerBridge.encodeRegisterZkAssetSignedTransaction(
            SigningAlgorithm.ED25519,
            "00000042",
            authority,
            1_736_000_000_000L,
            null,
            instruction,
            keypair.privateKey(),
            gasAssetId,
            gasLimit);
    final SignedTransaction signed =
        SignedTransactionEncoder.decodeVersioned(nativeTx.versionedSignedTransaction());
    final TransactionPayload payload =
        new NoritoJavaCodecAdapter().decodeTransaction(signed.encodedPayload());

    assert JsonValue.string(gasAssetId).equals(payload.metadata().get("gas_asset_id"))
        : "gas_asset_id metadata mismatch";
    assert JsonValue.number(gasLimit).equals(payload.metadata().get("gas_limit"))
        : "gas_limit metadata mismatch";
  }

  private static ConfidentialEncryptedPayload samplePayload() {
    return new ConfidentialEncryptedPayload(fill(0x11, 32), fill(0x22, 24), new byte[] {0x33, 0x34});
  }

  private static ProofAttachment sampleProof() {
    return new ProofAttachment(
        "halo2/ipa",
        new byte[] {0x44},
        new ProofVerifierKeyRef("halo2/ipa", "unshield-v3"),
        fill(0x55, 32),
        null);
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
