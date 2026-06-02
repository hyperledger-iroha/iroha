package org.hyperledger.iroha.android.offline;

/** Native record-backed Kagemusha recursive aggregation proof-bundle prover. */
public final class KagemushaRecursiveAggregationProofBundleProver {
  private static final String LIBRARY_NAME = "connect_norito_bridge";
  private static final boolean NATIVE_AVAILABLE = loadLibrary();

  private KagemushaRecursiveAggregationProofBundleProver() {}

  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }

  public static byte[] proveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
      final byte[] recordBundleArchive, final byte[] pallasOpenEnvelopesArchive) {
    if (recordBundleArchive == null || recordBundleArchive.length == 0) {
      throw new IllegalArgumentException("recordBundleArchive must not be empty");
    }
    if (pallasOpenEnvelopesArchive == null || pallasOpenEnvelopesArchive.length == 0) {
      throw new IllegalArgumentException("pallasOpenEnvelopesArchive must not be empty");
    }
    requireNative();
    final byte[] proofBundleArchive =
        nativeProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
            recordBundleArchive, pallasOpenEnvelopesArchive);
    return KagemushaCompactPaymentTokenProver.requireNativeOutput(
        proofBundleArchive,
        "nativeProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes");
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
                () ->
                    nativeProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                        new byte[0], new byte[0])));
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

  private static native byte[]
      nativeProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
          byte[] recordBundleArchive, byte[] pallasOpenEnvelopesArchive);
}
