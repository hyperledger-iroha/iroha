package org.hyperledger.iroha.android.offline;

/** Plain Java entrypoint that exercises the native balance proof bridge. */
public final class OfflineBalanceProofTest {

  private OfflineBalanceProofTest() {}

  public static void main(final String[] args) {
    nativeCommitmentAndProofGenerationSucceeds();
    System.out.println("[IrohaAndroid] OfflineBalanceProofTest passed.");
  }

  private static void nativeCommitmentAndProofGenerationSucceeds() {
    if (!OfflineBalanceProof.isNativeAvailable()) {
      System.out.println(
          "[IrohaAndroid] OfflineBalanceProofTest skipped (native bridge unavailable).");
      return;
    }

    final OfflineBalanceProof.Artifacts artifacts =
        OfflineBalanceProof.advanceCommitment(
            "sdk-chain",
            "5",
            "5",
            hexZeros(64),
            hexZeros(64),
            "01" + hexZeros(62));
    expectNotNull(artifacts, "artifacts");
    expectEquals(32, artifacts.getResultingCommitment().length, "commitment bytes");
    expectEquals(12385, artifacts.getProof().length, "proof bytes");
    expectEquals(64, artifacts.getResultingCommitmentHex().length(), "commitment hex");
    expectEquals(24770, artifacts.getProofHex().length(), "proof hex");
  }

  private static String hexZeros(final int count) {
    final StringBuilder builder = new StringBuilder(count);
    for (int i = 0; i < count; i++) {
      builder.append('0');
    }
    return builder.toString();
  }

  private static void expectEquals(
      final int expected, final int actual, final String fieldName) {
    if (expected != actual) {
      throw new AssertionError(
          fieldName + " mismatch (expected=" + expected + " actual=" + actual + ")");
    }
  }

  private static void expectNotNull(final Object value, final String fieldName) {
    if (value == null) {
      throw new AssertionError(fieldName + " must not be null");
    }
  }
}
