package org.hyperledger.iroha.android.model.instructions;

import java.util.Locale;
import java.util.regex.Pattern;
import java.util.Map;
import java.util.Objects;

/** Helper utilities shared across governance instruction builders. */
public final class GovernanceInstructionUtils {

  private static final Pattern HEX_PATTERN = Pattern.compile("^[0-9a-fA-F]+$");

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
      final String normalised = raw.trim().toLowerCase(Locale.ROOT);
      if ("zk".equals(normalised)) {
        return ZK;
      }
      if ("plain".equals(normalised)) {
        return PLAIN;
      }
      throw new IllegalArgumentException("Unknown voting mode: " + raw);
    }
  }

  /** Derivation kind recorded for persisted councils. */
  public enum CouncilDerivationKind {
    VRF("Vrf"),
    FALLBACK("Fallback");

    private final String wireValue;

    CouncilDerivationKind(final String wireValue) {
      this.wireValue = wireValue;
    }

    public String wireValue() {
      return wireValue;
    }

    public static CouncilDerivationKind parse(final String raw) {
      if (raw == null || raw.isBlank()) {
        throw new IllegalArgumentException("derived_by must not be blank");
      }
      final String normalised = raw.trim().toLowerCase(Locale.ROOT);
      if ("vrf".equals(normalised)) {
        return VRF;
      }
      if ("fallback".equals(normalised)) {
        return FALLBACK;
      }
      throw new IllegalArgumentException("Unknown council derivation: " + raw);
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
}
