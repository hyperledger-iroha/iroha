package org.hyperledger.iroha.android.offline;

/** Native ABI-7 Kagemusha recursive compact-token prover and verifier. */
public final class KagemushaRecursiveCompactPaymentTokenProver {
  private static final String LIBRARY_NAME = "connect_norito_bridge";
  public static final int REQUIRED_BRIDGE_ABI_VERSION = 7;
  public static final String RECURSIVE_COMPACT_CIRCUIT_ID_V1 =
      "kagemusha-recursive-compact-v1";
  private static final boolean NATIVE_AVAILABLE = loadLibrary();

  private KagemushaRecursiveCompactPaymentTokenProver() {}

  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }

  public static byte[] proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
      final byte[] recordBundleArchive, final byte[] pallasOpenEnvelopesArchive) {
    requireNativeInput(recordBundleArchive, "recordBundleArchive");
    requireNativeInput(pallasOpenEnvelopesArchive, "pallasOpenEnvelopesArchive");
    requireNative();
    final byte[] tokenArchive =
        nativeProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
            recordBundleArchive, pallasOpenEnvelopesArchive);
    return KagemushaCompactPaymentTokenProver.requireNativeOutput(
        tokenArchive,
        "nativeProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes");
  }

  private static void requireNativeInput(final byte[] archive, final String archiveName) {
    if (archive == null || archive.length == 0) {
      throw new IllegalArgumentException(archiveName + " must not be empty");
    }
    if (!KagemushaCompactPaymentTokenProver.isValidNoritoArchive(archive)) {
      throw new IllegalArgumentException(archiveName + " must be a valid Norito archive");
    }
    if (!KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(archive)) {
      throw new IllegalArgumentException(
          archiveName + " must contain a non-empty Norito payload");
    }
  }

  public static boolean verifyRecursiveCompactPaymentToken(final byte[] compactTokenArchive) {
    if (compactTokenArchive == null || compactTokenArchive.length == 0) {
      throw new IllegalArgumentException("compactTokenArchive must not be empty");
    }
    if (!KagemushaCompactPaymentTokenProver.isValidNoritoArchive(compactTokenArchive)) {
      throw new IllegalArgumentException("compactTokenArchive must be a valid Norito archive");
    }
    if (!KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(compactTokenArchive)) {
      throw new IllegalArgumentException(
          "compactTokenArchive must contain a non-empty Norito payload");
    }
    requireNative();
    return nativeVerifyRecursiveCompactPaymentToken(compactTokenArchive);
  }

  private static void requireNative() {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException(
          LIBRARY_NAME
              + " ABI 7 recursive compact-token prover/verifier is not available in this runtime");
    }
  }

  private static boolean loadLibrary() {
    return KagemushaRecursiveSpendProver.detectNativeAvailability(
        () -> System.loadLibrary(LIBRARY_NAME),
        KagemushaRecursiveCompactPaymentTokenProver::nativeBridgeAbiVersion,
        () -> {
          final boolean proverRejects =
              KagemushaCompactPaymentTokenProver.expectIllegalArgumentProbe(
                  () ->
                      nativeProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                          new byte[0], new byte[0]));
          final boolean verifierRejects =
              KagemushaCompactPaymentTokenProver.expectIllegalArgumentProbe(
                  () -> nativeVerifyRecursiveCompactPaymentToken(new byte[0]));
          return proverRejects && verifierRejects;
        },
        REQUIRED_BRIDGE_ABI_VERSION);
  }

  private static native int nativeBridgeAbiVersion();

  private static native byte[]
      nativeProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
          byte[] recordBundleArchive, byte[] pallasOpenEnvelopesArchive);

  private static native boolean nativeVerifyRecursiveCompactPaymentToken(
      byte[] compactTokenArchive);
}
