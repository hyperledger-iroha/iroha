package org.hyperledger.iroha.android.multisig;

import java.util.Objects;

/** TTL preview for multisig proposals/relayers. */
public final class MultisigProposalTtlPreview {

  private final long effectiveTtlMs;
  private final long policyCapMs;
  private final long expiresAtMs;
  private final boolean wasCapped;

  MultisigProposalTtlPreview(
      final long effectiveTtlMs, final long policyCapMs, final long expiresAtMs, final boolean wasCapped) {
    this.effectiveTtlMs = effectiveTtlMs;
    this.policyCapMs = policyCapMs;
    this.expiresAtMs = expiresAtMs;
    this.wasCapped = wasCapped;
  }

  public long effectiveTtlMs() {
    return effectiveTtlMs;
  }

  public long policyCapMs() {
    return policyCapMs;
  }

  public long expiresAtMs() {
    return expiresAtMs;
  }

  public boolean wasCapped() {
    return wasCapped;
  }

  @Override
  public boolean equals(final Object other) {
    if (this == other) {
      return true;
    }
    if (!(other instanceof MultisigProposalTtlPreview that)) {
      return false;
    }
    return effectiveTtlMs == that.effectiveTtlMs
        && policyCapMs == that.policyCapMs
        && expiresAtMs == that.expiresAtMs
        && wasCapped == that.wasCapped;
  }

  @Override
  public int hashCode() {
    return Objects.hash(effectiveTtlMs, policyCapMs, expiresAtMs, wasCapped);
  }

  @Override
  public String toString() {
    return "MultisigProposalTtlPreview{"
        + "effectiveTtlMs="
        + effectiveTtlMs
        + ", policyCapMs="
        + policyCapMs
        + ", expiresAtMs="
        + expiresAtMs
        + ", wasCapped="
        + wasCapped
        + '}';
  }
}
