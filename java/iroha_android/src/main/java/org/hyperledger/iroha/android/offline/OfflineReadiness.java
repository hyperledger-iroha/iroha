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
  private final String evaluatedBlockHash;
  private final boolean ready;
  private final List<OfflineReadinessBlocker> blockers;

  public OfflineReadiness(
      final String assetDefinitionId,
      final BigInteger evaluatedBlockHeight,
      final String evaluatedBlockHash,
      final boolean ready,
      final List<OfflineReadinessBlocker> blockers) {
    this.assetDefinitionId = requireExactText(assetDefinitionId, "assetDefinitionId");
    this.evaluatedBlockHeight =
        Objects.requireNonNull(evaluatedBlockHeight, "evaluatedBlockHeight");
    if (evaluatedBlockHeight.signum() < 0 || evaluatedBlockHeight.compareTo(U64_MAX) > 0) {
      throw new IllegalArgumentException(
          "evaluatedBlockHeight must fit in an unsigned 64-bit integer");
    }
    this.evaluatedBlockHash = requireLowercaseHash(evaluatedBlockHash);
    this.ready = ready;
    Objects.requireNonNull(blockers, "blockers");
    final ArrayList<OfflineReadinessBlocker> blockerCopy = new ArrayList<>(blockers.size());
    for (final OfflineReadinessBlocker blocker : blockers) {
      blockerCopy.add(Objects.requireNonNull(blocker, "blockers must not contain null"));
    }
    if (ready != blockerCopy.isEmpty()) {
      throw new IllegalArgumentException("ready must be true exactly when blockers is empty");
    }
    this.blockers = Collections.unmodifiableList(blockerCopy);
  }

  public String assetDefinitionId() {
    return assetDefinitionId;
  }

  public BigInteger evaluatedBlockHeight() {
    return evaluatedBlockHeight;
  }

  public String evaluatedBlockHash() {
    return evaluatedBlockHash;
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
        && evaluatedBlockHash.equals(that.evaluatedBlockHash)
        && blockers.equals(that.blockers);
  }

  @Override
  public int hashCode() {
    return Objects.hash(assetDefinitionId, evaluatedBlockHeight, evaluatedBlockHash, ready, blockers);
  }

  private static String requireExactText(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(field + " must be exact non-empty text");
    }
    return value;
  }


  private static String requireLowercaseHash(final String value) {
    Objects.requireNonNull(value, "evaluatedBlockHash");
    if (value.length() != 64) {
      throw new IllegalArgumentException(
          "evaluatedBlockHash must be exact lowercase 32-byte hexadecimal");
    }
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (!((character >= '0' && character <= '9') || (character >= 'a' && character <= 'f'))) {
        throw new IllegalArgumentException(
            "evaluatedBlockHash must be exact lowercase 32-byte hexadecimal");
      }
    }
    return value;
  }
}
