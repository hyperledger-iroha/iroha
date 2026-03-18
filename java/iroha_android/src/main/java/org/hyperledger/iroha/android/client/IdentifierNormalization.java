package org.hyperledger.iroha.android.client;

import java.util.Locale;
import java.util.Objects;

/** Canonicalization modes advertised by `/v1/identifier-policies`. */
public enum IdentifierNormalization {
  EXACT("exact"),
  LOWERCASE_TRIMMED("lowercase_trimmed"),
  PHONE_E164("phone_e164"),
  EMAIL_ADDRESS("email_address"),
  ACCOUNT_NUMBER("account_number");

  private final String wireValue;

  IdentifierNormalization(final String wireValue) {
    this.wireValue = Objects.requireNonNull(wireValue, "wireValue");
  }

  /** Returns the JSON wire value used by Torii. */
  public String wireValue() {
    return wireValue;
  }

  /** Normalizes {@code value} using this policy's canonicalization rule. */
  public String normalize(final String value) {
    return normalize(value, "identifier");
  }

  /** Normalizes {@code value} using this policy's canonicalization rule. */
  public String normalize(final String value, final String field) {
    Objects.requireNonNull(field, "field");
    final String trimmed =
        Objects.requireNonNull(value, field + " must not be null").trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException(field + " must not be blank");
    }
    return switch (this) {
      case EXACT -> trimmed;
      case LOWERCASE_TRIMMED -> trimmed.toLowerCase(Locale.ROOT);
      case PHONE_E164 -> normalizePhone(trimmed, field);
      case EMAIL_ADDRESS -> normalizeEmail(trimmed, field);
      case ACCOUNT_NUMBER -> normalizeAccountNumber(trimmed, field);
    };
  }

  /** Parses the Torii JSON wire value into an SDK enum. */
  public static IdentifierNormalization fromWireValue(final String wireValue) {
    final String normalized =
        Objects.requireNonNull(wireValue, "wireValue").trim().toLowerCase(Locale.ROOT);
    for (final IdentifierNormalization candidate : values()) {
      if (candidate.wireValue.equals(normalized)) {
        return candidate;
      }
    }
    throw new IllegalArgumentException("Unsupported identifier normalization: " + wireValue);
  }

  private static String normalizePhone(final String value, final String field) {
    final StringBuilder compact = new StringBuilder(value.length());
    for (int i = 0; i < value.length(); i++) {
      final char c = value.charAt(i);
      if (" \t\n\r-().".indexOf(c) >= 0) {
        continue;
      }
      compact.append(c);
    }
    final String withoutPrefix;
    if (compact.length() > 0 && compact.charAt(0) == '+') {
      withoutPrefix = compact.substring(1);
    } else if (compact.length() >= 2 && compact.charAt(0) == '0' && compact.charAt(1) == '0') {
      withoutPrefix = compact.substring(2);
    } else {
      withoutPrefix = compact.toString();
    }
    if (withoutPrefix.isEmpty()) {
      throw new IllegalArgumentException(field + " must contain digits");
    }
    for (int i = 0; i < withoutPrefix.length(); i++) {
      if (!Character.isDigit(withoutPrefix.charAt(i))) {
        throw new IllegalArgumentException(
            field + " must contain digits with an optional leading '+' or '00'");
      }
    }
    return "+" + withoutPrefix;
  }

  private static String normalizeEmail(final String value, final String field) {
    final String lowered = value.toLowerCase(Locale.ROOT);
    final int atIndex = lowered.indexOf('@');
    if (atIndex <= 0 || atIndex != lowered.lastIndexOf('@') || atIndex == lowered.length() - 1) {
      throw new IllegalArgumentException(
          field + " must contain exactly one '@' with non-empty local and domain parts");
    }
    return lowered;
  }

  private static String normalizeAccountNumber(final String value, final String field) {
    final StringBuilder builder = new StringBuilder(value.length());
    for (int i = 0; i < value.length(); i++) {
      final char c = value.charAt(i);
      if (" \t\n\r-".indexOf(c) >= 0) {
        continue;
      }
      final char upper = Character.toUpperCase(c);
      if (upper > 0x7F
          || !(Character.isLetterOrDigit(upper) || upper == '_' || upper == '/' || upper == '.')) {
        throw new IllegalArgumentException(
            field + " must contain ASCII alphanumeric characters, '_', '/', or '.'");
      }
      builder.append(upper);
    }
    if (builder.length() == 0) {
      throw new IllegalArgumentException(field + " must not be blank");
    }
    return builder.toString();
  }
}
