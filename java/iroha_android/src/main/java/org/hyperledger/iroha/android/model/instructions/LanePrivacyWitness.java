package org.hyperledger.iroha.android.model.instructions;

import java.util.Objects;

/** Canonical first-release lane privacy witness variants. */
public abstract class LanePrivacyWitness {
  private LanePrivacyWitness() {}

  /** Construct a Merkle membership witness. */
  public static LanePrivacyWitness merkle(final LanePrivacyMerkleWitness value) {
    return new Merkle(value);
  }

  /** Merkle membership witness. */
  public static final class Merkle extends LanePrivacyWitness {
    private final LanePrivacyMerkleWitness value;

    public Merkle(final LanePrivacyMerkleWitness value) {
      this.value = Objects.requireNonNull(value, "value");
    }

    public LanePrivacyMerkleWitness value() {
      return value;
    }

    @Override
    public boolean equals(final Object obj) {
      return this == obj || (obj instanceof Merkle other && value.equals(other.value));
    }

    @Override
    public int hashCode() {
      return value.hashCode();
    }
  }
}
