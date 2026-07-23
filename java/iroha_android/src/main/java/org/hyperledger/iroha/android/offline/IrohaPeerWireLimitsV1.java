package org.hyperledger.iroha.android.offline;

/** Allocation limits shared by all peer V1 transports. */
public final class IrohaPeerWireLimitsV1 {
  public static final IrohaPeerWireLimitsV1 PEER_V1 =
      new IrohaPeerWireLimitsV1(32 * 1024, 24_576, 24_576);

  private final int maximumCanonicalBytes;
  private final int maximumOfflineNoteEncodedBytes;
  private final int maximumKagemushaEncodedBytes;

  public IrohaPeerWireLimitsV1(
      final int maximumCanonicalBytes,
      final int maximumOfflineNoteEncodedBytes,
      final int maximumKagemushaEncodedBytes) {
    require(maximumCanonicalBytes > 0 && maximumCanonicalBytes <= 32 * 1_024);
    require(maximumOfflineNoteEncodedBytes > 0 && maximumOfflineNoteEncodedBytes <= 24_576);
    require(maximumKagemushaEncodedBytes > 0 && maximumKagemushaEncodedBytes <= 24_576);
    this.maximumCanonicalBytes = maximumCanonicalBytes;
    this.maximumOfflineNoteEncodedBytes = maximumOfflineNoteEncodedBytes;
    this.maximumKagemushaEncodedBytes = maximumKagemushaEncodedBytes;
  }

  public int maximumCanonicalBytes() {
    return maximumCanonicalBytes;
  }

  public int maximumOfflineNoteEncodedBytes() {
    return maximumOfflineNoteEncodedBytes;
  }

  public int maximumKagemushaEncodedBytes() {
    return maximumKagemushaEncodedBytes;
  }

  public int maximumEncodedBytes(final IrohaPeerPayloadProfile profile) {
    return switch (profile) {
      case OFFLINE_NOTE -> maximumOfflineNoteEncodedBytes;
      case KAGEMUSHA_RECURSIVE_SPEND -> maximumKagemushaEncodedBytes;
    };
  }

  private static void require(final boolean condition) {
    if (!condition) throw new IllegalArgumentException("Peer wire limit is outside V1 bounds");
  }
}
