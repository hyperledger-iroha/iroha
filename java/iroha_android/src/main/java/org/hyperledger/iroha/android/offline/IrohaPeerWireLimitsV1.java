package org.hyperledger.iroha.android.offline;

/** Allocation limits shared by all peer V1 transports. */
public final class IrohaPeerWireLimitsV1 {
  public static final String OFFLINE_CASH_TEXT_PREFIX = "kgm2:";
  public static final int MAXIMUM_OFFLINE_CASH_PAYMENT_REQUEST_RAW_BYTES = 768;
  public static final int MAXIMUM_OFFLINE_CASH_PAYMENT_RAW_BYTES = 7_936;
  public static final int MAXIMUM_OFFLINE_CASH_ACKNOWLEDGEMENT_RAW_BYTES = 256;
  public static final int MAXIMUM_OFFLINE_CASH_PAYMENT_REQUEST_TEXT_BYTES = 1_029;
  public static final int MAXIMUM_OFFLINE_CASH_PAYMENT_TEXT_BYTES = 10_587;
  public static final int MAXIMUM_OFFLINE_CASH_ACKNOWLEDGEMENT_TEXT_BYTES = 347;
  public static final int MAXIMUM_OFFLINE_CASH_RAW_SESSION_BYTES = 9_211;
  public static final int MAXIMUM_OFFLINE_CASH_TEXT_SESSION_BYTES = 12_288;
  public static final int MAXIMUM_OFFLINE_CASH_PAIRED_PROOF_BYTES = 6_400;
  public static final int MAXIMUM_OFFLINE_CASH_PARITY_PROOF_BYTES = 3_200;
  public static final int MAXIMUM_OFFLINE_CASH_ENCRYPTED_CREDIT_BYTES = 384;
  public static final IrohaPeerWireLimitsV1 PEER_V1 =
      new IrohaPeerWireLimitsV1(
          32 * 1024, 24_576, MAXIMUM_OFFLINE_CASH_PAYMENT_TEXT_BYTES);

  private final int maximumCanonicalBytes;
  private final int maximumKagemushaEncodedBytes;
  private final int maximumOfflineCashEncodedBytes;

  public IrohaPeerWireLimitsV1(
      final int maximumCanonicalBytes,
      final int maximumKagemushaEncodedBytes) {
    this(
        maximumCanonicalBytes,
        maximumKagemushaEncodedBytes,
        MAXIMUM_OFFLINE_CASH_PAYMENT_TEXT_BYTES);
  }

  public IrohaPeerWireLimitsV1(
      final int maximumCanonicalBytes,
      final int maximumKagemushaEncodedBytes,
      final int maximumOfflineCashEncodedBytes) {
    require(maximumCanonicalBytes > 0 && maximumCanonicalBytes <= 32 * 1_024);
    require(maximumKagemushaEncodedBytes > 0 && maximumKagemushaEncodedBytes <= 24_576);
    require(
        maximumOfflineCashEncodedBytes > 0
            && maximumOfflineCashEncodedBytes <= MAXIMUM_OFFLINE_CASH_PAYMENT_TEXT_BYTES);
    this.maximumCanonicalBytes = maximumCanonicalBytes;
    this.maximumKagemushaEncodedBytes = maximumKagemushaEncodedBytes;
    this.maximumOfflineCashEncodedBytes = maximumOfflineCashEncodedBytes;
  }

  public int maximumCanonicalBytes() {
    return maximumCanonicalBytes;
  }

  public int maximumKagemushaEncodedBytes() {
    return maximumKagemushaEncodedBytes;
  }

  public int maximumOfflineCashEncodedBytes() {
    return maximumOfflineCashEncodedBytes;
  }

  public int maximumEncodedBytes(final IrohaPeerPayloadProfile profile) {
    return switch (profile) {
      case KAGEMUSHA_RECURSIVE_SPEND -> maximumKagemushaEncodedBytes;
      case OFFLINE_CASH_V1 -> maximumOfflineCashEncodedBytes;
    };
  }

  public int maximumEncodedBytes(
      final IrohaPeerPayloadProfile profile, final IrohaPeerPayloadKind kind) {
    if (profile == IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND) {
      return maximumKagemushaEncodedBytes;
    }
    final int protocolMaximum = switch (kind) {
      case RECEIVE_REQUEST -> MAXIMUM_OFFLINE_CASH_PAYMENT_REQUEST_TEXT_BYTES;
      case PAYMENT -> MAXIMUM_OFFLINE_CASH_PAYMENT_TEXT_BYTES;
      case ACKNOWLEDGEMENT -> MAXIMUM_OFFLINE_CASH_ACKNOWLEDGEMENT_TEXT_BYTES;
    };
    return Math.min(maximumOfflineCashEncodedBytes, protocolMaximum);
  }

  private static void require(final boolean condition) {
    if (!condition) throw new IllegalArgumentException("Peer wire limit is outside V1 bounds");
  }
}
