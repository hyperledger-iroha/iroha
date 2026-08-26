package org.hyperledger.iroha.android.alias;

import org.hyperledger.iroha.android.model.NetworkId;

/** Typed exact canonical committed-block hash for an atomic onboarding observation. */
public final class AccountOnboardingBlockHashV1 {
  private final String literal;

  /** Validates and retains one exact canonical checksummed Iroha hash literal. */
  public AccountOnboardingBlockHashV1(final String literal) {
    this.literal = NetworkId.parse(literal).literal();
  }

  /** Returns the exact canonical literal. */
  public String literal() {
    return literal;
  }

  @Override
  public boolean equals(final Object other) {
    return this == other
        || other instanceof AccountOnboardingBlockHashV1
            && literal.equals(((AccountOnboardingBlockHashV1) other).literal);
  }

  @Override
  public int hashCode() {
    return literal.hashCode();
  }

  @Override
  public String toString() {
    return literal;
  }
}
