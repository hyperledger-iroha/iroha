package org.hyperledger.iroha.android.offline;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;

/** Readiness of the requested asset definition for Offline operations. */
public final class OfflineReadiness {
  private static final BigInteger U64_MAX = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);

  private final String assetDefinitionId;
  private final BigInteger evaluatedBlockHeight;
  private final boolean ready;
  private final List<OfflineReadinessBlocker> blockers;

  public OfflineReadiness(
      final String assetDefinitionId,
      final BigInteger evaluatedBlockHeight,
      final boolean ready,
      final List<OfflineReadinessBlocker> blockers) {
    this.assetDefinitionId = requireExactText(assetDefinitionId, "assetDefinitionId");
    this.evaluatedBlockHeight =
        Objects.requireNonNull(evaluatedBlockHeight, "evaluatedBlockHeight");
    if (evaluatedBlockHeight.signum() < 0 || evaluatedBlockHeight.compareTo(U64_MAX) > 0) {
      throw new IllegalArgumentException(
          "evaluatedBlockHeight must fit in an unsigned 64-bit integer");
    }
    this.ready = ready;
    this.blockers = Collections.unmodifiableList(new ArrayList<>(blockers));
  }

  public String assetDefinitionId() {
    return assetDefinitionId;
  }

  public BigInteger evaluatedBlockHeight() {
    return evaluatedBlockHeight;
  }

  public boolean ready() {
    return ready;
  }

  public List<OfflineReadinessBlocker> blockers() {
    return blockers;
  }

  @Override
  public boolean equals(final Object other) {
    if (this == other) {
      return true;
    }
    if (!(other instanceof OfflineReadiness)) {
      return false;
    }
    final OfflineReadiness that = (OfflineReadiness) other;
    return ready == that.ready
        && assetDefinitionId.equals(that.assetDefinitionId)
        && evaluatedBlockHeight.equals(that.evaluatedBlockHeight)
        && blockers.equals(that.blockers);
  }

  @Override
  public int hashCode() {
    return Objects.hash(assetDefinitionId, evaluatedBlockHeight, ready, blockers);
  }

  private static String requireExactText(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(field + " must be exact non-empty text");
    }
    return value;
  }
}
