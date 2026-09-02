package org.hyperledger.iroha.android.offline;

/** Allocation limits shared by all peer V1 transports. */
public final class IrohaPeerWireLimitsV1 {
  public static final IrohaPeerWireLimitsV1 PEER_V1 =
      new IrohaPeerWireLimitsV1(
          OfflineCashWireV1.MAXIMUM_PAYMENT_BYTES,
          OfflineCashWireV1.MAXIMUM_PAYMENT_BYTES);

  private final int maximumCanonicalBytes;
  private final int maximumOfflineCashEncodedBytes;

  public IrohaPeerWireLimitsV1(
      final int maximumCanonicalBytes,
      final int maximumOfflineCashEncodedBytes) {
    require(maximumCanonicalBytes > 0
        && maximumCanonicalBytes <= OfflineCashWireV1.MAXIMUM_PAYMENT_BYTES);
    require(maximumOfflineCashEncodedBytes > 0
        && maximumOfflineCashEncodedBytes <= OfflineCashWireV1.MAXIMUM_PAYMENT_BYTES);
    this.maximumCanonicalBytes = maximumCanonicalBytes;
    this.maximumOfflineCashEncodedBytes = maximumOfflineCashEncodedBytes;
  }

  public int maximumCanonicalBytes() {
    return maximumCanonicalBytes;
  }

  public int maximumOfflineCashEncodedBytes() {
    return maximumOfflineCashEncodedBytes;
  }

  private static void require(final boolean condition) {
    if (!condition) throw new IllegalArgumentException("Peer wire limit is outside V1 bounds");
  }
}
