package org.hyperledger.iroha.android.offline;

import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Arrays;

/** Plain Java entrypoint that mirrors the old JUnit receipt challenge test. */
public final class OfflineReceiptChallengeTest {

  private OfflineReceiptChallengeTest() {}

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
        "bob@wonderland",
        "usd#wonderland#treasury@wonderland",
        "12.5",
        1_700_000_000_000L,
        "ABCDEF1234",
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
            "bob@wonderland",
            "usd#wonderland#treasury@wonderland",
            "100",
            1_700_000_000_000L,
            "ABCDEF1234");

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
