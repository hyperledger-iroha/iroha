package org.hyperledger.iroha.android.offline;

/** Native recursive Kagemusha spend init/append/verify/redeem bridge. */
public final class KagemushaRecursiveSpendProver {
  private static final String LIBRARY_NAME = "connect_norito_bridge";
  private static final boolean NATIVE_AVAILABLE = loadLibrary();

  public enum Mode {
    RECURSIVE_SPEND_V1("recursive_spend_v1"),
    CHECKED_PREFOLD_V1("checked_prefold_v1");

    private final String wireName;

    Mode(final String wireName) {
      this.wireName = wireName;
    }

    public String wireName() {
      return wireName;
    }
  }

  private KagemushaRecursiveSpendProver() {}

  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }

  public static Mode preferredMode() {
    return preferredMode(NATIVE_AVAILABLE);
  }

  public static Mode preferredMode(final boolean recursiveSpendAvailable) {
    return recursiveSpendAvailable ? Mode.RECURSIVE_SPEND_V1 : Mode.CHECKED_PREFOLD_V1;
  }

  public static byte[] initSpend(final byte[] requestArchive) {
    return call("init", requestArchive, KagemushaRecursiveSpendProver::nativeInitSpend);
  }

  public static byte[] appendSpend(final byte[] requestArchive) {
    return call("append", requestArchive, KagemushaRecursiveSpendProver::nativeAppendSpend);
  }

  public static byte[] verifySpend(final byte[] requestArchive) {
    return call("verify", requestArchive, KagemushaRecursiveSpendProver::nativeVerifySpend);
  }

  public static byte[] redeemSpend(final byte[] requestArchive) {
    return call("redeem", requestArchive, KagemushaRecursiveSpendProver::nativeRedeemSpend);
  }

  private static byte[] call(final String label, final byte[] requestArchive, final NativeCall call) {
    if (requestArchive == null || requestArchive.length == 0) {
      throw new IllegalArgumentException("requestArchive must not be empty");
    }
    requireNative();
    final byte[] output = call.run(requestArchive);
    return KagemushaCompactPaymentTokenProver.requireNativeOutput(output, "native " + label);
  }

  private static void requireNative() {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException(LIBRARY_NAME + " is not available in this runtime");
    }
  }

  private static boolean loadLibrary() {
    return KagemushaRecursiveAggregationProofBundleProver.detectNativeAvailability(
        () -> System.loadLibrary(LIBRARY_NAME), () -> nativeVerifySpend(new byte[0]));
  }

  private interface NativeCall {
    byte[] run(byte[] requestArchive);
  }

  private static native byte[] nativeInitSpend(byte[] requestArchive);

  private static native byte[] nativeAppendSpend(byte[] requestArchive);

  private static native byte[] nativeVerifySpend(byte[] requestArchive);

  private static native byte[] nativeRedeemSpend(byte[] requestArchive);
}
