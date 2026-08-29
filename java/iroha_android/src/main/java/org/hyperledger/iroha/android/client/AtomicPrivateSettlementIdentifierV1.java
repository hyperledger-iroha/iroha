package org.hyperledger.iroha.android.client;

import java.util.Arrays;
import java.util.Locale;
import org.hyperledger.iroha.android.util.HashLiteral;

/** Immutable 32-byte Iroha hash identity used in settlement paths and response binding. */
public final class AtomicPrivateSettlementIdentifierV1 {
  private static final int HASH_BYTES = 32;
  private final byte[] value;

  private AtomicPrivateSettlementIdentifierV1(final byte[] value) {
    if (value == null || value.length != HASH_BYTES || (value[HASH_BYTES - 1] & 1) != 1) {
      throw new IllegalArgumentException(
          "settlement identifier must contain exactly 32 marked hash bytes");
    }
    this.value = value.clone();
  }

  /** Validate and copy an exact marked Iroha hash. */
  public static AtomicPrivateSettlementIdentifierV1 fromBytes(final byte[] value) {
    return new AtomicPrivateSettlementIdentifierV1(value);
  }

  /** Parse a raw 64-digit hash or one exact canonical Norito JSON hash literal. */
  public static AtomicPrivateSettlementIdentifierV1 parse(final String input) {
    if (input == null || input.isEmpty() || !input.equals(input.trim())) {
      throw new IllegalArgumentException("settlement identifier must be exact and non-empty");
    }
    if (input.matches("^[0-9A-Fa-f]{64}$")) {
      return new AtomicPrivateSettlementIdentifierV1(decodeHex(input));
    }
    final byte[] decoded = HashLiteral.decode(input);
    if (!HashLiteral.canonicalize(decoded).equals(input)) {
      throw new IllegalArgumentException(
          "settlement identifier must use the canonical Norito JSON hash literal");
    }
    return new AtomicPrivateSettlementIdentifierV1(decoded);
  }

  /** Lowercase raw hexadecimal path component accepted by Torii. */
  public String pathComponent() {
    final StringBuilder output = new StringBuilder(HASH_BYTES * 2);
    for (final byte element : value) {
      output.append(String.format(Locale.ROOT, "%02x", element & 0xff));
    }
    return output.toString();
  }

  /** Canonical Norito JSON hash literal used in response DTOs. */
  public String jsonLiteral() {
    return HashLiteral.canonicalize(value);
  }

  /** Defensive copy of the exact identity. */
  public byte[] bytes() {
    return value.clone();
  }

  @Override
  public boolean equals(final Object other) {
    return this == other
        || other instanceof AtomicPrivateSettlementIdentifierV1 identifier
            && Arrays.equals(value, identifier.value);
  }

  @Override
  public int hashCode() {
    return Arrays.hashCode(value);
  }

  @Override
  public String toString() {
    return pathComponent();
  }

  private static byte[] decodeHex(final String input) {
    final byte[] decoded = new byte[HASH_BYTES];
    for (int index = 0; index < decoded.length; index++) {
      decoded[index] =
          (byte) Integer.parseInt(input.substring(index * 2, index * 2 + 2), 16);
    }
    return decoded;
  }
}
