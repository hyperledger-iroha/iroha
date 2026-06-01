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
    return requireNativeOutput(
        tokenArchive, "nativeProveVerifiedCompactPaymentTokenWithRecords");
  }

  private static void requireNative() {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException(LIBRARY_NAME + " is not available in this runtime");
    }
  }

  private static boolean loadLibrary() {
    return detectNativeAvailability(
        () -> System.loadLibrary(LIBRARY_NAME),
        () -> nativeProveVerifiedCompactPaymentTokenWithRecords(new byte[0]));
  }

  static boolean detectNativeAvailability(
      final NativeProbe loadLibrary, final NativeProbe probeSymbol) {
    try {
      loadLibrary.run();
      probeSymbol.run();
      return true;
    } catch (final IllegalArgumentException error) {
      return true;
    } catch (final UnsatisfiedLinkError | SecurityException error) {
      return false;
    }
  }

  interface NativeProbe {
    void run();
  }

  static byte[] requireNativeOutput(final byte[] output, final String label) {
    if (output == null) {
      throw new IllegalStateException(label + " returned no output");
    }
    if (output.length == 0) {
      throw new IllegalStateException(label + " returned empty output");
    }
    return output;
  }

  private static native byte[] nativeProveVerifiedCompactPaymentTokenWithRecords(
      byte[] recordBundleArchive);
}
