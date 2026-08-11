package org.hyperledger.iroha.android.privacy;

import java.math.BigInteger;

final class ConfidentialNoteScalars {
  private static final BigInteger U128_MAX = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE);
  private static final BigInteger PASTA_MODULUS =
      new BigInteger("40000000000000000000000000000000224698fc094cf91b992d30ed00000001", 16);

  private ConfidentialNoteScalars() {}

  static BigInteger littleEndianScalar(final byte[] bytes, final String field) {
    final BigInteger value = scalarFromLittleEndianOrNull(fixedBytes(bytes, 32, field));
    if (value == null) {
      throw new IllegalArgumentException(field + " must be a canonical Pasta scalar");
    }
    return value;
  }

  static byte[] fixedScalar(final byte[] value, final String name) {
    final byte[] bytes = fixedBytes(value, 32, name);
    if (littleEndianScalar(bytes, name).signum() == 0) {
      throw new IllegalArgumentException(name + " must be non-zero");
    }
    return bytes;
  }

  static byte[] fixedBytes(final byte[] value, final int expected, final String name) {
    if (value == null || value.length != expected) {
      throw new IllegalArgumentException(name + " must be " + expected + " bytes");
    }
    return value.clone();
  }

  static byte[] fixedNonZeroBytes(final byte[] value, final int expected, final String name) {
    final byte[] bytes = fixedBytes(value, expected, name);
    boolean nonZero = false;
    for (final byte item : bytes) {
      nonZero |= item != 0;
    }
    if (!nonZero) {
      throw new IllegalArgumentException(name + " must be non-zero");
    }
    return bytes;
  }

  static String canonicalU128(final String value, final String name) {
    final String text = canonicalText(value, name);
    for (int i = 0; i < text.length(); i++) {
      final char ch = text.charAt(i);
      if (ch < '0' || ch > '9') {
        throw new IllegalArgumentException(name + " must be an unsigned decimal integer");
      }
    }
    if (text.length() > 1 && text.charAt(0) == '0') {
      throw new IllegalArgumentException(
          name + " must be canonical decimal without leading zeroes");
    }
    final BigInteger parsed = new BigInteger(text);
    if (parsed.signum() <= 0 || parsed.compareTo(U128_MAX) > 0) {
      throw new IllegalArgumentException(name + " must be a positive u128");
    }
    return text;
  }

  static String canonicalText(final String value, final String name) {
    if (value == null) {
      throw new IllegalArgumentException(name + " must be provided");
    }
    final String trimmed = value.trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException(name + " must not be blank");
    }
    if (!trimmed.equals(value)) {
      throw new IllegalArgumentException(name + " must not contain surrounding whitespace");
    }
    if (trimmed.indexOf('\0') >= 0) {
      throw new IllegalArgumentException(name + " must not contain NUL");
    }
    return trimmed;
  }

  private static BigInteger scalarFromLittleEndianOrNull(final byte[] bytes) {
    if (bytes == null || bytes.length != 32) {
      return null;
    }
    final byte[] bigEndian = bytes.clone();
    reverse(bigEndian);
    final BigInteger value = new BigInteger(1, bigEndian);
    return value.compareTo(PASTA_MODULUS) < 0 ? value : null;
  }

  private static void reverse(final byte[] bytes) {
    for (int left = 0, right = bytes.length - 1; left < right; left++, right--) {
      final byte tmp = bytes[left];
      bytes[left] = bytes[right];
      bytes[right] = tmp;
    }
  }
}
