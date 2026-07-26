package org.hyperledger.iroha.android.model.instructions;

import java.util.Arrays;
import java.util.Base64;
import java.util.Objects;

/** JSON-serializable proof attachment accepted by native zk transaction encoders. */
public final class ProofAttachment {
  private final String backend;
  private final byte[] proofBytes;
  private final ProofVerifierKeyRef verifyingKeyRef;
  private final byte[] verifyingKeyCommitment;
  private final byte[] envelopeHash;

  public ProofAttachment(
      final String backend,
      final byte[] proofBytes,
      final ProofVerifierKeyRef verifyingKeyRef) {
    this(backend, proofBytes, verifyingKeyRef, null, null);
  }

  public ProofAttachment(
      final String backend,
      final byte[] proofBytes,
      final ProofVerifierKeyRef verifyingKeyRef,
      final byte[] verifyingKeyCommitment,
      final byte[] envelopeHash) {
    this.backend = ZkInstructionUtils.requirePortableComponent(backend, "backend");
    this.proofBytes = ZkInstructionUtils.copyNonEmpty(proofBytes, "proofBytes");
    if (this.proofBytes.length > ZkInstructionUtils.PROOF_ATTACHMENT_MAX_BYTES) {
      throw new IllegalArgumentException(
          "proofBytes must not exceed " + ZkInstructionUtils.PROOF_ATTACHMENT_MAX_BYTES + " bytes");
    }
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

  public String toNativeJson() {
    final StringBuilder builder = new StringBuilder();
    builder.append('{');
    builder.append("\"backend\":");
    ZkInstructionUtils.appendJsonString(builder, backend);
    builder.append(",\"proof_backend\":");
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
    if (envelopeHash != null) {
      builder.append(",\"envelope_hash_hex\":");
      ZkInstructionUtils.appendJsonString(builder, ZkInstructionUtils.hexLower(envelopeHash));
    }
    builder.append('}');
    return builder.toString();
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
        && Arrays.equals(envelopeHash, other.envelopeHash);
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        backend,
        Arrays.hashCode(proofBytes),
        verifyingKeyRef,
        Arrays.hashCode(verifyingKeyCommitment),
        Arrays.hashCode(envelopeHash));
  }
}
