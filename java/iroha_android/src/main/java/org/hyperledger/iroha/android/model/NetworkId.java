package org.hyperledger.iroha.android.model;

import java.util.Arrays;
import org.hyperledger.iroha.android.util.HashLiteral;

/**
 * Exact immutable identity of one Iroha network.
 *
 * <p>A network identity is the canonical checksummed literal of the 32-byte genesis-header hash.
 * Ordinary transactions always carry this value through {@code TransactionDomain::Network}; the
 * genesis-only transaction domain is intentionally not representable by this type.
 */
public final class NetworkId {
  /** Exact byte width of the genesis-header hash. */
  public static final int BYTE_LENGTH = 32;

  private final byte[] bytes;
  private final String literal;

  private NetworkId(final byte[] bytes) {
    this.bytes = bytes.clone();
    this.literal = HashLiteral.canonicalize(this.bytes);
  }

  /** Parses one exact canonical {@code hash:...#....} network identity. */
  public static NetworkId parse(final String literal) {
    final byte[] bytes;
    try {
      bytes = HashLiteral.decode(literal);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(
          "NetworkId must be an exact canonical checksummed 32-byte hash literal", ex);
    }
    if ((bytes[BYTE_LENGTH - 1] & 1) != 1
        || !HashLiteral.canonicalize(bytes).equals(literal)) {
      throw new IllegalArgumentException(
          "NetworkId must be an exact canonical checksummed 32-byte hash literal");
    }
    return new NetworkId(bytes);
  }

  /** Creates a network identity from its exact canonical 32 raw bytes. */
  public static NetworkId fromBytes(final byte[] bytes) {
    if (bytes == null || bytes.length != BYTE_LENGTH) {
      throw new IllegalArgumentException(
          "NetworkId raw value must contain exactly " + BYTE_LENGTH + " bytes");
    }
    if ((bytes[BYTE_LENGTH - 1] & 1) != 1) {
      throw new IllegalArgumentException(
          "NetworkId genesis hash marker bit must be set");
    }
    return new NetworkId(bytes);
  }

  /** Returns the exact canonical checksummed literal. */
  public String literal() {
    return literal;
  }

  /** Returns a defensive copy of the exact 32-byte identity. */
  public byte[] bytes() {
    return bytes.clone();
  }

  @Override
  public String toString() {
    return literal;
  }

  @Override
  public boolean equals(final Object other) {
    return this == other
        || other instanceof NetworkId && Arrays.equals(bytes, ((NetworkId) other).bytes);
  }

  @Override
  public int hashCode() {
    return Arrays.hashCode(bytes);
  }
}
