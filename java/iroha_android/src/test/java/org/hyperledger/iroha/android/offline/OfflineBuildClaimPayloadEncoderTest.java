package org.hyperledger.iroha.android.offline;

/**
 * Tests for OfflineBuildClaimPayloadEncoder JNI binding.
 *
 * <p>Set IROHA_NATIVE_REQUIRED=1 to make tests fail when native library is unavailable.
 */
public final class OfflineBuildClaimPayloadEncoderTest {

  private OfflineBuildClaimPayloadEncoderTest() {}

  public static void main(final String[] args) {
    encodeReturnsNonEmptyBytes();
    System.out.println("[IrohaAndroid] OfflineBuildClaimPayloadEncoderTest passed.");
  }

  private static void encodeReturnsNonEmptyBytes() {
    if (!OfflineBuildClaimPayloadEncoder.isNativeAvailable()) {
      final boolean nativeRequired = "1".equals(System.getenv("IROHA_NATIVE_REQUIRED"));
      if (nativeRequired) {
        throw new AssertionError(
            "IROHA_NATIVE_REQUIRED=1 but connect_norito_bridge native library is unavailable. "
                + "This indicates the .so file was not rebuilt after adding "
                + "OfflineBuildClaimPayloadEncoder JNI function.");
      }
      System.out.println(
          "[IrohaAndroid] OfflineBuildClaimPayloadEncoderTest skipped (native unavailable). "
              + "Set IROHA_NATIVE_REQUIRED=1 to fail instead.");
      return;
    }

    final String claimIdHex = "11".repeat(32);
    // Hash parser requires the least-significant bit to be set.
    final String nonceHex = "23".repeat(32);

    final byte[] encoded =
        OfflineBuildClaimPayloadEncoder.encode(
            claimIdHex,
            "ios",
            "jp.co.soramitsu.cbdc.pkr",
            42L,
            1_700_000_000_000L,
            1_700_086_400_000L,
            null,
            nonceHex);

    if (encoded == null || encoded.length == 0) {
      throw new AssertionError("encode() returned empty bytes");
    }

    if (encoded[0] != 'N' || encoded[1] != 'R' || encoded[2] != 'T' || encoded[3] != '0') {
      throw new AssertionError(
          "encoded bytes missing Norito header (expected NRT0, got "
              + (char) encoded[0]
              + (char) encoded[1]
              + (char) encoded[2]
              + (char) encoded[3]
              + ")");
    }

    System.out.println(
        "[IrohaAndroid] OfflineBuildClaimPayloadEncoder.encode() returned "
            + encoded.length
            + " bytes with valid Norito header");
  }
}
