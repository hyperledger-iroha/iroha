package org.hyperledger.iroha.android.offline;

final class OfflineHashLiteral {

  private static final int BODY_LENGTH = 64;
  private static final int CHECKSUM_LENGTH = 4;

  private OfflineHashLiteral() {}

  static String normalize(final String value, final String context) {
    final String hex = parseHex(value, context);
    return format(hex);
  }

  static String parseHex(final String value, final String context) {
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException(context + " must be a non-empty hash literal or hex");
    }
    final String trimmed = value.trim();
    if (trimmed.toLowerCase().startsWith("hash:")) {
      return parseHashLiteral(trimmed, context);
    }
    return normalizeHex(trimmed, context);
  }

  private static String parseHashLiteral(final String literal, final String context) {
    final int separator = literal.lastIndexOf('#');
    if (separator < 0) {
      throw new IllegalArgumentException(context + " missing hash literal checksum");
    }
    final String body = literal.substring(5, separator);
    final String checksum = literal.substring(separator + 1);
    if (body.length() != BODY_LENGTH || !isHex(body)) {
      throw new IllegalArgumentException(context + " hash literal has invalid body");
    }
    if (checksum.length() != CHECKSUM_LENGTH || !isHex(checksum)) {
      throw new IllegalArgumentException(context + " hash literal has invalid checksum");
    }
    final String expected = crc16("hash", body.toUpperCase());
    if (!expected.equalsIgnoreCase(checksum)) {
      throw new IllegalArgumentException(
          context + " hash literal checksum mismatch (expected " + expected + ")");
    }
    return body.toLowerCase();
  }

  private static String normalizeHex(final String value, final String context) {
    final String hex = value.startsWith("0x") || value.startsWith("0X") ? value.substring(2) : value;
    if (hex.length() != BODY_LENGTH || !isHex(hex)) {
      throw new IllegalArgumentException(context + " must be a 32-byte hex string");
    }
    return hex.toLowerCase();
  }

  private static boolean isHex(final String value) {
    for (int i = 0; i < value.length(); i++) {
      final char c = value.charAt(i);
      final boolean isDigit = c >= '0' && c <= '9';
      final boolean isLower = c >= 'a' && c <= 'f';
      final boolean isUpper = c >= 'A' && c <= 'F';
      if (!(isDigit || isLower || isUpper)) {
        return false;
      }
    }
    return true;
  }

  private static String format(final String hex) {
    final String upper = hex.toUpperCase();
    final String checksum = crc16("hash", upper);
    return "hash:" + upper + "#" + checksum;
  }

  private static String crc16(final String tag, final String body) {
    int crc = 0xffff;
    crc = updateCrc(crc, tag);
    crc = updateCrc(crc, ":");
    crc = updateCrc(crc, body);
    return String.format("%04X", crc & 0xffff);
  }

  private static int updateCrc(final int crc, final String text) {
    int next = crc;
    for (int i = 0; i < text.length(); i++) {
      next = updateCrc(next, (byte) text.charAt(i));
    }
    return next;
  }

  private static int updateCrc(final int crc, final byte value) {
    int current = crc ^ (value & 0xff) << 8;
    for (int i = 0; i < 8; i++) {
      if ((current & 0x8000) != 0) {
        current = (current << 1) ^ 0x1021;
      } else {
        current <<= 1;
      }
    }
    return current & 0xffff;
  }
}
