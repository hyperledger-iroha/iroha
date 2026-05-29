package org.hyperledger.iroha.android.offline;

/** Native record-backed Kagemusha compact payment token prover. */
public final class KagemushaCompactPaymentTokenProver {
  private static final String LIBRARY_NAME = "connect_norito_bridge";
  private static final boolean NATIVE_AVAILABLE = loadLibrary();

  private KagemushaCompactPaymentTokenProver() {}

  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }

  public static byte[] proveVerifiedCompactPaymentTokenWithRecords(
      final byte[] recordBundleArchive) {
    if (recordBundleArchive == null || recordBundleArchive.length == 0) {
      throw new IllegalArgumentException("recordBundleArchive must not be empty");
    }
    requireNative();
    final byte[] tokenArchive =
        nativeProveVerifiedCompactPaymentTokenWithRecords(recordBundleArchive);
    if (tokenArchive == null || tokenArchive.length == 0) {
      throw new IllegalStateException(
          "nativeProveVerifiedCompactPaymentTokenWithRecords returned empty output");
    }
    return tokenArchive;
  }

  private static void requireNative() {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException(LIBRARY_NAME + " is not available in this runtime");
    }
  }

  private static boolean loadLibrary() {
    try {
      System.loadLibrary(LIBRARY_NAME);
      return true;
    } catch (final UnsatisfiedLinkError | SecurityException error) {
      return false;
    }
  }

  private static native byte[] nativeProveVerifiedCompactPaymentTokenWithRecords(
      byte[] recordBundleArchive);
}
