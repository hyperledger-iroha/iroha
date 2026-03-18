package org.hyperledger.iroha.android.offline;

import java.util.Locale;

/**
 * Encodes OfflineBuildClaim payload to Norito bytes for signature verification/signing checks.
 *
 * <p>This encoder serializes the build-claim payload structure using the Norito codec.
 */
public final class OfflineBuildClaimPayloadEncoder {
  private static final boolean NATIVE_AVAILABLE;

  static {
    boolean available;
    try {
      System.loadLibrary("connect_norito_bridge");
      available = true;
    } catch (UnsatisfiedLinkError error) {
      available = false;
    }
    NATIVE_AVAILABLE = available;
  }

  private OfflineBuildClaimPayloadEncoder() {}

  public static boolean isNativeAvailable() {
    return NATIVE_AVAILABLE;
  }

  /**
   * Encode OfflineBuildClaim payload to Norito bytes.
   *
   * @param claimIdHex 32-byte claim identifier as hex (64 chars)
   * @param platform build platform token (`Apple`/`Android`, with `ios` accepted as alias)
   * @param appId application identifier
   * @param buildNumber certified build number
   * @param issuedAtMs issuance timestamp in milliseconds
   * @param expiresAtMs expiration timestamp in milliseconds
   * @param lineageScope claim lineage scope (null/blank coerced to empty)
   * @param nonceHex 32-byte nonce as hex (64 chars)
   * @return Norito-encoded bytes
   * @throws IllegalStateException if native library is unavailable
   * @throws IllegalArgumentException if any parameter is invalid
   */
  public static byte[] encode(
      final String claimIdHex,
      final String platform,
      final String appId,
      final long buildNumber,
      final long issuedAtMs,
      final long expiresAtMs,
      final String lineageScope,
      final String nonceHex) {
    if (!NATIVE_AVAILABLE) {
      throw new IllegalStateException("connect_norito_bridge is not available in this runtime");
    }

    final String normalizedClaimIdHex = OfflineHashLiteral.parseHex(claimIdHex, "claimIdHex");
    final String normalizedPlatform = normalizePlatform(platform);
    final String normalizedAppId = requireNonBlank(appId, "appId");
    requireNonNegative(buildNumber, "buildNumber");
    requireNonNegative(issuedAtMs, "issuedAtMs");
    requireNonNegative(expiresAtMs, "expiresAtMs");
    final String normalizedLineageScope =
        lineageScope == null || lineageScope.isBlank() ? "" : lineageScope.trim();
    final String normalizedNonceHex = OfflineHashLiteral.parseHex(nonceHex, "nonceHex");

    final byte[] result =
        nativeEncode(
            normalizedClaimIdHex,
            normalizedPlatform,
            normalizedAppId,
            buildNumber,
            issuedAtMs,
            expiresAtMs,
            normalizedLineageScope,
            normalizedNonceHex);
    if (result == null) {
      throw new IllegalStateException("nativeEncode returned null");
    }
    return result;
  }

  private static String normalizePlatform(final String platform) {
    final String normalized = requireNonBlank(platform, "platform").toLowerCase(Locale.ROOT);
    if ("apple".equals(normalized) || "ios".equals(normalized)) {
      return "Apple";
    }
    if ("android".equals(normalized)) {
      return "Android";
    }
    throw new IllegalArgumentException("platform must be either \"Apple\" or \"Android\"");
  }

  private static String requireNonBlank(final String value, final String field) {
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException(field + " must not be blank");
    }
    return value.trim();
  }

  private static void requireNonNegative(final long value, final String field) {
    if (value < 0) {
      throw new IllegalArgumentException(field + " must not be negative");
    }
  }

  private static native byte[] nativeEncode(
      String claimIdHex,
      String platform,
      String appId,
      long buildNumber,
      long issuedAtMs,
      long expiresAtMs,
      String lineageScope,
      String nonceHex);
}
