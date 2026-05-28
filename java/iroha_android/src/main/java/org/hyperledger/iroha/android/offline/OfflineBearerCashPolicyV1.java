package org.hyperledger.iroha.android.offline;

/** Pilot policy defaults for Offline Bearer Cash v1 handoffs. */
public final class OfflineBearerCashPolicyV1 {
  // TODO: Enforce custody and lineage limits when note audit payloads carry those counters.
  public static final OfflineBearerCashPolicyV1 DEFAULT = new OfflineBearerCashPolicyV1();

  private final int maxCustodyHops;
  private final int maxLineageSteps;
  private final int maxSingleQrPayloadBytes;
  private final int maxStreamPayloadBytes;
  private final int androidKeyPoolTarget;
  private final int androidKeyPoolReplenishBelow;
  private final int androidKeyPoolCap;

  public OfflineBearerCashPolicyV1() {
    this(5, 32, 2048, 12288, 20, 8, 40);
  }

  public OfflineBearerCashPolicyV1(
      final int maxCustodyHops,
      final int maxLineageSteps,
      final int maxSingleQrPayloadBytes,
      final int maxStreamPayloadBytes,
      final int androidKeyPoolTarget,
      final int androidKeyPoolReplenishBelow,
      final int androidKeyPoolCap) {
    requirePositive(maxCustodyHops, "maxCustodyHops");
    requirePositive(maxLineageSteps, "maxLineageSteps");
    requirePositive(maxSingleQrPayloadBytes, "maxSingleQrPayloadBytes");
    if (maxStreamPayloadBytes < maxSingleQrPayloadBytes) {
      throw new IllegalArgumentException(
          "maxStreamPayloadBytes must cover maxSingleQrPayloadBytes");
    }
    requirePositive(androidKeyPoolReplenishBelow, "androidKeyPoolReplenishBelow");
    if (androidKeyPoolTarget < androidKeyPoolReplenishBelow) {
      throw new IllegalArgumentException(
          "androidKeyPoolTarget must cover androidKeyPoolReplenishBelow");
    }
    if (androidKeyPoolCap < androidKeyPoolTarget) {
      throw new IllegalArgumentException("androidKeyPoolCap must cover androidKeyPoolTarget");
    }
    this.maxCustodyHops = maxCustodyHops;
    this.maxLineageSteps = maxLineageSteps;
    this.maxSingleQrPayloadBytes = maxSingleQrPayloadBytes;
    this.maxStreamPayloadBytes = maxStreamPayloadBytes;
    this.androidKeyPoolTarget = androidKeyPoolTarget;
    this.androidKeyPoolReplenishBelow = androidKeyPoolReplenishBelow;
    this.androidKeyPoolCap = androidKeyPoolCap;
  }

  public int maxCustodyHops() {
    return maxCustodyHops;
  }

  public int maxLineageSteps() {
    return maxLineageSteps;
  }

  public int maxSingleQrPayloadBytes() {
    return maxSingleQrPayloadBytes;
  }

  public int maxStreamPayloadBytes() {
    return maxStreamPayloadBytes;
  }

  public int androidKeyPoolTarget() {
    return androidKeyPoolTarget;
  }

  public int androidKeyPoolReplenishBelow() {
    return androidKeyPoolReplenishBelow;
  }

  public int androidKeyPoolCap() {
    return androidKeyPoolCap;
  }

  public OfflineBearerCashTransport recommendedTransportForPayloadByteCount(
      final int payloadByteCount) {
    requirePositive(payloadByteCount, "payloadByteCount");
    if (payloadByteCount <= maxSingleQrPayloadBytes) {
      return OfflineBearerCashTransport.STATIC_QR;
    }
    if (payloadByteCount <= maxStreamPayloadBytes) {
      return OfflineBearerCashTransport.STREAMING_QR;
    }
    return OfflineBearerCashTransport.FRAMED_BYTE_TRANSPORT;
  }

  private static void requirePositive(final int value, final String field) {
    if (value <= 0) {
      throw new IllegalArgumentException(field + " must be positive");
    }
  }
}
