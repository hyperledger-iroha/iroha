package org.hyperledger.iroha.android.alias;

import java.math.BigInteger;
import java.net.IDN;
import java.text.Normalizer;
import java.util.Locale;
import org.hyperledger.iroha.android.util.HashLiteral;

final class AliasNameSupport {
  static final BigInteger U64_MAX = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);

  private AliasNameSupport() {}

  static String segment(final String raw, final String field) {
    if (raw == null || raw.isEmpty()) {
      throw new IllegalArgumentException(field + " must not be empty");
    }
    if (!raw.equals(raw.trim())) {
      throw new IllegalArgumentException(field + " must not contain surrounding whitespace");
    }
    for (int index = 0; index < raw.length(); index++) {
      final char value = raw.charAt(index);
      if (Character.isWhitespace(value)
          || Character.isISOControl(value)
          || value == '@'
          || value == '#'
          || value == '$'
          || value == '.') {
        throw new IllegalArgumentException(field + " is not a valid alias name segment");
      }
    }
    final String normalized = Normalizer.normalize(raw, Normalizer.Form.NFC);
    if (normalized.codePoints().anyMatch(value -> value >= 0x1E00 && value <= 0x1EFF)) {
      throw new IllegalArgumentException(field + " is not a supported alias name segment");
    }
    final String ascii;
    try {
      ascii = IDN.toASCII(normalized, IDN.ALLOW_UNASSIGNED).toLowerCase(Locale.ROOT);
    } catch (final IllegalArgumentException exception) {
      throw new IllegalArgumentException(field + " is not a valid alias name segment", exception);
    }
    if (ascii.isEmpty() || ascii.startsWith("-") || ascii.endsWith("-")) {
      throw new IllegalArgumentException(field + " is not a valid alias name segment");
    }
    for (int index = 0; index < ascii.length(); index++) {
      final char value = ascii.charAt(index);
      if (!(value >= 'a' && value <= 'z')
          && !(value >= '0' && value <= '9')
          && value != '-'
          && value != '_') {
        throw new IllegalArgumentException(field + " is not a valid alias name segment");
      }
    }
    return ascii;
  }

  static String qualifiedDomain(final String raw) {
    if (raw == null || !raw.equals(raw.trim())) {
      throw new IllegalArgumentException("canonicalName must not contain surrounding whitespace");
    }
    final int dot = raw.indexOf('.');
    if (dot <= 0 || dot != raw.lastIndexOf('.') || dot >= raw.length() - 1) {
      throw new IllegalArgumentException("canonicalName must use domain.dataspace format");
    }
    return segment(raw.substring(0, dot), "domain")
        + "."
        + segment(raw.substring(dot + 1), "dataspace");
  }

  static BigInteger requireU64(final BigInteger value, final String field) {
    if (value == null || value.signum() < 0 || value.compareTo(U64_MAX) > 0) {
      throw new IllegalArgumentException(field + " must be an unsigned 64-bit integer");
    }
    return value;
  }

  static long requireNonNegative(final long value, final String field) {
    if (value < 0) {
      throw new IllegalArgumentException(field + " must not be negative");
    }
    return value;
  }

  static String requireToken(final String value, final String field) {
    if (value == null || value.trim().isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(
          field + " must be non-blank without surrounding whitespace");
    }
    for (int index = 0; index < value.length(); index++) {
      if (Character.isWhitespace(value.charAt(index))
          || Character.isISOControl(value.charAt(index))) {
        throw new IllegalArgumentException(
            field + " must not contain whitespace or control characters");
      }
    }
    return value;
  }

  static String requireHash(final String value, final String field) {
    if (decodeHash(value) == null) {
      throw new IllegalArgumentException(field + " must be a canonical 32-byte hash");
    }
    return value;
  }

  static byte[] decodeHash(final String value) {
    if (value == null) return null;
    if (value.regionMatches(true, 0, "hash:", 0, 5)) {
      try {
        return HashLiteral.decode(value);
      } catch (final IllegalArgumentException ignored) {
        return null;
      }
    }
    String raw = value;
    if (raw.startsWith("0x")) raw = raw.substring(2);
    if (raw.startsWith("blake2b:")) raw = raw.substring(8);
    if (raw.length() != 64) return null;
    final byte[] result = new byte[32];
    for (int index = 0; index < result.length; index++) {
      final int high = Character.digit(raw.charAt(index * 2), 16);
      final int low = Character.digit(raw.charAt(index * 2 + 1), 16);
      if (high < 0 || low < 0) return null;
      result[index] = (byte) ((high << 4) | low);
    }
    return result;
  }
}
