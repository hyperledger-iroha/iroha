package org.hyperledger.iroha.android.sorafs;

import java.util.Base64;
import java.util.Locale;

final class SorafsInputValidator {

  private SorafsInputValidator() {}

  static String requireNonEmpty(final String value, final String field) {
    if (value == null || value.trim().isEmpty()) {
      throw new IllegalArgumentException(field + " must not be empty");
    }
    return value.trim();
  }

  static String normalizeHex(final String value, final String field) {
    final String trimmed = requireNonEmpty(value, field);
    final String normalized = stripHexPrefix(trimmed);
    if (normalized.isEmpty()) {
      throw new IllegalArgumentException(field + " must be a non-empty hex string");
    }
    if ((normalized.length() & 1) == 1) {
      throw new IllegalArgumentException(field + " must contain an even number of hex characters");
    }
    for (int i = 0; i < normalized.length(); i++) {
      final char c = normalized.charAt(i);
      if (!isHexDigit(c)) {
        throw new IllegalArgumentException(field + " must be a hex string");
      }
    }
    return normalized.toLowerCase(Locale.ROOT);
  }

  static String normalizeHexBytes(final String value, final String field, final int expectedBytes) {
    if (expectedBytes <= 0) {
      throw new IllegalArgumentException("expectedBytes must be positive");
    }
    final String normalized = normalizeHex(value, field);
    final int expectedLength = expectedBytes * 2;
    if (normalized.length() != expectedLength) {
      throw new IllegalArgumentException(
          field + " must be a " + expectedBytes + "-byte hex string");
    }
    return normalized;
  }

  static String normalizeBase64MaybeUrl(final String value, final String field) {
    final String compact = requireNonEmpty(value, field).replaceAll("\\s+", "");
    if (compact.isEmpty()) {
      throw new IllegalArgumentException(field + " must be a non-empty base64 string");
    }
    String normalized = compact;
    final boolean hadUrlChars =
        normalized.indexOf('-') >= 0 || normalized.indexOf('_') >= 0;
    if (hadUrlChars) {
      normalized = normalized.replace('-', '+').replace('_', '/');
    }
    final String padded = padBase64(normalized, field, hadUrlChars);
    final String errorLabel =
        field + " must be a valid base64" + (hadUrlChars ? " or base64url" : "") + " string";
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(padded);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(errorLabel, ex);
    }
    if (decoded.length == 0) {
      throw new IllegalArgumentException(field + " must be a non-empty base64 string");
    }
    return Base64.getEncoder().encodeToString(decoded);
  }

  private static String stripHexPrefix(final String value) {
    if (value.startsWith("0x") || value.startsWith("0X")) {
      return value.substring(2);
    }
    return value;
  }

  private static boolean isHexDigit(final char c) {
    return (c >= '0' && c <= '9')
        || (c >= 'a' && c <= 'f')
        || (c >= 'A' && c <= 'F');
  }

  private static String padBase64(
      final String value, final String field, final boolean allowUrlLabel) {
    final int paddingIndex = value.indexOf('=');
    if (paddingIndex >= 0) {
      for (int i = paddingIndex; i < value.length(); i++) {
        if (value.charAt(i) != '=') {
          throw new IllegalArgumentException(labelForBase64(field, allowUrlLabel));
        }
      }
      final int paddingCount = value.length() - paddingIndex;
      if (paddingCount > 2 || value.length() % 4 != 0) {
        throw new IllegalArgumentException(labelForBase64(field, allowUrlLabel));
      }
      return value;
    }
    final int mod = value.length() % 4;
    if (mod == 1) {
      throw new IllegalArgumentException(labelForBase64(field, allowUrlLabel));
    }
    if (mod == 0) {
      return value;
    }
    return value + "===".substring(0, 4 - mod);
  }

  private static String labelForBase64(final String field, final boolean allowUrlLabel) {
    return field + " must be a valid base64" + (allowUrlLabel ? " or base64url" : "") + " string";
  }
}
