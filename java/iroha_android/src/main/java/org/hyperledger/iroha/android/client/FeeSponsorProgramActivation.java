package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Delayed activation scheduled for an immutable sponsor-program revision. */
public final class FeeSponsorProgramActivation {
  private final long revision;
  private final long activateAtHeight;

  public FeeSponsorProgramActivation(final long revision, final long activateAtHeight) {
    if (revision <= 0L) {
      throw new IllegalArgumentException("revision must be positive");
    }
    if (activateAtHeight < 0L) {
      throw new IllegalArgumentException("activateAtHeight must be non-negative");
    }
    this.revision = revision;
    this.activateAtHeight = activateAtHeight;
  }

  public long revision() {
    return revision;
  }

  public long activateAtHeight() {
    return activateAtHeight;
  }

  @Override
  public boolean equals(final Object other) {
    if (this == other) return true;
    if (!(other instanceof FeeSponsorProgramActivation)) return false;
    final FeeSponsorProgramActivation that = (FeeSponsorProgramActivation) other;
    return revision == that.revision && activateAtHeight == that.activateAtHeight;
  }

  @Override
  public int hashCode() {
    return Objects.hash(revision, activateAtHeight);
  }
}
