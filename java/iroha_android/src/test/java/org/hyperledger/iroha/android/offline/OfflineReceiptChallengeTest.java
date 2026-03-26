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
        "6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9",
        "7EAD8EFYUx1aVKZPUU1fyKvr8dF1#6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9",
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
            "6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9",
            "7EAD8EFYUx1aVKZPUU1fyKvr8dF1#6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9",
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
