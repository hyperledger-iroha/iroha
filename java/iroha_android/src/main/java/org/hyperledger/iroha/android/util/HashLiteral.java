package org.hyperledger.iroha.android.util;

import java.nio.charset.StandardCharsets;
import java.util.Locale;

/** Canonicalises and validates Norito hash literals (`hash:...#....`). */
public final class HashLiteral {

  private static final String TAG = "hash";
  private static final int HASH_BYTES = 32;
  private static final int BODY_HEX_LENGTH = 64;
  private static final int CHECKSUM_HEX_LENGTH = 4;

  private HashLiteral() {}

  /** Returns the canonical literal for the provided 32-byte hash. */
  public static String canonicalize(final byte[] bytes) {
    if (bytes == null) {
      throw new IllegalArgumentException("hash value must not be null");
    }
    if (bytes.length != HASH_BYTES) {
      throw new IllegalArgumentException("hash literal requires exactly 32 bytes");
    }
    final byte[] normalized = bytes.clone();
    normalized[normalized.length - 1] |= 0x01;
    final StringBuilder builder = new StringBuilder(BODY_HEX_LENGTH);
    for (final byte value : normalized) {
      builder.append(String.format(Locale.ROOT, "%02X", value));
    }
    final String body = builder.toString();
    final int checksum = crc16(TAG, body);
    return literalFor(body, checksum);
  }

  /** Canonicalises either a hex digest or an existing literal. */
  public static String canonicalize(final String value) {
    if (value == null) {
      throw new IllegalArgumentException("hash literal must not be null");
    }
    final String trimmed = value.trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException("hash literal must not be empty");
    }
    if (startsWithIgnoreCase(trimmed, TAG + ":")) {
      validate(trimmed);
      return trimmed;
    }
    if (!trimmed.matches("^[0-9A-Fa-f]{" + BODY_HEX_LENGTH + "}$")) {
      throw new IllegalArgumentException(
          "hash literal must be a 64-character hexadecimal string or canonical hash literal");
    }
    return canonicalize(hexToBytes(trimmed));
  }

  public static String canonicalizeOptional(final String value) {
    if (value == null || value.trim().isEmpty()) {
      return null;
    }
    return canonicalize(value);
  }

  /** Returns the decoded bytes after validating the literal. */
  public static byte[] decode(final String literal) {
    final String canonical = canonicalize(literal);
    final int separator = canonical.lastIndexOf('#');
    final String body = canonical.substring(TAG.length() + 1, separator);
    return hexToBytes(body);
  }

  private static void validate(final String literal) {
    final String trimmed = literal.trim();
    if (!startsWithIgnoreCase(trimmed, TAG + ":")) {
      throw new IllegalArgumentException("hash literal must start with 'hash:'");
    }
    final int separator = trimmed.lastIndexOf('#');
    if (separator < 0) {
      throw new IllegalArgumentException("hash literal must include checksum suffix");
    }
    final String body = trimmed.substring(TAG.length() + 1, separator);
    final String checksum = trimmed.substring(separator + 1);
    if (!body.matches("^[0-9A-Fa-f]{" + BODY_HEX_LENGTH + "}$")) {
      throw new IllegalArgumentException("hash literal must contain 64 hexadecimal digits");
    }
    if (!checksum.matches("^[0-9A-Fa-f]{" + CHECKSUM_HEX_LENGTH + "}$")) {
      throw new IllegalArgumentException("hash literal checksum must contain four hexadecimal digits");
    }
    final int expected = crc16(TAG, body.toUpperCase(Locale.ROOT));
    final int provided = Integer.parseInt(checksum, 16);
    if (expected != provided) {
      throw new IllegalArgumentException(
          "hash literal checksum mismatch; expected " + String.format(Locale.ROOT, "%04X", expected));
    }
  }

  private static String literalFor(final String body, final int checksum) {
    return TAG
        + ":"
        + body
        + "#"
        + String.format(Locale.ROOT, "%0" + CHECKSUM_HEX_LENGTH + "X", checksum & 0xFFFF);
  }

  private static byte[] hexToBytes(final String hex) {
    final byte[] bytes = new byte[HASH_BYTES];
    for (int index = 0; index < bytes.length; index++) {
      final int offset = index * 2;
      bytes[index] = (byte) Integer.parseInt(hex.substring(offset, offset + 2), 16);
    }
    return bytes;
  }

  private static boolean startsWithIgnoreCase(final String value, final String prefix) {
    return value.regionMatches(true, 0, prefix, 0, prefix.length());
  }

  private static int crc16(final String tag, final String body) {
    int crc = 0xFFFF;
    for (final byte value : tag.getBytes(StandardCharsets.UTF_8)) {
      crc = updateCrc(crc, value);
    }
    crc = updateCrc(crc, (byte) ':');
    for (final byte value : body.getBytes(StandardCharsets.UTF_8)) {
      crc = updateCrc(crc, value);
    }
    return crc & 0xFFFF;
  }

  private static int updateCrc(int crc, final byte value) {
    crc ^= (value & 0xFF) << 8;
    for (int bit = 0; bit < 8; bit++) {
      if ((crc & 0x8000) != 0) {
        crc = ((crc << 1) ^ 0x1021) & 0xFFFF;
      } else {
        crc = (crc << 1) & 0xFFFF;
      }
    }
    return crc;
  }
}
