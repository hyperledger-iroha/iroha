package org.hyperledger.iroha.android.privacy;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Pure SDK encoders for the native confidential-v2 privacy proof witness/request ABI. */
public final class PrivacyConfidentialWitness {
  public static final String SCHEMA_PRIVACY_CONFIDENTIAL_WITNESS_V1 =
      "connect_norito_bridge::privacy_production::PrivacyConfidentialWitnessV1";
  public static final String SCHEMA_PRIVACY_PROOF_REQUEST_V1 =
      "connect_norito_bridge::PrivacyProofRequestV1";
  public static final String CONFIDENTIAL_TRANSFER_V2_ALGORITHM_ID =
      "confidential-transfer-v2";
  public static final String CONFIDENTIAL_TRANSFER_V2_ENTRYPOINT =
      "buildConfidentialTransferProofV2";
  public static final String CONFIDENTIAL_TRANSFER_V2_VERIFIER_REF =
      "halo2-ipa-pasta:confidential_transfer_v2";
  public static final String CONFIDENTIAL_UNSHIELD_V3_ALGORITHM_ID = "unshield";
  public static final String CONFIDENTIAL_UNSHIELD_V3_ENTRYPOINT =
      "buildConfidentialUnshieldProofV3";
  public static final String CONFIDENTIAL_UNSHIELD_V3_VERIFIER_REF =
      "halo2-ipa-pasta:confidential_unshield_v3";
  public static final int CONFIDENTIAL_TREE_CAPACITY_V2 = 1 << 16;
  public static final int CONFIDENTIAL_MAX_INPUTS_V2 = 2;
  public static final int CONFIDENTIAL_MAX_TRANSFER_OUTPUTS_V2 = 2;
  public static final int CONFIDENTIAL_MAX_UNSHIELD_CHANGE_OUTPUTS_V3 = 1;

  private static final int NORITO_HEADER_BYTES = 40;
  private static final int REQUEST_FLAGS = NoritoHeader.COMPACT_LEN;
  private static final int PRIVACY_REQUEST_SCHEMA_BYTE = 0x52;
  private static final int WITNESS_HEADER_PADDING_BYTES = 8;
  private static final BigInteger U128_MAX = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE);
  private static final BigInteger BYTE_MASK = BigInteger.valueOf(0xffL);
  private static final byte[] TRANSFER_PUBLIC_INPUTS_SCHEMA =
      ("{\"schema\":\"confidential_transfer_v2\",\"public_inputs\":[\"input_commitment_0\","
              + "\"input_commitment_1\",\"nullifier_0\",\"nullifier_1\",\"output_commitment_0\","
              + "\"output_commitment_1\",\"root\",\"asset_tag\",\"chain_tag\"]}")
          .getBytes(StandardCharsets.UTF_8);
  private static final byte[] UNSHIELD_PUBLIC_INPUTS_SCHEMA =
      ("{\"schema\":\"confidential_unshield_v3\",\"public_inputs\":[\"input_commitment_0\","
              + "\"input_commitment_1\",\"nullifier_0\",\"nullifier_1\",\"change_commitment_0\","
              + "\"root\",\"public_amount\",\"asset_tag\",\"chain_tag\"]}")
          .getBytes(StandardCharsets.UTF_8);

  private PrivacyConfidentialWitness() {}

  public static byte[] confidentialTransferPublicInputsSchemaV1() {
    return TRANSFER_PUBLIC_INPUTS_SCHEMA.clone();
  }

  public static byte[] confidentialUnshieldPublicInputsSchemaV1() {
    return UNSHIELD_PUBLIC_INPUTS_SCHEMA.clone();
  }

  public static byte[] encodeWitness(final WitnessV1 witness) {
    return addNoritoHeaderPadding(
        NoritoCodec.encode(
            Objects.requireNonNull(witness, "witness"),
            SCHEMA_PRIVACY_CONFIDENTIAL_WITNESS_V1,
            WITNESS_ADAPTER,
            REQUEST_FLAGS),
        WITNESS_HEADER_PADDING_BYTES);
  }

  public static byte[] encodeTransferWitness(final WitnessV1 witness) {
    validateTransferWitness(witness);
    return encodeWitness(witness);
  }

  public static byte[] encodeUnshieldWitness(final WitnessV1 witness) {
    validateUnshieldWitness(witness);
    return encodeWitness(witness);
  }

  public static byte[] buildConfidentialTransferProofRequestV1(final WitnessV1 witness) {
    return buildConfidentialTransferProofRequestV1(witness, CONFIDENTIAL_TRANSFER_V2_VERIFIER_REF);
  }

  public static byte[] buildConfidentialTransferProofRequestV1(
      final WitnessV1 witness, final String vkRef) {
    validateVkRef(vkRef, CONFIDENTIAL_TRANSFER_V2_VERIFIER_REF);
    return encodePrivacyProofRequest(
        CONFIDENTIAL_TRANSFER_V2_ALGORITHM_ID,
        CONFIDENTIAL_TRANSFER_V2_ENTRYPOINT,
        vkRef,
        TRANSFER_PUBLIC_INPUTS_SCHEMA,
        encodeTransferWitness(witness),
        new byte[0]);
  }

  public static byte[] buildConfidentialUnshieldProofRequestV1(final WitnessV1 witness) {
    return buildConfidentialUnshieldProofRequestV1(witness, CONFIDENTIAL_UNSHIELD_V3_VERIFIER_REF);
  }

  public static byte[] buildConfidentialUnshieldProofRequestV1(
      final WitnessV1 witness, final String vkRef) {
    validateVkRef(vkRef, CONFIDENTIAL_UNSHIELD_V3_VERIFIER_REF);
    return encodePrivacyProofRequest(
        CONFIDENTIAL_UNSHIELD_V3_ALGORITHM_ID,
        CONFIDENTIAL_UNSHIELD_V3_ENTRYPOINT,
        vkRef,
        UNSHIELD_PUBLIC_INPUTS_SCHEMA,
        encodeUnshieldWitness(witness),
        new byte[0]);
  }

  public static byte[] buildConfidentialTransferVerifyRequestV1(final byte[] proof) {
    return buildConfidentialTransferVerifyRequestV1(proof, CONFIDENTIAL_TRANSFER_V2_VERIFIER_REF);
  }

  public static byte[] buildConfidentialTransferVerifyRequestV1(
      final byte[] proof, final String vkRef) {
    validateVkRef(vkRef, CONFIDENTIAL_TRANSFER_V2_VERIFIER_REF);
    final byte[] proofBytes = copyNonEmpty(proof, "proof");
    return encodePrivacyProofRequest(
        CONFIDENTIAL_TRANSFER_V2_ALGORITHM_ID,
        CONFIDENTIAL_TRANSFER_V2_ENTRYPOINT,
        vkRef,
        TRANSFER_PUBLIC_INPUTS_SCHEMA,
        new byte[0],
        proofBytes);
  }

  public static byte[] buildConfidentialUnshieldVerifyRequestV1(final byte[] proof) {
    return buildConfidentialUnshieldVerifyRequestV1(proof, CONFIDENTIAL_UNSHIELD_V3_VERIFIER_REF);
  }

  public static byte[] buildConfidentialUnshieldVerifyRequestV1(
      final byte[] proof, final String vkRef) {
    validateVkRef(vkRef, CONFIDENTIAL_UNSHIELD_V3_VERIFIER_REF);
    final byte[] proofBytes = copyNonEmpty(proof, "proof");
    return encodePrivacyProofRequest(
        CONFIDENTIAL_UNSHIELD_V3_ALGORITHM_ID,
        CONFIDENTIAL_UNSHIELD_V3_ENTRYPOINT,
        vkRef,
        UNSHIELD_PUBLIC_INPUTS_SCHEMA,
        new byte[0],
        proofBytes);
  }

  private static void validateTransferWitness(final WitnessV1 witness) {
    Objects.requireNonNull(witness, "witness");
    require("0".equals(witness.publicAmount()),
        "confidential transfer witness must not include publicAmount");
    require(witness.unshieldChange().isEmpty(),
        "confidential transfer witness must not include unshieldChange");
    require(
        !witness.transferOutputs().isEmpty()
            && witness.transferOutputs().size() <= CONFIDENTIAL_MAX_TRANSFER_OUTPUTS_V2,
        "confidential transfer witness must include one or two transferOutputs");
  }

  private static void validateUnshieldWitness(final WitnessV1 witness) {
    Objects.requireNonNull(witness, "witness");
    require(witness.transferOutputs().isEmpty(),
        "confidential unshield witness must not include transferOutputs");
    require(witness.unshieldChange().size() <= CONFIDENTIAL_MAX_UNSHIELD_CHANGE_OUTPUTS_V3,
        "confidential unshield witness supports at most one unshieldChange output");
  }

  private static byte[] encodePrivacyProofRequest(
      final String algorithmId,
      final String entrypoint,
      final String vkRef,
      final byte[] publicInputs,
      final byte[] witness,
      final byte[] proof) {
    final byte[] archive =
        NoritoCodec.encode(
            new ProofRequestPayload(
                requestText(algorithmId, "algorithmId"),
                requestText(entrypoint, "entrypoint"),
                requestText(vkRef, "vkRef"),
                publicInputs.clone(),
                witness.clone(),
                proof.clone()),
            SCHEMA_PRIVACY_PROOF_REQUEST_V1,
            PROOF_REQUEST_ADAPTER,
            REQUEST_FLAGS);
    Arrays.fill(archive, 6, 22, (byte) PRIVACY_REQUEST_SCHEMA_BYTE);
    return archive;
  }

  private static byte[] addNoritoHeaderPadding(final byte[] archive, final int paddingBytes) {
    if (archive.length < NORITO_HEADER_BYTES) {
      throw new IllegalArgumentException("Norito archive is missing a header");
    }
    if (paddingBytes == 0) {
      return archive;
    }
    final byte[] out = new byte[archive.length + paddingBytes];
    System.arraycopy(archive, 0, out, 0, NORITO_HEADER_BYTES);
    System.arraycopy(
        archive,
        NORITO_HEADER_BYTES,
        out,
        NORITO_HEADER_BYTES + paddingBytes,
        archive.length - NORITO_HEADER_BYTES);
    return out;
  }

  /** Private input note carried by a production confidential-v2 proof witness. */
  public static final class NoteWitnessV1 {
    private final String amount;
    private final byte[] rho;
    private final byte[] diversifier;
    private final long leafIndex;

    public NoteWitnessV1(
        final String amount, final byte[] rho, final byte[] diversifier, final long leafIndex) {
      this.amount = canonicalU128(amount, "amount");
      this.rho = fixed32(rho, "rho");
      this.diversifier = fixed32(diversifier, "diversifier");
      require(leafIndex >= 0L, "leafIndex must be non-negative");
      this.leafIndex = leafIndex;
    }

    public String amount() {
      return amount;
    }

    public byte[] rho() {
      return rho.clone();
    }

    public byte[] diversifier() {
      return diversifier.clone();
    }

    public long leafIndex() {
      return leafIndex;
    }
  }

  /** Private output note carried by a production confidential transfer witness. */
  public static final class TransferOutputWitnessV1 {
    private final String amount;
    private final byte[] rho;
    private final byte[] ownerTag;

    public TransferOutputWitnessV1(final String amount, final byte[] rho, final byte[] ownerTag) {
      this.amount = canonicalU128(amount, "amount");
      this.rho = fixed32(rho, "rho");
      this.ownerTag = fixed32(ownerTag, "ownerTag");
    }

    public String amount() {
      return amount;
    }

    public byte[] rho() {
      return rho.clone();
    }

    public byte[] ownerTag() {
      return ownerTag.clone();
    }
  }

  /** Private change note carried by a production confidential unshield witness. */
  public static final class UnshieldChangeWitnessV1 {
    private final String amount;
    private final byte[] rho;

    public UnshieldChangeWitnessV1(final String amount, final byte[] rho) {
      this.amount = canonicalU128(amount, "amount");
      this.rho = fixed32(rho, "rho");
    }

    public String amount() {
      return amount;
    }

    public byte[] rho() {
      return rho.clone();
    }
  }

  /** Native {@code PrivacyConfidentialWitnessV1} payload accepted by the bridge. */
  public static final class WitnessV1 {
    private final String chainId;
    private final String assetDefinitionId;
    private final byte[] spendKey;
    private final List<byte[]> treeCommitments;
    private final List<NoteWitnessV1> inputs;
    private final List<TransferOutputWitnessV1> transferOutputs;
    private final List<UnshieldChangeWitnessV1> unshieldChange;
    private final String publicAmount;
    private final byte[] rootHint;

    public WitnessV1(
        final String chainId,
        final String assetDefinitionId,
        final byte[] spendKey,
        final List<byte[]> treeCommitments,
        final List<NoteWitnessV1> inputs,
        final List<TransferOutputWitnessV1> transferOutputs,
        final List<UnshieldChangeWitnessV1> unshieldChange,
        final String publicAmount,
        final byte[] rootHint) {
      this.chainId = canonicalText(chainId, "chainId");
      this.assetDefinitionId = canonicalText(assetDefinitionId, "assetDefinitionId");
      this.spendKey = fixed32(spendKey, "spendKey");
      this.treeCommitments = copyFixed32List(treeCommitments, "treeCommitments");
      this.inputs = List.copyOf(Objects.requireNonNull(inputs, "inputs"));
      this.transferOutputs = List.copyOf(Objects.requireNonNull(transferOutputs, "transferOutputs"));
      this.unshieldChange = List.copyOf(Objects.requireNonNull(unshieldChange, "unshieldChange"));
      this.publicAmount = canonicalU128(publicAmount, "publicAmount");
      this.rootHint = fixed32(rootHint, "rootHint");
      validateShape();
    }

    private void validateShape() {
      require(!treeCommitments.isEmpty(), "treeCommitments must not be empty");
      require(treeCommitments.size() <= CONFIDENTIAL_TREE_CAPACITY_V2,
          "treeCommitments must not exceed " + CONFIDENTIAL_TREE_CAPACITY_V2);
      require(!inputs.isEmpty() && inputs.size() <= CONFIDENTIAL_MAX_INPUTS_V2,
          "inputs must contain one or two notes");
      require(transferOutputs.size() <= CONFIDENTIAL_MAX_TRANSFER_OUTPUTS_V2,
          "transferOutputs must contain at most two notes");
      require(unshieldChange.size() <= CONFIDENTIAL_MAX_UNSHIELD_CHANGE_OUTPUTS_V3,
          "unshieldChange must contain at most one note");
      require(transferOutputs.isEmpty() || unshieldChange.isEmpty(),
          "witness must not mix transferOutputs and unshieldChange");
      require(transferOutputs.isEmpty() || "0".equals(publicAmount),
          "transfer witness must not include publicAmount");
      for (int i = 0; i < inputs.size(); i++) {
        final NoteWitnessV1 input = inputs.get(i);
        require(input.leafIndex() < treeCommitments.size(),
            "inputs[" + i + "].leafIndex must reference treeCommitments");
        for (int previousIndex = 0; previousIndex < i; previousIndex++) {
          final NoteWitnessV1 previous = inputs.get(previousIndex);
          require(previous.leafIndex() != input.leafIndex(),
              "inputs[" + i + "].leafIndex duplicates inputs[" + previousIndex + "]");
          require(!Arrays.equals(previous.rho(), input.rho()),
              "inputs[" + i + "].rho duplicates inputs[" + previousIndex + "]");
        }
      }
    }

    public String chainId() {
      return chainId;
    }

    public String assetDefinitionId() {
      return assetDefinitionId;
    }

    public byte[] spendKey() {
      return spendKey.clone();
    }

    public List<byte[]> treeCommitments() {
      return copyByteArrayList(treeCommitments);
    }

    public List<NoteWitnessV1> inputs() {
      return inputs;
    }

    public List<TransferOutputWitnessV1> transferOutputs() {
      return transferOutputs;
    }

    public List<UnshieldChangeWitnessV1> unshieldChange() {
      return unshieldChange;
    }

    public String publicAmount() {
      return publicAmount;
    }

    public byte[] rootHint() {
      return rootHint.clone();
    }
  }

  private static final TypeAdapter<WitnessV1> WITNESS_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final WitnessV1 value) {
          writeField(encoder, child -> writeString(child, value.chainId()));
          writeField(encoder, child -> writeString(child, value.assetDefinitionId()));
          writeField(encoder, child -> writeBytesVec(child, value.spendKey()));
          writeField(
              encoder,
              child -> writeSequence(child, value.treeCommitments(), PrivacyConfidentialWitness::writeBytesVec));
          writeField(encoder, child -> writeSequence(child, value.inputs(), NOTE_ADAPTER::encode));
          writeField(encoder, child -> writeSequence(child, value.transferOutputs(), TRANSFER_OUTPUT_ADAPTER::encode));
          writeField(encoder, child -> writeSequence(child, value.unshieldChange(), UNSHIELD_CHANGE_ADAPTER::encode));
          writeField(encoder, child -> writeU128(child, value.publicAmount()));
          writeField(encoder, child -> writeBytesVec(child, value.rootHint()));
        }

        @Override
        public WitnessV1 decode(final NoritoDecoder decoder) {
          throw new UnsupportedOperationException("privacy confidential witnesses are encode-only");
        }
      };

  private static final TypeAdapter<NoteWitnessV1> NOTE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final NoteWitnessV1 value) {
          writeField(encoder, child -> writeU128(child, value.amount()));
          writeField(encoder, child -> writeBytesVec(child, value.rho()));
          writeField(encoder, child -> writeBytesVec(child, value.diversifier()));
          writeField(encoder, child -> child.writeUInt(value.leafIndex(), 64));
        }

        @Override
        public NoteWitnessV1 decode(final NoritoDecoder decoder) {
          throw new UnsupportedOperationException("privacy confidential note witnesses are encode-only");
        }
      };

  private static final TypeAdapter<TransferOutputWitnessV1> TRANSFER_OUTPUT_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final TransferOutputWitnessV1 value) {
          writeField(encoder, child -> writeU128(child, value.amount()));
          writeField(encoder, child -> writeBytesVec(child, value.rho()));
          writeField(encoder, child -> writeBytesVec(child, value.ownerTag()));
        }

        @Override
        public TransferOutputWitnessV1 decode(final NoritoDecoder decoder) {
          throw new UnsupportedOperationException("privacy confidential transfer outputs are encode-only");
        }
      };

  private static final TypeAdapter<UnshieldChangeWitnessV1> UNSHIELD_CHANGE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final UnshieldChangeWitnessV1 value) {
          writeField(encoder, child -> writeU128(child, value.amount()));
          writeField(encoder, child -> writeBytesVec(child, value.rho()));
        }

        @Override
        public UnshieldChangeWitnessV1 decode(final NoritoDecoder decoder) {
          throw new UnsupportedOperationException("privacy confidential unshield changes are encode-only");
        }
      };

  private static final TypeAdapter<ProofRequestPayload> PROOF_REQUEST_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final ProofRequestPayload value) {
          writeField(encoder, child -> writeString(child, value.algorithmId));
          writeField(encoder, child -> writeString(child, value.entrypoint));
          writeField(encoder, child -> writeString(child, value.vkRef));
          writeField(encoder, child -> writeBytesVec(child, value.publicInputs));
          writeField(encoder, child -> writeBytesVec(child, value.witness));
          writeField(encoder, child -> writeBytesVec(child, value.proof));
        }

        @Override
        public ProofRequestPayload decode(final NoritoDecoder decoder) {
          throw new UnsupportedOperationException("privacy proof requests are encode-only");
        }
      };

  private static void writeField(final NoritoEncoder parent, final FieldWriter writer) {
    final NoritoEncoder child = parent.childEncoder();
    writer.write(child);
    final byte[] payload = child.toByteArray();
    parent.writeLength(payload.length, compact(parent));
    parent.writeBytes(payload);
  }

  private static void writeString(final NoritoEncoder encoder, final String value) {
    final byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
    encoder.writeLength(bytes.length, compact(encoder));
    encoder.writeBytes(bytes);
  }

  private static void writeBytesVec(final NoritoEncoder encoder, final byte[] value) {
    encoder.writeUInt(value.length, 64);
    encoder.writeBytes(value);
  }

  private static <T> void writeSequence(
      final NoritoEncoder encoder, final List<T> values, final ElementWriter<T> writer) {
    encoder.writeUInt(values.size(), 64);
    for (final T value : values) {
      writeField(encoder, child -> writer.write(child, value));
    }
  }

  private static void writeU128(final NoritoEncoder encoder, final String value) {
    BigInteger integer = new BigInteger(value);
    for (int i = 0; i < 16; i++) {
      encoder.writeByte(integer.and(BYTE_MASK).intValue());
      integer = integer.shiftRight(8);
    }
  }

  private static boolean compact(final NoritoEncoder encoder) {
    return (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
  }

  private static String canonicalU128(final String value, final String name) {
    final String text = canonicalText(value, name);
    for (int i = 0; i < text.length(); i++) {
      final char ch = text.charAt(i);
      require(ch >= '0' && ch <= '9', name + " must be an unsigned decimal integer");
    }
    require(text.length() == 1 || text.charAt(0) != '0',
        name + " must be canonical decimal without leading zeroes");
    final BigInteger parsed = new BigInteger(text);
    require(parsed.signum() >= 0 && parsed.compareTo(U128_MAX) <= 0, name + " must fit in u128");
    return text;
  }

  private static String canonicalText(final String value, final String name) {
    if (value == null) {
      throw new IllegalArgumentException(name + " must be provided");
    }
    final String trimmed = value.trim();
    require(!trimmed.isEmpty(), name + " must not be blank");
    require(trimmed.equals(value), name + " must not contain surrounding whitespace");
    require(trimmed.indexOf('\0') < 0, name + " must not contain NUL");
    return trimmed;
  }

  private static String requestText(final String value, final String name) {
    final String text = canonicalText(value, name);
    require(text.length() <= 1024, name + " must not exceed 1024 characters");
    for (int i = 0; i < text.length(); i++) {
      final char ch = text.charAt(i);
      require(ch >= 0x21 && ch <= 0x7e, name + " must be printable ASCII without whitespace");
      require(Character.isLetterOrDigit(ch) || ch == '-' || ch == '_' || ch == '.' || ch == ':',
          name + " must use portable privacy request characters");
    }
    return text;
  }

  private static void validateVkRef(final String value, final String expected) {
    final String text = requestText(value, "vkRef");
    require(expected.equals(text), "vkRef must be " + expected);
  }

  private static byte[] fixed32(final byte[] value, final String name) {
    if (value == null || value.length != 32) {
      throw new IllegalArgumentException(name + " must be 32 bytes");
    }
    return value.clone();
  }

  private static byte[] copyNonEmpty(final byte[] value, final String name) {
    if (value == null || value.length == 0) {
      throw new IllegalArgumentException(name + " must not be empty");
    }
    return value.clone();
  }

  private static List<byte[]> copyFixed32List(final List<byte[]> values, final String name) {
    Objects.requireNonNull(values, name);
    final List<byte[]> out = new ArrayList<>(values.size());
    for (int i = 0; i < values.size(); i++) {
      out.add(fixed32(values.get(i), name + "[" + i + "]"));
    }
    return Collections.unmodifiableList(out);
  }

  private static List<byte[]> copyByteArrayList(final List<byte[]> values) {
    final List<byte[]> out = new ArrayList<>(values.size());
    for (final byte[] value : values) {
      out.add(value.clone());
    }
    return Collections.unmodifiableList(out);
  }

  private static void require(final boolean condition, final String message) {
    if (!condition) {
      throw new IllegalArgumentException(message);
    }
  }

  @FunctionalInterface
  private interface FieldWriter {
    void write(NoritoEncoder encoder);
  }

  @FunctionalInterface
  private interface ElementWriter<T> {
    void write(NoritoEncoder encoder, T value);
  }

  private static final class ProofRequestPayload {
    private final String algorithmId;
    private final String entrypoint;
    private final String vkRef;
    private final byte[] publicInputs;
    private final byte[] witness;
    private final byte[] proof;

    private ProofRequestPayload(
        final String algorithmId,
        final String entrypoint,
        final String vkRef,
        final byte[] publicInputs,
        final byte[] witness,
        final byte[] proof) {
      this.algorithmId = algorithmId;
      this.entrypoint = entrypoint;
      this.vkRef = vkRef;
      this.publicInputs = publicInputs;
      this.witness = witness;
      this.proof = proof;
    }
  }
}
