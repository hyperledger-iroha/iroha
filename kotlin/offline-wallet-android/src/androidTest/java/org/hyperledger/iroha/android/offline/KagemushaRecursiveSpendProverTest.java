package org.hyperledger.iroha.android.offline;

import java.math.BigInteger;
import org.hyperledger.iroha.sdk.offline.KagemushaRecursiveCompactPaymentTokenProver;
import org.hyperledger.iroha.sdk.offline.KagemushaRecursiveSpendProver;
import org.junit.Test;
import org.junit.runner.RunWith;
import androidx.test.ext.junit.runners.AndroidJUnit4;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;

@RunWith(AndroidJUnit4.class)
public final class KagemushaRecursiveSpendProverTest {

  @Test
  public void productionHarnessResolvesKagemushaRecursiveSpendSurface() {
    assertEquals(6, KagemushaRecursiveSpendProver.REQUIRED_BRIDGE_ABI_VERSION);
    assertEquals(7, KagemushaRecursiveSpendProver.RECURSIVE_COMPACT_REQUIRED_BRIDGE_ABI_VERSION);
    assertEquals(7, KagemushaRecursiveCompactPaymentTokenProver.REQUIRED_BRIDGE_ABI_VERSION);
    assertTrue(
        "ABI-6 recursive spend JNI bridge should load on the Android runtime",
        KagemushaRecursiveSpendProver.isNativeAvailable());
    assertTrue(
        "ABI-7 recursive compact JNI prover/verifier bridge should load on the Android runtime",
        KagemushaRecursiveCompactPaymentTokenProver.isNativeAvailable());
    assertTrue(
        "ABI-7 recursive compact verifier bridge should load on the Android runtime",
        KagemushaRecursiveCompactPaymentTokenProver.isVerifierNativeAvailable());
    assertTrue(
        "ABI-7 recursive spend projection verifier bridge should load on the Android runtime",
        KagemushaRecursiveCompactPaymentTokenProver.isProjectionVerifierNativeAvailable());
    assertEquals(
        KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND_V1,
        KagemushaRecursiveSpendProver.preferredMode());
    assertEquals(
        KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND_V1,
        KagemushaRecursiveSpendProver.preferredMode(true, true));
    assertEquals(
        KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND_V1,
        KagemushaRecursiveSpendProver.preferredMode(false, true));
    assertEquals(
        KagemushaRecursiveSpendProver.Mode.CHECKED_PREFOLD_V1,
        KagemushaRecursiveSpendProver.preferredMode(false, false));
    assertEquals(
        "kagemusha-recursive-compact-v1",
        KagemushaRecursiveCompactPaymentTokenProver.RECURSIVE_COMPACT_CIRCUIT_ID_V1);
  }

  @Test
  public void recursiveSpendWitnesslessPolicyFailsClosedAtBounds() {
    assertTrue(
        KagemushaRecursiveSpendProver.canRedeemWitnessless(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            1));
    assertTrue(
        KagemushaRecursiveSpendProver.canRedeemWitnessless(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
            1));
    assertTrue(
        KagemushaRecursiveSpendProver.canRedeemWitnessless(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1));
    assertFalse(
        KagemushaRecursiveSpendProver.canRedeemWitnessless(
            "unknown-kagemusha-recursive-spend-circuit",
            1));
    assertFalse(
        KagemushaRecursiveSpendProver.canRedeemWitnessless(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            0));
    assertFalse(
        KagemushaRecursiveSpendProver.canRedeemWitnessless(
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1,
            KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1 + 1));
    assertTrue(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(63));
    assertFalse(KagemushaRecursiveSpendProver.canAppendWitnesslessLineage(64));
  }

  @Test
  public void recursiveSpendKeyArtifactsRejectInvalidPackagesBeforeNativeDispatch() {
    expectIllegalArgument(
        "lineage_verifier_key",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                4,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                new byte[] {0x01},
                new byte[] {0x02}));
    expectIllegalArgument(
        "verifier_opening_len",
        () ->
            KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
                3,
                KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
                new byte[] {0x01},
                new byte[] {0x02}));
  }

  @Test
  public void recursiveCompactProjectionRejectsInvalidInputsBeforeNativeDispatch() {
    final byte[] empty = new byte[0];
    expectIllegalArgument(
        "blockHeight must be non-negative",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    empty, empty, -1L));
    expectIllegalArgument(
        "blockHeight must be a canonical unsigned decimal integer",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    empty, empty, "01"));
    expectIllegalArgument(
        "blockHeight must fit in u64",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    empty, empty, "18446744073709551616"));
    expectIllegalArgument(
        "blockHeight must be non-negative",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    empty, empty, new BigInteger("-1")));
    expectIllegalArgument(
        "blockHeight must not be null",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    empty, empty, (String) null));
    expectIllegalArgument(
        "blockHeight must not be null",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    empty, empty, (BigInteger) null));
    expectIllegalArgument(
        "compactTokenArchive must not be empty",
        () ->
            KagemushaRecursiveCompactPaymentTokenProver
                .verifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                    empty, empty, new BigInteger("18446744073709551615")));
  }

  private static void expectIllegalArgument(
      final String expectedMessage, final ThrowingRunnable runnable) {
    try {
      runnable.run();
      fail("expected IllegalArgumentException containing: " + expectedMessage);
    } catch (final IllegalArgumentException expected) {
      assertTrue(
          "expected message containing " + expectedMessage + ", got " + expected.getMessage(),
          expected.getMessage() != null && expected.getMessage().contains(expectedMessage));
    } catch (final Exception unexpected) {
      throw new AssertionError("unexpected exception type", unexpected);
    }
  }

  private interface ThrowingRunnable {
    void run() throws Exception;
  }
}
