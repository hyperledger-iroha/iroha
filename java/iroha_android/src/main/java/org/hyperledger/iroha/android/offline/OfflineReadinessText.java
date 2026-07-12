package org.hyperledger.iroha.android.offline;

import java.util.Objects;

/** Shared Unicode-scalar validation for first-release Offline readiness text. */
final class OfflineReadinessText {
  private OfflineReadinessText() {}

  static String requireExact(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (value.isEmpty()
        || isWhitespace(value.codePointAt(0))
        || isWhitespace(value.codePointBefore(value.length()))) {
      throw new IllegalArgumentException(field + " must be exact non-empty text");
    }
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (Character.isISOControl(character)) {
        throw new IllegalArgumentException(field + " must be exact non-empty text");
      }
      if (Character.isHighSurrogate(character)) {
        if (++index >= value.length() || !Character.isLowSurrogate(value.charAt(index))) {
          throw new IllegalArgumentException(field + " must contain well-formed Unicode");
        }
      } else if (Character.isLowSurrogate(character)) {
        throw new IllegalArgumentException(field + " must contain well-formed Unicode");
      }
    }
    return value;
  }

  static String requireBounded(
      final String value, final String field, final int maximumCodePoints) {
    final String exact = requireExact(value, field);
    if (exact.codePointCount(0, exact.length()) > maximumCodePoints) {
      throw new IllegalArgumentException(
          field + " must not exceed " + maximumCodePoints + " Unicode characters");
    }
    return exact;
  }

  private static boolean isWhitespace(final int codePoint) {
    return Character.isWhitespace(codePoint) || Character.isSpaceChar(codePoint);
  }
}
