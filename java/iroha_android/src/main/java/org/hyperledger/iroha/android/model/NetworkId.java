package org.hyperledger.iroha.android.model;

import java.util.Arrays;
import org.hyperledger.iroha.android.util.HashLiteral;

/**
 * Exact immutable identity of one Iroha network.
 *
 * <p>A network identity is the canonical 64-character lowercase hexadecimal form of the 32-byte
 * genesis-header hash. Ordinary transactions always carry this value through {@code
 * TransactionDomain::Network}; the genesis-only transaction domain is intentionally not
 * representable by this type.
 */
public final class NetworkId {
  /** Exact byte width of the genesis-header hash. */
  public static final int BYTE_LENGTH = 32;
  private static final int LITERAL_LENGTH = BYTE_LENGTH * 2;
  private static final char[] LOWER_HEX = "0123456789abcdef".toCharArray();

  private final byte[] bytes;
  private final String literal;

  private NetworkId(final byte[] bytes) {
    this.bytes = bytes.clone();
    this.literal = encodeLowerHex(this.bytes);
  }

  /** Parses one exact 64-character lowercase hexadecimal network identity. */
  public static NetworkId parse(final String literal) {
    if (literal == null || literal.length() != LITERAL_LENGTH) {
      throw invalidLiteral();
    }
    final byte[] bytes = new byte[BYTE_LENGTH];
    for (int index = 0; index < BYTE_LENGTH; index++) {
      final int high = decodeLowerHex(literal.charAt(index * 2));
      final int low = decodeLowerHex(literal.charAt(index * 2 + 1));
      if (high < 0 || low < 0) {
        throw invalidLiteral();
      }
      bytes[index] = (byte) ((high << 4) | low);
    }
    if ((bytes[BYTE_LENGTH - 1] & 1) != 1) {
      throw invalidLiteral();
    }
    return new NetworkId(bytes);
  }

  /** Parses the distinct canonical checksummed representation emitted by Norito JSON. */
  public static NetworkId parseNoritoJsonLiteral(final String literal) {
    if (literal == null || !literal.matches("^hash:[0-9A-F]{64}#[0-9A-F]{4}$")) {
      throw invalidNoritoJsonLiteral();
    }
    final byte[] bytes;
    try {
      bytes = HashLiteral.decode(literal);
    } catch (final IllegalArgumentException error) {
      throw new IllegalArgumentException(
          "Norito JSON NetworkId must be one canonical checksummed uppercase hash literal",
          error);
    }
    if (!HashLiteral.canonicalize(bytes).equals(literal)) {
      throw invalidNoritoJsonLiteral();
    }
    return fromBytes(bytes);
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

  /** Returns the exact 64-character lowercase hexadecimal literal. */
  public String literal() {
    return literal;
  }

  /** Canonical checksummed representation used only by Norito JSON. */
  public String noritoJsonLiteral() {
    return HashLiteral.canonicalize(bytes);
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

  private static String encodeLowerHex(final byte[] value) {
    final char[] encoded = new char[value.length * 2];
    for (int index = 0; index < value.length; index++) {
      final int item = value[index] & 0xFF;
      encoded[index * 2] = LOWER_HEX[item >>> 4];
      encoded[index * 2 + 1] = LOWER_HEX[item & 0x0F];
    }
    return new String(encoded);
  }

  private static int decodeLowerHex(final char value) {
    if (value >= '0' && value <= '9') {
      return value - '0';
    }
    if (value >= 'a' && value <= 'f') {
      return value - 'a' + 10;
    }
    return -1;
  }

  private static IllegalArgumentException invalidLiteral() {
    return new IllegalArgumentException(
        "NetworkId must be exactly 64 lowercase hexadecimal characters with its marker bit set");
  }

  private static IllegalArgumentException invalidNoritoJsonLiteral() {
    return new IllegalArgumentException(
        "Norito JSON NetworkId must be one canonical checksummed uppercase hash literal");
  }
}
