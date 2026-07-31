package org.hyperledger.iroha.android.privacy;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Objects;

/** One byte-complete row of the canonical first-release exact-12 fixture. */
public final class PrivacyExact12TypedFixtureRowV1 {
  private final PrivacyNativeBridge.ProtocolIdV1 protocolId;
  private final byte[] statementNorito;
  private final byte[] envelopeNorito;
  private final String submitProofWireId;
  private final byte[] submitProofInstructionNorito;
  private final byte[] transactionIntentProjectionNorito;
  private final byte[] transactionIntentDigest;
  private final byte[] unsignedTransactionPayloadNorito;
  private final byte[] signedTransactionVersionedNorito;
  private final byte[] signedTransactionHash;

  public PrivacyExact12TypedFixtureRowV1(
      final PrivacyNativeBridge.ProtocolIdV1 protocolId,
      final byte[] statementNorito,
      final byte[] envelopeNorito,
      final String submitProofWireId,
      final byte[] submitProofInstructionNorito,
      final byte[] transactionIntentProjectionNorito,
      final byte[] transactionIntentDigest,
      final byte[] unsignedTransactionPayloadNorito,
      final byte[] signedTransactionVersionedNorito,
      final byte[] signedTransactionHash) {
    this.protocolId = Objects.requireNonNull(protocolId, "protocolId");
    this.statementNorito =
        boundedBytes(
            statementNorito, PrivacyExact12FixtureCodecV1.MAX_STATEMENT_BYTES, "statementNorito");
    this.envelopeNorito =
        boundedBytes(
            envelopeNorito, PrivacyExact12FixtureCodecV1.MAX_ENVELOPE_BYTES, "envelopeNorito");
    this.submitProofWireId = Objects.requireNonNull(submitProofWireId, "submitProofWireId");
    if (!PrivacyExact12FixtureCodecV1.SUBMIT_PROOF_WIRE_ID.equals(submitProofWireId)) {
      throw new IllegalArgumentException(
          "submitProofWireId must be the canonical first-release wire id");
    }
    this.submitProofInstructionNorito =
        boundedBytes(
            submitProofInstructionNorito,
            PrivacyExact12FixtureCodecV1.MAX_INSTRUCTION_BYTES,
            "submitProofInstructionNorito");
    this.transactionIntentProjectionNorito =
        boundedBytes(
            transactionIntentProjectionNorito,
            PrivacyExact12FixtureCodecV1.MAX_INTENT_PROJECTION_BYTES,
            "transactionIntentProjectionNorito");
    this.transactionIntentDigest =
        fixedBytes(
            transactionIntentDigest,
            PrivacyExact12FixtureCodecV1.HASH_BYTES,
            "transactionIntentDigest");
    this.unsignedTransactionPayloadNorito =
        boundedBytes(
            unsignedTransactionPayloadNorito,
            PrivacyExact12FixtureCodecV1.MAX_UNSIGNED_TRANSACTION_BYTES,
            "unsignedTransactionPayloadNorito");
    this.signedTransactionVersionedNorito =
        boundedBytes(
            signedTransactionVersionedNorito,
            PrivacyExact12FixtureCodecV1.MAX_SIGNED_TRANSACTION_BYTES,
            "signedTransactionVersionedNorito");
    this.signedTransactionHash =
        fixedBytes(
            signedTransactionHash,
            PrivacyExact12FixtureCodecV1.HASH_BYTES,
            "signedTransactionHash");
    if (nestedByteCount() > PrivacyExact12FixtureCodecV1.MAX_AGGREGATE_NESTED_BYTES) {
      throw new IllegalArgumentException("exact-12 row exceeds the aggregate nested-byte limit");
    }
  }

  public PrivacyNativeBridge.ProtocolIdV1 protocolId() {
    return protocolId;
  }

  public byte[] statementNorito() {
    return statementNorito.clone();
  }

  public byte[] envelopeNorito() {
    return envelopeNorito.clone();
  }

  public String submitProofWireId() {
    return submitProofWireId;
  }

  public byte[] submitProofInstructionNorito() {
    return submitProofInstructionNorito.clone();
  }

  public byte[] transactionIntentProjectionNorito() {
    return transactionIntentProjectionNorito.clone();
  }

  public byte[] transactionIntentDigest() {
    return transactionIntentDigest.clone();
  }

  public byte[] unsignedTransactionPayloadNorito() {
    return unsignedTransactionPayloadNorito.clone();
  }

  public byte[] signedTransactionVersionedNorito() {
    return signedTransactionVersionedNorito.clone();
  }

  public byte[] signedTransactionHash() {
    return signedTransactionHash.clone();
  }

  long nestedByteCount() {
    long total = submitProofWireId.getBytes(StandardCharsets.UTF_8).length;
    for (final byte[] bytes :
        new byte[][] {
          statementNorito,
          envelopeNorito,
          submitProofInstructionNorito,
          transactionIntentProjectionNorito,
          transactionIntentDigest,
          unsignedTransactionPayloadNorito,
          signedTransactionVersionedNorito,
          signedTransactionHash
        }) {
      total = Math.addExact(total, bytes.length);
    }
    return total;
  }

  @Override
  public boolean equals(final Object object) {
    if (this == object) {
      return true;
    }
    if (!(object instanceof PrivacyExact12TypedFixtureRowV1 other)) {
      return false;
    }
    return protocolId == other.protocolId
        && submitProofWireId.equals(other.submitProofWireId)
        && Arrays.equals(statementNorito, other.statementNorito)
        && Arrays.equals(envelopeNorito, other.envelopeNorito)
        && Arrays.equals(submitProofInstructionNorito, other.submitProofInstructionNorito)
        && Arrays.equals(
            transactionIntentProjectionNorito, other.transactionIntentProjectionNorito)
        && Arrays.equals(transactionIntentDigest, other.transactionIntentDigest)
        && Arrays.equals(unsignedTransactionPayloadNorito, other.unsignedTransactionPayloadNorito)
        && Arrays.equals(
            signedTransactionVersionedNorito, other.signedTransactionVersionedNorito)
        && Arrays.equals(signedTransactionHash, other.signedTransactionHash);
  }

  @Override
  public int hashCode() {
    int result = protocolId.hashCode();
    result = 31 * result + Arrays.hashCode(statementNorito);
    result = 31 * result + Arrays.hashCode(envelopeNorito);
    result = 31 * result + submitProofWireId.hashCode();
    result = 31 * result + Arrays.hashCode(submitProofInstructionNorito);
    result = 31 * result + Arrays.hashCode(transactionIntentProjectionNorito);
    result = 31 * result + Arrays.hashCode(transactionIntentDigest);
    result = 31 * result + Arrays.hashCode(unsignedTransactionPayloadNorito);
    result = 31 * result + Arrays.hashCode(signedTransactionVersionedNorito);
    result = 31 * result + Arrays.hashCode(signedTransactionHash);
    return result;
  }

  private static byte[] boundedBytes(
      final byte[] value, final int maximum, final String name) {
    if (value == null) {
      throw new IllegalArgumentException(name + " must be provided");
    }
    if (value.length == 0) {
      throw new IllegalArgumentException(name + " must not be empty");
    }
    if (value.length > maximum) {
      throw new IllegalArgumentException(name + " must not exceed " + maximum + " bytes");
    }
    return value.clone();
  }

  private static byte[] fixedBytes(
      final byte[] value, final int expected, final String name) {
    if (value == null || value.length != expected) {
      throw new IllegalArgumentException(name + " must contain exactly " + expected + " bytes");
    }
    return value.clone();
  }
}
