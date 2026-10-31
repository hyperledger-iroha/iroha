package org.hyperledger.iroha.android.model.instructions;

/** Helper utilities shared across governance instruction builders. */
public final class GovernanceInstructionUtils {

  private static final int GOVERNANCE_SELECTOR_V1_MAX_LENGTH = 128;
  private static final String GOVERNANCE_SELECTOR_V1_PATTERN =
      "^[A-Za-z0-9_~-][A-Za-z0-9._~-]{0,127}$";

  private GovernanceInstructionUtils() {}

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
