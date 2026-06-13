package org.hyperledger.iroha.android.model.instructions;

import java.math.BigInteger;
import java.util.List;

final class ZkInstructionUtils {
  static final BigInteger U128_MAX = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE);
  static final int PROOF_ATTACHMENT_MAX_BYTES = 64 * 1024 * 1024;
  private static final char[] HEX = "0123456789abcdef".toCharArray();

  private ZkInstructionUtils() {}

  static String requireText(final String value, final String name) {
    if (value == null) {
      throw new IllegalArgumentException(name + " must be provided");
    }
    if (value.trim().isEmpty()) {
      throw new IllegalArgumentException(name + " must not be blank");
    }
    if (!value.trim().equals(value)) {
      throw new IllegalArgumentException(name + " must not contain surrounding whitespace");
    }
    if (value.indexOf('\0') >= 0) {
      throw new IllegalArgumentException(name + " must not contain NUL");
    }
    return value;
  }

  static String requirePortableComponent(final String value, final String name) {
    final String text = requireText(value, name);
    if (text.length() > 256) {
      throw new IllegalArgumentException(name + " must not exceed 256 characters");
    }
    for (int i = 0; i < text.length(); i++) {
      final char ch = text.charAt(i);
      if (ch < 0x21 || ch > 0x7e || ch == ':') {
        throw new IllegalArgumentException(name + " must use portable ASCII without ':'");
      }
    }
    return text;
  }

  static String optionalVerifyingKeyId(final String value, final String name) {
    if (value == null) {
      return null;
    }
    final String text = requireText(value, name);
    ProofVerifierKeyRef.fromWireId(text);
    return text;
  }

  static String canonicalU128(final String value, final String name) {
    final String text = requireText(value, name);
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
    if (parsed.compareTo(BigInteger.ZERO) < 0 || parsed.compareTo(U128_MAX) > 0) {
      throw new IllegalArgumentException(name + " must fit in u128");
    }
    return text;
  }

  static byte[] fixedBytes(final byte[] value, final int expected, final String name) {
    if (value == null) {
      throw new IllegalArgumentException(name + " must be provided");
    }
    if (value.length != expected) {
      throw new IllegalArgumentException(name + " must be exactly " + expected + " bytes");
    }
    return value.clone();
  }

  static byte[] fixedNonZeroBytes(final byte[] value, final int expected, final String name) {
    final byte[] bytes = fixedBytes(value, expected, name);
    if (isAllZero(bytes)) {
      throw new IllegalArgumentException(name + " must not be all zero");
    }
    return bytes;
  }

  static byte[] copyNonEmpty(final byte[] value, final String name) {
    if (value == null) {
      throw new IllegalArgumentException(name + " must be provided");
    }
    if (value.length == 0) {
      throw new IllegalArgumentException(name + " must not be empty");
    }
    return value.clone();
  }

  static boolean isAllZero(final byte[] bytes) {
    for (final byte b : bytes) {
      if (b != 0) {
        return false;
      }
    }
    return true;
  }

  static byte[] flattenFixed32(final List<byte[]> values) {
    final byte[] out = new byte[values.size() * 32];
    for (int i = 0; i < values.size(); i++) {
      final byte[] value = values.get(i);
      if (value.length != 32) {
        throw new IllegalArgumentException("value[" + i + "] must be exactly 32 bytes");
      }
      System.arraycopy(value, 0, out, i * 32, 32);
    }
    return out;
  }

  static String hexLower(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      final int value = b & 0xff;
      builder.append(HEX[value >>> 4]);
      builder.append(HEX[value & 0x0f]);
    }
    return builder.toString();
  }

  static void appendJsonString(final StringBuilder builder, final String value) {
    builder.append('"');
    for (int i = 0; i < value.length(); i++) {
      final char ch = value.charAt(i);
      switch (ch) {
        case '"':
          builder.append("\\\"");
          break;
        case '\\':
          builder.append("\\\\");
          break;
        case '\b':
          builder.append("\\b");
          break;
        case '\f':
          builder.append("\\f");
          break;
        case '\n':
          builder.append("\\n");
          break;
        case '\r':
          builder.append("\\r");
          break;
        case '\t':
          builder.append("\\t");
          break;
        default:
          if (ch < ' ') {
            builder.append("\\u00");
            builder.append(HEX[(ch >>> 4) & 0x0f]);
            builder.append(HEX[ch & 0x0f]);
          } else {
            builder.append(ch);
          }
      }
    }
    builder.append('"');
  }
}
