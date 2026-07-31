package org.hyperledger.iroha.android.model.instructions;

import java.util.Objects;

/** Proof payload bound to a registered Nexus lane commitment. */
public final class LanePrivacyProof {
  private final int commitmentId;
  private final LanePrivacyWitness witness;

  public LanePrivacyProof(final int commitmentId, final LanePrivacyWitness witness) {
    if (commitmentId < 0 || commitmentId > 0xffff) {
      throw new IllegalArgumentException("commitmentId must fit in u16");
    }
    this.commitmentId = commitmentId;
    this.witness = Objects.requireNonNull(witness, "witness");
  }

  public int commitmentId() {
    return commitmentId;
  }

  public LanePrivacyWitness witness() {
    return witness;
  }

  @Override
  public boolean equals(final Object obj) {
    return this == obj
        || (obj instanceof LanePrivacyProof other
            && commitmentId == other.commitmentId
            && witness.equals(other.witness));
  }

  @Override
  public int hashCode() {
    return Objects.hash(commitmentId, witness);
  }
}
