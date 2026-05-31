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
    if (proofBundleArchive == null || proofBundleArchive.length == 0) {
      throw new IllegalStateException(
          "nativeProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes returned empty output");
    }
    return proofBundleArchive;
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
            nativeProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                new byte[0], new byte[0]));
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

  private static native byte[]
      nativeProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
          byte[] recordBundleArchive, byte[] pallasOpenEnvelopesArchive);
}
