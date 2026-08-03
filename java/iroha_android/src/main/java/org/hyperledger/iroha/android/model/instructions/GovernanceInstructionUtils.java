package org.hyperledger.iroha.android.model.instructions;

import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.regex.Pattern;

/** Helper utilities shared across governance instruction builders. */
public final class GovernanceInstructionUtils {

  private static final Pattern HEX_PATTERN = Pattern.compile("^[0-9a-fA-F]+$");
  private static final Pattern LOWERCASE_HEX_PATTERN = Pattern.compile("^[0-9a-f]+$");
  private static final int GOVERNANCE_SELECTOR_V1_MAX_LENGTH = 128;
  private static final String GOVERNANCE_SELECTOR_V1_PATTERN =
      "^[A-Za-z0-9_~-][A-Za-z0-9._~-]{0,127}$";

  private GovernanceInstructionUtils() {}

  /** Inclusive enactment window expressed in block heights. */
  public static final class AtWindow {
    private final long lower;
    private final long upper;

    public AtWindow(final long lower, final long upper) {
      if (lower < 0 || upper < lower) {
        throw new IllegalArgumentException("window bounds must satisfy 0 <= lower <= upper");
      }
      this.lower = lower;
      this.upper = upper;
    }

    public long lower() {
      return lower;
    }

    public long upper() {
      return upper;
    }
  }

  /** Voting mode applied to referendums spawned by a proposal. */
  public enum VotingMode {
    ZK("Zk"),
    PLAIN("Plain");

    private final String wireValue;

    VotingMode(final String wireValue) {
      this.wireValue = wireValue;
    }

    public String wireValue() {
      return wireValue;
    }

    public static VotingMode parse(final String raw) {
      if (raw == null || raw.isBlank()) {
        throw new IllegalArgumentException("mode must not be blank");
      }
      if ("Zk".equals(raw)) {
        return ZK;
      }
      if ("Plain".equals(raw)) {
        return PLAIN;
      }
      throw new IllegalArgumentException("Unknown voting mode: " + raw);
    }
  }

  static void appendAtWindow(
      final Map<String, String> arguments, final AtWindow window, final String prefix) {
    Objects.requireNonNull(arguments, "arguments");
    Objects.requireNonNull(window, "window");
    final String base = Objects.requireNonNull(prefix, "prefix");
    arguments.put(base + ".lower", Long.toString(window.lower()));
    arguments.put(base + ".upper", Long.toString(window.upper()));
  }

  static AtWindow parseAtWindow(
      final Map<String, String> arguments, final String prefix, final String displayName) {
    Objects.requireNonNull(arguments, "arguments");
    final String base = Objects.requireNonNull(prefix, "prefix");
    final String lowerRaw = arguments.get(base + ".lower");
    final String upperRaw = arguments.get(base + ".upper");
    if (lowerRaw == null || upperRaw == null) {
      throw new IllegalArgumentException(displayName + " must include lower and upper bounds");
    }
    try {
      final long lower = Long.parseLong(lowerRaw);
      final long upper = Long.parseLong(upperRaw);
      return new AtWindow(lower, upper);
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(
          "Window bounds must be numeric for " + displayName + ": lower="
              + lowerRaw
              + ", upper="
              + upperRaw,
          ex);
    }
  }

  static String requireHex(
      final String value, final String fieldName, final int expectedBytes) {
    if (value == null || value.isBlank()) {
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

  /** Requires an exact lowercase hexadecimal value without compatibility normalization. */
  static String requireExactLowercaseHex(
      final String value, final String fieldName, final int expectedBytes) {
    if (expectedBytes <= 0) {
      throw new IllegalArgumentException("expectedBytes must be positive");
    }
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException(fieldName + " must not be blank");
    }
    if (value.length() != expectedBytes * 2
        || !LOWERCASE_HEX_PATTERN.matcher(value).matches()) {
      throw new IllegalArgumentException(
          fieldName + " must be exactly " + (expectedBytes * 2) + " lowercase hex chars");
    }
    return value;
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
