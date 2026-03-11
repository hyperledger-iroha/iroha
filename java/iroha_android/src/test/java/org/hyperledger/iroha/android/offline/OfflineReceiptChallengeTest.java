package org.hyperledger.iroha.android.offline;

import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Arrays;

/** Plain Java entrypoint that mirrors the old JUnit receipt challenge test. */
public final class OfflineReceiptChallengeTest {

  private OfflineReceiptChallengeTest() {}

  private static final String SAMPLE_CERTIFICATE_ID_HEX =
      "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
  private static final String SAMPLE_NONCE_HEX =
      "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
  private static final String SAMPLE_RECEIVER_ACCOUNT_ID =
      "6cmzPVPX8DcdUnE1nGLZBU1opw24wjxczQNqhCCYvMzKfJR2rGs9tan";
  private static final String SAMPLE_ASSET_ID =
      "norito:4e52543000000eaf5ef05db6ed320eaf5ef05db6ed3200c0000000000000005c0d942ec37b449c00810000000000000017000000000000000f00000000000000070000000000000064656661756c745a00000000000000000000004e000000000000004600000000000000656430313230413938424146423036363343453038443735454244353036464543333841383445353736413743394230383937363933454434423034464439454632443138442f0000000000000014000000000000000c000000000000000400000000000000736f72610b000000000000000300000000000000757364";

  public static void main(final String[] args) throws Exception {
    computeAcceptsScaledAmount();
    computeProducesClientHash();
    System.out.println("[IrohaAndroid] OfflineReceiptChallengeTest passed.");
  }

  private static void computeAcceptsScaledAmount() {
    if (!OfflineReceiptChallenge.isNativeAvailable()) {
      System.out.println(
          "[IrohaAndroid] OfflineReceiptChallengeTest skipped (native bridge unavailable).");
      return;
    }
    OfflineReceiptChallenge.compute(
        "testnet",
        "inv-frac",
        SAMPLE_RECEIVER_ACCOUNT_ID,
        SAMPLE_ASSET_ID,
        "12.5",
        1_700_000_000_000L,
        SAMPLE_CERTIFICATE_ID_HEX,
        SAMPLE_NONCE_HEX,
        1);
  }

  private static void computeProducesClientHash() throws NoSuchAlgorithmException {
    if (!OfflineReceiptChallenge.isNativeAvailable()) {
      System.out.println(
          "[IrohaAndroid] OfflineReceiptChallengeTest skipped (native bridge unavailable).");
      return;
    }

    final OfflineReceiptChallenge challenge =
        OfflineReceiptChallenge.compute(
            "testnet",
            "inv-android-tests",
            SAMPLE_RECEIVER_ACCOUNT_ID,
            SAMPLE_ASSET_ID,
            "100",
            1_700_000_000_000L,
            SAMPLE_CERTIFICATE_ID_HEX,
            SAMPLE_NONCE_HEX);

    expectByteEquals('N', challenge.getPreimage()[0], "preimage[0]");
    expectByteEquals('R', challenge.getPreimage()[1], "preimage[1]");
    expectByteEquals('T', challenge.getPreimage()[2], "preimage[2]");
    expectByteEquals('0', challenge.getPreimage()[3], "preimage[3]");

    final MessageDigest sha256 = MessageDigest.getInstance("SHA-256");
    expectArrayEquals(
        sha256.digest(challenge.getIrohaHash()),
        challenge.getClientDataHash(),
        "clientDataHash");
  }

  private static void expectByteEquals(
      final int expected, final byte actual, final String fieldName) {
    if ((byte) expected != actual) {
      throw new AssertionError(
          fieldName + " mismatch (expected=" + (char) expected + " actual=" + (char) actual + ")");
    }
  }

  private static void expectArrayEquals(
      final byte[] expected, final byte[] actual, final String fieldName) {
    if (!Arrays.equals(expected, actual)) {
      throw new AssertionError(fieldName + " mismatch");
    }
  }
}
