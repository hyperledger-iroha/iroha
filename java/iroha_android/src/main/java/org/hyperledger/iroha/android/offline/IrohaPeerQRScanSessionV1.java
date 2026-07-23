package org.hyperledger.iroha.android.offline;

import java.util.ArrayList;
import java.util.List;
import java.util.Objects;

/**
 * Thread-safe, bounded, multi-stream animated QR decoder.
 *
 * <p>The state machine is shared with the default Kotlin/JVM SDK through the
 * reproducible Gradle composite declared by this project. Java owns its public
 * value types and converts a completed, fully verified IPM1 message back into
 * the Java representation. This keeps scanner lifetime, quarantine, and
 * memory limits byte-for-byte aligned across Android SDKs.
 */
public final class IrohaPeerQRScanSessionV1 {
  private static final long CLOCK_ORIGIN_NANOS = System.nanoTime();
  private final org.hyperledger.iroha.sdk.offline.IrohaPeerQRScanSessionV1 delegate;

  public IrohaPeerQRScanSessionV1() {
    this(null, null, null, IrohaPeerQRScanLimitsV1.STANDARD,
        IrohaPeerQRScanSessionV1::monotonicMillis);
  }

  public IrohaPeerQRScanSessionV1(final IrohaPeerQRScanLimitsV1 limits) {
    this(null, null, null, limits, IrohaPeerQRScanSessionV1::monotonicMillis);
  }

  public IrohaPeerQRScanSessionV1(
      final IrohaPeerPayloadProfile expectedProfile,
      final IrohaPeerPayloadKind expectedKind,
      final Integer expectedSchemaVersion) {
    this(
        expectedProfile,
        expectedKind,
        expectedSchemaVersion,
        IrohaPeerQRScanLimitsV1.STANDARD,
        IrohaPeerQRScanSessionV1::monotonicMillis);
  }

  public IrohaPeerQRScanSessionV1(
      final IrohaPeerPayloadProfile expectedProfile,
      final IrohaPeerPayloadKind expectedKind,
      final Integer expectedSchemaVersion,
      final IrohaPeerQRScanLimitsV1 limits,
      final IrohaPeerQRClockV1 clock) {
    Objects.requireNonNull(limits, "limits");
    Objects.requireNonNull(clock, "clock");
    delegate =
        new org.hyperledger.iroha.sdk.offline.IrohaPeerQRScanSessionV1(
            sharedProfile(expectedProfile),
            sharedKind(expectedKind),
            expectedSchemaVersion,
            limits.toShared(),
            clock::nowMillis);
  }

  public synchronized int activeStreamCount() {
    return delegate.getActiveStreamCount();
  }

  public synchronized void reset() {
    delegate.reset();
  }

  /** Expires idle/absolute-age candidates and returns defensive stream-ID copies. */
  public synchronized List<byte[]> expire() {
    return copyStreamIds(delegate.expire());
  }

  /** Expires at an explicit monotonic test time. */
  public synchronized List<byte[]> expire(final long nowMillis) {
    return copyStreamIds(delegate.expire(nowMillis));
  }

  /** Quarantines an application-rejected completed IPM1 stream using the scanner bounds. */
  public synchronized void quarantine(final byte[] streamId) {
    Objects.requireNonNull(streamId, "streamId");
    delegate.quarantine(streamId.clone());
  }

  /** Quarantines at an explicit monotonic test time. */
  public synchronized void quarantine(final byte[] streamId, final long nowMillis) {
    Objects.requireNonNull(streamId, "streamId");
    delegate.quarantine(streamId.clone(), nowMillis);
  }

  public synchronized IrohaPeerQRScanResultV1 ingest(final String value) {
    return convert(delegate.ingest(value));
  }

  public synchronized IrohaPeerQRScanResultV1 ingestAt(
      final String value, final long nowMillis) {
    return convert(delegate.ingestAt(value, nowMillis));
  }

  private static IrohaPeerQRScanResultV1 convert(
      final org.hyperledger.iroha.sdk.offline.IrohaPeerQRScanResultV1 result) {
    final org.hyperledger.iroha.sdk.offline.IrohaPeerWireMessageV1 shared = result.getMessage();
    final IrohaPeerWireMessageV1 message =
        shared == null ? null : IrohaPeerWireMessageV1.decode(shared.encode());
    return new IrohaPeerQRScanResultV1(
        message,
        result.getProfile() == null
            ? null
            : IrohaPeerPayloadProfile.fromCode(result.getProfile().getCode()),
        result.getPayloadKind() == null
            ? null
            : IrohaPeerPayloadKind.fromCode(result.getPayloadKind().getCode()),
        result.getReceivedDataFrames(),
        result.getTotalDataFrames(),
        result.getRecoveredDataFrames());
  }

  private static org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile sharedProfile(
      final IrohaPeerPayloadProfile profile) {
    return profile == null
        ? null
        : org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.fromCode(profile.code());
  }

  private static org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadKind sharedKind(
      final IrohaPeerPayloadKind kind) {
    return kind == null
        ? null
        : org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadKind.fromCode(kind.code());
  }

  private static List<byte[]> copyStreamIds(final List<byte[]> source) {
    final List<byte[]> copy = new ArrayList<>(source.size());
    for (final byte[] streamId : source) copy.add(streamId.clone());
    return List.copyOf(copy);
  }

  private static long monotonicMillis() {
    return (System.nanoTime() - CLOCK_ORIGIN_NANOS) / 1_000_000L;
  }
}
