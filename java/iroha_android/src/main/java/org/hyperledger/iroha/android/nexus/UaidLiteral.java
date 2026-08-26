package org.hyperledger.iroha.android.nexus;

import java.util.Objects;

/** Helpers for validating exact UAID literals before issuing Torii requests. */
public final class UaidLiteral {

  private UaidLiteral() {}

  /**
   * Validates and returns the provided exact {@code uaid:<64 lowercase hex>} literal.
   *
   * @param value exact canonical UAID literal
   * @return the unchanged literal
   */
  public static String canonicalize(final String value) {
    return canonicalize(value, "uaid");
  }

  /**
   * Validates and returns the provided exact {@code uaid:<64 lowercase hex>} literal.
   *
   * @param value exact canonical UAID literal
   * @param context field description used in validation errors
   * @return the unchanged literal
   */
  public static String canonicalize(final String value, final String context) {
    Objects.requireNonNull(context, "context");
    final String literal = requireExactNonEmpty(value, context);
    if (!literal.matches("uaid:[0-9a-f]{64}")) {
      throw new IllegalArgumentException(
          context + " must be an exact canonical uaid:<64 lowercase hex> literal");
    }
    final char lastChar = literal.charAt(literal.length() - 1);
    if ("13579bdf".indexOf(lastChar) < 0) {
      throw new IllegalArgumentException(context + " must have least significant bit set to 1");
    }
    return literal;
  }

  private static String requireExactNonEmpty(final String value, final String context) {
    final String literal = Objects.requireNonNull(value, context + " must not be null");
    if (literal.trim().isEmpty()) {
      throw new IllegalArgumentException(context + " must not be blank");
    }
    if (!literal.trim().equals(literal)) {
      throw new IllegalArgumentException(context + " must not contain surrounding whitespace");
    }
    return literal;
  }
}
