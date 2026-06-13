package org.hyperledger.iroha.android.privacy;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.List;
import org.hyperledger.iroha.android.crypto.Blake3;

final class ConfidentialNoteScalars {
  private static final BigInteger U128_MAX = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE);
  private static final BigInteger PASTA_MODULUS =
      new BigInteger("40000000000000000000000000000000224698fc094cf91b992d30ed00000001", 16);
  private static final BigInteger TWO = BigInteger.valueOf(2L);
  private static final BigInteger THREE = BigInteger.valueOf(3L);
  private static final BigInteger SEVEN = BigInteger.valueOf(7L);
  private static final BigInteger THIRTEEN = BigInteger.valueOf(13L);

  private ConfidentialNoteScalars() {}

  static BigInteger poseidonPair(final BigInteger lhs, final BigInteger rhs) {
    final BigInteger left = lhs.add(SEVEN).mod(PASTA_MODULUS);
    final BigInteger right = rhs.add(THIRTEEN).mod(PASTA_MODULUS);
    return TWO.multiply(pow5(left)).add(THREE.multiply(pow5(right))).mod(PASTA_MODULUS);
  }

  static BigInteger hashToScalar(final String label, final List<byte[]> parts) {
    final byte[] labelBytes = label.getBytes(StandardCharsets.UTF_8);
    long counter = 0L;
    while (true) {
      final ByteArrayOutputStream buffer = new ByteArrayOutputStream();
      buffer.write(labelBytes, 0, labelBytes.length);
      final byte[] counterBytes = leU64(counter);
      buffer.write(counterBytes, 0, counterBytes.length);
      for (final byte[] part : parts) {
        final byte[] len = leU64(part.length);
        buffer.write(len, 0, len.length);
        buffer.write(part, 0, part.length);
      }
      final BigInteger candidate = scalarFromLittleEndianOrNull(Blake3.hash(buffer.toByteArray()));
      if (candidate != null) {
        return candidate;
      }
      counter += 1L;
    }
  }

  static BigInteger scalarFromU128(final String amount) {
    return new BigInteger(canonicalU128(amount, "amount"));
  }

  static BigInteger littleEndianScalar(final byte[] bytes, final String field) {
    final BigInteger value = scalarFromLittleEndianOrNull(fixedBytes(bytes, 32, field));
    if (value == null) {
      throw new IllegalArgumentException(field + " must be a canonical Pasta scalar");
    }
    return value;
  }

  static byte[] scalarToLittleEndian(final BigInteger value) {
    byte[] bigEndian = value.mod(PASTA_MODULUS).toByteArray();
    int first = 0;
    while (first < bigEndian.length && bigEndian[first] == 0) {
      first++;
    }
    final int size = bigEndian.length - first;
    if (size > 32) {
      throw new IllegalStateException("scalar encoding overflow");
    }
    final byte[] out = new byte[32];
    for (int i = 0; i < size; i++) {
      out[i] = bigEndian[bigEndian.length - 1 - i];
    }
    return out;
  }

  static byte[] fixedScalar(final byte[] value, final String name) {
    final byte[] bytes = fixedBytes(value, 32, name);
    littleEndianScalar(bytes, name);
    return bytes;
  }

  static byte[] fixedBytes(final byte[] value, final int expected, final String name) {
    if (value == null || value.length != expected) {
      throw new IllegalArgumentException(name + " must be " + expected + " bytes");
    }
    return value.clone();
  }

  static byte[] copyNonEmpty(final byte[] value, final String name) {
    if (value == null || value.length == 0) {
      throw new IllegalArgumentException(name + " must not be empty");
    }
    return value.clone();
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
      throw new IllegalArgumentException(name + " must be canonical decimal without leading zeroes");
    }
    final BigInteger parsed = new BigInteger(text);
    if (parsed.signum() < 0 || parsed.compareTo(U128_MAX) > 0) {
      throw new IllegalArgumentException(name + " must fit in u128");
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

  private static BigInteger pow5(final BigInteger value) {
    final BigInteger square = value.multiply(value).mod(PASTA_MODULUS);
    final BigInteger fourth = square.multiply(square).mod(PASTA_MODULUS);
    return fourth.multiply(value).mod(PASTA_MODULUS);
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

  private static byte[] leU64(final long value) {
    final byte[] out = new byte[8];
    for (int i = 0; i < out.length; i++) {
      out[i] = (byte) (value >>> (8 * i));
    }
    return out;
  }

  private static void reverse(final byte[] bytes) {
    for (int left = 0, right = bytes.length - 1; left < right; left++, right--) {
      final byte tmp = bytes[left];
      bytes[left] = bytes[right];
      bytes[right] = tmp;
    }
  }
}
