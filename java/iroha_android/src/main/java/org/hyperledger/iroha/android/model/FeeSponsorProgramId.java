package org.hyperledger.iroha.android.model;

import java.nio.charset.StandardCharsets;
import java.text.Normalizer;
import java.util.Arrays;
import java.util.Objects;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AccountIdLiteral;

/** Exact immutable sponsor-program identifier. */
public final class FeeSponsorProgramId {
  private static final int MAX_PROGRAM_NAME_UTF8_BYTES = 255;

  private final String sponsor;
  private final byte[] sponsorIdentity;
  private final String name;

  public FeeSponsorProgramId(final String sponsor, final String name) {
    if (sponsor != null && sponsor.indexOf('/') >= 0) {
      throw new IllegalArgumentException("sponsor must not contain `/`");
    }
    this.sponsor = AccountIdLiteral.requireCanonicalI105Address(sponsor, "sponsor");
    try {
      this.sponsorIdentity =
          AccountAddress.parseEncodedIgnoringCurveSupport(this.sponsor, null).canonicalBytes();
    } catch (final AccountAddress.AccountAddressException error) {
      throw new IllegalStateException("validated sponsor account could not be parsed", error);
    }
    validateName(name);
    this.name = name;
  }

  private static void validateName(final String name) {
    if (name == null || name.isEmpty()) {
      throw new IllegalArgumentException("program name must not be empty");
    }
    if (name.getBytes(StandardCharsets.UTF_8).length > MAX_PROGRAM_NAME_UTF8_BYTES) {
      throw new IllegalArgumentException("program name exceeds the 255-byte UTF-8 limit");
    }
    int offset = 0;
    while (offset < name.length()) {
      final char first = name.charAt(offset);
      final int codePoint;
      if (Character.isHighSurrogate(first)) {
        if (offset + 1 >= name.length() || !Character.isLowSurrogate(name.charAt(offset + 1))) {
          throw new IllegalArgumentException(
              "program name must contain only Unicode scalar values");
        }
        codePoint = Character.toCodePoint(first, name.charAt(offset + 1));
      } else if (Character.isLowSurrogate(first)) {
        throw new IllegalArgumentException("program name must contain only Unicode scalar values");
      } else {
        codePoint = first;
      }
      if (Character.isISOControl(codePoint)) {
        throw new IllegalArgumentException(
            "program name must not contain Unicode control characters");
      }
      if (isBidiControl(codePoint)) {
        throw new IllegalArgumentException(
            "program name must not contain Unicode bidirectional control characters");
      }
      if (isUnicodeWhitespace(codePoint)) {
        throw new IllegalArgumentException("program name must not contain whitespace");
      }
      if (codePoint == '@' || codePoint == '#' || codePoint == '$') {
        throw new IllegalArgumentException("program name contains a reserved character");
      }
      if (codePoint == '/') {
        throw new IllegalArgumentException("program name must not contain `/`");
      }
      offset += Character.charCount(codePoint);
    }
    if (!Normalizer.normalize(name, Normalizer.Form.NFC).equals(name)) {
      throw new IllegalArgumentException("program name must use NFC normalization");
    }
  }

  private static boolean isBidiControl(final int codePoint) {
    return codePoint == 0x061C
        || codePoint == 0x200E
        || codePoint == 0x200F
        || (codePoint >= 0x202A && codePoint <= 0x202E)
        || (codePoint >= 0x2066 && codePoint <= 0x2069);
  }

  private static boolean isUnicodeWhitespace(final int codePoint) {
    return (codePoint >= 0x0009 && codePoint <= 0x000D)
        || codePoint == 0x0020
        || codePoint == 0x0085
        || codePoint == 0x00A0
        || codePoint == 0x1680
        || (codePoint >= 0x2000 && codePoint <= 0x200A)
        || codePoint == 0x2028
        || codePoint == 0x2029
        || codePoint == 0x202F
        || codePoint == 0x205F
        || codePoint == 0x3000;
  }

  public String sponsor() { return sponsor; }
  public String name() { return name; }
  public String literal() { return sponsor + "/" + name; }

  public static FeeSponsorProgramId parse(final String literal) {
    Objects.requireNonNull(literal, "literal");
    if (!literal.trim().equals(literal)) {
      throw new IllegalArgumentException("programId must not contain surrounding whitespace");
    }
    final int slash = literal.indexOf('/');
    if (slash <= 0 || slash != literal.lastIndexOf('/') || slash == literal.length() - 1) {
      throw new IllegalArgumentException("programId must use sponsor/program");
    }
    return new FeeSponsorProgramId(literal.substring(0, slash), literal.substring(slash + 1));
  }

  @Override
  public String toString() { return literal(); }

  @Override
  public boolean equals(final Object other) {
    if (this == other) return true;
    if (!(other instanceof FeeSponsorProgramId)) return false;
    final FeeSponsorProgramId that = (FeeSponsorProgramId) other;
    return Arrays.equals(sponsorIdentity, that.sponsorIdentity) && name.equals(that.name);
  }

  @Override
  public int hashCode() { return 31 * Arrays.hashCode(sponsorIdentity) + name.hashCode(); }
}
