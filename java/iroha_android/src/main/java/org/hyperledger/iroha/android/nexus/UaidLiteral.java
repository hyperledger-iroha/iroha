package org.hyperledger.iroha.android.nexus;

import java.util.Locale;
import java.util.Objects;

/** Helpers for canonicalizing exact UAID literals before issuing Torii requests. */
public final class UaidLiteral {

  private UaidLiteral() {}

  /**
   * Canonicalizes the provided exact UAID literal and returns the canonical `uaid:<hex>` form.
   *
   * @param value raw UAID literal (with or without the {@code uaid:} prefix)
   * @return canonical literal
   */
  public static String canonicalize(final String value) {
    return canonicalize(value, "uaid");
  }

  /**
   * Canonicalizes the provided exact UAID literal and returns the canonical `uaid:<hex>` form.
   *
   * @param value raw UAID literal (with or without the {@code uaid:} prefix)
   * @param context field description used in validation errors
   * @return canonical literal
   */
  public static String canonicalize(final String value, final String context) {
    Objects.requireNonNull(context, "context");
    final String literal = requireExactNonEmpty(value, context);
    final String lower = literal.toLowerCase(Locale.ROOT);
    final String hexPortion =
        lower.startsWith("uaid:") ? literal.substring("uaid:".length()) : literal;
    if (!hexPortion.trim().equals(hexPortion)) {
      throw new IllegalArgumentException(context + " must not contain surrounding whitespace");
    }
    if (hexPortion.length() != 64 || !hexPortion.matches("(?i)[0-9a-f]{64}")) {
      throw new IllegalArgumentException(context + " must contain 64 hex characters");
    }
    final char lastChar = hexPortion.charAt(hexPortion.length() - 1);
    if ("13579bdf".indexOf(Character.toLowerCase(lastChar)) < 0) {
      throw new IllegalArgumentException(context + " must have least significant bit set to 1");
    }
    return "uaid:" + hexPortion.toLowerCase(Locale.ROOT);
  }

  private static String requireExactNonEmpty(final String value, final String context) {
    final String literal = Objects.requireNonNull(value, context + " must not be null");
    if (literal.isBlank()) {
      throw new IllegalArgumentException(context + " must not be blank");
    }
    if (!literal.trim().equals(literal)) {
      throw new IllegalArgumentException(context + " must not contain surrounding whitespace");
    }
    return literal;
  }
}
