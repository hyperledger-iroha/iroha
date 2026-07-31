package org.hyperledger.iroha.android.model.instructions;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Base64;
import java.util.List;
import java.util.Objects;
import org.hyperledger.iroha.android.crypto.IrohaHash;

/** JSON-serializable proof attachment accepted by native zk transaction encoders. */
public final class ProofAttachment {
  public static final long MAXIMUM_ENCODED_PROOF_BOX_BYTES = 64L * 1024L * 1024L;
  private static final long PROOF_BOX_CANONICAL_FIELD_OVERHEAD = 32L;

  private final String backend;
  private final byte[] proofBytes;
  private final ProofVerifierKeyRef verifyingKeyRef;
  private final byte[] verifyingKeyCommitment;
  private final byte[] envelopeHash;
  private final LanePrivacyProof lanePrivacy;

  public ProofAttachment(
      final String backend,
      final byte[] proofBytes,
      final ProofVerifierKeyRef verifyingKeyRef) {
    this(backend, proofBytes, verifyingKeyRef, null, null, null);
  }

  public ProofAttachment(
      final String backend,
      final byte[] proofBytes,
      final ProofVerifierKeyRef verifyingKeyRef,
      final byte[] verifyingKeyCommitment,
      final byte[] envelopeHash) {
    this(
        backend,
        proofBytes,
        verifyingKeyRef,
        verifyingKeyCommitment,
        envelopeHash,
        null);
  }

  public ProofAttachment(
      final String backend,
      final byte[] proofBytes,
      final ProofVerifierKeyRef verifyingKeyRef,
      final byte[] verifyingKeyCommitment,
      final byte[] envelopeHash,
      final LanePrivacyProof lanePrivacy) {
    this.backend = ZkInstructionUtils.requirePortableComponent(backend, "backend");
    if (proofBytes == null) {
      throw new IllegalArgumentException("proofBytes must be provided");
    }
    if (proofBytes.length == 0) {
      throw new IllegalArgumentException("proofBytes must not be empty");
    }
    final long proofBoxLength =
        canonicalProofBoxEncodedLength(
            this.backend.getBytes(StandardCharsets.UTF_8).length, proofBytes.length);
    if (proofBoxLength > MAXIMUM_ENCODED_PROOF_BOX_BYTES) {
      throw new IllegalArgumentException(
          "encoded ProofBox must not exceed "
              + MAXIMUM_ENCODED_PROOF_BOX_BYTES
              + " bytes");
    }
    this.proofBytes = proofBytes.clone();
    this.verifyingKeyRef = Objects.requireNonNull(verifyingKeyRef, "verifyingKeyRef");
    if (!this.backend.equals(this.verifyingKeyRef.backend())) {
      throw new IllegalArgumentException("verifyingKeyRef.backend must match backend");
    }
    this.verifyingKeyCommitment =
        verifyingKeyCommitment == null
            ? null
            : ZkInstructionUtils.fixedNonZeroBytes(
                verifyingKeyCommitment, 32, "verifyingKeyCommitment");
    this.envelopeHash =
        envelopeHash == null
            ? null
            : ZkInstructionUtils.fixedBytes(envelopeHash, 32, "envelopeHash");
    if (this.envelopeHash != null
        && !Arrays.equals(this.envelopeHash, IrohaHash.prehash(this.proofBytes))) {
      throw new IllegalArgumentException("envelopeHash must match proofBytes");
    }
    this.lanePrivacy = lanePrivacy;
  }

  public String backend() {
    return backend;
  }

  public byte[] proofBytes() {
    return proofBytes.clone();
  }

  public ProofVerifierKeyRef verifyingKeyRef() {
    return verifyingKeyRef;
  }

  public byte[] verifyingKeyCommitment() {
    return verifyingKeyCommitment == null ? null : verifyingKeyCommitment.clone();
  }

  public byte[] envelopeHash() {
    return envelopeHash == null ? null : envelopeHash.clone();
  }

  public LanePrivacyProof lanePrivacy() {
    return lanePrivacy;
  }

  public String toNativeJson() {
    final StringBuilder builder = new StringBuilder();
    builder.append('{');
    builder.append("\"backend\":");
    ZkInstructionUtils.appendJsonString(builder, backend);
    builder.append(",\"proof_b64\":");
    ZkInstructionUtils.appendJsonString(
        builder, Base64.getEncoder().encodeToString(proofBytes));
    builder.append(",\"vk_ref\":{\"backend\":");
    ZkInstructionUtils.appendJsonString(builder, verifyingKeyRef.backend());
    builder.append(",\"name\":");
    ZkInstructionUtils.appendJsonString(builder, verifyingKeyRef.name());
    builder.append('}');
    if (verifyingKeyCommitment != null) {
      builder.append(",\"vk_commitment_hex\":");
      ZkInstructionUtils.appendJsonString(
          builder, ZkInstructionUtils.hexLower(verifyingKeyCommitment));
    }
    builder.append(",\"envelope_hash_hex\":");
    ZkInstructionUtils.appendJsonString(
        builder,
        ZkInstructionUtils.hexLower(
            envelopeHash == null ? IrohaHash.prehash(proofBytes) : envelopeHash));
    if (lanePrivacy != null) {
      builder.append(",\"lane_privacy\":");
      appendLanePrivacyJson(builder, lanePrivacy);
    }
    builder.append('}');
    return builder.toString();
  }

  /** Calculate the complete encoded `ProofBox` length without allocating proof bytes. */
  public static long canonicalProofBoxEncodedLength(
      final long backendUtf8ByteCount, final long proofByteCount) {
    if (backendUtf8ByteCount < 0L || proofByteCount < 0L) {
      throw new IllegalArgumentException("ProofBox component lengths must be non-negative");
    }
    try {
      return Math.addExact(
          Math.addExact(PROOF_BOX_CANONICAL_FIELD_OVERHEAD, backendUtf8ByteCount),
          proofByteCount);
    } catch (final ArithmeticException error) {
      throw new IllegalArgumentException(
          "encoded ProofBox length overflows the supported range", error);
    }
  }

  private static void appendLanePrivacyJson(
      final StringBuilder builder, final LanePrivacyProof lanePrivacy) {
    builder.append("{\"commitment_id\":").append(lanePrivacy.commitmentId());
    builder.append(",\"witness\":{\"kind\":\"merkle\",\"payload\":");
    if (!(lanePrivacy.witness() instanceof LanePrivacyWitness.Merkle merkle)) {
      throw new IllegalStateException("unsupported lane privacy witness variant");
    }
    builder.append("{\"leaf\":");
    appendJsonByteArray(builder, merkle.value().leaf());
    builder.append(",\"proof\":{\"leaf_index\":").append(merkle.value().leafIndex());
    builder.append(",\"audit_path\":[");
    final List<byte[]> auditPath = merkle.value().auditPath();
    for (int index = 0; index < auditPath.size(); index++) {
      if (index != 0) {
        builder.append(',');
      }
      appendJsonByteArray(builder, auditPath.get(index));
    }
    builder.append("]}}}}");
  }

  private static void appendJsonByteArray(final StringBuilder builder, final byte[] bytes) {
    builder.append('[');
    for (int index = 0; index < bytes.length; index++) {
      if (index != 0) {
        builder.append(',');
      }
      builder.append(bytes[index] & 0xff);
    }
    builder.append(']');
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof ProofAttachment other)) {
      return false;
    }
    return backend.equals(other.backend)
        && Arrays.equals(proofBytes, other.proofBytes)
        && verifyingKeyRef.equals(other.verifyingKeyRef)
        && Arrays.equals(verifyingKeyCommitment, other.verifyingKeyCommitment)
        && Arrays.equals(envelopeHash, other.envelopeHash)
        && Objects.equals(lanePrivacy, other.lanePrivacy);
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        backend,
        Arrays.hashCode(proofBytes),
        verifyingKeyRef,
        Arrays.hashCode(verifyingKeyCommitment),
        Arrays.hashCode(envelopeHash),
        lanePrivacy);
  }
}
