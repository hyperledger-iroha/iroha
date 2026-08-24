package org.hyperledger.iroha.android.model.instructions;

import java.util.Locale;
import java.util.regex.Pattern;

/** Helper utilities shared across governance instruction builders. */
public final class GovernanceInstructionUtils {

  private static final Pattern HEX_PATTERN = Pattern.compile("^[0-9a-fA-F]+$");
  private static final int GOVERNANCE_SELECTOR_V1_MAX_LENGTH = 128;
  private static final String GOVERNANCE_SELECTOR_V1_PATTERN =
      "^[A-Za-z0-9_~-][A-Za-z0-9._~-]{0,127}$";

  private GovernanceInstructionUtils() {}

  static String requireHex(
      final String value, final String fieldName, final int expectedBytes) {
    if (value == null || value.trim().isEmpty()) {
      throw new IllegalArgumentException(fieldName + " must not be blank");
    }
    String normalized = value.startsWith("0x") ? value.substring(2) : value;
    if (!HEX_PATTERN.matcher(normalized).matches()) {
      throw new IllegalArgumentException(fieldName + " must be hexadecimal: " + value);
    }
    if (expectedBytes > 0 && normalized.length() != expectedBytes * 2) {
      throw new IllegalArgumentException(
          fieldName
              + " must be "
              + (expectedBytes * 2)
              + " hex chars, found "
              + normalized.length());
    }
    return normalized.toLowerCase(Locale.ROOT);
  }

  /** Requires one canonical first-release governance selector without normalizing it. */
  static String requireGovernanceSelectorV1(final String value, final String fieldName) {
    if (value == null
        || value.length() == 0
        || value.length() > GOVERNANCE_SELECTOR_V1_MAX_LENGTH
        || value.charAt(0) == '.') {
      throw invalidGovernanceSelector(fieldName);
    }
    for (int index = 0; index < value.length(); index++) {
      if (!isGovernanceSelectorUnreservedAscii(value.charAt(index))) {
        throw invalidGovernanceSelector(fieldName);
      }
    }
    return value;
  }

  private static boolean isGovernanceSelectorUnreservedAscii(final char character) {
    return (character >= 'A' && character <= 'Z')
        || (character >= 'a' && character <= 'z')
        || (character >= '0' && character <= '9')
        || character == '-'
        || character == '.'
        || character == '_'
        || character == '~';
  }

  private static IllegalArgumentException invalidGovernanceSelector(final String fieldName) {
    return new IllegalArgumentException(
        fieldName + " must match " + GOVERNANCE_SELECTOR_V1_PATTERN);
  }
}
