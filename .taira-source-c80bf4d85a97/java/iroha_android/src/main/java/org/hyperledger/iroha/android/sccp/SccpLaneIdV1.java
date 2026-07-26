package org.hyperledger.iroha.android.sccp;

import java.util.Objects;

/** Directed SCCP lane between exact network profiles. */
public final class SccpLaneIdV1 {
  private final SccpNetworkV1 source;
  private final SccpNetworkV1 target;

  public SccpLaneIdV1(final SccpNetworkV1 source, final SccpNetworkV1 target) {
    this.source = Objects.requireNonNull(source, "source");
    this.target = Objects.requireNonNull(target, "target");
    if (source.isSora() == target.isSora() || source.domainId() == target.domainId()) {
      throw new IllegalArgumentException(
          "SCCP lane must join exactly one SORA profile and one external profile");
    }
  }

  public SccpNetworkV1 source() {
    return source;
  }

  public SccpNetworkV1 target() {
    return target;
  }

  public boolean isOutbound() {
    return source.isSora() && target.isExternal();
  }

  public boolean isInbound() {
    return source.isExternal() && target.isSora();
  }

  @Override
  public boolean equals(final Object other) {
    if (!(other instanceof SccpLaneIdV1)) {
      return false;
    }
    final SccpLaneIdV1 lane = (SccpLaneIdV1) other;
    return source == lane.source && target == lane.target;
  }

  @Override
  public int hashCode() {
    return 31 * source.hashCode() + target.hashCode();
  }

  @Override
  public String toString() {
    return source.profileKey() + "->" + target.profileKey();
  }
}
