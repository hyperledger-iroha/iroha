package org.hyperledger.iroha.android.model;

import java.text.Normalizer;
import java.util.Objects;
import org.hyperledger.iroha.android.address.AccountIdLiteral;

/** Exact immutable sponsor-program identifier. */
public final class FeeSponsorProgramId {
  private final String sponsor;
  private final String name;

  public FeeSponsorProgramId(final String sponsor, final String name) {
    this.sponsor = AccountIdLiteral.requireCanonicalI105Address(sponsor, "sponsor");
    if (name == null || name.isEmpty()) {
      throw new IllegalArgumentException("program name must not be empty");
    }
    for (int index = 0; index < name.length(); index++) {
      final char value = name.charAt(index);
      if (Character.isWhitespace(value) || value == '@' || value == '#'
          || value == '$' || value == '/') {
        throw new IllegalArgumentException("program name contains a reserved character");
      }
    }
    if (!Normalizer.normalize(name, Normalizer.Form.NFC).equals(name)) {
      throw new IllegalArgumentException("program name must use NFC normalization");
    }
    this.name = name;
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
    return sponsor.equals(that.sponsor) && name.equals(that.name);
  }

  @Override
  public int hashCode() { return Objects.hash(sponsor, name); }
}
