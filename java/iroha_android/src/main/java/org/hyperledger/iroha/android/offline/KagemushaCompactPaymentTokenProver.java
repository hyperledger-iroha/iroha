package org.hyperledger.iroha.android.offline;

/** Native record-backed Kagemusha compact payment token prover. */
public final class KagemushaCompactPaymentTokenProver {
  private static final String LIBRARY_NAME = "connect_norito_bridge";
  public static final int NATIVE_ARCHIVE_MAX_BYTES = 64 * 1024 * 1024;
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
        () ->
            expectIllegalArgumentProbe(
                () -> nativeProveVerifiedCompactPaymentTokenWithRecords(new byte[0])));
  }

  static boolean detectNativeAvailability(
      final NativeProbe loadLibrary, final NativeSymbolProbe probeSymbol) {
    try {
      loadLibrary.run();
    } catch (final IllegalArgumentException error) {
      return false;
    } catch (final UnsatisfiedLinkError | SecurityException error) {
      return false;
    }
    try {
      return probeSymbol.run();
    } catch (final IllegalArgumentException error) {
      return false;
    } catch (final UnsatisfiedLinkError | SecurityException error) {
      return false;
    }
  }

  interface NativeProbe {
    void run();
  }

  interface NativeSymbolProbe {
    boolean run();
  }

  static boolean expectIllegalArgumentProbe(final NativeProbe probe) {
    try {
      probe.run();
      return false;
    } catch (final IllegalArgumentException expected) {
      return true;
    }
  }

  static byte[] requireNativeOutput(final byte[] output, final String label) {
    if (output == null) {
      throw new IllegalStateException(label + " returned no output");
    }
    if (output.length == 0) {
      throw new IllegalStateException(label + " returned empty output");
    }
    if (output.length > NATIVE_ARCHIVE_MAX_BYTES) {
      throw new IllegalStateException(label + " returned oversized output");
    }
    return output;
  }

  private static native byte[] nativeProveVerifiedCompactPaymentTokenWithRecords(
      byte[] recordBundleArchive);
}
