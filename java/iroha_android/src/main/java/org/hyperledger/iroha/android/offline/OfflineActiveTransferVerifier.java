package org.hyperledger.iroha.android.offline;

import java.math.BigInteger;
import java.util.Objects;

/** Key-material-free transfer verifier active at a readiness snapshot. */
public final class OfflineActiveTransferVerifier {
  private static final long U32_MAX = 0xffff_ffffL;
  private static final BigInteger U64_MAX = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);

  private final OfflineVerifierId id;
  private final long version;
  private final String circuitId;
  private final String commitment;
  private final String publicInputsSchemaHash;
  private final long maxProofBytes;
  private final BigInteger activationHeight;
  private final BigInteger withdrawalHeight;

  public OfflineActiveTransferVerifier(
      final OfflineVerifierId id,
      final long version,
      final String circuitId,
      final String commitment,
      final String publicInputsSchemaHash,
      final long maxProofBytes,
      final BigInteger activationHeight,
      final BigInteger withdrawalHeight) {
    this.id = Objects.requireNonNull(id, "id");
    if (version < 0 || version > U32_MAX) {
      throw new IllegalArgumentException("version must fit in an unsigned 32-bit integer");
    }
    this.version = version;
    this.circuitId = OfflineReadinessText.requireExact(circuitId, "circuitId");
    this.commitment = requireLowercaseHash(commitment, "commitment");
    this.publicInputsSchemaHash =
        requireLowercaseHash(publicInputsSchemaHash, "publicInputsSchemaHash");
    if (maxProofBytes <= 0 || maxProofBytes > U32_MAX) {
      throw new IllegalArgumentException(
          "maxProofBytes must fit in a positive unsigned 32-bit integer");
    }
    this.maxProofBytes = maxProofBytes;
    this.activationHeight = requireU64(activationHeight, "activationHeight");
    if (withdrawalHeight != null) {
      requireU64(withdrawalHeight, "withdrawalHeight");
      if (withdrawalHeight.signum() == 0
          || withdrawalHeight.compareTo(this.activationHeight) <= 0) {
        throw new IllegalArgumentException(
            "withdrawalHeight must be greater than activationHeight");
      }
    }
    this.withdrawalHeight = withdrawalHeight;
  }

  public OfflineVerifierId id() {
    return id;
  }

  public long version() {
    return version;
  }

  public String circuitId() {
    return circuitId;
  }

  public String commitment() {
    return commitment;
  }

  public String publicInputsSchemaHash() {
    return publicInputsSchemaHash;
  }

  public long maxProofBytes() {
    return maxProofBytes;
  }

  public BigInteger activationHeight() {
    return activationHeight;
  }

  public BigInteger withdrawalHeight() {
    return withdrawalHeight;
  }

  public boolean isActiveAt(final BigInteger height) {
    Objects.requireNonNull(height, "height");
    return activationHeight.compareTo(height) <= 0
        && (withdrawalHeight == null || height.compareTo(withdrawalHeight) < 0);
  }

  @Override
  public boolean equals(final Object other) {
    if (this == other) {
      return true;
    }
    if (!(other instanceof OfflineActiveTransferVerifier)) {
      return false;
    }
    final OfflineActiveTransferVerifier that = (OfflineActiveTransferVerifier) other;
    return version == that.version
        && maxProofBytes == that.maxProofBytes
        && id.equals(that.id)
        && circuitId.equals(that.circuitId)
        && commitment.equals(that.commitment)
        && publicInputsSchemaHash.equals(that.publicInputsSchemaHash)
        && activationHeight.equals(that.activationHeight)
        && Objects.equals(withdrawalHeight, that.withdrawalHeight);
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        id,
        version,
        circuitId,
        commitment,
        publicInputsSchemaHash,
        maxProofBytes,
        activationHeight,
        withdrawalHeight);
  }

  private static BigInteger requireU64(final BigInteger value, final String field) {
    Objects.requireNonNull(value, field);
    if (value.signum() < 0 || value.compareTo(U64_MAX) > 0) {
      throw new IllegalArgumentException(field + " must fit in an unsigned 64-bit integer");
    }
    return value;
  }

  private static String requireLowercaseHash(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (value.length() != 64) {
      throw new IllegalArgumentException(field + " must be exact lowercase 32-byte hexadecimal");
    }
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (!((character >= '0' && character <= '9')
          || (character >= 'a' && character <= 'f'))) {
        throw new IllegalArgumentException(
            field + " must be exact lowercase 32-byte hexadecimal");
      }
    }
    return value;
  }
}
