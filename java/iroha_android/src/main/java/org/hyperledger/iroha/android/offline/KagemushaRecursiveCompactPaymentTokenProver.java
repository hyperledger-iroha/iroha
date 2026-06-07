package org.hyperledger.iroha.android.offline;

import java.util.Arrays;

/** Native ABI-7 Kagemusha recursive compact-token prover and verifier. */
public final class KagemushaRecursiveCompactPaymentTokenProver {
  private static final String LIBRARY_NAME = "connect_norito_bridge";
  public static final int REQUIRED_BRIDGE_ABI_VERSION = 7;
  public static final String RECURSIVE_COMPACT_CIRCUIT_ID_V1 =
      "kagemusha-recursive-compact-v1";
  private static final String RECURSIVE_COMPACT_PAYMENT_TOKEN_UNAVAILABLE_FRAGMENT =
      "recursive compact Kagemusha payment-token proving requires a composed private-hop verifier-slice proof";
  private static final String RECURSIVE_COMPACT_MULTI_HOP_UNAVAILABLE_FRAGMENT =
      "recursive compact Kagemusha multi-hop payment-token proving requires the composed private-hop verifier batch";
  private static final boolean NATIVE_VERIFIER_AVAILABLE = loadVerifierLibrary();
  private static final boolean NATIVE_AVAILABLE = loadLibrary();

  private KagemushaRecursiveCompactPaymentTokenProver() {}

  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }

  public static boolean isVerifierNativeAvailable() {
    return NATIVE_VERIFIER_AVAILABLE;
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

  private static boolean loadVerifierLibrary() {
    return KagemushaRecursiveSpendProver.detectNativeAvailability(
        () -> System.loadLibrary(LIBRARY_NAME),
        KagemushaRecursiveCompactPaymentTokenProver::nativeBridgeAbiVersion,
        () ->
            KagemushaCompactPaymentTokenProver.expectIllegalArgumentProbe(
                () -> nativeVerifyRecursiveCompactPaymentToken(new byte[0])),
        REQUIRED_BRIDGE_ABI_VERSION);
  }

  private static native int nativeBridgeAbiVersion();

  private static native byte[]
      nativeProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
          byte[] recordBundleArchive, byte[] pallasOpenEnvelopesArchive);

  private static native boolean nativeVerifyRecursiveCompactPaymentToken(
      byte[] compactTokenArchive);
}
