package org.hyperledger.iroha.android.offline;

import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;

/** Source-level contract checks for the non-spending ABI-19 artifact bridge. */
public final class KagemushaRecursiveSpendProverTest {
  public static void main(final String[] args) {
    exactAbiIsRequired();
    artifactContractIsFixed();
    publicSurfaceIsArtifactOnly();
  }

  private static void exactAbiIsRequired() {
    assert KagemushaRecursiveSpendProver.isExactBridgeAbi(19);
    assert !KagemushaRecursiveSpendProver.isExactBridgeAbi(20);
    assert KagemushaRecursiveSpendProver.detectExactNativeAvailability(
        () -> {}, () -> 19, () -> true);
    assert !KagemushaRecursiveSpendProver.detectExactNativeAvailability(
        () -> {}, () -> 19, () -> false);
    assert !KagemushaRecursiveSpendProver.detectExactNativeAvailability(
        () -> { throw new UnsatisfiedLinkError("missing"); }, () -> 19, () -> true);
  }

  private static void artifactContractIsFixed() {
    assert KagemushaRecursiveSpendProver.REQUIRED_NATIVE_BRIDGE_ABI_VERSION == 19;
    assert KagemushaRecursiveSpendProver.ARTIFACT_COUNT == 6;
    assert "kagemusha.offline.recursive_spend.artifact_manifest.v3"
        .equals(KagemushaRecursiveSpendProver.ARTIFACT_MANIFEST_SCHEMA);
    assert KagemushaRecursiveSpendProver.ARTIFACT_FILES.equals(
        List.of(
            "step-eq.parameters.krv3",
            "step-eq.proving-key.krv3",
            "step-eq.verifying-key.krv3",
            "step-ep.parameters.krv3",
            "step-ep.proving-key.krv3",
            "step-ep.verifying-key.krv3"));
  }

  private static void publicSurfaceIsArtifactOnly() {
    final Set<String> methods = new TreeSet<>();
    for (final Method method : KagemushaRecursiveSpendProver.class.getDeclaredMethods()) {
      if (Modifier.isPublic(method.getModifiers())) {
        methods.add(method.getName());
      }
    }
    assert methods.equals(
        Set.of(
            "beginArtifactIngest",
            "beginArtifactInstallSession",
            "isArtifactStreamingAvailable",
            "isProofBackendAvailable")) : methods;
  }
}
