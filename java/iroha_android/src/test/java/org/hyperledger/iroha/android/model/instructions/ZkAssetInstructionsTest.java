package org.hyperledger.iroha.android.model.instructions;

import java.util.Arrays;
import java.util.Collections;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.crypto.NativeSignedTransaction;
import org.hyperledger.iroha.android.crypto.NativeSignerBridge;
import org.hyperledger.iroha.android.crypto.SigningAlgorithm;
import org.hyperledger.iroha.android.model.FeeChargeKind;
import org.hyperledger.iroha.android.model.FeeChargeLimit;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
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
    unshieldInstructionValidatesInputsAndProofWithExactShape();
    unshieldFromArgumentsRejectsStaleOutputBearingPayload();
    registerZkAssetInstructionValidatesModeAndVerifierIds();
    nativeSignerZkMethodsRejectBadInputsBeforeNativeDispatch();
    nativeSignerFeePaymentRejectsInvalidBoundsBeforeNativeDispatch();
    nativeSignedTransactionCopiesInputsAndOutputs();
    nativeSignerZkMethodsBindFeePaymentWhenBridgeAvailable();
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
    final byte[] proofBytes = new byte[] {0x40, 0x41};
    final ProofAttachment attachment =
        new ProofAttachment(
            "halo2/ipa",
            proofBytes,
            new ProofVerifierKeyRef("halo2/ipa", "unshield-v3"),
            fill(0x55, 32),
            IrohaHash.prehash(proofBytes));
    final String json = attachment.toNativeJson();
    assert json.contains("\"backend\":\"halo2/ipa\"");
    assert json.contains("\"proof_b64\":\"QEE=\"");
    assert json.contains("\"vk_ref\":{\"backend\":\"halo2/ipa\",\"name\":\"unshield-v3\"}");
    assert json.contains(
        "\"envelope_hash_hex\":\"99108c58a4d312fe46d8e0d5d36340d62413cd2ffb4b1c4ec8d78ea40b8679a1\"");
    assert !json.contains("vk_inline");
    assert new ProofAttachment(
            "halo2/ipa",
            proofBytes,
            new ProofVerifierKeyRef("halo2/ipa", "unshield-v3"))
        .toNativeJson()
        .contains(
            "\"envelope_hash_hex\":\"99108c58a4d312fe46d8e0d5d36340d62413cd2ffb4b1c4ec8d78ea40b8679a1\"");

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
    expectThrows(
        () -> new ProofAttachment(
            "halo2/ipa",
            proofBytes,
            new ProofVerifierKeyRef("halo2/ipa", "vk"),
            null,
            fill(0x66, 32)));
    expectThrows(() -> ProofVerifierKeyRef.fromWireId("missing-separator"));
  }

  private static void shieldInstructionValidatesCanonicalFieldsAndCopiesBytes() {
    final byte[] commitment = fill(0x7a, 32);
    final ShieldInstruction instruction =
        ShieldInstruction.builder()
            .setAsset("rose#wonderland")
            .setFrom("alice")
            .setAmount("340282366920938463463374607431768211456.25")
            .setNoteCommitment(commitment)
            .setEncryptedPayload(samplePayload())
            .build();
    commitment[0] = 0;
    assert "Shield".equals(instruction.toArguments().get("action"));
    assert "340282366920938463463374607431768211456.25".equals(instruction.amount());
    assert instruction.noteCommitment()[0] == 0x7a;
    final byte[] exposed = instruction.noteCommitment();
    exposed[0] = 0;
    assert instruction.noteCommitment()[0] == 0x7a;

    expectThrows(() -> ShieldInstruction.builder().setAmount("01"));
    expectThrows(() -> ShieldInstruction.builder().setAmount("-1"));
    expectThrows(() -> ShieldInstruction.builder().setAmount("1.0"));
    expectThrows(() -> ShieldInstruction.builder().setNoteCommitment(new byte[32]));
  }

  private static void unshieldInstructionValidatesInputsAndProofWithExactShape() {
    final byte[] input = fill(0x20, 32);
    final byte[] root = fill(0x22, 32);
    final UnshieldInstruction instruction =
        UnshieldInstruction.builder()
            .setAsset("rose#wonderland")
            .setTo("bob")
            .setPublicAmount("0.25")
            .addInput(input)
            .setProof(sampleProof())
            .setRootHint(root)
            .build();
    input[0] = 0;
    root[0] = 0;
    assert "Unshield".equals(instruction.toArguments().get("action"));
    assert "0.25".equals(instruction.publicAmount());
    assert Arrays.equals(
        new String[] {"action", "asset", "to", "public_amount", "inputs", "proof", "root_hint"},
        instruction.toArguments().keySet().toArray(new String[0]));
    assert !instruction.toArguments().containsKey("outputs");
    expectThrows(() -> UnshieldInstruction.builder().setPublicAmount("00.25"));
    expectThrows(() -> UnshieldInstruction.builder().setPublicAmount("-0.25"));
    assert instruction.inputs().size() == 1;
    assert instruction.inputs().get(0)[0] == 0x20;
    assert instruction.rootHint()[0] == 0x22;

    expectThrows(
        () -> UnshieldInstruction.builder()
            .setAsset("rose#wonderland")
            .setTo("bob")
            .setPublicAmount("1")
            .setProof(sampleProof())
            .build());
    expectThrows(() -> UnshieldInstruction.builder().addInput(new byte[32]));
    expectThrows(() -> UnshieldInstruction.builder().setRootHint(fill(1, 31)));
  }

  private static void unshieldFromArgumentsRejectsStaleOutputBearingPayload() {
    final java.util.LinkedHashMap<String, String> staleArguments = new java.util.LinkedHashMap<>();
    staleArguments.put("action", "Unshield");
    staleArguments.put("asset", "rose#wonderland");
    staleArguments.put("to", "bob");
    staleArguments.put("public_amount", "1");
    staleArguments.put("inputs", repeat("20", 32));
    staleArguments.put("outputs", repeat("21", 32));
    staleArguments.put("proof", sampleProof().toNativeJson());
    staleArguments.put("root_hint", "");
    expectUnsupported(() -> UnshieldInstruction.fromArguments(staleArguments));
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
        () ->
            NativeSignerBridge.encodeShieldSignedTransaction(
                SigningAlgorithm.ED25519,
                "chain",
                AccountAddress.DEFAULT_I105_DISCRIMINANT,
                "alice",
                -1,
                null,
                shield,
                new byte[] {1},
                noFeePayment()));
    expectThrows(
        () ->
            NativeSignerBridge.encodeUnshieldSignedTransaction(
                SigningAlgorithm.ED25519,
                " chain ",
                AccountAddress.DEFAULT_I105_DISCRIMINANT,
                "alice",
                0,
                null,
                unshield,
                new byte[] {1},
                noFeePayment()));
    expectThrows(
        () ->
            NativeSignerBridge.encodeRegisterZkAssetSignedTransaction(
                SigningAlgorithm.ED25519,
                "chain",
                AccountAddress.DEFAULT_I105_DISCRIMINANT,
                "alice",
                0,
                0L,
                register,
                new byte[] {1},
                noFeePayment()));
    expectThrows(
        () ->
            NativeSignerBridge.encodeShieldSignedTransaction(
                SigningAlgorithm.ED25519,
                "chain",
                AccountAddress.DEFAULT_I105_DISCRIMINANT,
                "alice",
                0,
                null,
                shield,
                new byte[0],
                noFeePayment()));
  }

  private static void nativeSignerFeePaymentRejectsInvalidBoundsBeforeNativeDispatch() {
    expectThrows(() -> FeePaymentIntent.authority(Collections.emptyList(), 0L));
    expectThrows(() -> new FeeChargeLimit(FeeChargeKind.PIPELINE_GAS, "xor#universal", "1"));
    expectThrows(
        () ->
            new FeeChargeLimit(
                FeeChargeKind.PIPELINE_GAS, "7EAD8EFYUx1aVKZPUU1fyKvr8dF1", "0"));
    expectThrows(
        () ->
            FeePaymentIntent.authority(
                Arrays.asList(
                    new FeeChargeLimit(
                        FeeChargeKind.PIPELINE_GAS,
                        "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
                        "1"),
                    new FeeChargeLimit(
                        FeeChargeKind.NEXUS, "7EAD8EFYUx1aVKZPUU1fyKvr8dF1", "1"))));
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

  private static void nativeSignerZkMethodsBindFeePaymentWhenBridgeAvailable()
      throws Exception {
    assert NativeSignerBridge.REQUIRED_BRIDGE_ABI_VERSION == 21;
    assert NativeSignerBridge.REQUIRED_NATIVE_SIGNER_CONTRACT_REVISION == 2;
    if (!NativeSignerBridge.isNativeAvailable()) {
      throw new AssertionError(
          "connect_norito_bridge ABI 21 native-signer contract revision 2 is required");
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
    final FeePaymentIntent feePayment =
        FeePaymentIntent.authority(
            Collections.singletonList(
                new FeeChargeLimit(
                    FeeChargeKind.PIPELINE_GAS, gasAssetId, Long.toString(gasLimit))),
            gasLimit);
    final RegisterZkAssetInstruction register =
        RegisterZkAssetInstruction.builder()
            .setAsset(gasAssetId)
            .setMode(ZkAssetMode.HYBRID)
            .setAllowShield(true)
            .setAllowUnshield(true)
            .build();

    assertNativeFeePayment(
        NativeSignerBridge.encodeRegisterZkAssetSignedTransaction(
            SigningAlgorithm.ED25519,
            "00000042",
            AccountAddress.DEFAULT_I105_DISCRIMINANT,
            authority,
            1_736_000_000_000L,
            null,
            register,
            keypair.privateKey(),
            feePayment),
        feePayment);

    final ShieldInstruction shield =
        ShieldInstruction.builder()
            .setAsset(gasAssetId)
            .setFrom(authority)
            .setAmount("1")
            .setNoteCommitment(fill(3, 32))
            .setEncryptedPayload(samplePayload())
            .build();
    assertNativeFeePayment(
        NativeSignerBridge.encodeShieldSignedTransaction(
            SigningAlgorithm.ED25519,
            "00000042",
            AccountAddress.DEFAULT_I105_DISCRIMINANT,
            authority,
            1_736_000_000_001L,
            null,
            shield,
            keypair.privateKey(),
            feePayment),
        feePayment);

    final UnshieldInstruction unshield =
        UnshieldInstruction.builder()
            .setAsset(gasAssetId)
            .setTo(authority)
            .setPublicAmount("1")
            .addInput(fill(4, 32))
            .setProof(sampleProof())
            .build();
    assertNativeFeePayment(
        NativeSignerBridge.encodeUnshieldSignedTransaction(
            SigningAlgorithm.ED25519,
            "00000042",
            AccountAddress.DEFAULT_I105_DISCRIMINANT,
            authority,
            1_736_000_000_002L,
            null,
            unshield,
            keypair.privateKey(),
            feePayment),
        feePayment);
  }

  private static void assertNativeFeePayment(
      final NativeSignedTransaction nativeTx, final FeePaymentIntent expected)
      throws Exception {
    final SignedTransaction signed =
        SignedTransactionEncoder.decodeVersioned(nativeTx.versionedSignedTransaction());
    final TransactionPayload payload =
        new NoritoJavaCodecAdapter(org.hyperledger.iroha.android.address.AccountAddress.DEFAULT_I105_DISCRIMINANT).decodeTransaction(signed.encodedPayload());

    assert expected.equals(payload.feePayment()) : "fee payment mismatch";
    assert !payload.metadata().containsKey("gas_asset_id") : "legacy gas_asset_id must be absent";
    assert !payload.metadata().containsKey("gas_limit") : "legacy gas_limit must be absent";
  }

  private static FeePaymentIntent noFeePayment() {
    return FeePaymentIntent.authority(Collections.emptyList());
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

  private static String repeat(final String value, final int count) {
    final StringBuilder repeated = new StringBuilder(value.length() * count);
    for (int i = 0; i < count; i++) {
      repeated.append(value);
    }
    return repeated.toString();
  }

  private static void expectThrows(final Runnable runnable) {
    try {
      runnable.run();
    } catch (final IllegalArgumentException | IllegalStateException expected) {
      return;
    }
    throw new AssertionError("expected exception");
  }

  private static void expectUnsupported(final Runnable runnable) {
    try {
      runnable.run();
    } catch (final UnsupportedOperationException expected) {
      return;
    }
    throw new AssertionError("expected UnsupportedOperationException");
  }
}
