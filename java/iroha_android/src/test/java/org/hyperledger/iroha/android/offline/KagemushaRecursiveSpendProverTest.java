package org.hyperledger.iroha.android.offline;

import java.lang.reflect.Method;
import java.lang.reflect.Constructor;
import java.util.Arrays;
import java.util.Set;
import java.util.stream.Collectors;

/** First-release ABI-18/V3 source and fail-closed input contract. */
public final class KagemushaRecursiveSpendProverTest {
  private KagemushaRecursiveSpendProverTest() {}

  public static void main(final String[] args) {
    exactAbiAndSingleModeAreFailClosed();
    malformedArtifactInputsFailBeforeNativeDispatch();
    installSessionRejectsPartialAndClosedUse();
    publicSurfaceOmitsRetiredRecursiveApis();
    System.out.println("[IrohaAndroid] Kagemusha first-release bridge tests passed.");
  }

  private static void exactAbiAndSingleModeAreFailClosed() {
    assert KagemushaRecursiveSpendProver.REQUIRED_NATIVE_BRIDGE_ABI_VERSION == 18;
    assert KagemushaRecursiveSpendProver.isExactBridgeAbi(18);
    assert !KagemushaRecursiveSpendProver.isExactBridgeAbi(17);
    assert !KagemushaRecursiveSpendProver.isExactBridgeAbi(19);
    assert KagemushaRecursiveSpendProver.Mode.values().length == 1;
    assert "recursive_spend_v2".equals(KagemushaRecursiveSpendProver.MODE);
    assert "recursive_spend_v2".equals(
        KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND.wireName());
    assert KagemushaRecursiveSpendProver.preferredMode(false) == null;
    assert KagemushaRecursiveSpendProver.preferredMode(true)
        == KagemushaRecursiveSpendProver.Mode.RECURSIVE_SPEND;
  }

  private static void malformedArtifactInputsFailBeforeNativeDispatch() {
    final byte[] digest = filledDigest(1);
    assertIllegalArgument(
        () -> KagemushaRecursiveSpendProver.beginArtifactInstallSession(null, digest));
    assertIllegalArgument(
        () -> KagemushaRecursiveSpendProver.beginArtifactInstallSession(new byte[0], digest));
    assertIllegalArgument(
        () ->
            KagemushaRecursiveSpendProver.beginArtifactInstallSession(
                new byte[KagemushaRecursiveSpendProver.MAX_MANIFEST_BYTES + 1], digest));
    assertIllegalArgument(
        () -> KagemushaRecursiveSpendProver.beginArtifactInstallSession(new byte[] {1}, null));
    assertIllegalArgument(
        () ->
            KagemushaRecursiveSpendProver.beginArtifactInstallSession(
                new byte[] {1}, new byte[31]));
    assertIllegalArgument(
        () ->
            KagemushaRecursiveSpendProver.beginArtifactInstallSession(
                new byte[] {1}, new byte[32]));
    assertIllegalArgument(
        () ->
            KagemushaRecursiveSpendProver.beginArtifactIngest(
                new byte[] {1}, digest, new byte[32]));
  }

  private static void publicSurfaceOmitsRetiredRecursiveApis() {
    final Set<String> publicMethods =
        Arrays.stream(KagemushaRecursiveSpendProver.class.getDeclaredMethods())
            .filter(method -> java.lang.reflect.Modifier.isPublic(method.getModifiers()))
            .map(Method::getName)
            .collect(Collectors.toSet());
    for (final String retired :
        new String[] {
          "initSpend",
          "appendSpend",
          "topUpSpend",
          "verifySpend",
          "redeemSpend",
          "transitionProfileInit",
          "transitionProfileAppend",
          "lineageAppendBoundary",
          "lineageWitnessFromInitResult",
          "lineageWitnessAppendResult",
          "buildPallasOpenEnvelopesArchive",
          "buildPreviousProofOpenEnvelopesArchive"
        }) {
      assert !publicMethods.contains(retired) : "retired public method remains: " + retired;
    }
  }

  private static void installSessionRejectsPartialAndClosedUse() {
    try {
      final Constructor<KagemushaRecursiveSpendProver.ArtifactInstallSession> constructor =
          KagemushaRecursiveSpendProver.ArtifactInstallSession.class.getDeclaredConstructor(
              byte[].class, byte[].class);
      constructor.setAccessible(true);
      final KagemushaRecursiveSpendProver.ArtifactInstallSession session =
          constructor.newInstance(new byte[] {1}, filledDigest(1));
      assertIllegalState(session::install);
      session.close();
      assert !session.isInstalled();
      assertIllegalState(() -> session.beginArtifact(filledDigest(2)));
    } catch (final ReflectiveOperationException error) {
      throw new AssertionError("failed to construct artifact session test fixture", error);
    }
  }

  private static byte[] filledDigest(final int value) {
    final byte[] digest = new byte[32];
    Arrays.fill(digest, (byte) value);
    return digest;
  }

  private static void assertIllegalArgument(final Runnable operation) {
    try {
      operation.run();
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // Expected.
    }
  }

  private static void assertIllegalState(final Runnable operation) {
    try {
      operation.run();
      throw new AssertionError("expected IllegalStateException");
    } catch (final IllegalStateException expected) {
      // Expected.
    }
  }
}
