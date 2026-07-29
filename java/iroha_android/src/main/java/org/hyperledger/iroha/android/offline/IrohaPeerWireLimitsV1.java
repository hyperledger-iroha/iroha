package org.hyperledger.iroha.android.offline;

/** Allocation limits shared by all peer V1 transports. */
public final class IrohaPeerWireLimitsV1 {
  public static final IrohaPeerWireLimitsV1 PEER_V1 =
      new IrohaPeerWireLimitsV1(32 * 1024, 24_576);

  private final int maximumCanonicalBytes;
  private final int maximumKagemushaEncodedBytes;

  public IrohaPeerWireLimitsV1(
      final int maximumCanonicalBytes,
      final int maximumKagemushaEncodedBytes) {
    require(maximumCanonicalBytes > 0 && maximumCanonicalBytes <= 32 * 1_024);
    require(maximumKagemushaEncodedBytes > 0 && maximumKagemushaEncodedBytes <= 24_576);
    this.maximumCanonicalBytes = maximumCanonicalBytes;
    this.maximumKagemushaEncodedBytes = maximumKagemushaEncodedBytes;
  }

  public int maximumCanonicalBytes() {
    return maximumCanonicalBytes;
  }

  public int maximumKagemushaEncodedBytes() {
    return maximumKagemushaEncodedBytes;
  }

  public int maximumEncodedBytes(final IrohaPeerPayloadProfile profile) {
    return maximumKagemushaEncodedBytes;
  }

  private static void require(final boolean condition) {
    if (!condition) throw new IllegalArgumentException("Peer wire limit is outside V1 bounds");
  }
}
