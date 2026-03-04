package org.hyperledger.iroha.android.nexus;

import java.util.Locale;
import java.util.Objects;

/** Helpers for normalising UAID literals before issuing Torii requests. */
public final class UaidLiteral {

  private UaidLiteral() {}

  /**
   * Normalises the provided UAID literal and returns the canonical `uaid:<hex>` form.
   *
   * @param value raw UAID literal (with or without the {@code uaid:} prefix)
   * @return canonical literal
   */
  public static String canonicalize(final String value) {
    return canonicalize(value, "uaid");
  }

  /**
   * Normalises the provided UAID literal and returns the canonical `uaid:<hex>` form.
   *
   * @param value raw UAID literal (with or without the {@code uaid:} prefix)
   * @param context field description used in validation errors
   * @return canonical literal
   */
  public static String canonicalize(final String value, final String context) {
    Objects.requireNonNull(context, "context");
    final String literal =
        Objects.requireNonNull(value, context + " must not be null").trim();
    if (literal.isEmpty()) {
      throw new IllegalArgumentException(context + " must not be blank");
    }
    final String lower = literal.toLowerCase(Locale.ROOT);
    final String hexPortion =
        lower.startsWith("uaid:") ? literal.substring("uaid:".length()) : literal;
    final String trimmedHex = hexPortion.trim();
    if (trimmedHex.length() != 64 || !trimmedHex.matches("(?i)[0-9a-f]{64}")) {
      throw new IllegalArgumentException(context + " must contain 64 hex characters");
    }
    final char lastChar = trimmedHex.charAt(trimmedHex.length() - 1);
    if ("13579bdf".indexOf(Character.toLowerCase(lastChar)) < 0) {
      throw new IllegalArgumentException(context + " must have least significant bit set to 1");
    }
    return "uaid:" + trimmedHex.toLowerCase(Locale.ROOT);
  }
}
