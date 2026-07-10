package org.hyperledger.iroha.android.sccp;

import java.util.Arrays;
import java.util.Objects;

/** Exact governed context for a SORA-origin SCCP message. */
public final class SccpOutboundMessageContextV1 {
  private final SccpLaneIdV1 lane;
  private final byte[] destinationBindingHash;

  public SccpOutboundMessageContextV1(
      final SccpLaneIdV1 lane, final byte[] destinationBindingHash) {
    this.lane = Objects.requireNonNull(lane, "lane");
    if (!lane.isOutbound()) {
      throw new IllegalArgumentException(
          "outbound SCCP context must use a SORA-to-external lane");
    }
    this.destinationBindingHash =
        SccpV1.requireHash(destinationBindingHash, "destinationBindingHash");
  }

  public SccpLaneIdV1 lane() {
    return lane;
  }

  public byte[] destinationBindingHash() {
    return Arrays.copyOf(destinationBindingHash, destinationBindingHash.length);
  }

  @Override
  public boolean equals(final Object other) {
    if (!(other instanceof SccpOutboundMessageContextV1)) {
      return false;
    }
    final SccpOutboundMessageContextV1 context = (SccpOutboundMessageContextV1) other;
    return lane.equals(context.lane)
        && Arrays.equals(destinationBindingHash, context.destinationBindingHash);
  }

  @Override
  public int hashCode() {
    return 31 * lane.hashCode() + Arrays.hashCode(destinationBindingHash);
  }
}
