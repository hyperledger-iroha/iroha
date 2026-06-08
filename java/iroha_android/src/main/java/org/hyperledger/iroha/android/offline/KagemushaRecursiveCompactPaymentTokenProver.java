package org.hyperledger.iroha.android.offline;

import java.util.Arrays;

/** Native ABI-7 Kagemusha recursive compact-token prover and verifier. */
public final class KagemushaRecursiveCompactPaymentTokenProver {
  private static final String LIBRARY_NAME = "connect_norito_bridge";
  public static final int REQUIRED_BRIDGE_ABI_VERSION = 7;
  public static final String RECURSIVE_COMPACT_CIRCUIT_ID_V1 =
      "kagemusha-recursive-compact-v1";
  private static final String RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT =
      "recursive compact Kagemusha payment-token multi-hop proving requires the append verifier batch";
  private static final String RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT =
      "recursive compact Kagemusha multi-hop payment-token proving requires the append verifier batch";
  private static final boolean NATIVE_VERIFIER_AVAILABLE = loadVerifierLibrary();
  private static final boolean NATIVE_PROJECTION_VERIFIER_AVAILABLE = loadProjectionVerifierLibrary();
  private static final boolean NATIVE_AVAILABLE = loadLibrary();

  private KagemushaRecursiveCompactPaymentTokenProver() {}

  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }

  public static boolean isVerifierNativeAvailable() {
    return NATIVE_VERIFIER_AVAILABLE;
  }

  public static boolean isProjectionVerifierNativeAvailable() {
    return NATIVE_PROJECTION_VERIFIER_AVAILABLE;
  }

  public static byte[] proveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
      final byte[] recordBundleArchive, final byte[] pallasOpenEnvelopesArchive) {
    requireNativeInput(recordBundleArchive, "recordBundleArchive");
    requireNativeInput(pallasOpenEnvelopesArchive, "pallasOpenEnvelopesArchive");
    final byte[] recordBundle = ownedNativeInput(recordBundleArchive, "recordBundleArchive");
    final byte[] pallasOpenEnvelopes =
        ownedNativeInput(pallasOpenEnvelopesArchive, "pallasOpenEnvelopesArchive");
    requireNative();
    final byte[] tokenArchive;
    try {
      tokenArchive =
          nativeProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
              recordBundle, pallasOpenEnvelopes);
    } catch (final IllegalArgumentException error) {
      if (isRecursiveCompactUnavailable(error)) {
        throw new IllegalStateException(
            "Kagemusha recursive compact proof composition is unavailable: "
                + error.getMessage(),
            error);
      }
      throw error;
    }
    return KagemushaCompactPaymentTokenProver.requireNativeOutput(
        tokenArchive,
        "nativeProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes");
  }

  public static byte[] recursiveSpendCompactPaymentTokenFromBundle(
      final byte[] bundleArchive) {
    final byte[] bundle = ownedNativeInput(bundleArchive, "bundleArchive");
    requireNative();
    final byte[] tokenArchive = nativeRecursiveSpendCompactPaymentTokenFromBundle(bundle);
    return KagemushaCompactPaymentTokenProver.requireNativeOutput(
        tokenArchive, "nativeRecursiveSpendCompactPaymentTokenFromBundle");
  }

  public static boolean isRecursiveCompactUnavailable(final Throwable error) {
    return error != null && isRecursiveCompactUnavailableMessage(error.getMessage());
  }

  private static boolean isRecursiveCompactUnavailableMessage(final String message) {
    return message != null
        && (message.contains(RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT)
            || message.contains(RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT));
  }

  static byte[] ownedNativeInput(final byte[] archive, final String archiveName) {
    requireNativeInput(archive, archiveName);
    return Arrays.copyOf(archive, archive.length);
  }

  private static void requireNativeInput(final byte[] archive, final String archiveName) {
    if (archive == null || archive.length == 0) {
      throw new IllegalArgumentException(archiveName + " must not be empty");
    }
    if (archive.length > KagemushaCompactPaymentTokenProver.NATIVE_ARCHIVE_MAX_BYTES) {
      throw new IllegalArgumentException(
          archiveName
              + " must not exceed "
              + KagemushaCompactPaymentTokenProver.NATIVE_ARCHIVE_MAX_BYTES
              + " bytes");
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
    final byte[] compactToken = ownedNativeInput(compactTokenArchive, "compactTokenArchive");
    requireVerifierNative();
    return nativeVerifyRecursiveCompactPaymentToken(compactToken);
  }

  public static boolean verifyRecursiveSpendCompactPaymentTokenProjection(
      final byte[] compactTokenArchive, final byte[] verifierRecordArchive) {
    final byte[] compactToken = ownedNativeInput(compactTokenArchive, "compactTokenArchive");
    final byte[] verifierRecord =
        ownedNativeInput(verifierRecordArchive, "verifierRecordArchive");
    requireProjectionVerifierNative();
    return nativeVerifyRecursiveSpendCompactPaymentTokenProjection(compactToken, verifierRecord);
  }

  public static boolean verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
      final byte[] compactTokenArchive,
      final byte[] verifierRecordArchive,
      final long blockHeight) {
    if (blockHeight < 0) {
      throw new IllegalArgumentException("blockHeight must be non-negative");
    }
    final byte[] compactToken = ownedNativeInput(compactTokenArchive, "compactTokenArchive");
    final byte[] verifierRecord =
        ownedNativeInput(verifierRecordArchive, "verifierRecordArchive");
    requireProjectionVerifierNative();
    return nativeVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
        compactToken, verifierRecord, blockHeight);
  }

  private static void requireNative() {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException(
          LIBRARY_NAME
              + " ABI 7 recursive compact-token prover/verifier is not available in this runtime");
    }
  }

  private static void requireVerifierNative() {
    if (!NATIVE_VERIFIER_AVAILABLE) {
      throw new IllegalStateException(
          LIBRARY_NAME
              + " ABI 7 recursive compact-token verifier is not available in this runtime");
    }
  }

  private static void requireProjectionVerifierNative() {
    if (!NATIVE_PROJECTION_VERIFIER_AVAILABLE) {
      throw new IllegalStateException(
          LIBRARY_NAME
              + " ABI 7 recursive spend compact-token projection verifier is not available in this runtime");
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
          final boolean projectionRejects =
              KagemushaCompactPaymentTokenProver.expectIllegalArgumentProbe(
                  () -> nativeRecursiveSpendCompactPaymentTokenFromBundle(new byte[0]));
          return proverRejects && verifierRejects && projectionRejects;
        },
        REQUIRED_BRIDGE_ABI_VERSION);
  }

  private static boolean loadVerifierLibrary() {
    return KagemushaRecursiveSpendProver.detectNativeAvailability(
        () -> System.loadLibrary(LIBRARY_NAME),
        KagemushaRecursiveCompactPaymentTokenProver::nativeBridgeAbiVersion,
        () ->
            KagemushaCompactPaymentTokenProver.expectIllegalArgumentProbe(
                () -> nativeVerifyRecursiveCompactPaymentToken(new byte[0])),
        REQUIRED_BRIDGE_ABI_VERSION);
  }

  private static boolean loadProjectionVerifierLibrary() {
    return KagemushaRecursiveSpendProver.detectNativeAvailability(
        () -> System.loadLibrary(LIBRARY_NAME),
        KagemushaRecursiveCompactPaymentTokenProver::nativeBridgeAbiVersion,
        () -> {
          final boolean noHeightRejects =
              KagemushaCompactPaymentTokenProver.expectIllegalArgumentProbe(
                  () ->
                      nativeVerifyRecursiveSpendCompactPaymentTokenProjection(
                          new byte[0], new byte[0]));
          final boolean heightRejects =
              KagemushaCompactPaymentTokenProver.expectIllegalArgumentProbe(
                  () ->
                      nativeVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                          new byte[0], new byte[0], 0L));
          return noHeightRejects && heightRejects;
        },
        REQUIRED_BRIDGE_ABI_VERSION);
  }

  private static native int nativeBridgeAbiVersion();

  private static native byte[]
      nativeProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
          byte[] recordBundleArchive, byte[] pallasOpenEnvelopesArchive);

  private static native boolean nativeVerifyRecursiveCompactPaymentToken(
      byte[] compactTokenArchive);

  private static native byte[] nativeRecursiveSpendCompactPaymentTokenFromBundle(
      byte[] bundleArchive);

  private static native boolean nativeVerifyRecursiveSpendCompactPaymentTokenProjection(
      byte[] compactTokenArchive, byte[] verifierRecordArchive);

  private static native boolean nativeVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
      byte[] compactTokenArchive, byte[] verifierRecordArchive, long blockHeight);
}
