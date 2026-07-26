package org.hyperledger.iroha.android.offline;

/** Bounded resource and lifetime policy for animated QR scanning. */
public final class IrohaPeerQRScanLimitsV1 {
  public static final IrohaPeerQRScanLimitsV1 STANDARD =
      new IrohaPeerQRScanLimitsV1(3, 12, 3_072, 30_000, 180_000);

  private final int maximumActiveStreams;
  private final int maximumPreheaderFramesPerStream;
  private final int maximumPreheaderPayloadBytesPerStream;
  private final long idleTimeoutMillis;
  private final long absoluteTimeoutMillis;

  public IrohaPeerQRScanLimitsV1(
      final int maximumActiveStreams,
      final int maximumPreheaderFramesPerStream,
      final int maximumPreheaderPayloadBytesPerStream,
      final long idleTimeoutMillis,
      final long absoluteTimeoutMillis) {
    if (maximumActiveStreams <= 0
        || maximumActiveStreams > 3
        || maximumPreheaderFramesPerStream <= 0
        || maximumPreheaderFramesPerStream > 12
        || maximumPreheaderPayloadBytesPerStream <= 0
        || maximumPreheaderPayloadBytesPerStream > 3_072
        || idleTimeoutMillis <= 0
        || idleTimeoutMillis > 30_000
        || absoluteTimeoutMillis < idleTimeoutMillis
        || absoluteTimeoutMillis > 180_000) {
      throw new IllegalArgumentException("Invalid QR scanner limits");
    }
    this.maximumActiveStreams = maximumActiveStreams;
    this.maximumPreheaderFramesPerStream = maximumPreheaderFramesPerStream;
    this.maximumPreheaderPayloadBytesPerStream = maximumPreheaderPayloadBytesPerStream;
    this.idleTimeoutMillis = idleTimeoutMillis;
    this.absoluteTimeoutMillis = absoluteTimeoutMillis;
  }

  public int maximumActiveStreams() {
    return maximumActiveStreams;
  }

  public int maximumPreheaderFramesPerStream() {
    return maximumPreheaderFramesPerStream;
  }

  public int maximumPreheaderPayloadBytesPerStream() {
    return maximumPreheaderPayloadBytesPerStream;
  }

  public long idleTimeoutMillis() {
    return idleTimeoutMillis;
  }

  public long absoluteTimeoutMillis() {
    return absoluteTimeoutMillis;
  }

  org.hyperledger.iroha.sdk.offline.IrohaPeerQRScanLimitsV1 toShared() {
    return new org.hyperledger.iroha.sdk.offline.IrohaPeerQRScanLimitsV1(
        maximumActiveStreams,
        maximumPreheaderFramesPerStream,
        maximumPreheaderPayloadBytesPerStream,
        idleTimeoutMillis,
        absoluteTimeoutMillis);
  }
}
