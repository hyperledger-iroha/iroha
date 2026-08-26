package org.hyperledger.iroha.android.alias;

import java.math.BigInteger;
import java.util.Objects;

/** Closed classification derived from one committed state snapshot. */
public final class AccountOnboardingCurrentStateV1 {
  /** First-release atomic onboarding-state outcomes. */
  public enum Outcome {
    APPLIED,
    ALIAS_ABSENT,
    ALIAS_CONFLICT
  }

  private final Outcome outcome;
  private final BigInteger blockHeight;
  private final AccountOnboardingBlockHashV1 blockHash;

  /** Constructs one anchored classification. */
  public AccountOnboardingCurrentStateV1(
      final Outcome outcome,
      final BigInteger blockHeight,
      final AccountOnboardingBlockHashV1 blockHash) {
    this.outcome = Objects.requireNonNull(outcome, "outcome");
    this.blockHeight = requirePositiveU64(blockHeight, "blockHeight");
    this.blockHash = Objects.requireNonNull(blockHash, "blockHash");
  }

  public Outcome outcome() {
    return outcome;
  }

  public BigInteger blockHeight() {
    return blockHeight;
  }

  public AccountOnboardingBlockHashV1 blockHash() {
    return blockHash;
  }

  @Override
  public boolean equals(final Object other) {
    if (this == other) return true;
    if (!(other instanceof AccountOnboardingCurrentStateV1)) return false;
    final AccountOnboardingCurrentStateV1 state =
        (AccountOnboardingCurrentStateV1) other;
    return outcome == state.outcome
        && blockHeight.equals(state.blockHeight)
        && blockHash.equals(state.blockHash);
  }

  @Override
  public int hashCode() {
    return Objects.hash(outcome, blockHeight, blockHash);
  }

  static BigInteger requirePositiveU64(final BigInteger value, final String field) {
    if (value == null || value.signum() <= 0 || value.bitLength() > 64) {
      throw new IllegalArgumentException(
          field + " must be a positive unsigned 64-bit integer");
    }
    return value;
  }
}
